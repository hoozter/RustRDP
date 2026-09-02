use crate::autostart;
use crate::credentials::CredentialStore;
use crate::desktop;
use crate::display::DisplayCatalog;
use crate::file_dialog;
use crate::freerdp::FreeRdpBackend;
use crate::icons;
use crate::model::{
    AppData, DisplayMode, Drive, Profile, QuickConnection, Resolution, Settings, ThemeMode,
};
use crate::sessions::{SessionManager, SessionState};
use crate::storage;
use crate::theme::{self, Colors};
use crate::tray::{TrayAction, TrayIntegration};
use crossbeam_channel::{Receiver, unbounded};
use eframe::egui::{self, Align, Color32, Layout, RichText, Vec2};
use std::collections::HashSet;
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
    editor_section: EditorSection,
    main_view: MainView,
    quick_draft: QuickConnection,
    search: String,
    show_settings: bool,
    password_prompt: Option<PasswordPrompt>,
    delete_prompt: Option<DeletePrompt>,
    status: Option<StatusMessage>,
    folder_picker_error: Option<String>,
    tray: Option<TrayIntegration>,
    tray_actions: Receiver<TrayAction>,
    quitting: bool,
    minimize_on_first_frame: bool,
    colors: Colors,
    display_catalog: DisplayCatalog,
}

struct PasswordPrompt {
    profile: Profile,
    allow_remember: bool,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum EditorSection {
    #[default]
    Connection,
    Display,
    Resources,
}

enum QuickAction {
    None,
    Connect(QuickConnection),
    Save(QuickConnection),
    Load(QuickConnection),
}

enum SessionAction {
    None,
    Reconnect(Profile),
}

#[derive(Clone, Copy)]
enum DataAction {
    None,
    Export,
    Import,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum MainView {
    #[default]
    Connections,
    QuickConnect,
}

impl RustRdpApp {
    pub fn new(cc: &eframe::CreationContext<'_>, start_minimized: bool) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "material-icons".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/MaterialSymbolsFilled.ttf"
            ))),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("material-icons".to_owned());
        cc.egui_ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let config_path = storage::default_config_path().ok();
        let (mut data, load_error) = match config_path.as_deref().map(storage::load) {
            Some(Ok(data)) => (data, None),
            Some(Err(error)) => (AppData::default(), Some(error.to_string())),
            None => (
                AppData::default(),
                Some("Could not determine the XDG configuration directory".to_owned()),
            ),
        };
        let display_catalog = DisplayCatalog::detect();
        let preferred_resolution = display_catalog.preferred_resolution();
        for profile in &mut data.profiles {
            profile
                .display
                .constrain_to_supported_mode(preferred_resolution);
        }
        let colors = theme::apply(&cc.egui_ctx, data.settings.theme);
        let main_view = if data.profiles.is_empty() {
            MainView::QuickConnect
        } else {
            MainView::Connections
        };
        let backend = FreeRdpBackend::detect().map_err(|error| error.to_string());
        let (tray_tx, tray_actions) = unbounded();
        let tray = match TrayIntegration::start(tray_tx, cc.egui_ctx.clone(), &data.profiles) {
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
            editor_section: EditorSection::default(),
            main_view,
            quick_draft: QuickConnection::default(),
            search: String::new(),
            show_settings: false,
            password_prompt: None,
            delete_prompt: None,
            status: load_error.map(|text| StatusMessage {
                text,
                is_error: true,
            }),
            folder_picker_error: None,
            tray,
            tray_actions,
            quitting: false,
            minimize_on_first_frame: start_minimized,
            colors,
            display_catalog,
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
                .map(|session| (session.id, session.profile_name.clone()))
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
        self.main_view = MainView::Connections;
        self.selected = None;
        self.draft = Some(Profile::default());
        self.folder_picker_error = None;
        self.editor_section = EditorSection::Connection;
    }

    fn begin_edit(&mut self, profile_id: Uuid) {
        self.main_view = MainView::Connections;
        self.selected = Some(profile_id);
        self.draft = self
            .data
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned();
        self.folder_picker_error = None;
        self.editor_section = EditorSection::Connection;
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
        self.request_connect_profile(profile, true);
    }

    fn request_connect_profile(&mut self, profile: Profile, allow_remember: bool) {
        if let Err(error) = profile.validate() {
            self.error(error.to_string());
            return;
        }
        if allow_remember && profile.connection.save_password {
            match CredentialStore::retrieve(&profile.credential_id()) {
                Ok(password) => self.launch(profile, Some(password), true),
                Err(error) => {
                    self.password_prompt = Some(PasswordPrompt {
                        profile,
                        allow_remember,
                        password: String::new(),
                        remember: true,
                        visible: false,
                        error: Some(error.to_string()),
                    });
                }
            }
        } else {
            self.password_prompt = Some(PasswordPrompt {
                profile,
                allow_remember,
                password: String::new(),
                remember: false,
                visible: false,
                error: None,
            });
        }
    }

    fn connect_quick(&mut self, connection: QuickConnection) {
        let profile = connection.to_profile();
        if let Err(error) = profile.validate() {
            self.error(error.to_string());
            return;
        }
        self.quick_draft = connection.clone();
        self.data.record_recent(connection);
        self.persist();
        self.request_connect_profile(profile, false);
    }

    fn save_quick_as_profile(&mut self, connection: QuickConnection) {
        self.main_view = MainView::Connections;
        self.selected = None;
        self.draft = Some(connection.to_profile());
    }

