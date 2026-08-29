use crate::autostart;
use crate::credentials::CredentialStore;
use crate::freerdp::FreeRdpBackend;
use crate::icons;
use crate::model::{AppData, DisplayMode, Drive, Profile, Resolution, ThemeMode};
use crate::sessions::{SessionManager, SessionState};
use crate::storage;
use crate::theme::{self, Colors};
use crate::tray::{TrayAction, TrayIntegration};
use crossbeam_channel::{Receiver, unbounded};
use eframe::egui::{self, Align, Color32, Layout, RichText, Vec2};
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;
use zeroize::Zeroize;

pub struct RustRdpApp {
    data: AppData,
    config_path: Option<PathBuf>,
    backend: Result<FreeRdpBackend, String>,
    sessions: SessionManager,
    selected: Option<Uuid>,
    draft: Option<Profile>,
    search: String,
    show_settings: bool,
    password_prompt: Option<PasswordPrompt>,
    delete_prompt: Option<DeletePrompt>,
    status: Option<StatusMessage>,
    tray: Option<TrayIntegration>,
    tray_actions: Receiver<TrayAction>,
    quitting: bool,
    colors: Colors,
}

struct PasswordPrompt {
    profile_id: Uuid,
    password: String,
    remember: bool,
    visible: bool,
    error: Option<String>,
}

struct DeletePrompt {
    profile_id: Uuid,
    remove_credential: bool,
}

