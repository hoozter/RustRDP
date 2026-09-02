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
    pub recent_connections: Vec<QuickConnection>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            settings: Settings::default(),
            profiles: Vec::new(),
            recent_connections: Vec::new(),
        }
    }
}

impl AppData {
    pub fn record_recent(&mut self, connection: QuickConnection) {
        self.recent_connections
            .retain(|recent| recent != &connection);
        self.recent_connections.insert(0, connection);
        self.recent_connections.truncate(8);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub start_with_system: bool,
    pub start_minimized: bool,
    pub close_to_tray: bool,
    pub theme: ThemeMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            start_with_system: false,
            start_minimized: false,
            close_to_tray: true,
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
        if !(100..=500).contains(&self.display.scale_percent) {
            return Err(ProfileError::InvalidScale);
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuickConnection {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub domain: String,
}

impl Default for QuickConnection {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 3389,
            username: String::new(),
            domain: String::new(),
        }
    }
}

impl QuickConnection {
    pub fn to_profile(&self) -> Profile {
        let mut profile = Profile {
            name: self.host.trim().to_owned(),
            ..Profile::default()
        };
        profile.connection.host = self.host.trim().to_owned();
        profile.connection.port = self.port;
        profile.connection.username = self.username.trim().to_owned();
        profile.connection.domain = self.domain.trim().to_owned();
        profile.connection.save_password = false;
        profile
    }
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
    pub match_local_scale: bool,
    pub scale_percent: u16,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            dynamic_resolution: false,
            mode: DisplayMode::BorderlessMaximized,
            resolution: None,
            match_local_scale: false,
            scale_percent: 100,
        }
    }
}

impl Display {
    pub fn effective_scale_percent(&self, local_scale_percent: Option<u16>) -> u16 {
        let percent = if self.match_local_scale {
            local_scale_percent.unwrap_or(100)
        } else {
            self.scale_percent
        };
        percent.clamp(100, 500)
    }

    pub fn dynamic_resolution_available(&self, local_scale_percent: Option<u16>) -> bool {
        self.mode == DisplayMode::Windowed
            && local_scale_percent.is_none_or(|percent| percent / 100 * 100 == percent)
    }

    pub fn constrain_to_supported_mode(
        &mut self,
        preferred_resolution: Resolution,
        local_scale_percent: Option<u16>,
    ) -> bool {
        if self.dynamic_resolution && !self.dynamic_resolution_available(local_scale_percent) {
            self.dynamic_resolution = false;
            self.resolution = Some(preferred_resolution);
            true
        } else {
            false
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
    #[error("Remote scaling must be between 100% and 500%")]
    InvalidScale,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_crisp_and_convenient() {
        let profile = Profile::default();
        assert!(!profile.display.dynamic_resolution);
        assert_eq!(profile.display.mode, DisplayMode::BorderlessMaximized);
        assert_eq!(profile.display.effective_scale_percent(None), 100);
        assert!(profile.resources.clipboard);
        assert!(profile.resources.audio);
        assert!(profile.resources.printers);
        assert!(!profile.resources.microphone);
    }

    #[test]
    fn borderless_and_fractional_display_modes_fall_back_to_fixed_resolution() {
        let preferred = Resolution {
            width: 2880,
            height: 1800,
        };
        let mut borderless = Display {
            dynamic_resolution: true,
            ..Display::default()
        };
        assert!(borderless.constrain_to_supported_mode(preferred, Some(100)));
        assert!(!borderless.dynamic_resolution);
        assert_eq!(borderless.resolution, Some(preferred));

        let mut fractional = Display {
            dynamic_resolution: true,
            mode: DisplayMode::Windowed,
            ..Display::default()
        };
        assert!(fractional.constrain_to_supported_mode(preferred, Some(175)));
        assert!(!fractional.dynamic_resolution);
        assert_eq!(fractional.resolution, Some(preferred));
    }

    #[test]
    fn live_resize_remains_available_for_an_unscaled_window() {
        let mut display = Display {
            dynamic_resolution: true,
            mode: DisplayMode::Windowed,
            resolution: None,
            ..Display::default()
        };

        assert!(!display.constrain_to_supported_mode(
            Resolution {
                width: 1920,
                height: 1080,
            },
            Some(100),
        ));
        assert!(display.dynamic_resolution);
        assert_eq!(display.resolution, None);
    }

    #[test]
    fn duplicate_gets_new_identity_and_never_inherits_saved_secret_preference() {
        let mut original = Profile::default();
        original.connection.save_password = true;
        let copy = original.duplicate();
        assert_ne!(copy.id, original.id);
        assert!(!copy.connection.save_password);
    }

    #[test]
    fn recent_connections_are_deduplicated_most_recent_first_and_bounded() {
        let mut data = AppData::default();
        for index in 0..12 {
            data.record_recent(QuickConnection {
                host: format!("server-{index}.example.com"),
                ..QuickConnection::default()
            });
        }
        assert_eq!(data.recent_connections.len(), 8);
        assert_eq!(data.recent_connections[0].host, "server-11.example.com");

        let repeated = data.recent_connections[4].clone();
        data.record_recent(repeated.clone());
        assert_eq!(data.recent_connections.len(), 8);
        assert_eq!(data.recent_connections[0], repeated);
    }

    #[test]
    fn quick_connection_becomes_an_unsaved_profile_without_secret_preferences() {
        let quick = QuickConnection {
            host: "lab.example.com".to_owned(),
            port: 3390,
            username: "david".to_owned(),
            domain: "LAB".to_owned(),
        };
        let profile = quick.to_profile();
        assert_eq!(profile.name, "lab.example.com");
        assert_eq!(profile.connection.host, quick.host);
        assert_eq!(profile.connection.port, 3390);
        assert!(!profile.connection.save_password);
    }
}