    fn launch(&mut self, profile: Profile, password: Option<String>, used_saved_credential: bool) {
        let backend = match &self.backend {
            Ok(backend) => backend,
            Err(error) => {
                self.error(error.clone());
                return;
            }
        };
        let local_scale_percent = if profile.display.match_local_scale {
            let detected = DisplayCatalog::detect().scale_percent;
            if detected.is_some() {
                self.display_catalog.scale_percent = detected;
            }
            detected.or(self.display_catalog.scale_percent)
        } else {
            self.display_catalog.scale_percent
        };
        let command =
            match backend.build_connection(&profile, password.is_some(), local_scale_percent) {
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
        match self
            .sessions
            .launch(&profile, command, password, used_saved_credential, true)
        {
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
                TrayAction::ShowSession(id) => {
                    if let Err(error) = self.sessions.show(id) {
                        self.error(error.to_string());
                    }
                }
                TrayAction::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn show_top_bar(&mut self, root: &mut egui::Ui) {
        egui::Frame::new()
            .fill(self.colors.toolbar)
            .inner_margin(egui::Margin::symmetric(14, 7))
            .show(root, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Image::from_bytes(
                            "bytes://rustrdp-full.svg",
                            include_bytes!("../assets/rustrdp-full.svg"),
                        )
                        .fit_to_exact_size(Vec2::new(160.0, 22.0)),
                    );
                    ui.add_space(12.0);
                    match &self.backend {
                        Ok(backend) => status_chip(
                            ui,
                            &format!("SDL3 · {}", backend.version.replace("This is ", "")),
                            self.colors.success,
                        ),
                        Err(_) => status_chip(ui, "FreeRDP SDL3 not found", self.colors.error),
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if toolbar_button(ui, icons::SETTINGS, "Settings", self.colors).clicked() {
                            self.show_settings = true;
                        }
                    });
                });
            });
    }

    fn show_library(&mut self, root: &mut egui::Ui) -> Option<Uuid> {
        let mut connect = None;
        root.vertical(|ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("WORKSPACE")
                    .size(11.5)
                    .strong()
                    .color(self.colors.muted),
            );
            ui.add_space(4.0);
            let quick_selected = self.main_view == MainView::QuickConnect;
            if navigation_button(
                ui,
                icons::PLAY,
                "Quick connect",
                quick_selected,
                self.colors,
            )
            .on_hover_text("Connect without creating a saved profile")
            .clicked()
            {
                self.main_view = MainView::QuickConnect;
                self.draft = None;
            }
            ui.add_space(18.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("SAVED CONNECTIONS")
                        .size(11.5)
                        .strong()
                        .color(self.colors.muted),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, icons::ADD, self.colors)
                        .on_hover_text("Create a connection")
                        .clicked()
                    {
                        self.begin_new();
                    }
                });
            });
            ui.add_space(6.0);
            egui::Frame::new()
                .fill(self.colors.bg)
                .stroke(egui::Stroke::new(1.0, self.colors.border))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(9, 5))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(icons::SEARCH)
                                .size(17.0)
                                .color(self.colors.muted),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .frame(egui::Frame::NONE)
                                .hint_text("Search connections…")
                                .desired_width(ui.available_width()),
                        );
                    });
                });
            ui.add_space(10.0);
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
            egui::ScrollArea::vertical()
                .id_salt("connection-library")
                .show(ui, |ui| {
                    for profile in profiles {
                        let selected = self.selected == Some(profile.id);
                        let marker = if profile.favorite {
                            icons::FAVORITE
                        } else {
                            icons::DESKTOP
                        };
                        let selected = selected && self.main_view == MainView::Connections;
                        let response = connection_row(ui, &profile, marker, selected, self.colors);
                        if response.clicked() {
                            self.main_view = MainView::Connections;
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
        let mut quick_action = QuickAction::None;
        root.add_space(12.0);
        if let Err(error) = &self.backend {
            backend_required_banner(root, error, self.colors);
            root.add_space(12.0);
        }
        if self.main_view == MainView::QuickConnect {
            quick_action = quick_connect_view(
                root,
                &mut self.quick_draft,
                &self.data.recent_connections,
                self.colors,
            );
        } else if let Some(draft) = self.draft.as_mut() {
            editor_action = profile_editor(
                root,
                draft,
                &mut self.editor_section,
                self.colors,
                &self.display_catalog,
                &mut self.folder_picker_error,
            );
        } else if let Some(profile) = self.selected_profile() {
            summary_action = profile_summary(root, profile, self.colors, &self.display_catalog);
        } else {
            empty_state(root, self.colors);
        }
        root.add_space(16.0);
        let session_action = session_list(root, &mut self.sessions, self.colors);
        match quick_action {
            QuickAction::None => {}
            QuickAction::Connect(connection) => self.connect_quick(connection),
            QuickAction::Save(connection) => self.save_quick_as_profile(connection),
            QuickAction::Load(connection) => self.quick_draft = connection,
        }
        match editor_action {
            EditorAction::None => {}
            EditorAction::Save => {
                self.save_draft();
            }
            EditorAction::Cancel => {
                self.draft = None;
                self.folder_picker_error = None;
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
        match session_action {
            SessionAction::None => {}
            SessionAction::Reconnect(profile) => {
                self.request_connect_profile(profile, true);
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
        let mut data_action = DataAction::None;
        let previous_start_with_system = self.data.settings.start_with_system;
        let previous_start_minimized = self.data.settings.start_minimized;
        let previous_close_to_tray = self.data.settings.close_to_tray;
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(820.0)
            .default_height(560.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("application-settings")
                    .max_height(720.0)
                    .show(ui, |ui| {
                        ui.columns(2, |columns| {
                            let (left, right) = columns.split_at_mut(1);
                            general_settings_card(
                                &mut left[0],
                                &mut self.data.settings,
                                self.colors,
                            );
                            left[0].add_space(10.0);
                            security_settings_card(&mut left[0], self.colors);

                            changed |= appearance_settings_card(
                                &mut right[0],
                                &mut self.data.settings.theme,
                                self.colors,
                            );
                            right[0].add_space(10.0);
                            data_action = backup_settings_card(&mut right[0], self.colors);
                        });
                    });
            });
        if self.data.settings.start_with_system != previous_start_with_system {
            autostart_changed = Some(self.data.settings.start_with_system);
            changed = true;
        }
        changed |= self.data.settings.start_minimized != previous_start_minimized;
        changed |= self.data.settings.close_to_tray != previous_close_to_tray;
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
        match data_action {
            DataAction::None => {}
            DataAction::Export => self.export_connections(),
            DataAction::Import => self.import_connections(),
        }
    }

    fn export_connections(&mut self) {
        if self.data.profiles.is_empty() {
            self.error("There are no saved connections to export");
            return;
        }
        match file_dialog::choose_export_path() {
            Ok(Some(mut path)) => {
                if path.extension().is_none() {
                    path.set_extension("toml");
                }
                match storage::export_profiles(&path, &self.data.profiles) {
                    Ok(()) => self.success(format!(
                        "Exported {} connection{} to {}",
                        self.data.profiles.len(),
                        if self.data.profiles.len() == 1 {
                            ""
                        } else {
                            "s"
                        },
                        path.display()
                    )),
                    Err(error) => self.error(error.to_string()),
                }
            }
            Ok(None) => {}
            Err(error) => self.error(error.to_string()),
        }
    }

    fn import_connections(&mut self) {
        let path = match file_dialog::choose_import_path() {
            Ok(Some(path)) => path,
            Ok(None) => return,
            Err(error) => {
                self.error(error.to_string());
                return;
            }
        };
        match storage::import_profiles(&path) {
            Ok(mut profiles) => {
                if profiles.is_empty() {
                    self.error("The selected backup contains no connections");
                    return;
                }
                let mut ids: HashSet<_> = self
                    .data
                    .profiles
                    .iter()
                    .map(|profile| profile.id)
                    .collect();
                for profile in &mut profiles {
                    profile
                        .display
                        .constrain_to_supported_mode(self.display_catalog.preferred_resolution());
                    if !ids.insert(profile.id) {
                        profile.id = Uuid::new_v4();
                        while !ids.insert(profile.id) {
                            profile.id = Uuid::new_v4();
                        }
                    }
                }
                let count = profiles.len();
                let first = profiles[0].id;
                self.data.profiles.extend(profiles);
                self.selected = Some(first);
                self.main_view = MainView::Connections;
                self.persist();
                self.success(format!(
                    "Imported {count} connection{} from {}",
                    if count == 1 { "" } else { "s" },
                    path.display()
                ));
            }
            Err(error) => self.error(error.to_string()),
        }
    }

    fn show_password_prompt(&mut self, ctx: &egui::Context) {
        let Some(mut prompt) = self.password_prompt.take() else {
            return;
        };
        let profile = prompt.profile.clone();
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
                        .margin(egui::Margin::symmetric(8, 5))
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
                if prompt.allow_remember {
                    ui.checkbox(&mut prompt.remember, "Save securely in the desktop wallet");
                }
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
            if prompt.allow_remember && prompt.remember {
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
            self.launch(profile, Some(password), false);
        } else if connect_without {
            prompt.password.zeroize();
            self.launch(profile, None, false);
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
}

impl eframe::App for RustRdpApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.process_tray_actions(&ctx);
        if self.minimize_on_first_frame {
            self.minimize_on_first_frame = false;
            hide_main_window(&ctx);
        }
        if self.sessions.poll() {
            self.update_tray();
        }
        if let Some(profile) = self.sessions.take_saved_credential_failure() {
            self.password_prompt = Some(PasswordPrompt {
                profile,
                allow_remember: true,
                password: String::new(),
                remember: true,
                visible: false,
                error: Some(
                    "The saved password was rejected. Enter the current password to replace it."
                        .to_owned(),
                ),
            });
        }
        if ctx.input(|input| input.viewport().close_requested())
            && !self.quitting
            && self.data.settings.close_to_tray
            && self.tray.is_some()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            hide_main_window(&ctx);
            self.success("RustRDP is still available in the system tray");
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(self.colors.bg))
            .show(root, |ui| {
                self.show_top_bar(ui);
                self.show_status(ui);
                let content_height = ui.available_height();
                ui.horizontal_top(|ui| {
                    egui::Frame::new()
                        .fill(self.colors.sidebar)
                        .stroke(egui::Stroke::new(1.0, self.colors.border))
                        .inner_margin(egui::Margin::symmetric(10, 11))
                        .show(ui, |ui| {
                            ui.set_width(248.0);
                            ui.set_height(content_height);
                            if let Some(id) = self.show_library(ui) {
                                self.request_connect(id);
                            }
                        });
                    egui::Frame::new()
                        .fill(self.colors.bg)
                        .inner_margin(egui::Margin::symmetric(16, 6))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.set_height(content_height);
                            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                                self.show_main_content(ui);
                            });
                        });
                });
            });
        self.show_settings_window(&ctx);
        self.show_password_prompt(&ctx);
        self.show_delete_prompt(&ctx);
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

fn profile_summary(
    ui: &mut egui::Ui,
    profile: &Profile,
    colors: Colors,
    displays: &DisplayCatalog,
) -> SummaryAction {
    let mut action = SummaryAction::None;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(icons::DESKTOP)
                .size(34.0)
                .color(colors.accent),
        );
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
            if primary_button(ui, icons::PLAY, "Connect", colors).clicked() {
                action = SummaryAction::Connect(profile.id);
            }
            if toolbar_button(ui, icons::EDIT, "Edit", colors).clicked() {
                action = SummaryAction::Edit(profile.id);
            }
            if icon_button(
                ui,
                if profile.favorite {
                    icons::FAVORITE
                } else {
                    icons::FAVORITE_BORDER
                },
                colors,
            )
            .on_hover_text("Favorite")
            .clicked()
            {
                action = SummaryAction::ToggleFavorite(profile.id);
            }
        });
    });
    ui.add_space(18.0);
    ui.columns(2, |columns| {
        summary_card(
            &mut columns[0],
            icons::KEY,
            "Connection",
            190.0,
            colors,
            |ui| {
                detail_row(ui, "Computer", &profile.connection.host, colors);
                detail_row(ui, "Port", &profile.connection.port.to_string(), colors);
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
            },
        );
        summary_card(
            &mut columns[1],
            icons::DISPLAY,
            "Display",
            190.0,
            colors,
            |ui| {
                detail_row(
                    ui,
                    "Window mode",
                    display_mode_name(profile.display.mode),
                    colors,
                );
                let resolution = if profile.display.dynamic_resolution {
                    "Dynamic — adapts to the window".to_owned()
                } else if let Some(resolution) = profile.display.resolution {
                    format!("{} × {} pixels", resolution.width, resolution.height)
                } else {
                    "Fixed".to_owned()
                };
                detail_row(ui, "Resolution", &resolution, colors);
                let scale = if profile.display.match_local_scale {
                    displays.scale_percent.map_or_else(
                        || "Match current display".to_owned(),
                        |percent| format!("Current display — {percent}%"),
                    )
                } else {
                    format!("{}%", profile.display.scale_percent)
                };
                detail_row(ui, "Scaling", &scale, colors);
                if profile.display.mode == DisplayMode::Fullscreen {
                    ui.add_space(8.0);
                    info_banner(
                        ui,
                        icons::LOCK,
                        "Keyboard stays remote; the safety bar remains available.",
                        colors,
                    );
                }
            },
        );
    });
    ui.add_space(12.0);
    summary_card(ui, icons::TUNE, "Resources", 80.0, colors, |ui| {
        ui.horizontal_wrapped(|ui| {
            if profile.resources.clipboard {
                resource_chip(ui, icons::CLIPBOARD, "Clipboard", colors);
            }
            if profile.resources.printers {
                resource_chip(ui, icons::PRINT, "Printers", colors);
            }
            if profile.resources.audio {
                resource_chip(ui, icons::VOLUME, "Audio output", colors);
            }
            if profile.resources.microphone {
                resource_chip(ui, icons::MICROPHONE, "Microphone", colors);
            }
            for drive in &profile.resources.drives {
                resource_chip(ui, icons::FOLDER_OPEN, &drive.name, colors);
            }
            if !profile.resources.clipboard
                && !profile.resources.printers
                && !profile.resources.audio
                && !profile.resources.microphone
                && profile.resources.drives.is_empty()
            {
                ui.label(RichText::new("No redirected resources").color(colors.muted));
            }
        });
    });
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        if toolbar_button(ui, icons::CONTENT_COPY, "Duplicate", colors).clicked() {
            action = SummaryAction::Duplicate;
        }
        if toolbar_button(ui, icons::DELETE, "Delete", colors).clicked() {
            action = SummaryAction::Delete(profile.id);
        }
    });
    action
}