struct StatusMessage {
    text: String,
    is_error: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditorAction {
    None,
    Save,
    Cancel,
    Connect,
}

impl RustRdpApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = storage::default_config_path().ok();
        let (data, load_error) = match config_path.as_deref().map(storage::load) {
            Some(Ok(data)) => (data, None),
            Some(Err(error)) => (AppData::default(), Some(error.to_string())),
            None => (
                AppData::default(),
                Some("Could not determine the XDG configuration directory".to_owned()),
            ),
        };
        let colors = theme::apply(&cc.egui_ctx, data.settings.theme);
        let backend = FreeRdpBackend::detect().map_err(|error| error.to_string());
        let (tray_tx, tray_actions) = unbounded();
        let tray = match TrayIntegration::start(tray_tx, &data.profiles) {
            Ok(tray) => Some(tray),
            Err(error) => {
                tracing::warn!(%error, "system tray unavailable");
                None
            }
        };
        let selected = data.profiles.first().map(|profile| profile.id);
        Self {
            data,
            config_path,
            backend,
            sessions: SessionManager::default(),
            selected,
            draft: None,
            search: String::new(),
            show_settings: false,
            password_prompt: None,
            delete_prompt: None,
            status: load_error.map(|text| StatusMessage {
                text,
                is_error: true,
            }),
            tray,
            tray_actions,
            quitting: false,
            colors,
        }
    }

    fn persist(&mut self) {
        let Some(path) = self.config_path.as_deref() else {
            return;
        };
        if let Err(error) = storage::save(path, &self.data) {
            self.error(error.to_string());
        }
        self.update_tray();
    }

    fn update_tray(&self) {
        if let Some(tray) = &self.tray {
            let active = self
                .sessions
                .sessions()
                .filter(|session| {
                    matches!(
                        session.state,
                        SessionState::Connecting
                            | SessionState::Active
                            | SessionState::Disconnecting
                    )
                })
                .map(|session| session.profile_name.clone())
                .collect();
            tray.update(&self.data.profiles, active);
        }
    }

    fn error(&mut self, text: impl Into<String>) {
        self.status = Some(StatusMessage {
            text: text.into(),
            is_error: true,
        });
    }

    fn success(&mut self, text: impl Into<String>) {
        self.status = Some(StatusMessage {
            text: text.into(),
            is_error: false,
        });
    }

    fn selected_profile(&self) -> Option<&Profile> {
        let id = self.selected?;
        self.data.profiles.iter().find(|profile| profile.id == id)
    }

    fn begin_new(&mut self) {
        self.selected = None;
        self.draft = Some(Profile::default());
    }

    fn begin_edit(&mut self, profile_id: Uuid) {
        self.selected = Some(profile_id);
        self.draft = self
            .data
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned();
    }

    fn save_draft(&mut self) -> bool {
        let Some(draft) = self.draft.as_ref() else {
            return false;
        };
        if let Err(error) = draft.validate() {
            self.error(error.to_string());
            return false;
        }
        let draft = draft.clone();
        let old_saved = self
            .data
            .profiles
            .iter()
            .find(|profile| profile.id == draft.id)
            .is_some_and(|profile| profile.connection.save_password);
        if let Some(existing) = self
            .data
            .profiles
            .iter_mut()
            .find(|profile| profile.id == draft.id)
        {
            *existing = draft.clone();
        } else {
            self.data.profiles.push(draft.clone());
        }
        if old_saved
            && !draft.connection.save_password
            && let Err(error) = CredentialStore::delete(&draft.credential_id())
        {
            tracing::warn!(%error, "could not remove disabled saved credential");
        }
        self.selected = Some(draft.id);
        self.draft = None;
        self.persist();
        self.success("Connection saved");
        true
    }

    fn duplicate_selected(&mut self) {
        if let Some(profile) = self.selected_profile() {
            self.draft = Some(profile.duplicate());
            self.selected = None;
        }
    }

    fn request_connect(&mut self, profile_id: Uuid) {
        let Some(profile) = self
            .data
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned()
        else {
            self.error("Connection not found");
            return;
        };
        if let Err(error) = profile.validate() {
            self.error(error.to_string());
            return;
        }
        if profile.connection.save_password {
            match CredentialStore::retrieve(&profile.credential_id()) {
                Ok(password) => self.launch(profile, Some(password)),
                Err(error) => {
                    self.password_prompt = Some(PasswordPrompt {
                        profile_id,
                        password: String::new(),
                        remember: true,
                        visible: false,
                        error: Some(error.to_string()),
                    });
                }
            }
        } else {
            self.password_prompt = Some(PasswordPrompt {
                profile_id,
                password: String::new(),
                remember: false,
                visible: false,
                error: None,
            });
        }
    }

    fn launch(&mut self, profile: Profile, password: Option<String>) {
        let backend = match &self.backend {
            Ok(backend) => backend,
            Err(error) => {
                self.error(error.clone());
                return;
            }
        };
        let command = match backend.build_connection(&profile, password.is_some()) {
            Ok(command) => command,
            Err(error) => {
                self.error(error.to_string());
                return;
            }
        };
        tracing::info!(
            profile = %profile.name,
            command = %command.display_redacted(),
            "starting remote desktop session"
        );
        match self.sessions.launch(&profile, command, password) {
            Ok(_) => {
                self.success(format!("Connecting to {}…", profile.name));
                self.update_tray();
            }
            Err(error) => self.error(error.to_string()),
        }
    }

    fn process_tray_actions(&mut self, ctx: &egui::Context) {
        while let Ok(action) = self.tray_actions.try_recv() {
            match action {
                TrayAction::OpenWindow => show_main_window(ctx),
                TrayAction::OpenSettings => {
                    self.show_settings = true;
                    show_main_window(ctx);
                }
                TrayAction::Connect(id) => self.request_connect(id),
                TrayAction::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn show_top_bar(&mut self, root: &mut egui::Ui) {
        root.horizontal(|ui| {
            ui.label(
                RichText::new(icons::DESKTOP)
                    .size(25.0)
                    .color(self.colors.accent),
            );
            ui.label(
                RichText::new("RustRDP")
                    .size(20.0)
                    .strong()
                    .color(self.colors.heading),
            );
            ui.add_space(8.0);
            match &self.backend {
                Ok(backend) => {
                    status_chip(
                        ui,
                        &format!(
                            "{} · {}",
                            match backend.kind {
                                crate::freerdp::BackendKind::Sdl => "SDL",
                                crate::freerdp::BackendKind::Wayland => "Wayland",
                                crate::freerdp::BackendKind::X11 => "X11",
                            },
                            backend.version.replace("This is ", "")
                        ),
                        self.colors.success,
                    );
                }
                Err(_) => status_chip(ui, "FreeRDP not found", self.colors.error),
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .button(format!("{}  Settings", icons::SETTINGS))
                    .clicked()
                {
                    self.show_settings = true;
                }
            });
        });
        root.separator();
    }

    fn show_library(&mut self, root: &mut egui::Ui) -> Option<Uuid> {
        let mut connect = None;
        root.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading("Connections");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .button(format!("{}  New", icons::ADD))
                        .on_hover_text("Create a connection")
                        .clicked()
                    {
                        self.begin_new();
                    }
                });
            });
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search connections…")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(8.0);
            let search = self.search.trim().to_lowercase();
            let mut profiles: Vec<_> = self
                .data
                .profiles
                .iter()
                .filter(|profile| {
                    search.is_empty()
                        || profile.name.to_lowercase().contains(&search)
                        || profile.connection.host.to_lowercase().contains(&search)
                })
                .cloned()
                .collect();
            profiles.sort_by_key(|profile| (!profile.favorite, profile.name.to_lowercase()));
            if profiles.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    ui.label(
                        RichText::new(icons::DESKTOP)
                            .size(40.0)
                            .color(self.colors.muted),
                    );
                    ui.label(
                        RichText::new(if self.data.profiles.is_empty() {
                            "No connections yet"
                        } else {
                            "No matching connections"
                        })
                        .color(self.colors.dim),
                    );
                });
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for profile in profiles {
                    let selected = self.selected == Some(profile.id);
                    let marker = if profile.favorite {
                        icons::FAVORITE
                    } else {
                        icons::DESKTOP
                    };
                    let response = ui.selectable_label(
                        selected,
                        format!(
                            "{marker}  {}\n    {}",
                            profile.name, profile.connection.host
                        ),
                    );
                    if response.clicked() {
                        self.selected = Some(profile.id);
                        self.draft = None;
                    }
                    if response.double_clicked() {
                        connect = Some(profile.id);
                    }
                }
            });
        });
        connect
    }

    fn show_main_content(&mut self, root: &mut egui::Ui) {
        let mut editor_action = EditorAction::None;
        let mut summary_action = SummaryAction::None;
        root.add_space(12.0);
        if let Some(draft) = self.draft.as_mut() {
            editor_action = profile_editor(root, draft, self.colors);
        } else if let Some(profile) = self.selected_profile() {
            summary_action = profile_summary(root, profile, self.colors);
        } else {
            empty_state(root, self.colors);
        }
        root.add_space(16.0);
        session_list(root, &mut self.sessions, self.colors);
        match editor_action {
            EditorAction::None => {}
            EditorAction::Save => {
                self.save_draft();
            }
            EditorAction::Cancel => {
                self.draft = None;
                if self.selected.is_none() {
                    self.selected = self.data.profiles.first().map(|profile| profile.id);
                }
            }
            EditorAction::Connect => {
                if self.save_draft()
                    && let Some(id) = self.selected
                {
                    self.request_connect(id);
                }
            }
        }
        match summary_action {
            SummaryAction::None => {}
            SummaryAction::Connect(id) => self.request_connect(id),
            SummaryAction::Edit(id) => self.begin_edit(id),
            SummaryAction::Duplicate => self.duplicate_selected(),
            SummaryAction::Delete(id) => {
                let remove_credential = self
                    .data
                    .profiles
                    .iter()
                    .find(|profile| profile.id == id)
                    .is_some_and(|profile| profile.connection.save_password);
                self.delete_prompt = Some(DeletePrompt {
                    profile_id: id,
                    remove_credential,
                });
            }
            SummaryAction::ToggleFavorite(id) => {
                if let Some(profile) = self.data.profiles.iter_mut().find(|p| p.id == id) {
                    profile.favorite = !profile.favorite;
                    self.persist();
                }
            }
        }
    }

    fn show_settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut open = self.show_settings;
        let mut changed = false;
        let mut autostart_changed = None;
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .show(ctx, |ui| {
                section_heading(ui, "General", self.colors);
                if ui
                    .checkbox(
                        &mut self.data.settings.start_with_system,
                        "Start when I log in",
                    )
                    .changed()
                {
                    autostart_changed = Some(self.data.settings.start_with_system);
                    changed = true;
                }
                changed |= ui
                    .checkbox(
                        &mut self.data.settings.start_minimized,
                        "Start minimized to the system tray",
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut self.data.settings.close_to_tray,
                        "Close the main window to the tray",
                    )
                    .changed();
                ui.add_space(14.0);
                section_heading(ui, "Sessions", self.colors);
                changed |= ui
                    .checkbox(
                        &mut self.data.settings.show_session_controller,
                        "Show a floating controller for active sessions",
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut self.data.settings.auto_hide_session_controller,
                        "Keep the controller compact",
                    )
                    .changed();
                ui.add_space(14.0);
                section_heading(ui, "Security", self.colors);
                match CredentialStore::availability() {
                    Ok(()) => ui.label(format!(
                        "{}  Desktop wallet / Secret Service is available",
                        icons::LOCK
                    )),
                    Err(error) => ui.colored_label(self.colors.error, error.to_string()),
                };
                ui.label(
                    RichText::new("Passwords are never written to the RustRDP configuration file.")
                        .small()
                        .color(self.colors.muted),
                );
                ui.add_space(14.0);
                section_heading(ui, "Appearance", self.colors);
                egui::ComboBox::from_label("Theme")
                    .selected_text(theme_name(self.data.settings.theme))
                    .show_ui(ui, |ui| {
                        changed |= ui
                            .selectable_value(
                                &mut self.data.settings.theme,
                                ThemeMode::System,
                                "Follow system",
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(
                                &mut self.data.settings.theme,
                                ThemeMode::Light,
                                "Light",
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(
                                &mut self.data.settings.theme,
                                ThemeMode::Dark,
                                "Dark",
                            )
                            .changed();
                    });
            });
        self.show_settings = open;
        if let Some(enabled) = autostart_changed
            && let Err(error) = autostart::set_current_executable_enabled(enabled)
        {
            self.data.settings.start_with_system = !enabled;
            self.error(error.to_string());
        }
        if changed {
            self.colors = theme::apply(ctx, self.data.settings.theme);
            self.persist();
        }
    }

    fn show_password_prompt(&mut self, ctx: &egui::Context) {
        let Some(mut prompt) = self.password_prompt.take() else {
            return;
        };
        let profile = self
            .data
            .profiles
            .iter()
            .find(|profile| profile.id == prompt.profile_id)
            .cloned();
        let Some(profile) = profile else {
            return;
        };
        let mut keep_open = true;
        let mut connect = false;
        let mut connect_without = false;
        egui::Window::new(format!("Connect to {}", profile.name))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!(
                        "{}@{}",
                        if profile.connection.username.is_empty() {
                            "Remote Desktop"
                        } else {
                            &profile.connection.username
                        },
                        profile.connection.host
                    ))
                    .color(self.colors.dim),
                );
                if let Some(error) = &prompt.error {
                    ui.colored_label(self.colors.error, error);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Password");
                    let field = egui::TextEdit::singleline(&mut prompt.password)
                        .password(!prompt.visible)
                        .desired_width(280.0);
                    let response = ui.add(field);
                    if response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter))
                    {
                        connect = true;
                    }
                    if ui
                        .button(if prompt.visible {
                            icons::VISIBILITY_OFF
                        } else {
                            icons::VISIBILITY
                        })
                        .clicked()
                    {
                        prompt.visible = !prompt.visible;
                    }
                });
                ui.checkbox(&mut prompt.remember, "Save securely in the desktop wallet");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        keep_open = false;
                    }
                    if ui.button("Connect without password").clicked() {
                        connect_without = true;
                    }
                    if ui
                        .add_enabled(
                            !prompt.password.is_empty(),
                            egui::Button::new(format!("{}  Connect", icons::PLAY)),
                        )
                        .clicked()
                    {
                        connect = true;
                    }
                });
            });
        if connect {
            let password = std::mem::take(&mut prompt.password);
            if prompt.remember {
                match CredentialStore::store(&profile.credential_id(), &password) {
                    Ok(()) => {
                        if let Some(saved_profile) = self
                            .data
                            .profiles
                            .iter_mut()
                            .find(|saved| saved.id == profile.id)
                        {
                            saved_profile.connection.save_password = true;
                            self.persist();
                        }
                    }
                    Err(error) => {
                        prompt.error = Some(error.to_string());
                        prompt.password = password;
                        self.password_prompt = Some(prompt);
                        return;
                    }
                }
            }
            self.launch(profile, Some(password));
        } else if connect_without {
            prompt.password.zeroize();
            self.launch(profile, None);
        } else if keep_open {
            self.password_prompt = Some(prompt);
        } else {
            prompt.password.zeroize();
        }
    }

    fn show_delete_prompt(&mut self, ctx: &egui::Context) {
        let Some(mut prompt) = self.delete_prompt.take() else {
            return;
        };
        let profile = self
            .data
            .profiles
            .iter()
            .find(|profile| profile.id == prompt.profile_id)
            .cloned();
        let Some(profile) = profile else {
            return;
        };
        let mut keep = true;
        let mut delete = false;
        egui::Window::new("Delete connection?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Delete “{}” from RustRDP?", profile.name));
                if profile.connection.save_password {
                    ui.checkbox(
                        &mut prompt.remove_credential,
                        "Also remove its password from the desktop wallet",
                    );
                }
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        keep = false;
                    }
                    if ui
                        .add(egui::Button::new(format!("{}  Delete", icons::DELETE)))
                        .clicked()
                    {
                        delete = true;
                    }
                });
            });
        if delete {
            if prompt.remove_credential
                && let Err(error) = CredentialStore::delete(&profile.credential_id())
            {
                self.error(error.to_string());
                self.delete_prompt = Some(prompt);
                return;
            }
            self.data.profiles.retain(|saved| saved.id != profile.id);
            self.selected = self.data.profiles.first().map(|saved| saved.id);
            self.persist();
            self.success("Connection deleted");
        } else if keep {
            self.delete_prompt = Some(prompt);
        }
    }

    fn show_status(&mut self, root: &mut egui::Ui) {
        let Some(status) = &self.status else {
            return;
        };
        let mut dismiss = false;
        root.horizontal(|ui| {
            let color = if status.is_error {
                self.colors.error
            } else {
                self.colors.success
            };
            ui.colored_label(
                color,
                if status.is_error {
                    icons::ERROR
                } else {
                    icons::CHECK
                },
            );
            ui.colored_label(color, &status.text);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                dismiss = ui.button(icons::CLOSE).clicked();
            });
        });
        root.separator();
        if dismiss {
            self.status = None;
        }
    }

    fn show_session_controllers(&mut self, ctx: &egui::Context) {
        if !self.data.settings.show_session_controller {
            return;
        }
        let sessions: Vec<_> = self
            .sessions
            .sessions()
            .filter(|session| matches!(session.state, SessionState::Active))
            .map(|session| (session.id, session.profile_name.clone(), session.pid))
            .collect();
        let mut disconnect = Vec::new();
        for (id, name, pid) in sessions {
            let colors = self.colors;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of(("session-controller", id)),
                egui::ViewportBuilder::default()
                    .with_title(format!("{name} — RustRDP"))
                    .with_inner_size([380.0, 58.0])
                    .with_min_inner_size([280.0, 52.0])
                    .with_decorations(false)
                    .with_resizable(false)
                    .with_always_on_top(),
                |ui, _| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(icons::DESKTOP)
                                .size(20.0)
                                .color(colors.accent),
                        );
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&name).strong());
                            ui.label(
                                RichText::new(format!("Active · PID {pid}"))
                                    .small()
                                    .color(colors.muted),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button(format!("{}  Disconnect", icons::STOP)).clicked() {
                                disconnect.push(id);
                            }
                        });
                    });
                },
            );
        }
        for id in disconnect {
            if let Err(error) = self.sessions.disconnect(id) {
                self.error(error.to_string());
            }
        }
    }
}

