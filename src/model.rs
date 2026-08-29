use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppData {
    pub schema_version: u32,
    pub settings: Settings,
    pub profiles: Vec<Profile>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            settings: Settings::default(),
            profiles: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub start_with_system: bool,
    pub start_minimized: bool,
    pub close_to_tray: bool,
    pub show_session_controller: bool,
    pub auto_hide_session_controller: bool,
    pub theme: ThemeMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            start_with_system: false,
            start_minimized: false,
            close_to_tray: true,
            show_session_controller: true,
            auto_hide_session_controller: true,
            theme: ThemeMode::System,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub id: Uuid,
    pub name: String,
    pub favorite: bool,
    pub connection: Connection,
    pub display: Display,
    pub resources: Resources,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "New connection".to_owned(),
            favorite: false,
            connection: Connection::default(),
            display: Display::default(),
            resources: Resources::default(),
        }
    }
}

impl Profile {
    pub fn credential_id(&self) -> String {
        self.id.to_string()
    }

    pub fn duplicate(&self) -> Self {
        let mut copy = self.clone();
        copy.id = Uuid::new_v4();
        copy.name = format!("{} copy", self.name);
        copy.connection.save_password = false;
        copy
    }

    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.name.trim().is_empty() {
            return Err(ProfileError::MissingName);
        }
        if self.connection.host.trim().is_empty() {
            return Err(ProfileError::MissingHost);
        }
        if self.connection.port == 0 {
            return Err(ProfileError::InvalidPort);
        }
        if let Some(resolution) = self.display.resolution
            && (resolution.width < 320 || resolution.height < 240)
        {
            return Err(ProfileError::InvalidResolution);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Connection {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub domain: String,
    /// Only the preference is persisted. The password itself lives in Secret Service.
    pub save_password: bool,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 3389,
            username: String::new(),
            domain: String::new(),
            save_password: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Display {
    pub dynamic_resolution: bool,
    pub mode: DisplayMode,
    pub resolution: Option<Resolution>,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            dynamic_resolution: true,
            mode: DisplayMode::BorderlessMaximized,
            resolution: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DisplayMode {
    Windowed,
    #[default]
    BorderlessMaximized,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Resources {
    pub clipboard: bool,
    pub printers: bool,
    pub audio: bool,
    pub microphone: bool,
    pub drives: Vec<Drive>,
}

impl Default for Resources {
    fn default() -> Self {
        Self {
            clipboard: true,
            printers: true,
            audio: true,
            microphone: false,
            drives: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drive {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProfileError {
    #[error("Enter a name for this connection")]
    MissingName,
    #[error("Enter the hostname or IP address of the remote computer")]
    MissingHost,
    #[error("The port must be between 1 and 65535")]
    InvalidPort,
    #[error("The explicit resolution is too small")]
    InvalidResolution,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_crisp_and_convenient() {
        let profile = Profile::default();
        assert!(profile.display.dynamic_resolution);
        assert_eq!(profile.display.mode, DisplayMode::BorderlessMaximized);
        assert!(profile.resources.clipboard);
        assert!(profile.resources.audio);
        assert!(profile.resources.printers);
        assert!(!profile.resources.microphone);
    }

    #[test]
    fn duplicate_gets_new_identity_and_never_inherits_saved_secret_preference() {
        let mut original = Profile::default();
        original.connection.save_password = true;
        let copy = original.duplicate();
        assert_ne!(copy.id, original.id);
        assert!(!copy.connection.save_password);
    }
}
