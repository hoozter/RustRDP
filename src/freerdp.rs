use crate::model::{DisplayMode, Drive, Profile};
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FreeRdpBackend {
    pub executable: PathBuf,
    pub version: String,
    pub capabilities: Capabilities,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub dynamic_resolution: bool,
    pub credential_stdin: bool,
    pub clipboard: bool,
    pub printers: bool,
    pub audio: bool,
    pub microphone: bool,
    pub drives: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionCommand {
    pub program: PathBuf,
    pub arguments: Vec<OsString>,
    pub password_via_stdin: bool,
}

impl ConnectionCommand {
    pub fn display_redacted(&self) -> String {
        std::iter::once(self.program.to_string_lossy().into_owned())
            .chain(
                self.arguments
                    .iter()
                    .map(|argument| shell_quote(&argument.to_string_lossy())),
            )
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("The FreeRDP SDL3 client was not found. Install the freerdp-sdl package.")]
    NotFound,
    #[error("this FreeRDP client cannot receive credentials safely through standard input")]
    UnsafeCredentialHandoff,
    #[error("the selected FreeRDP client does not support dynamic resolution")]
    DynamicResolutionUnsupported,
}

impl FreeRdpBackend {
    pub fn detect() -> Result<Self, BackendError> {
        find_executable("sdl-freerdp")
            .map(Self::inspect)
            .ok_or(BackendError::NotFound)
    }

    fn inspect(executable: PathBuf) -> Self {
        let version_output = Command::new(&executable).arg("/version").output();
        let version = version_output
            .ok()
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .chain(String::from_utf8_lossy(&output.stderr).lines())
                    .find(|line| line.contains("FreeRDP version"))
                    .unwrap_or("FreeRDP version unknown")
                    .trim()
                    .to_owned()
            })
            .unwrap_or_else(|| "FreeRDP version unknown".to_owned());
        let help = Command::new(&executable)
            .arg("/help")
            .output()
            .ok()
            .map(|output| {
                let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
                text.push_str(&String::from_utf8_lossy(&output.stderr));
                text
            })
            .unwrap_or_default();
        let has = |needle: &str| help.contains(needle);
        Self {
            executable,
            version,
            capabilities: Capabilities {
                dynamic_resolution: has("dynamic-resolution"),
                credential_stdin: has("from-stdin"),
                clipboard: has("clipboard"),
                printers: has("/printer"),
                audio: has("/sound"),
                microphone: has("/microphone"),
                drives: has("/drive"),
            },
        }
    }

    pub fn build_connection(
        &self,
        profile: &Profile,
        has_password: bool,
    ) -> Result<ConnectionCommand, BackendError> {
        if has_password && !self.capabilities.credential_stdin {
            return Err(BackendError::UnsafeCredentialHandoff);
        }
        if profile.display.dynamic_resolution && !self.capabilities.dynamic_resolution {
            return Err(BackendError::DynamicResolutionUnsupported);
        }
        let mut arguments = vec![OsString::from(format!(
            "/v:{}",
            connection_target(profile.connection.host.trim(), profile.connection.port)
        ))];
        // Trust on first use persists the first certificate and rejects a
        // changed certificate later, without requiring an unusable terminal prompt.
        arguments.push(OsString::from("/cert:tofu"));
        if !profile.connection.username.trim().is_empty() {
            arguments.push(OsString::from(format!(
                "/u:{}",
                profile.connection.username.trim()
            )));
        }
        if !profile.connection.domain.trim().is_empty() {
            arguments.push(OsString::from(format!(
                "/d:{}",
                profile.connection.domain.trim()
            )));
        }
        if has_password {
            arguments.push(OsString::from("/from-stdin:force"));
        }
        if profile.display.dynamic_resolution {
            arguments.push(OsString::from("+dynamic-resolution"));
        }
        match profile.display.mode {
            DisplayMode::Windowed => {}
            DisplayMode::BorderlessMaximized => arguments.push(OsString::from("-decorations")),
            DisplayMode::Fullscreen => arguments.push(OsString::from("+f")),
        }
        if let Some(resolution) = profile.display.resolution {
            arguments.push(OsString::from(format!(
                "/size:{}x{}",
                resolution.width, resolution.height
            )));
        }
        arguments.push(OsString::from(format!("/t:{}", profile.name)));
        arguments.push(if profile.resources.clipboard {
            OsString::from("+clipboard")
        } else {
            OsString::from("-clipboard")
        });
        if profile.resources.printers {
            arguments.push(OsString::from("/printer"));
        }
        if profile.resources.audio {
            arguments.push(OsString::from("/sound"));
        }
        if profile.resources.microphone {
            arguments.push(OsString::from("/microphone"));
        }
        for drive in &profile.resources.drives {
            arguments.push(drive_argument(drive));
        }
        arguments.push(OsString::from("+auto-reconnect"));
        Ok(ConnectionCommand {
            program: self.executable.clone(),
            arguments,
            password_via_stdin: has_password,
        })
    }
}