impl eframe::App for RustRdpApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.process_tray_actions(&ctx);
        if self.sessions.poll() {
            self.update_tray();
        }
        if ctx.input(|input| input.viewport().close_requested())
            && !self.quitting
            && self.data.settings.close_to_tray
            && self.tray.is_some()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.success("RustRDP is still available in the system tray");
        }
        egui::CentralPanel::default().show(root, |ui| {
            self.show_top_bar(ui);
            self.show_status(ui);
            let content_height = ui.available_height();
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(285.0, content_height),
                    Layout::top_down(Align::Min),
                    |ui| {
                        if let Some(id) = self.show_library(ui) {
                            self.request_connect(id);
                        }
                    },
                );
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), content_height),
                    Layout::top_down(Align::Min),
                    |ui| self.show_main_content(ui),
                );
            });
        });
        self.show_settings_window(&ctx);
        self.show_password_prompt(&ctx);
        self.show_delete_prompt(&ctx);
        self.show_session_controllers(&ctx);
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

impl Drop for RustRdpApp {
    fn drop(&mut self) {
        if let Some(path) = self.config_path.as_deref()
            && let Err(error) = storage::save(path, &self.data)
        {
            tracing::error!(%error, "could not save configuration on exit");
        }
    }
}

#[derive(Clone, Copy)]
enum SummaryAction {
    None,
    Connect(Uuid),
    Edit(Uuid),
    Duplicate,
    Delete(Uuid),
    ToggleFavorite(Uuid),
}

