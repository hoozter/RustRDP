use crate::model::{AppData, CURRENT_SCHEMA_VERSION};
use directories::ProjectDirs;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = "config.toml";

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
    let temporary = parent.join(format!(".{CONFIG_FILE}.tmp-{}", std::process::id()));
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
}
