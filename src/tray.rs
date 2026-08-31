use crate::{desktop, model::Profile};
use crossbeam_channel::Sender;
use eframe::egui::Context;
use ksni::blocking::{Handle, TrayMethods};
use std::sync::LazyLock;
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
    repaint: Context,
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
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        static ICONS: LazyLock<Vec<ksni::Icon>> = LazyLock::new(|| {
            [
                include_bytes!("../assets/rustrdp-16.png").as_slice(),
                include_bytes!("../assets/rustrdp-22.png").as_slice(),
                include_bytes!("../assets/rustrdp-32.png").as_slice(),
                include_bytes!("../assets/rustrdp-48.png").as_slice(),
                include_bytes!("../assets/rustrdp-64.png").as_slice(),
            ]
            .into_iter()
            .map(tray_icon_from_png)
            .collect()
        });
        ICONS.clone()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        tracing::debug!("tray requested main window");
        let _ = self.actions.send(TrayAction::OpenWindow);
        wake_app(&self.repaint);
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
                let repaint = self.repaint.clone();
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
                            tracing::debug!(profile_id = %id, "tray requested connection");
                            let _ = actions.send(TrayAction::Connect(id));
                            wake_ui(&repaint);
                        }),
                        ..Default::default()
                    }
                    .into(),
                );
            }
            menu.push(MenuItem::Separator);
        }
        let actions = self.actions.clone();
        let repaint = self.repaint.clone();
        menu.push(
            StandardItem {
                label: "Open RustRDP".to_owned(),
                icon_name: "rustrdp".to_owned(),
                activate: Box::new(move |_| {
                    tracing::debug!("tray requested main window");
                    let _ = actions.send(TrayAction::OpenWindow);
                    wake_app(&repaint);
                }),
                ..Default::default()
            }
            .into(),
        );
        let actions = self.actions.clone();
        let repaint = self.repaint.clone();
        menu.push(
            StandardItem {
                label: "Settings".to_owned(),
                icon_name: "settings-configure".to_owned(),
                activate: Box::new(move |_| {
                    tracing::debug!("tray requested settings");
                    let _ = actions.send(TrayAction::OpenSettings);
                    wake_app(&repaint);
                }),
                ..Default::default()
            }
            .into(),
        );
        menu.push(MenuItem::Separator);
        let actions = self.actions.clone();
        let repaint = self.repaint.clone();
        menu.push(
            StandardItem {
                label: "Quit".to_owned(),
                icon_name: "application-exit".to_owned(),
                activate: Box::new(move |_| {
                    tracing::debug!("tray requested quit");
                    let _ = actions.send(TrayAction::Quit);
                    wake_ui(&repaint);
                }),
                ..Default::default()
            }
            .into(),
        );
        menu
    }
}

fn tray_icon_from_png(bytes: &[u8]) -> ksni::Icon {
    let icon = eframe::icon_data::from_png_bytes(bytes)
        .expect("the bundled RustRDP tray icon must be a valid PNG");
    let mut data = icon.rgba;
    for pixel in data.chunks_exact_mut(4) {
        pixel.rotate_right(1);
    }
    ksni::Icon {
        width: icon.width as i32,
        height: icon.height as i32,
        data,
    }
}

pub struct TrayIntegration {
    handle: Handle<RustRdpTray>,
}

impl TrayIntegration {
    pub fn start(
        actions: Sender<TrayAction>,
        repaint: Context,
        profiles: &[Profile],
    ) -> Result<Self, String> {
        let tray = RustRdpTray {
            actions,
            repaint,
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

fn wake_app(repaint: &Context) {
    // KWin can withhold redraws from a minimized Wayland surface. Restore it
    // first so the queued action is guaranteed a frame in which to run.
    desktop::set_app_tray_hidden(std::process::id(), false);
    repaint.request_repaint();
}

fn wake_ui(repaint: &Context) {
    repaint.request_repaint();
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
