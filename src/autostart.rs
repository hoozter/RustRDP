use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const AUTOSTART_FILE: &str = "rustrdp.desktop";

#[derive(Debug, thiserror::Error)]
pub enum AutostartError {
    #[error("could not determine the XDG configuration directory")]
    NoConfigDirectory,
    #[error("could not determine the current executable: {0}")]
    NoExecutable(io::Error),
    #[error("could not update autostart at {path}: {source}")]
    Update { path: PathBuf, source: io::Error },
}

pub fn default_path() -> Result<PathBuf, AutostartError> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or(AutostartError::NoConfigDirectory)?;
    Ok(config.join("autostart").join(AUTOSTART_FILE))
}

pub fn is_enabled(path: &Path) -> bool {
    path.is_file()
}

pub fn set_enabled(path: &Path, executable: &Path, enabled: bool) -> Result<(), AutostartError> {
    if !enabled {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(AutostartError::Update {
                path: path.to_owned(),
                source,
            }),
        };
    }
    let parent = path.parent().ok_or_else(|| AutostartError::Update {
        path: path.to_owned(),
        source: io::Error::new(io::ErrorKind::InvalidInput, "autostart path has no parent"),
    })?;
    fs::create_dir_all(parent).map_err(|source| AutostartError::Update {
        path: parent.to_owned(),
        source,
    })?;
    let contents = format!(
        "[Desktop Entry]\nType=Application\nName=RustRDP\nComment=FreeRDP connection manager\nExec={} --minimized\nIcon=com.hoozter.RustRDP\nTerminal=false\nCategories=Network;RemoteAccess;\nX-GNOME-Autostart-enabled=true\n",
        desktop_exec_quote(executable)
    );
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|source| AutostartError::Update {
            path: path.to_owned(),
            source,
        })?;
    file.write_all(contents.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|source| AutostartError::Update {
            path: path.to_owned(),
            source,
        })
}

pub fn set_current_executable_enabled(enabled: bool) -> Result<(), AutostartError> {
    let executable = std::env::current_exe().map_err(AutostartError::NoExecutable)?;
    set_enabled(&default_path()?, &executable, enabled)
}

fn desktop_exec_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('`', "\\`")
            .replace('$', "\\$")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_removes_xdg_autostart_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("autostart/rustrdp.desktop");
        set_enabled(&path, Path::new("/opt/Rust RDP/rustrdp"), true).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("Exec=\"/opt/Rust RDP/rustrdp\" --minimized"));
        assert!(contents.contains("Icon=com.hoozter.RustRDP"));
        assert!(is_enabled(&path));
        set_enabled(&path, Path::new("ignored"), false).unwrap();
        assert!(!path.exists());
    }
}
