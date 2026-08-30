use crate::model::{AppData, CURRENT_SCHEMA_VERSION, Profile, ProfileError};
use directories::ProjectDirs;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = "config.toml";
const PROFILE_ARCHIVE_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
struct ProfileArchive {
    archive_version: u32,
    profiles: Vec<Profile>,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("could not determine the XDG configuration directory")]
    NoConfigDirectory,
    #[error("could not read {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("could not parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("configuration schema {found} is newer than this application supports ({supported})")]
    NewerSchema { found: u32, supported: u32 },
    #[error(
        "connection backup version {found} is newer than this application supports ({supported})"
    )]
    NewerArchive { found: u32, supported: u32 },
    #[error("connection backup contains an invalid profile '{profile}': {source}")]
    InvalidProfile {
        profile: String,
        source: ProfileError,
    },
    #[error("could not serialize configuration: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("could not save {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
}

pub fn default_config_path() -> Result<PathBuf, StorageError> {
    ProjectDirs::from("com", "hoozter", "RustRDP")
        .map(|dirs| dirs.config_dir().join(CONFIG_FILE))
        .ok_or(StorageError::NoConfigDirectory)
}

pub fn load(path: &Path) -> Result<AppData, StorageError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(AppData::default()),
        Err(source) => {
            return Err(StorageError::Read {
                path: path.to_owned(),
                source,
            });
        }
    };
    let data: AppData = toml::from_str(&text).map_err(|source| StorageError::Parse {
        path: path.to_owned(),
        source,
    })?;
    if data.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(StorageError::NewerSchema {
            found: data.schema_version,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }
    Ok(data)
}

pub fn save(path: &Path, data: &AppData) -> Result<(), StorageError> {
    let text = toml::to_string_pretty(data)?;
    write_atomic(path, &text)
}

pub fn export_profiles(path: &Path, profiles: &[Profile]) -> Result<(), StorageError> {
    let mut profiles = profiles.to_vec();
    for profile in &mut profiles {
        profile.connection.save_password = false;
    }
    let text = toml::to_string_pretty(&ProfileArchive {
        archive_version: PROFILE_ARCHIVE_VERSION,
        profiles,
    })?;
    write_atomic(path, &text)
}

pub fn import_profiles(path: &Path) -> Result<Vec<Profile>, StorageError> {
    let text = fs::read_to_string(path).map_err(|source| StorageError::Read {
        path: path.to_owned(),
        source,
    })?;
    let mut archive: ProfileArchive =
        toml::from_str(&text).map_err(|source| StorageError::Parse {
            path: path.to_owned(),
            source,
        })?;
    if archive.archive_version > PROFILE_ARCHIVE_VERSION {
        return Err(StorageError::NewerArchive {
            found: archive.archive_version,
            supported: PROFILE_ARCHIVE_VERSION,
        });
    }
    for profile in &mut archive.profiles {
        profile
            .validate()
            .map_err(|source| StorageError::InvalidProfile {
                profile: profile.name.clone(),
                source,
            })?;
        profile.connection.save_password = false;
    }
    Ok(archive.profiles)
}

fn write_atomic(path: &Path, text: &str) -> Result<(), StorageError> {
    let parent = path.parent().ok_or_else(|| StorageError::Write {
        path: path.to_owned(),
        source: io::Error::new(
            io::ErrorKind::InvalidInput,
            "configuration path has no parent",
        ),
    })?;
    fs::create_dir_all(parent).map_err(|source| StorageError::Write {
        path: parent.to_owned(),
        source,
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("data");
    let temporary = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok::<_, io::Error>(())
    })();
    if let Err(source) = result {
        let _ = fs::remove_file(&temporary);
        return Err(StorageError::Write {
            path: path.to_owned(),
            source,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Profile;

    #[test]
    fn missing_file_loads_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let data = load(&dir.path().join("missing.toml")).unwrap();
        assert_eq!(data, AppData::default());
    }

    #[test]
    fn round_trip_never_serializes_password_material() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config.toml");
        let mut data = AppData::default();
        data.profiles.push(Profile::default());
        save(&path, &data).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            text.lines()
                .all(|line| !line.trim_start().starts_with("password ="))
        );
        assert_eq!(load(&path).unwrap(), data);
    }

    #[test]
    fn rejects_unknown_future_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "schema_version = 999\nprofiles = []\n").unwrap();
        assert!(matches!(
            load(&path),
            Err(StorageError::NewerSchema { found: 999, .. })
        ));
    }

    #[test]
    fn existing_schema_without_recent_connections_remains_compatible() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "schema_version = 1\nprofiles = []\n").unwrap();
        let data = load(&path).unwrap();
        assert!(data.recent_connections.is_empty());
    }

    #[test]
    fn profile_archive_round_trip_excludes_credential_preferences() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("connections.toml");
        let profile = Profile {
            name: "Work PC".to_owned(),
            connection: crate::model::Connection {
                host: "work.example.com".to_owned(),
                save_password: true,
                ..crate::model::Connection::default()
            },
            ..Profile::default()
        };

        export_profiles(&path, &[profile]).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("save_password = true"));
        let imported = import_profiles(&path).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].name, "Work PC");
        assert!(!imported[0].connection.save_password);
    }

    #[test]
    fn profile_archive_rejects_invalid_connections() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("connections.toml");
        fs::write(
            &path,
            "archive_version = 1\n[[profiles]]\nname = 'Broken'\n",
        )
        .unwrap();
        assert!(matches!(
            import_profiles(&path),
            Err(StorageError::InvalidProfile { .. })
        ));
    }
}