fn quick_connect_view(
    ui: &mut egui::Ui,
    draft: &mut QuickConnection,
    recent: &[QuickConnection],
    colors: Colors,
) -> QuickAction {
    let mut action = QuickAction::None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.heading("Quick connect");
            ui.label(
                RichText::new(
                    "Test a computer now. Save it as a connection whenever it is useful.",
                )
                .color(colors.dim),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let can_connect = !draft.host.trim().is_empty();
            if ui
                .add_enabled_ui(can_connect, |ui| {
                    primary_button(ui, icons::PLAY, "Connect", colors)
                })
                .inner
                .clicked()
            {
                action = QuickAction::Connect(draft.clone());
            }
            if ui
                .add_enabled_ui(can_connect, |ui| {
                    toolbar_button(ui, icons::SAVE, "Save as connection", colors)
                })
                .inner
                .clicked()
            {
                action = QuickAction::Save(draft.clone());
            }
        });
    });
    ui.add_space(10.0);
    settings_card(
        ui,
        icons::PLAY,
        "Computer",
        "Connect now without creating a saved profile.",
        0.0,
        colors,
        |ui| {
            ui.columns(2, |columns| {
                labeled_field(&mut columns[0], "Computer or IP address", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.host)
                            .hint_text("workpc.example.com or 192.168.1.10")
                            .margin(egui::Margin::symmetric(8, 5))
                            .desired_width(f32::INFINITY),
                    );
                });
                labeled_field(&mut columns[1], "Port", |ui| {
                    port_input(ui, &mut draft.port, ui.id().with("quick-port"));
                });
            });
            ui.columns(2, |columns| {
                labeled_field(&mut columns[0], "Username", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.username)
                            .hint_text("Optional")
                            .margin(egui::Margin::symmetric(8, 5))
                            .desired_width(f32::INFINITY),
                    );
                });
                labeled_field(&mut columns[1], "Domain", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.domain)
                            .hint_text("Optional")
                            .margin(egui::Margin::symmetric(8, 5))
                            .desired_width(f32::INFINITY),
                    );
                });
            });
        },
    );

    ui.add_space(14.0);
    ui.label(RichText::new("Recent").strong().color(colors.heading));
    ui.label(
        RichText::new("Your latest quick connections are kept here without passwords.")
            .small()
            .color(colors.muted),
    );
    ui.add_space(6.0);
    if recent.is_empty() {
        editor_card(ui, "No recent connections", colors, |ui| {
            ui.label(
                RichText::new("Connections you test from this screen will appear here.")
                    .color(colors.dim),
            );
        });
    } else {
        egui::ScrollArea::vertical()
            .id_salt("quick-connect-recent")
            .max_height(260.0)
            .show(ui, |ui| {
                for connection in recent {
                    let mut row_action = QuickAction::None;
                    egui::Frame::new()
                        .fill(colors.raised)
                        .stroke(egui::Stroke::new(1.0, colors.border))
                        .corner_radius(egui::CornerRadius::same(7))
                        .inner_margin(10)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let user = if connection.username.is_empty() {
                                    "Remote Desktop".to_owned()
                                } else if connection.domain.is_empty() {
                                    connection.username.clone()
                                } else {
                                    format!("{}\\{}", connection.domain, connection.username)
                                };
                                let response = ui
                                    .vertical(|ui| {
                                        ui.label(RichText::new(&connection.host).strong());
                                        ui.label(
                                            RichText::new(format!(
                                                "{user} · port {}",
                                                connection.port
                                            ))
                                            .small()
                                            .color(colors.muted),
                                        );
                                    })
                                    .response
                                    .interact(egui::Sense::click());
                                if response.clicked() {
                                    row_action = QuickAction::Load(connection.clone());
                                }
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if toolbar_button(ui, icons::PLAY, "Connect", colors).clicked()
                                    {
                                        row_action = QuickAction::Connect(connection.clone());
                                    }
                                    if toolbar_button(ui, icons::SAVE, "Save", colors).clicked() {
                                        row_action = QuickAction::Save(connection.clone());
                                    }
                                });
                            });
                        });
                    ui.add_space(6.0);
                    if !matches!(row_action, QuickAction::None) {
                        action = row_action;
                    }
                }
            });
    }
    action
}