fn connection_target(host: &str, port: u16) -> String {
    if host.contains(':') && !(host.starts_with('[') && host.ends_with(']')) {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.components().count() > 1 && path.is_file() {
        return Some(path.to_owned());
    }
    env::split_paths(&env::var_os("PATH")?).find_map(|directory| {
        let candidate = directory.join(name);
        candidate.is_file().then_some(candidate)
    })
}

fn drive_argument(drive: &Drive) -> OsString {
    let name: String = drive
        .name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();
    OsString::from(format!(
        "/drive:{},{}",
        if name.is_empty() { "share" } else { &name },
        drive.path.display()
    ))
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-._/:+".contains(character))
    {
        value.to_owned()
    } else {
        format!("'{value}'")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DisplayMode, Drive, Profile, Resolution};

    fn capable_backend() -> FreeRdpBackend {
        FreeRdpBackend {
            executable: PathBuf::from("/usr/bin/sdl-freerdp"),
            version: "3.30.0".to_owned(),
            capabilities: Capabilities {
                dynamic_resolution: true,
                credential_stdin: true,
                clipboard: true,
                printers: true,
                audio: true,
                microphone: true,
                drives: true,
            },
        }
    }

    #[test]
    fn command_uses_secure_stdin_and_maps_profile_options() {
        let mut profile = Profile {
            name: "Work PC".to_owned(),
            ..Profile::default()
        };
        profile.connection.host = "work.example.com".to_owned();
        profile.connection.port = 3390;
        profile.connection.username = "david".to_owned();
        profile.connection.domain = "WORK".to_owned();
        profile.display.mode = DisplayMode::Fullscreen;
        profile.display.resolution = Some(Resolution {
            width: 1920,
            height: 1080,
        });
        profile.resources.microphone = true;
        profile.resources.drives.push(Drive {
            name: "My files".to_owned(),
            path: PathBuf::from("/home/david/Documents"),
        });

        let command = capable_backend().build_connection(&profile, true).unwrap();
        let args: Vec<_> = command
            .arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"/from-stdin:force".to_owned()));
        assert!(args.contains(&"/cert:tofu".to_owned()));
        assert!(args.contains(&"+dynamic-resolution".to_owned()));
        assert!(args.contains(&"+f".to_owned()));
        assert!(args.contains(&"/size:1920x1080".to_owned()));
        assert!(args.contains(&"/microphone".to_owned()));
        assert!(args.contains(&"/drive:My_files,/home/david/Documents".to_owned()));
        assert!(!command.display_redacted().contains("password"));
    }

    #[test]
    fn formats_ipv4_hostnames_and_ipv6_targets_unambiguously() {
        assert_eq!(
            connection_target("work.example.com", 3389),
            "work.example.com:3389"
        );
        assert_eq!(connection_target("2001:db8::1", 3390), "[2001:db8::1]:3390");
        assert_eq!(
            connection_target("[2001:db8::1]", 3390),
            "[2001:db8::1]:3390"
        );
    }

    #[test]
    fn refuses_password_when_backend_cannot_keep_it_out_of_process_arguments() {
        let mut backend = capable_backend();
        backend.capabilities.credential_stdin = false;
        assert!(matches!(
            backend.build_connection(&Profile::default(), true),
            Err(BackendError::UnsafeCredentialHandoff)
        ));
    }
}
