use std::{
    fs, io,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const PREFIX: &str = "com.hoozter.RustRDP.connection-";

#[derive(Debug)]
pub struct Launcher {
    pub id: Uuid,
    pub name: String,
    pub path: PathBuf,
}

pub fn directory() -> io::Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|dirs| dirs.data_dir().join("applications"))
        .ok_or_else(|| io::Error::other("Could not locate the application menu directory"))
}

fn path(directory: &Path, id: Uuid) -> PathBuf {
    directory.join(format!("{PREFIX}{id}.desktop"))
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn unescape(value: &str) -> String {
    let mut chars = value.chars();
    let mut result = String::new();
    while let Some(c) = chars.next() {
        if c == '\\' {
            result.push(match chars.next() {
                Some('n') => '\n',
                Some('r') => '\r',
                Some('t') => '\t',
                Some('s') => ' ',
                Some(c) => c,
                None => '\\',
            });
        } else {
            result.push(c);
        }
    }
    result
}

/// Desktop Exec has its own argument quoting, followed by desktop-string escaping.
pub(crate) fn executable_argument(executable: &Path) -> io::Result<String> {
    let value = executable
        .to_str()
        .ok_or_else(|| io::Error::other("Executable path is not UTF-8"))?;
    if value.chars().any(char::is_control) {
        return Err(io::Error::other(
            "Executable path contains control characters",
        ));
    }
    let quoted = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`")
        .replace('%', "%%");
    Ok(escape(&format!("\"{quoted}\"")))
}

fn read_owned(directory: &Path, id: Uuid) -> io::Result<Option<Launcher>> {
    let path = path(directory, id);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(value) => value,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if !metadata.file_type().is_file() {
        return Err(io::Error::other("Launcher path is not a regular file"));
    }
    let text = fs::read_to_string(&path)?;
    let marker = format!("X-RustRDP-Profile={id}");
    if !text.lines().any(|line| line == marker) {
        return Err(io::Error::other(
            "Existing file is not a RustRDP connection launcher",
        ));
    }
    let name = text
        .lines()
        .find_map(|line| line.strip_prefix("Name="))
        .map(unescape)
        .ok_or_else(|| io::Error::other("Launcher has no name"))?;
    Ok(Some(Launcher { id, name, path }))
}

pub fn list(directory: &Path) -> io::Result<Vec<Launcher>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut launchers = Vec::new();
    for entry in entries {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if let Some(id) = name
            .strip_prefix(PREFIX)
            .and_then(|s| s.strip_suffix(".desktop"))
            .and_then(|s| Uuid::parse_str(s).ok())
            && let Some(launcher) = read_owned(directory, id)?
        {
            launchers.push(launcher);
        }
    }
    launchers.sort_by_key(|l| l.name.to_lowercase());
    Ok(launchers)
}

pub fn save(directory: &Path, id: Uuid, name: &str, executable: &Path) -> io::Result<PathBuf> {
    let name = name.trim();
    if name.is_empty() || name.chars().any(char::is_control) {
        return Err(io::Error::other(
            "Enter a launcher name without line breaks or control characters",
        ));
    }
    read_owned(directory, id)?;
    let text = format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment=Connect using RustRDP\nExec={} --connect {}\nIcon=com.hoozter.RustRDP\nTerminal=false\nCategories=Network;RemoteAccess;\nStartupNotify=false\nX-RustRDP-Profile={}\n",
        escape(name),
        executable_argument(executable)?,
        id,
        id
    );
    let path = path(directory, id);
    crate::storage::write_atomic(&path, &text).map_err(io::Error::other)?;
    Ok(path)
}

pub fn remove(directory: &Path, id: Uuid) -> io::Result<()> {
    if let Some(launcher) = read_owned(directory, id)? {
        fs::remove_file(launcher.path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_updates_same_file_and_removal_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let id = Uuid::new_v4();
        let original = save(dir.path(), id, "Work", Path::new("/opt/Rust RDP/app")).unwrap();
        assert_eq!(
            save(dir.path(), id, "Office", Path::new("/opt/Rust RDP/app")).unwrap(),
            original
        );
        let entries = list(dir.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Office");
        let contents = fs::read_to_string(original).unwrap();
        assert!(contents.contains(&format!("--connect {id}")));
        remove(dir.path(), id).unwrap();
        remove(dir.path(), id).unwrap();
        assert!(list(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn refuses_to_overwrite_or_remove_unowned_files_and_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let id = Uuid::new_v4();
        fs::write(path(dir.path(), id), "unrelated").unwrap();
        assert!(save(dir.path(), id, "Work", Path::new("/app")).is_err());
        assert!(remove(dir.path(), id).is_err());
        #[cfg(unix)]
        {
            let other = Uuid::new_v4();
            std::os::unix::fs::symlink(path(dir.path(), id), path(dir.path(), other)).unwrap();
            assert!(save(dir.path(), other, "Work", Path::new("/app")).is_err());
            assert!(remove(dir.path(), other).is_err());
        }
    }

    #[test]
    fn escapes_names_and_exec_without_shell_or_desktop_field_injection() {
        let dir = tempfile::tempdir().unwrap();
        let id = Uuid::new_v4();
        assert!(save(dir.path(), id, "Work\nExec=bad", Path::new("/app")).is_err());
        assert!(save(dir.path(), id, "   ", Path::new("/app")).is_err());
        save(dir.path(), id, r"Work \ team %", Path::new("/app")).unwrap();
        assert_eq!(list(dir.path()).unwrap()[0].name, r"Work \ team %");
        assert_eq!(
            executable_argument(Path::new("/opt/a b%/app")).unwrap(),
            "\"/opt/a b%%/app\""
        );
        assert_eq!(
            executable_argument(Path::new("/opt/$app")).unwrap(),
            "\"/opt/\\\\$app\""
        );
    }
}