fn profile_editor(
    ui: &mut egui::Ui,
    profile: &mut Profile,
    section: &mut EditorSection,
    colors: Colors,
    displays: &DisplayCatalog,
    folder_picker_error: &mut Option<String>,
) -> EditorAction {
    let mut action = EditorAction::None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.heading(if profile.connection.host.is_empty() {
                "New connection"
            } else {
                "Edit connection"
            });
            ui.label(
                RichText::new("Connection details and the options used when FreeRDP starts.")
                    .color(colors.dim),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if toolbar_button(ui, icons::CLOSE, "Cancel", colors).clicked() {
                action = EditorAction::Cancel;
            }
            if primary_button(ui, icons::PLAY, "Save and connect", colors).clicked() {
                action = EditorAction::Connect;
            }
            if toolbar_button(ui, icons::SAVE, "Save", colors).clicked() {
                action = EditorAction::Save;
            }
        });
    });
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let tab_width = ((ui.available_width() - 14.0) / 3.0).max(120.0);
        editor_tab(
            ui,
            icons::KEY,
            "Connection",
            section,
            EditorSection::Connection,
            tab_width,
            colors,
        );
        editor_tab(
            ui,
            icons::DISPLAY,
            "Display",
            section,
            EditorSection::Display,
            tab_width,
            colors,
        );
        editor_tab(
            ui,
            icons::TUNE,
            "Resources",
            section,
            EditorSection::Resources,
            tab_width,
            colors,
        );
    });
    ui.add_space(8.0);
    egui::ScrollArea::vertical()
        .id_salt("profile-editor")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if *section == EditorSection::Connection {
                settings_card(
                    ui,
                    icons::KEY,
                    "Connection",
                    "Where to connect and which account to use.",
                    350.0,
                    colors,
                    |ui| {
                    labeled_field(ui, "Connection name", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut profile.name)
                                .margin(egui::Margin::symmetric(8, 5))
                                .desired_width(f32::INFINITY),
                        );
                    });
                    labeled_field(ui, "Computer or IP address", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut profile.connection.host)
                                .hint_text("workpc.example.com or 192.168.1.10")
                                .margin(egui::Margin::symmetric(8, 5))
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.columns(2, |columns| {
                        labeled_field(&mut columns[0], "Port", |ui| {
                            port_input(
                                ui,
                                &mut profile.connection.port,
                                ui.id().with(("profile-port", profile.id)),
                            );
                        });
                        labeled_field(&mut columns[1], "Domain", |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut profile.connection.domain)
                                    .hint_text("Optional")
                                    .margin(egui::Margin::symmetric(8, 5))
                                    .desired_width(f32::INFINITY),
                            );
                        });
                    });
                    labeled_field(ui, "Username", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut profile.connection.username)
                                .hint_text("Optional")
                                .margin(egui::Margin::symmetric(8, 5))
                                .desired_width(f32::INFINITY),
                        );
                    });
                    setting_toggle_row(
                        ui,
                        icons::LOCK,
                        "Save password securely",
                        "Stored in the desktop wallet, never in the profile file.",
                        &mut profile.connection.save_password,
                        colors,
                    );
                });
            }
            if *section == EditorSection::Display {
                settings_card(
                    ui,
                    icons::DISPLAY,
                    "Display",
                    "Choose how the remote desktop fits this screen.",
                    350.0,
                    colors,
                    |ui| {
                    let previous_mode = profile.display.mode;
                    field_label(ui, "Window mode", colors);
                    egui::ComboBox::from_id_salt("profile-display-mode")
                        .selected_text(display_mode_name(profile.display.mode))
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut profile.display.mode,
                                DisplayMode::Windowed,
                                "Windowed",
                            );
                            ui.selectable_value(
                                &mut profile.display.mode,
                                DisplayMode::BorderlessMaximized,
                                "Desktop — borderless",
                            );
                            ui.selectable_value(
                                &mut profile.display.mode,
                                DisplayMode::Fullscreen,
                                "Fullscreen with safety bar",
                            );
                        });
                    if profile.display.mode != previous_mode {
                        profile.display.constrain_to_supported_mode(
                            displays.preferred_resolution(),
                        );
                    }
                    let dynamic_available = profile.display.dynamic_resolution_available();
                    profile.display.constrain_to_supported_mode(
                        displays.preferred_resolution(),
                    );
                    ui.add_space(8.0);
                    field_label(ui, "Resolution", colors);
                    if dynamic_available {
                        ui.columns(2, |columns| {
                            if choice_card(
                                &mut columns[0],
                                "Live resize",
                                "Updates after you finish resizing the window",
                                profile.display.dynamic_resolution,
                                colors,
                            )
                            .clicked()
                            {
                                profile.display.dynamic_resolution = true;
                                profile.display.resolution = None;
                            }
                            if choice_card(
                                &mut columns[1],
                                "Fixed & fit",
                                "Keep one remote size and scale it locally",
                                !profile.display.dynamic_resolution,
                                colors,
                            )
                            .clicked()
                            {
                                profile.display.dynamic_resolution = false;
                                profile.display.resolution =
                                    Some(displays.preferred_resolution());
                            }
                        });
                    } else {
                        let explanation = match profile.display.mode {
                            DisplayMode::BorderlessMaximized => {
                                "Borderless desktop uses the work area, so it has no resize handles. Choose the remote desktop size below."
                            }
                            DisplayMode::Fullscreen => {
                                "Fullscreen uses a fixed remote desktop size fitted to this display."
                            }
                            DisplayMode::Windowed => {
                                "Live resize is unavailable with this FreeRDP client. Choose a fixed remote desktop size below."
                            }
                        };
                        info_banner(ui, icons::INFO, explanation, colors);
                    }
                    if !profile.display.dynamic_resolution {
                        let resolution = profile
                            .display
                            .resolution
                            .get_or_insert_with(|| displays.preferred_resolution());
                        ui.add_space(6.0);
                        egui::ComboBox::from_id_salt("profile-resolution-preset")
                            .selected_text(resolution_label(*resolution, displays.current))
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                for mode in &displays.modes {
                                    ui.selectable_value(
                                        resolution,
                                        *mode,
                                        resolution_label(*mode, displays.current),
                                    );
                                }
                            });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Custom").color(colors.muted));
                            ui.add(
                                egui::DragValue::new(&mut resolution.width)
                                    .range(320..=16384)
                                    .suffix(" px"),
                            );
                            ui.label("×");
                            ui.add(
                                egui::DragValue::new(&mut resolution.height)
                                    .range(240..=16384)
                                    .suffix(" px"),
                            );
                        });
                        if let Some(name) = &displays.name {
                            ui.label(
                                RichText::new(format!("Supported modes from {name}"))
                                    .small()
                                    .color(colors.muted),
                            );
                        }
                    }
                    ui.add_space(8.0);
                    field_label(ui, "Remote scaling", colors);
                    let scale_description = displays.scale_percent.map_or_else(
                        || "Use the desktop environment's current scale when connecting."
                            .to_owned(),
                        |percent| {
                            let display = displays.name.as_deref().unwrap_or("this display");
                            format!("Use {display}'s current {percent}% scale when connecting.")
                        },
                    );
                    setting_toggle_row(
                        ui,
                        icons::DISPLAY,
                        "Match current display",
                        &scale_description,
                        &mut profile.display.match_local_scale,
                        colors,
                    );
                    if !profile.display.match_local_scale {
                        ui.add_space(5.0);
                        egui::ComboBox::from_id_salt("profile-display-scale")
                            .selected_text(format!("{}%", profile.display.scale_percent))
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                for percent in [100_u16, 125, 150, 175, 200, 225, 250, 300] {
                                    ui.selectable_value(
                                        &mut profile.display.scale_percent,
                                        percent,
                                        format!("{percent}%"),
                                    );
                                }
                            });
                    }
                    ui.label(
                        RichText::new(
                            "Changes Windows text and app sizing without lowering the selected resolution.",
                        )
                        .small()
                        .color(colors.muted),
                    );
                    ui.add_space(7.0);
                    if profile.display.mode == DisplayMode::Fullscreen {
                        info_banner(
                            ui,
                            icons::LOCK,
                            "All keys go to the remote desktop. Use the safety bar or Right Shift + D to exit.",
                            colors,
                        );
                    } else if profile.display.dynamic_resolution {
                        info_banner(
                            ui,
                            icons::INFO,
                            "Live resize asks Windows for a new desktop size after resizing settles.",
                            colors,
                        );
                    }
                });
            }
            if *section == EditorSection::Resources {
                settings_card(
                ui,
                icons::TUNE,
                "Resources",
                "Choose which local devices and folders are available remotely.",
                0.0,
                colors,
                |ui| {
                ui.columns(2, |columns| {
                    setting_toggle_row(
                        &mut columns[0],
                        icons::CLIPBOARD,
                        "Clipboard",
                        "Copy text and files between computers.",
                        &mut profile.resources.clipboard,
                        colors,
                    );
                    setting_toggle_row(
                        &mut columns[1],
                        icons::PRINT,
                        "Printers",
                        "Make local printers available remotely.",
                        &mut profile.resources.printers,
                        colors,
                    );
                });
                ui.columns(2, |columns| {
                    setting_toggle_row(
                        &mut columns[0],
                        icons::VOLUME,
                        "Audio output",
                        "Play remote sound on this computer.",
                        &mut profile.resources.audio,
                        colors,
                    );
                    setting_toggle_row(
                        &mut columns[1],
                        icons::MICROPHONE,
                        "Microphone",
                        "Share this computer's microphone.",
                        &mut profile.resources.microphone,
                        colors,
                    );
                });
                ui.add_space(6.0);
                egui::Frame::new()
                    .fill(colors.bg)
                    .stroke(egui::Stroke::new(1.0, colors.border))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(9)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icons::FOLDER_OPEN).size(18.0).color(colors.accent));
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Local folders").strong());
                                ui.label(
                                    RichText::new("Share selected folders with the remote computer.")
                                        .small()
                                        .color(colors.muted),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if toolbar_button(ui, icons::ADD, "Add folder", colors).clicked() {
                                    match file_dialog::choose_folder(None) {
                                        Ok(Some(path)) => {
                                            let name = folder_share_name(&path);
                                            profile.resources.drives.push(Drive { name, path });
                                            *folder_picker_error = None;
                                        }
                                        Ok(None) => {}
                                        Err(error) => *folder_picker_error = Some(error.to_string()),
                                    }
                                }
                            });
                        });
                        let mut remove = None;
                        for (index, drive) in profile.resources.drives.iter_mut().enumerate() {
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut drive.name)
                                    .hint_text("Share name")
                                    .margin(egui::Margin::symmetric(8, 5))
                                    .desired_width(170.0),
                            );
                            let mut path = drive.path.to_string_lossy().into_owned();
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut path)
                                        .hint_text("/home/me/Documents")
                                        .margin(egui::Margin::symmetric(8, 5))
                                        .desired_width((ui.available_width() - 145.0).max(160.0)),
                                )
                                .changed()
                            {
                                    drive.path = PathBuf::from(path);
                                }
                                if toolbar_button(ui, icons::FOLDER_OPEN, "Browse", colors).clicked() {
                                    match file_dialog::choose_folder(Some(&drive.path)) {
                                        Ok(Some(path)) => {
                                            if drive.name.trim().is_empty() || drive.name == "share" {
                                                drive.name = folder_share_name(&path);
                                            }
                                            drive.path = path;
                                            *folder_picker_error = None;
                                        }
                                        Ok(None) => {}
                                        Err(error) => *folder_picker_error = Some(error.to_string()),
                                    }
                                }
                                if icon_button(ui, icons::DELETE, colors).clicked() {
                                remove = Some(index);
                            }
                        });
                        }
                        if profile.resources.drives.is_empty() {
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new("No local folders shared yet.")
                                    .color(colors.muted),
                            );
                        }
                        if let Some(error) = folder_picker_error.as_deref() {
                            ui.add_space(5.0);
                            ui.colored_label(colors.error, error);
                        }
                        if let Some(index) = remove {
                            profile.resources.drives.remove(index);
                        }
                    });
                });
            }
        });
    action
}