fn profile_summary(ui: &mut egui::Ui, profile: &Profile, colors: Colors) -> SummaryAction {
    let mut action = SummaryAction::None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.heading(RichText::new(&profile.name).size(25.0));
            ui.label(
                RichText::new(format!(
                    "{}:{}",
                    profile.connection.host, profile.connection.port
                ))
                .color(colors.dim),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .button(if profile.favorite {
                    icons::FAVORITE
                } else {
                    icons::FAVORITE_BORDER
                })
                .on_hover_text("Favorite")
                .clicked()
            {
                action = SummaryAction::ToggleFavorite(profile.id);
            }
        });
    });
    ui.add_space(16.0);
    detail_row(
        ui,
        "Username",
        value_or_dash(&profile.connection.username),
        colors,
    );
    detail_row(
        ui,
        "Domain",
        value_or_dash(&profile.connection.domain),
        colors,
    );
    detail_row(
        ui,
        "Display",
        match profile.display.mode {
            DisplayMode::Windowed => "Windowed",
            DisplayMode::BorderlessMaximized => "Borderless maximized",
            DisplayMode::Fullscreen => "FreeRDP fullscreen",
        },
        colors,
    );
    detail_row(
        ui,
        "Resolution",
        if profile.display.dynamic_resolution {
            "Dynamic — sharp on resize"
        } else {
            "Fixed"
        },
        colors,
    );
    ui.separator();
    section_heading(ui, "Resources", colors);
    ui.horizontal_wrapped(|ui| {
        if profile.resources.clipboard {
            resource_chip(ui, "Clipboard", colors);
        }
        if profile.resources.printers {
            resource_chip(ui, "Printers", colors);
        }
        if profile.resources.audio {
            resource_chip(ui, "Audio", colors);
        }
        if profile.resources.microphone {
            resource_chip(ui, "Microphone", colors);
        }
        for drive in &profile.resources.drives {
            resource_chip(ui, &drive.name, colors);
        }
    });
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        if ui
            .add(egui::Button::new(
                RichText::new(format!("{}  Connect", icons::PLAY)).strong(),
            ))
            .clicked()
        {
            action = SummaryAction::Connect(profile.id);
        }
        if ui.button(format!("{}  Edit", icons::EDIT)).clicked() {
            action = SummaryAction::Edit(profile.id);
        }
        if ui
            .button(format!("{}  Duplicate", icons::CONTENT_COPY))
            .clicked()
        {
            action = SummaryAction::Duplicate;
        }
        if ui.button(format!("{}  Delete", icons::DELETE)).clicked() {
            action = SummaryAction::Delete(profile.id);
        }
    });
    action
}

