use crate::model::Profile;
use crossbeam_channel::Sender;
use ksni::blocking::{Handle, TrayMethods};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum TrayAction {
    OpenWindow,
    OpenSettings,
    Connect(Uuid),
    Quit,
}

#[derive(Clone, Debug)]
struct TrayProfile {
    id: Uuid,
    name: String,
    favorite: bool,
}

#[derive(Clone, Debug)]
struct RustRdpTray {
    actions: Sender<TrayAction>,
    profiles: Vec<TrayProfile>,
    active_sessions: Vec<String>,
}

impl ksni::Tray for RustRdpTray {
    fn id(&self) -> String {
        "rustrdp".to_owned()
    }

    fn title(&self) -> String {
        "RustRDP".to_owned()
    }

    fn icon_name(&self) -> String {
        "rustrdp".to_owned()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.actions.send(TrayAction::OpenWindow);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{MenuItem, StandardItem};
        let mut menu = Vec::new();
        if !self.active_sessions.is_empty() {
            menu.push(
                StandardItem {
                    label: "Active sessions".to_owned(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
            for name in &self.active_sessions {
                menu.push(
                    StandardItem {
                        label: format!("● {name}"),
                        enabled: false,
                        ..Default::default()
                    }
                    .into(),
                );
            }
            menu.push(MenuItem::Separator);
        }
        if !self.profiles.is_empty() {
            menu.push(
                StandardItem {
                    label: "Connections".to_owned(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
            let mut profiles = self.profiles.clone();
            profiles.sort_by_key(|profile| (!profile.favorite, profile.name.to_lowercase()));
            for profile in profiles {
                let actions = self.actions.clone();
                let id = profile.id;
                menu.push(
                    StandardItem {
                        label: if profile.favorite {
                            format!("★ {}", profile.name)
                        } else {
                            profile.name
                        },
                        icon_name: "network-connect".to_owned(),
                        activate: Box::new(move |_| {
                            let _ = actions.send(TrayAction::Connect(id));
                        }),
                        ..Default::default()
                    }
                    .into(),
                );
            }
            menu.push(MenuItem::Separator);
        }
        let actions = self.actions.clone();
        menu.push(
            StandardItem {
                label: "Open RustRDP".to_owned(),
                icon_name: "rustrdp".to_owned(),
                activate: Box::new(move |_| {
                    let _ = actions.send(TrayAction::OpenWindow);
                }),
                ..Default::default()
            }
            .into(),
        );
        let actions = self.actions.clone();
        menu.push(
            StandardItem {
                label: "Settings".to_owned(),
                icon_name: "settings-configure".to_owned(),
                activate: Box::new(move |_| {
                    let _ = actions.send(TrayAction::OpenSettings);
                }),
                ..Default::default()
            }
            .into(),
        );
        menu.push(MenuItem::Separator);
        let actions = self.actions.clone();
        menu.push(
            StandardItem {
                label: "Quit".to_owned(),
                icon_name: "application-exit".to_owned(),
                activate: Box::new(move |_| {
                    let _ = actions.send(TrayAction::Quit);
                }),
                ..Default::default()
            }
            .into(),
        );
        menu
    }
}

pub struct TrayIntegration {
    handle: Handle<RustRdpTray>,
}

impl TrayIntegration {
    pub fn start(actions: Sender<TrayAction>, profiles: &[Profile]) -> Result<Self, String> {
        let tray = RustRdpTray {
            actions,
            profiles: snapshot_profiles(profiles),
            active_sessions: Vec::new(),
        };
        let handle = tray.spawn().map_err(|error| error.to_string())?;
        Ok(Self { handle })
    }

    pub fn update(&self, profiles: &[Profile], active_sessions: Vec<String>) {
        let profiles = snapshot_profiles(profiles);
        self.handle.update(move |tray| {
            tray.profiles = profiles;
            tray.active_sessions = active_sessions;
        });
    }
}

fn snapshot_profiles(profiles: &[Profile]) -> Vec<TrayProfile> {
    profiles
        .iter()
        .map(|profile| TrayProfile {
            id: profile.id,
            name: profile.name.clone(),
            favorite: profile.favorite,
        })
        .collect()
}
