use crate::model::ThemeMode;
use eframe::egui::{self, Color32, CornerRadius, Stroke};

#[derive(Clone, Copy)]
pub struct Colors {
    pub bg: Color32,
    pub panel: Color32,
    pub sidebar: Color32,
    pub toolbar: Color32,
    pub raised: Color32,
    pub border: Color32,
    pub heading: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_dim: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub error: Color32,
    pub success: Color32,
}

impl Colors {
    pub fn for_dark(dark: bool) -> Self {
        if dark {
            Self {
                // Ottrin's cool, layered dark palette.
                bg: Color32::from_rgb(30, 34, 39),
                panel: Color32::from_rgb(38, 43, 49),
                sidebar: Color32::from_rgb(35, 40, 46),
                toolbar: Color32::from_rgb(38, 43, 49),
                raised: Color32::from_rgb(49, 56, 66),
                border: Color32::from_rgb(59, 68, 80),
                heading: Color32::from_rgb(231, 236, 240),
                text: Color32::from_rgb(231, 236, 240),
                dim: Color32::from_rgb(192, 200, 210),
                muted: Color32::from_rgb(154, 163, 174),
                accent: Color32::from_rgb(78, 161, 242),
                accent_dim: Color32::from_rgba_unmultiplied(78, 161, 242, 70),
                hover: Color32::from_rgba_unmultiplied(255, 255, 255, 14),
                selected: Color32::from_rgba_unmultiplied(78, 161, 242, 70),
                error: Color32::from_rgb(224, 108, 117),
                success: Color32::from_rgb(141, 193, 73),
            }
        } else {
            Self {
                bg: Color32::from_rgb(255, 255, 255),
                panel: Color32::from_rgb(244, 246, 249),
                sidebar: Color32::from_rgb(239, 242, 246),
                toolbar: Color32::from_rgb(244, 246, 249),
                raised: Color32::from_rgb(235, 239, 244),
                border: Color32::from_rgb(208, 216, 225),
                heading: Color32::from_rgb(25, 31, 38),
                text: Color32::from_rgb(25, 31, 38),
                dim: Color32::from_rgb(78, 90, 104),
                muted: Color32::from_rgb(132, 143, 155),
                accent: Color32::from_rgb(37, 113, 185),
                accent_dim: Color32::from_rgba_unmultiplied(37, 113, 185, 40),
                hover: Color32::from_rgba_unmultiplied(0, 0, 0, 12),
                selected: Color32::from_rgba_unmultiplied(37, 113, 185, 45),
                error: Color32::from_rgb(196, 48, 56),
                success: Color32::from_rgb(74, 132, 48),
            }
        }
    }
}

pub fn apply(ctx: &egui::Context, mode: ThemeMode) -> Colors {
    let dark = match mode {
        ThemeMode::System => ctx.system_theme() != Some(egui::Theme::Light),
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
    };
    let colors = Colors::for_dark(dark);
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = colors.panel;
    visuals.window_fill = colors.panel;
    visuals.extreme_bg_color = colors.bg;
    visuals.faint_bg_color = colors.hover;
    visuals.override_text_color = Some(colors.text);
    visuals.selection.bg_fill = colors.selected;
    visuals.selection.stroke = Stroke::new(1.0, colors.accent);
    visuals.widgets.noninteractive.bg_fill = colors.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, colors.border);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);
    visuals.widgets.inactive.bg_fill = colors.raised;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, colors.border);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
    visuals.widgets.hovered.bg_fill = colors.hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, colors.accent);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
    visuals.widgets.active.bg_fill = colors.accent_dim;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, colors.accent);
    visuals.widgets.active.corner_radius = CornerRadius::same(6);
    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.window_stroke = Stroke::new(1.0, colors.border);
    visuals.menu_corner_radius = CornerRadius::same(6);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(7.0, 7.0);
    style.spacing.button_padding = egui::vec2(11.0, 6.0);
    style.spacing.interact_size.y = 34.0;
    style.spacing.text_edit_width = 280.0;
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(14.5, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(19.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(12.5, egui::FontFamily::Proportional),
    );
    ctx.set_global_style(style);
    colors
}