fn profile_editor(ui: &mut egui::Ui, profile: &mut Profile, colors: Colors) -> EditorAction {
    let mut action = EditorAction::None;
    ui.horizontal(|ui| {
        ui.heading(if profile.connection.host.is_empty() {
            "New connection"
        } else {
            "Edit connection"
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button(format!("{}  Cancel", icons::CLOSE)).clicked() {
                action = EditorAction::Cancel;
            }
        });
    });
    ui.add_space(8.0);
    egui::ScrollArea::vertical().show(ui, |ui| {
        editor_section(ui, "Identity", colors, |ui| {
            field_row(ui, "Name", |ui| {
                ui.text_edit_singleline(&mut profile.name);
            });
            field_row(ui, "Host", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut profile.connection.host)
                        .hint_text("workpc.example.com"),
                );
            });
            field_row(ui, "Port", |ui| {
                ui.add(egui::DragValue::new(&mut profile.connection.port).range(1..=65535));
            });
            field_row(ui, "Username", |ui| {
                ui.text_edit_singleline(&mut profile.connection.username);
            });
            field_row(ui, "Domain", |ui| {
                ui.text_edit_singleline(&mut profile.connection.domain);
            });
            ui.checkbox(
                &mut profile.connection.save_password,
                "Save password securely when I connect",
            );
        });
        ui.add_space(12.0);
        editor_section(ui, "Display", colors, |ui| {
            ui.checkbox(
                &mut profile.display.dynamic_resolution,
                "Dynamic resolution (recommended for a sharp desktop)",
            );
            egui::ComboBox::from_label("Window mode")
                .selected_text(display_mode_name(profile.display.mode))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut profile.display.mode,
                        DisplayMode::Windowed,
                        "Windowed",
                    );
                    ui.selectable_value(
                        &mut profile.display.mode,
                        DisplayMode::BorderlessMaximized,
                        "Borderless maximized",
                    );
                    ui.selectable_value(
                        &mut profile.display.mode,
                        DisplayMode::Fullscreen,
                        "FreeRDP fullscreen",
                    );
                });
            let mut explicit = profile.display.resolution.is_some();
            if ui.checkbox(&mut explicit, "Use an explicit size").changed() {
                profile.display.resolution = explicit.then_some(Resolution {
                    width: 1920,
                    height: 1080,
                });
            }
            if let Some(resolution) = profile.display.resolution.as_mut() {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut resolution.width).range(320..=16384));
                    ui.label("×");
                    ui.add(egui::DragValue::new(&mut resolution.height).range(240..=16384));
                });
            }
        });
        ui.add_space(12.0);
        editor_section(ui, "Resources", colors, |ui| {
            ui.checkbox(&mut profile.resources.clipboard, "Clipboard");
            ui.checkbox(&mut profile.resources.printers, "Printers");
            ui.checkbox(&mut profile.resources.audio, "Audio output");
            ui.checkbox(&mut profile.resources.microphone, "Microphone");
            ui.separator();
            ui.label(RichText::new("Local folders").strong());
            let mut remove = None;
            for (index, drive) in profile.resources.drives.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut drive.name)
                            .hint_text("Share name")
                            .desired_width(130.0),
                    );
                    let mut path = drive.path.to_string_lossy().into_owned();
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut path)
                                .hint_text("/home/me/Documents")
                                .desired_width(260.0),
                        )
                        .changed()
                    {
                        drive.path = PathBuf::from(path);
                    }
                    if ui.button(icons::DELETE).clicked() {
                        remove = Some(index);
                    }
                });
            }
            if let Some(index) = remove {
                profile.resources.drives.remove(index);
            }
            if ui
                .button(format!("{}  Add local folder", icons::FOLDER))
                .clicked()
            {
                profile.resources.drives.push(Drive {
                    name: "share".to_owned(),
                    path: PathBuf::new(),
                });
            }
        });
    });
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if ui
            .add(egui::Button::new(format!("{}  Save", icons::CHECK)))
            .clicked()
        {
            action = EditorAction::Save;
        }
        if ui
            .add(egui::Button::new(format!(
                "{}  Save and connect",
                icons::PLAY
            )))
            .clicked()
        {
            action = EditorAction::Connect;
        }
    });
    action
}

