use directories::UserDirs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FILTER: &str = "*.toml|RustRDP connection backups (*.toml)";

#[derive(Debug, thiserror::Error)]
pub enum FileDialogError {
    #[error("no supported desktop file picker was found (install kdialog or zenity)")]
    Unavailable,
    #[error("the desktop file picker failed: {0}")]
    Failed(String),
    #[error("the desktop file picker returned an invalid path")]
    InvalidPath,
}

pub fn choose_export_path() -> Result<Option<PathBuf>, FileDialogError> {
    let suggested = suggested_archive_path();
    run_first_available([
        (
            "kdialog",
            vec![
                "--title".into(),
                "Export RustRDP connections".into(),
                "--getsavefilename".into(),
                suggested.as_os_str().to_owned(),
                FILTER.into(),
            ],
        ),
        (
            "zenity",
            vec![
                "--file-selection".into(),
                "--save".into(),
                "--confirm-overwrite".into(),
                "--title=Export RustRDP connections".into(),
                format!("--filename={}", suggested.display()).into(),
                "--file-filter=RustRDP connection backups | *.toml".into(),
            ],
        ),
    ])
}

pub fn choose_import_path() -> Result<Option<PathBuf>, FileDialogError> {
    let initial = suggested_archive_path();
    run_first_available([
        (
            "kdialog",
            vec![
                "--title".into(),
                "Import RustRDP connections".into(),
                "--getopenfilename".into(),
                initial.as_os_str().to_owned(),
                FILTER.into(),
            ],
        ),
        (
            "zenity",
            vec![
                "--file-selection".into(),
                "--title=Import RustRDP connections".into(),
                format!("--filename={}", initial.display()).into(),
                "--file-filter=RustRDP connection backups | *.toml".into(),
            ],
        ),
    ])
}

pub fn choose_folder(initial: Option<&Path>) -> Result<Option<PathBuf>, FileDialogError> {
    let initial = initial
        .filter(|path| path.is_dir())
        .map(Path::to_owned)
        .or_else(|| UserDirs::new().map(|dirs| dirs.home_dir().to_owned()))
        .unwrap_or_else(std::env::temp_dir);
    run_first_available([
        (
            "kdialog",
            vec![
                "--title".into(),
                "Choose a local folder to share".into(),
                "--getexistingdirectory".into(),
                initial.as_os_str().to_owned(),
            ],
        ),
        (
            "zenity",
            vec![
                "--file-selection".into(),
                "--directory".into(),
                "--title=Choose a local folder to share".into(),
                format!("--filename={}/", initial.display()).into(),
            ],
        ),
    ])
}

fn suggested_archive_path() -> PathBuf {
    UserDirs::new()
        .and_then(|dirs| {
            dirs.document_dir()
                .map(Path::to_owned)
                .or_else(|| Some(dirs.home_dir().to_owned()))
        })
        .unwrap_or_else(std::env::temp_dir)
        .join("rustrdp-connections.toml")
}

fn run_first_available<const N: usize>(
    choices: [(&str, Vec<std::ffi::OsString>); N],
) -> Result<Option<PathBuf>, FileDialogError> {
    for (program, arguments) in choices {
        match Command::new(program).args(arguments).output() {
            Ok(output) => return parse_output(output),
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(FileDialogError::Failed(error.to_string())),
        }
    }
    Err(FileDialogError::Unavailable)
}

fn parse_output(output: Output) -> Result<Option<PathBuf>, FileDialogError> {
    if !output.status.success() {
        return if output.status.code() == Some(1) {
            Ok(None)
        } else {
            Err(FileDialogError::Failed(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ))
        };
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if path.is_empty() {
        return Err(FileDialogError::InvalidPath);
    }
    Ok(Some(PathBuf::from(path)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_selected_path_and_treats_exit_one_as_cancel() {
        let selected = Command::new("/bin/sh")
            .args(["-c", "printf '/tmp/connections.toml\\n'"])
            .output()
            .unwrap();
        assert_eq!(
            parse_output(selected).unwrap(),
            Some(PathBuf::from("/tmp/connections.toml"))
        );

        let cancelled = Command::new("/bin/sh")
            .args(["-c", "exit 1"])
            .output()
            .unwrap();
        assert_eq!(parse_output(cancelled).unwrap(), None);
    }
}