fn session_list(ui: &mut egui::Ui, sessions: &mut SessionManager, colors: Colors) -> SessionAction {
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
        return SessionAction::None;
    }
    ui.separator();
    section_heading(ui, "Sessions", colors);
    let mut disconnect = None;
    let mut show = None;
    let mut dismiss = None;
    let mut reconnect = None;
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
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| match &state {
                SessionState::Connecting | SessionState::Active => {
                    if ui.button("Disconnect").clicked() {
                        disconnect = Some(id);
                    }
                    if ui.button("Show remote").clicked() {
                        show = Some(id);
                    }
                }
                SessionState::Exited(exit) => {
                    if ui.button("Dismiss").clicked() {
                        dismiss = Some(id);
                    }
                    if ui.button("Reconnect").clicked() {
                        reconnect = Some(id);
                    }
                    ui.label(RichText::new(&exit.message).small().color(colors.dim));
                }
                SessionState::Disconnecting => {}
            });
        });
        if let SessionState::Exited(exit) = &state
            && !exit.technical_details.is_empty()
        {
            egui::CollapsingHeader::new("Technical details")
                .id_salt(("session-details", id))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("FreeRDP output").small().color(colors.dim));
                        if ui.button("Copy all").clicked() {
                            ui.ctx().copy_text(exit.technical_details.clone());
                        }
                    });
                    egui::ScrollArea::both()
                        .id_salt(("session-details-scroll", id))
                        .max_height(220.0)
                        .show(ui, |ui| {
                            let mut details = exit.technical_details.clone();
                            ui.add(
                                egui::TextEdit::multiline(&mut details)
                                    .font(egui::TextStyle::Monospace)
                                    .code_editor()
                                    .desired_rows(8)
                                    .desired_width(ui.available_width()),
                            );
                        });
                });
        }
        ui.separator();
        ui.add_space(5.0);
    }
    if let Some(id) = disconnect {
        let _ = sessions.disconnect(id);
    }
    if let Some(id) = show {
        let _ = sessions.show(id);
    }
    if let Some(id) = dismiss {
        sessions.dismiss_exited(id);
    }
    reconnect
        .and_then(|id| sessions.reconnect_profile(id))
        .map_or(SessionAction::None, SessionAction::Reconnect)
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