fn session_list(ui: &mut egui::Ui, sessions: &mut SessionManager, colors: Colors) {
    let snapshot: Vec<_> = sessions
        .sessions()
        .map(|session| {
            (
                session.id,
                session.profile_name.clone(),
                session.pid,
                session.state.clone(),
            )
        })
        .collect();
    if snapshot.is_empty() {
        return;
    }
    ui.separator();
    section_heading(ui, "Sessions", colors);
    let mut disconnect = None;
    let mut dismiss = None;
    for (id, name, pid, state) in snapshot {
        ui.horizontal(|ui| {
            let (label, color) = match &state {
                SessionState::Connecting => ("Connecting…", colors.accent),
                SessionState::Active => ("Active", colors.success),
                SessionState::Disconnecting => ("Disconnecting…", colors.muted),
                SessionState::Exited(exit) => (&*exit.title, colors.muted),
            };
            ui.label(RichText::new("●").color(color));
            ui.label(RichText::new(&name).strong());
            ui.label(RichText::new(format!("{label} · PID {pid}")).color(colors.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| match state {
                SessionState::Connecting | SessionState::Active => {
                    if ui.button("Disconnect").clicked() {
                        disconnect = Some(id);
                    }
                }
                SessionState::Exited(exit) => {
                    if ui.button("Dismiss").clicked() {
                        dismiss = Some(id);
                    }
                    ui.label(RichText::new(exit.message).small().color(colors.dim));
                }
                SessionState::Disconnecting => {}
            });
        });
        ui.separator();
        ui.add_space(5.0);
    }
    if let Some(id) = disconnect {
        let _ = sessions.disconnect(id);
    }
    if let Some(id) = dismiss {
        sessions.dismiss_exited(id);
    }
}

fn empty_state(ui: &mut egui::Ui, colors: Colors) {
    ui.vertical_centered(|ui| {
        ui.add_space(100.0);
        ui.label(
            RichText::new(icons::DESKTOP)
                .size(64.0)
                .color(colors.accent),
        );
        ui.heading("Your remote computers, one click away");
        ui.label(
            RichText::new("Create a connection to begin.")
                .color(colors.dim)
                .size(15.0),
        );
    });
}

fn editor_section(
    ui: &mut egui::Ui,
    title: &str,
    colors: Colors,
    content: impl FnOnce(&mut egui::Ui),
) {
    section_heading(ui, title, colors);
    content(ui);
    ui.separator();
}

fn section_heading(ui: &mut egui::Ui, title: &str, colors: Colors) {
    ui.label(RichText::new(title).strong().color(colors.heading));
    ui.add_space(3.0);
}

fn field_row(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized([100.0, 28.0], egui::Label::new(label));
        content(ui);
    });
}

fn detail_row(ui: &mut egui::Ui, label: &str, value: &str, colors: Colors) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [115.0, 24.0],
            egui::Label::new(RichText::new(label).color(colors.muted)),
        );
        ui.label(value);
    });
}

fn value_or_dash(value: &str) -> &str {
    if value.trim().is_empty() {
        "—"
    } else {
        value
    }
}

fn resource_chip(ui: &mut egui::Ui, label: &str, colors: Colors) {
    ui.label(
        RichText::new(format!("{}  {label}", icons::CHECK))
            .small()
            .color(colors.accent),
    );
}

fn status_chip(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(RichText::new(format!("●  {text}")).small().color(color));
}

fn display_mode_name(mode: DisplayMode) -> &'static str {
    match mode {
        DisplayMode::Windowed => "Windowed",
        DisplayMode::BorderlessMaximized => "Borderless maximized",
        DisplayMode::Fullscreen => "FreeRDP fullscreen",
    }
}

fn theme_name(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::System => "Follow system",
        ThemeMode::Light => "Light",
        ThemeMode::Dark => "Dark",
    }
}

fn show_main_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
}
