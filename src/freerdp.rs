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
    pub smart_sizing: bool,
    pub display_scaling: bool,
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
    #[error("dynamic resolution is only available in a resizable window")]
    DynamicResolutionModeUnsupported,
    #[error("the selected FreeRDP client does not support fixed-resolution scaling")]
    SmartSizingUnsupported,
    #[error("the selected FreeRDP client does not support remote display scaling")]
    DisplayScalingUnsupported,
    #[error("the selected FreeRDP client does not support {0} redirection")]
    RedirectionUnsupported(&'static str),
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
                smart_sizing: has("smart-sizing"),
                display_scaling: has("scale-desktop") && has("scale-device"),
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
        local_scale_percent: Option<u16>,
    ) -> Result<ConnectionCommand, BackendError> {
        if has_password && !self.capabilities.credential_stdin {
            return Err(BackendError::UnsafeCredentialHandoff);
        }
        if profile.display.dynamic_resolution && !self.capabilities.dynamic_resolution {
            return Err(BackendError::DynamicResolutionUnsupported);
        }
        if profile.display.dynamic_resolution && profile.display.mode != DisplayMode::Windowed {
            return Err(BackendError::DynamicResolutionModeUnsupported);
        }
        if !profile.display.dynamic_resolution
            && profile.display.resolution.is_some()
            && !self.capabilities.smart_sizing
        {
            return Err(BackendError::SmartSizingUnsupported);
        }
        let scale_percent = profile.display.effective_scale_percent(local_scale_percent);
        if scale_percent != 100 && !self.capabilities.display_scaling {
            return Err(BackendError::DisplayScalingUnsupported);
        }
        for (enabled, supported, name) in [
            (
                profile.resources.clipboard,
                self.capabilities.clipboard,
                "clipboard",
            ),
            (
                profile.resources.printers,
                self.capabilities.printers,
                "printer",
            ),
            (profile.resources.audio, self.capabilities.audio, "audio"),
            (
                profile.resources.microphone,
                self.capabilities.microphone,
                "microphone",
            ),
            (
                !profile.resources.drives.is_empty(),
                self.capabilities.drives,
                "local folder",
            ),
        ] {
            if enabled && !supported {
                return Err(BackendError::RedirectionUnsupported(name));
            }
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
            DisplayMode::BorderlessMaximized => {
                arguments.push(OsString::from("-decorations"));
                arguments.push(OsString::from("+workarea"));
            }
            DisplayMode::Fullscreen => arguments.push(OsString::from("+f")),
        }
        if !profile.display.dynamic_resolution
            && let Some(resolution) = profile.display.resolution
        {
            arguments.push(OsString::from(format!(
                "/size:{}x{}",
                resolution.width, resolution.height
            )));
            // SDL otherwise replaces DesktopWidth/DesktopHeight with the local
            // monitor dimensions during startup, making every fixed preset
            // produce the same remote resolution.
            arguments.push(OsString::from("/smart-sizing"));
        }
        if scale_percent != 100 {
            arguments.push(OsString::from(format!("/scale-desktop:{scale_percent}")));
            arguments.push(OsString::from(format!(
                "/scale-device:{}",
                device_scale_percent(scale_percent)
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

fn device_scale_percent(desktop_scale_percent: u16) -> u16 {
    [100_u16, 140, 180]
        .into_iter()
        .min_by_key(|candidate| candidate.abs_diff(desktop_scale_percent))
        .unwrap_or(100)
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
                smart_sizing: true,
                display_scaling: true,
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
        profile.display.dynamic_resolution = false;
        profile.display.resolution = Some(Resolution {
            width: 1920,
            height: 1080,
        });
        profile.resources.microphone = true;
        profile.resources.drives.push(Drive {
            name: "My files".to_owned(),
            path: PathBuf::from("/home/david/Documents"),
        });

        let command = capable_backend()
            .build_connection(&profile, true, None)
            .unwrap();
        let args: Vec<_> = command
            .arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"/from-stdin:force".to_owned()));
        assert!(args.contains(&"/cert:tofu".to_owned()));
        assert!(!args.contains(&"+dynamic-resolution".to_owned()));
        assert!(args.contains(&"+f".to_owned()));
        assert!(!args.contains(&"-grab-keyboard".to_owned()));
        assert!(!args.iter().any(|arg| arg.starts_with("/floatbar")));
        assert!(args.contains(&"/size:1920x1080".to_owned()));
        assert!(args.contains(&"/smart-sizing".to_owned()));
        assert!(args.contains(&"/microphone".to_owned()));
        assert!(args.contains(&"/drive:My_files,/home/david/Documents".to_owned()));
        assert!(!command.display_redacted().contains("password"));
    }

    #[test]
    fn borderless_mode_fills_the_workarea_without_exclusive_fullscreen() {
        let mut profile = Profile::default();
        profile.display.mode = DisplayMode::BorderlessMaximized;
        let command = capable_backend()
            .build_connection(&profile, false, None)
            .unwrap();
        let args: Vec<_> = command
            .arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();

        assert!(args.contains(&"-decorations".to_owned()));
        assert!(args.contains(&"+workarea".to_owned()));
        assert!(!args.contains(&"+f".to_owned()));
    }

    #[test]
    fn dynamic_resolution_does_not_also_force_a_stale_fixed_size() {
        let mut profile = Profile::default();
        profile.display.mode = DisplayMode::Windowed;
        profile.display.dynamic_resolution = true;
        profile.display.resolution = Some(Resolution {
            width: 1920,
            height: 1080,
        });
        let command = capable_backend()
            .build_connection(&profile, false, None)
            .unwrap();
        let args: Vec<_> = command
            .arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();

        assert!(args.contains(&"+dynamic-resolution".to_owned()));
        assert!(!args.iter().any(|argument| argument.starts_with("/size:")));
    }

    #[test]
    fn dynamic_resolution_rejects_non_resizable_desktop_modes() {
        let mut profile = Profile::default();
        profile.display.mode = DisplayMode::BorderlessMaximized;
        profile.display.dynamic_resolution = true;

        assert!(
            capable_backend()
                .build_connection(&profile, false, Some(100))
                .is_err()
        );
    }

    #[test]
    fn dynamic_resolution_supports_fractional_remote_scaling() {
        let mut profile = Profile::default();
        profile.display.mode = DisplayMode::Windowed;
        profile.display.dynamic_resolution = true;

        let command = capable_backend()
            .build_connection(&profile, false, Some(175))
            .unwrap();
        let args: Vec<_> = command
            .arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"+dynamic-resolution".to_owned()));
    }

    #[test]
    fn local_fractional_scale_maps_to_exact_desktop_and_nearest_device_scale() {
        let mut profile = Profile::default();
        profile.display.match_local_scale = true;
        let command = capable_backend()
            .build_connection(&profile, false, Some(175))
            .unwrap();
        let args: Vec<_> = command
            .arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();

        assert!(args.contains(&"/scale-desktop:175".to_owned()));
        assert!(args.contains(&"/scale-device:180".to_owned()));
    }

    #[test]
    fn one_hundred_percent_scale_keeps_freerdp_defaults() {
        let command = capable_backend()
            .build_connection(&Profile::default(), false, Some(175))
            .unwrap();
        assert!(
            !command
                .arguments
                .iter()
                .any(|argument| argument.to_string_lossy().starts_with("/scale-"))
        );
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
            backend.build_connection(&Profile::default(), true, None),
            Err(BackendError::UnsafeCredentialHandoff)
        ));
    }

    #[test]
    fn rejects_selected_redirection_that_the_backend_does_not_support() {
        let mut backend = capable_backend();
        backend.capabilities.printers = false;
        assert!(matches!(
            backend.build_connection(&Profile::default(), false, None),
            Err(BackendError::RedirectionUnsupported("printer"))
        ));
    }
}