fn editor_card(
    ui: &mut egui::Ui,
    title: &str,
    colors: Colors,
    content: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::new()
        .fill(colors.raised)
        .stroke(egui::Stroke::new(1.0, colors.border))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            section_heading(ui, title, colors);
            ui.add_space(4.0);
            content(ui);
        });
}

fn settings_card(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    description: &str,
    min_height: f32,
    colors: Colors,
    content: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::new()
        .fill(colors.panel)
        .stroke(egui::Stroke::new(1.0, colors.border))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(min_height);
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(19.0).color(colors.accent));
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(title)
                            .size(16.0)
                            .strong()
                            .color(colors.heading),
                    );
                    ui.label(RichText::new(description).small().color(colors.muted));
                });
            });
            ui.add_space(9.0);
            content(ui);
        });
}

fn general_settings_card(ui: &mut egui::Ui, settings: &mut Settings, colors: Colors) {
    settings_card(
        ui,
        icons::SETTINGS,
        "General",
        "Choose how RustRDP behaves on this computer.",
        0.0,
        colors,
        |ui| {
            setting_toggle_row(
                ui,
                icons::PLAY,
                "Start when I log in",
                "Keep saved connections ready in the tray.",
                &mut settings.start_with_system,
                colors,
            );
            ui.add_space(6.0);
            setting_toggle_row(
                ui,
                icons::VISIBILITY_OFF,
                "Start minimized",
                "Open in the tray instead of showing the window.",
                &mut settings.start_minimized,
                colors,
            );
            ui.add_space(6.0);
            setting_toggle_row(
                ui,
                icons::CLOSE,
                "Close to the tray",
                "Keep RustRDP running when this window closes.",
                &mut settings.close_to_tray,
                colors,
            );
        },
    );
}

fn security_settings_card(ui: &mut egui::Ui, colors: Colors) {
    settings_card(
        ui,
        icons::LOCK,
        "Security",
        "Passwords stay outside connection profiles.",
        0.0,
        colors,
        |ui| match CredentialStore::availability() {
            Ok(()) => info_banner(
                ui,
                icons::LOCK,
                "Desktop wallet available. Saved passwords are protected by Secret Service.",
                colors,
            ),
            Err(error) => {
                ui.colored_label(colors.error, error.to_string());
            }
        },
    );
}

fn appearance_settings_card(ui: &mut egui::Ui, theme: &mut ThemeMode, colors: Colors) -> bool {
    let mut changed = false;
    settings_card(
        ui,
        icons::TUNE,
        "Appearance",
        "Follow the desktop or choose a fixed theme.",
        0.0,
        colors,
        |ui| {
            field_label(ui, "Theme", colors);
            egui::ComboBox::from_id_salt("application-theme")
                .width(ui.available_width())
                .selected_text(theme_name(*theme))
                .show_ui(ui, |ui| {
                    changed |= ui
                        .selectable_value(theme, ThemeMode::System, "Follow system")
                        .changed();
                    changed |= ui
                        .selectable_value(theme, ThemeMode::Light, "Light")
                        .changed();
                    changed |= ui
                        .selectable_value(theme, ThemeMode::Dark, "Dark")
                        .changed();
                });
        },
    );
    changed
}

fn backup_settings_card(ui: &mut egui::Ui, colors: Colors) -> DataAction {
    let mut action = DataAction::None;
    settings_card(
        ui,
        icons::SAVE,
        "Connection backups",
        "Move saved connections without exporting passwords.",
        0.0,
        colors,
        |ui| {
            ui.horizontal(|ui| {
                if toolbar_button(ui, icons::EXPORT, "Export", colors).clicked() {
                    action = DataAction::Export;
                }
                if toolbar_button(ui, icons::IMPORT, "Import", colors).clicked() {
                    action = DataAction::Import;
                }
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Includes display and resource settings. Passwords remain in this computer's wallet.",
                )
                .small()
                .color(colors.dim),
            );
        },
    );
    action
}

fn summary_card(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    min_height: f32,
    colors: Colors,
    content: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::new()
        .fill(colors.panel)
        .stroke(egui::Stroke::new(1.0, colors.border))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(min_height);
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(18.0).color(colors.accent));
                ui.label(
                    RichText::new(title)
                        .size(15.0)
                        .strong()
                        .color(colors.heading),
                );
            });
            ui.add_space(8.0);
            content(ui);
        });
}

fn primary_button(ui: &mut egui::Ui, icon: &str, label: &str, colors: Colors) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(format!("{icon}  {label}"))
                .strong()
                .color(Color32::WHITE),
        )
        .fill(colors.accent)
        .stroke(egui::Stroke::new(1.0, colors.accent)),
    )
}

fn toolbar_button(ui: &mut egui::Ui, icon: &str, label: &str, colors: Colors) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(format!("{icon}  {label}")).color(colors.text))
            .fill(colors.panel)
            .stroke(egui::Stroke::new(1.0, colors.border)),
    )
}

fn editor_tab(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    section: &mut EditorSection,
    value: EditorSection,
    width: f32,
    colors: Colors,
) {
    let selected = *section == value;
    let response = ui.add_sized(
        [width, 36.0],
        egui::Button::new(
            RichText::new(format!("{icon}  {label}"))
                .strong()
                .color(if selected { Color32::WHITE } else { colors.dim }),
        )
        .fill(if selected {
            colors.accent
        } else {
            colors.panel
        })
        .stroke(egui::Stroke::new(
            1.0,
            if selected {
                colors.accent
            } else {
                colors.border
            },
        )),
    );
    if response.clicked() {
        *section = value;
    }
}

fn icon_button(ui: &mut egui::Ui, icon: &str, colors: Colors) -> egui::Response {
    ui.add_sized(
        [34.0, 34.0],
        egui::Button::new(RichText::new(icon).size(17.0).color(colors.dim))
            .fill(colors.panel)
            .stroke(egui::Stroke::new(1.0, colors.border)),
    )
}

fn navigation_button(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    selected: bool,
    colors: Colors,
) -> egui::Response {
    ui.add_sized(
        [ui.available_width(), 38.0],
        egui::Button::new(
            RichText::new(format!("{icon}  {label}"))
                .size(14.5)
                .color(if selected { colors.heading } else { colors.dim }),
        )
        .selected(selected)
        .fill(if selected {
            colors.selected
        } else {
            Color32::TRANSPARENT
        })
        .stroke(egui::Stroke::NONE),
    )
}

fn connection_row(
    ui: &mut egui::Ui,
    profile: &Profile,
    icon: &str,
    selected: bool,
    colors: Colors,
) -> egui::Response {
    let frame = egui::Frame::new()
        .fill(if selected {
            colors.selected
        } else {
            Color32::TRANSPARENT
        })
        .stroke(egui::Stroke::new(
            1.0,
            if selected {
                colors.accent
            } else {
                Color32::TRANSPARENT
            },
        ))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(9, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(18.0).color(if selected {
                    colors.accent
                } else {
                    colors.dim
                }));
                ui.vertical(|ui| {
                    ui.label(RichText::new(&profile.name).strong().color(colors.text));
                    ui.label(
                        RichText::new(&profile.connection.host)
                            .small()
                            .color(colors.muted),
                    );
                });
            });
        });
    ui.interact(
        frame.response.rect,
        ui.id().with(("connection-row", profile.id)),
        egui::Sense::click(),
    )
}

fn field_label(ui: &mut egui::Ui, label: &str, colors: Colors) {
    ui.label(RichText::new(label).size(12.5).strong().color(colors.dim));
    ui.add_space(3.0);
}

fn labeled_field(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.label(RichText::new(label).size(12.5).strong());
    ui.add_space(3.0);
    content(ui);
    ui.add_space(5.0);
}

fn setting_toggle_row(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    description: &str,
    enabled: &mut bool,
    colors: Colors,
) {
    let frame_response = egui::Frame::new()
        .fill(if *enabled {
            colors.accent_dim
        } else {
            colors.bg
        })
        .stroke(egui::Stroke::new(
            1.0,
            if *enabled {
                colors.accent
            } else {
                colors.border
            },
        ))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(9, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(18.0).color(if *enabled {
                    colors.accent
                } else {
                    colors.muted
                }));
                let text_width = (ui.available_width() - 48.0).max(80.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(text_width, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.label(RichText::new(title).strong().color(colors.text));
                        ui.add(
                            egui::Label::new(
                                RichText::new(description).small().color(colors.muted),
                            )
                            .wrap(),
                        );
                    },
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    toggle_switch(ui, *enabled, colors);
                });
            });
        })
        .response;
    let response = ui.interact(
        frame_response.rect,
        ui.id().with(("setting-toggle", title)),
        egui::Sense::click(),
    );
    if response.clicked() {
        *enabled = !*enabled;
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

fn toggle_switch(ui: &mut egui::Ui, enabled: bool, colors: Colors) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(34.0, 19.0), egui::Sense::hover());
    let track = if enabled {
        colors.accent
    } else {
        colors.raised
    };
    ui.painter().rect_filled(rect, 10.0, track);
    ui.painter().rect_stroke(
        rect,
        10.0,
        egui::Stroke::new(
            1.0,
            if enabled {
                colors.accent
            } else {
                colors.border
            },
        ),
        egui::StrokeKind::Inside,
    );
    let center_x = if enabled {
        rect.right() - 9.5
    } else {
        rect.left() + 9.5
    };
    ui.painter()
        .circle_filled(egui::pos2(center_x, rect.center().y), 6.0, Color32::WHITE);
}

fn choice_card(
    ui: &mut egui::Ui,
    title: &str,
    description: &str,
    selected: bool,
    colors: Colors,
) -> egui::Response {
    let frame_response = egui::Frame::new()
        .fill(if selected {
            colors.accent_dim
        } else {
            colors.bg
        })
        .stroke(egui::Stroke::new(
            1.0,
            if selected {
                colors.accent
            } else {
                colors.border
            },
        ))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(8)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if selected { icons::CHECK } else { "" })
                        .size(17.0)
                        .color(colors.accent),
                );
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).strong());
                    ui.label(RichText::new(description).small().color(colors.muted));
                });
            });
        })
        .response;
    let response = ui.interact(
        frame_response.rect,
        ui.id().with(("choice-card", title)),
        egui::Sense::click(),
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn info_banner(ui: &mut egui::Ui, icon: &str, text: &str, colors: Colors) {
    egui::Frame::new()
        .fill(colors.accent_dim)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(8)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(16.0).color(colors.accent));
                ui.add(egui::Label::new(RichText::new(text).small().color(colors.dim)).wrap());
            });
        });
}

fn backend_required_banner(ui: &mut egui::Ui, error: &str, colors: Colors) {
    egui::Frame::new()
        .fill(colors.panel)
        .stroke(egui::Stroke::new(1.0, colors.error))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.label(RichText::new(icons::ERROR).size(22.0).color(colors.error));
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("FreeRDP SDL3 is required")
                            .strong()
                            .color(colors.heading),
                    );
                    ui.label(RichText::new(error).color(colors.dim));
                    ui.add_space(7.0);
                    ui.horizontal(|ui| {
                        ui.monospace("sudo apt install freerdp-sdl");
                        if ui.button("Copy command").clicked() {
                            ui.ctx()
                                .copy_text("sudo apt install freerdp-sdl".to_owned());
                        }
                    });
                });
            });
        });
}

fn port_input(ui: &mut egui::Ui, port: &mut u16, id: egui::Id) {
    let mut text = ui
        .data_mut(|data| data.get_temp::<String>(id))
        .unwrap_or_else(|| port.to_string());
    let response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .id(id)
            .margin(egui::Margin::symmetric(8, 5))
            .desired_width(f32::INFINITY)
            .char_limit(5),
    );
    text.retain(|character| character.is_ascii_digit());
    if let Ok(parsed) = text.parse::<u16>()
        && parsed > 0
    {
        *port = parsed;
    }
    if response.lost_focus() {
        text = port.to_string();
    }
    ui.data_mut(|data| data.insert_temp(id, text));
}

fn section_heading(ui: &mut egui::Ui, title: &str, colors: Colors) {
    ui.label(RichText::new(title).strong().color(colors.heading));
    ui.add_space(3.0);
}

fn detail_row(ui: &mut egui::Ui, label: &str, value: &str, colors: Colors) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [115.0, 28.0],
            egui::Label::new(RichText::new(label).color(colors.muted)),
        );
        ui.label(RichText::new(value).color(colors.text));
    });
}

fn value_or_dash(value: &str) -> &str {
    if value.trim().is_empty() {
        "—"
    } else {
        value
    }
}

fn resource_chip(ui: &mut egui::Ui, icon: &str, label: &str, colors: Colors) {
    egui::Frame::new()
        .fill(colors.accent_dim)
        .stroke(egui::Stroke::new(1.0, colors.accent))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("{icon}  {label}"))
                    .size(12.5)
                    .color(colors.text),
            );
        });
}

fn status_chip(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            26,
        ))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(9, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(format!("●  {text}")).small().color(color));
        });
}

fn resolution_label(resolution: Resolution, current: Option<Resolution>) -> String {
    let suffix = if Some(resolution) == current {
        " — current"
    } else {
        ""
    };
    format!(
        "{} × {} · {}{suffix}",
        resolution.width,
        resolution.height,
        aspect_ratio_label(resolution)
    )
}

fn folder_share_name(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("share")
        .to_owned()
}

fn aspect_ratio_label(resolution: Resolution) -> String {
    const COMMON: &[(u32, u32)] = &[(4, 3), (5, 4), (3, 2), (16, 10), (16, 9), (21, 9), (32, 9)];
    let actual = resolution.width as f64 / resolution.height as f64;
    if let Some(&(width, height)) = COMMON.iter().min_by(|left, right| {
        let left_delta = (actual - left.0 as f64 / left.1 as f64).abs();
        let right_delta = (actual - right.0 as f64 / right.1 as f64).abs();
        left_delta.total_cmp(&right_delta)
    }) && (actual - width as f64 / height as f64).abs() / actual < 0.03
    {
        return format!("{width}:{height}");
    }
    let divisor = greatest_common_divisor(resolution.width, resolution.height);
    format!(
        "{}:{}",
        resolution.width / divisor,
        resolution.height / divisor
    )
}

fn greatest_common_divisor(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left.max(1)
}

fn display_mode_name(mode: DisplayMode) -> &'static str {
    match mode {
        DisplayMode::Windowed => "Windowed",
        DisplayMode::BorderlessMaximized => "Desktop — borderless",
        DisplayMode::Fullscreen => "Fullscreen with safety bar",
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
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    if !desktop::set_app_tray_hidden(std::process::id(), false) {
        tracing::debug!("KWin restore was unavailable; used the native viewport request");
    }
}

fn hide_main_window(ctx: &egui::Context) {
    // Keep winit's minimized state in sync even when KWin also removes the
    // window from desktop lists. eframe uses this state to service tray
    // repaint requests without waiting for a compositor redraw.
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    if !desktop::set_app_tray_hidden(std::process::id(), true) {
        tracing::debug!("KWin tray hiding was unavailable; used native minimization");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_labels_include_friendly_aspect_ratios_and_current_state() {
        let current = Resolution {
            width: 3840,
            height: 2160,
        };
        assert_eq!(
            resolution_label(current, Some(current)),
            "3840 × 2160 · 16:9 — current"
        );
        assert_eq!(
            resolution_label(
                Resolution {
                    width: 1920,
                    height: 1200,
                },
                Some(current)
            ),
            "1920 × 1200 · 16:10"
        );
    }

    #[test]
    fn folder_share_name_uses_the_selected_directory_name() {
        assert_eq!(
            folder_share_name(std::path::Path::new("/home/campbell/Documents")),
            "Documents"
        );
    }
}
