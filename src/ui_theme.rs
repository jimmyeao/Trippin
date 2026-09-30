//! Shared egui theme + building blocks for the control window and the
//! timeline editor (see Docs/mockups.html — the 1b look: dark cards, cyan
//! accent, mono for names/numbers). Plain egui only: frames, labels,
//! painter rects — no custom shaders, no blur, no shadows.

use egui::{Color32, CornerRadius, Frame, Margin, Stroke, Ui};

// ---- Palette tokens -------------------------------------------------------

pub const BG: Color32 = Color32::from_rgb(0x0E, 0x0F, 0x11);
pub const PANEL: Color32 = Color32::from_rgb(0x12, 0x14, 0x16);
pub const CARD: Color32 = Color32::from_rgb(0x14, 0x16, 0x19);
pub const RAISED: Color32 = Color32::from_rgb(0x1F, 0x22, 0x26);
pub const HOVER: Color32 = Color32::from_rgb(0x26, 0x2A, 0x2F);
pub const INSET: Color32 = Color32::from_rgb(0x0E, 0x0F, 0x11);
pub const BORDER: Color32 = Color32::from_rgb(0x24, 0x27, 0x2B);
pub const BORDER_HI: Color32 = Color32::from_rgb(0x2A, 0x2E, 0x33);
pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xE7, 0xE9);
pub const MUTED: Color32 = Color32::from_rgb(0x8B, 0x90, 0x96);
pub const FAINT: Color32 = Color32::from_rgb(0x6C, 0x71, 0x77);
pub const ACCENT: Color32 = Color32::from_rgb(0x3F, 0xB0, 0xD8);
pub const ACCENT_SEL: Color32 = Color32::from_rgb(0x1A, 0x4D, 0x61);
pub const BREAKDOWN: Color32 = Color32::from_rgb(0xA9, 0xA6, 0xF0);
pub const BREAKDOWN_BG: Color32 = Color32::from_rgb(0x1E, 0x1D, 0x33);
pub const WARN: Color32 = Color32::from_rgb(0xF0, 0xA6, 0x50);
pub const WARN_BG: Color32 = Color32::from_rgb(0x33, 0x23, 0x0F);
pub const DANGER: Color32 = Color32::from_rgb(0xF0, 0x8A, 0x84);
pub const DANGER_BG: Color32 = Color32::from_rgb(0x3A, 0x17, 0x16);
pub const GOOD: Color32 = Color32::from_rgb(0x5F, 0xD3, 0x9A);

/// Timeline lane colours (scenes / dancer / fx / show / text).
pub const LANE_SCENE: Color32 = Color32::from_rgb(0x8A, 0x7F, 0xF0);
pub const LANE_DANCER: Color32 = Color32::from_rgb(0xE2, 0x5F, 0xA8);
pub const LANE_FX: Color32 = ACCENT;
pub const LANE_SHOW: Color32 = WARN;
pub const LANE_TEXT: Color32 = GOOD;

/// Apply once per egui context, from `EguiWin::new` (replaces `Visuals::dark`).
pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.dark_mode = true;
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.faint_bg_color = CARD;
    v.extreme_bg_color = INSET;
    v.code_bg_color = INSET;
    v.override_text_color = None; // per-widget strokes below carry the colour
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(8);
    v.selection.bg_fill = ACCENT_SEL;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.warn_fg_color = WARN;
    v.error_fg_color = DANGER;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = CARD;
    w.noninteractive.weak_bg_fill = CARD;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    w.noninteractive.corner_radius = CornerRadius::same(5);
    w.inactive.bg_fill = RAISED;
    w.inactive.weak_bg_fill = RAISED;
    w.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.inactive.corner_radius = CornerRadius::same(5);
    w.hovered.bg_fill = HOVER;
    w.hovered.weak_bg_fill = HOVER;
    w.hovered.bg_stroke = Stroke::new(1.0, BORDER_HI);
    w.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    w.hovered.corner_radius = CornerRadius::same(5);
    w.active.bg_fill = ACCENT_SEL;
    w.active.weak_bg_fill = ACCENT_SEL;
    w.active.bg_stroke = Stroke::new(1.0, ACCENT);
    w.active.fg_stroke = Stroke::new(1.0, TEXT);
    w.active.corner_radius = CornerRadius::same(5);
    w.open.bg_fill = ACCENT_SEL;
    w.open.weak_bg_fill = ACCENT_SEL;

    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(10.0, 8.0);
        s.spacing.button_padding = egui::vec2(10.0, 4.0);
        s.spacing.window_margin = Margin::same(14);
        s.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
        );
        s.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
        );
        s.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::new(11.0, egui::FontFamily::Proportional),
        );
        s.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(12.5, egui::FontFamily::Monospace),
        );
    });
}

// ---- Building blocks -------------------------------------------------------

/// A grouped card: card fill, 1px border, 8px radius, padded inside.
pub fn card() -> Frame {
    Frame::default()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(12))
}

/// Small ALL-CAPS section label, like the mockup's card headings.
pub fn section_label(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(10.0)
            .color(MUTED)
            .strong(),
    );
}

/// A status pill: coloured text on a tinted rounded chip.
pub fn pill(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) {
    Frame::default()
        .fill(bg)
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(11.0).color(fg));
        });
}

/// A hotkey badge: bordered mono chip, e.g. `B`.
pub fn key_badge(ui: &mut Ui, text: &str) {
    Frame::default()
        .fill(INSET)
        .stroke(Stroke::new(1.0, BORDER_HI))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .monospace()
                    .size(11.0)
                    .color(MUTED),
            );
        });
}

/// Segmented control on an inset track: one option highlighted at a time.
/// Returns true when the selection changed.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let mut changed = false;
    Frame::default()
        .fill(INSET)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (v, label) in options {
                    let on = *value == *v;
                    let b = egui::Button::new(
                        egui::RichText::new(*label).color(if on { TEXT } else { MUTED }),
                    )
                    .fill(if on { ACCENT_SEL } else { Color32::TRANSPARENT })
                    .corner_radius(CornerRadius::same(4));
                    if ui.add(b).clicked() && !on {
                        *value = *v;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// Segmented control stretched to the full width — equal-width options,
/// as in mockup 1a's Mode/Scene-length pickers.
pub fn segmented_wide<T: PartialEq + Copy>(
    ui: &mut Ui,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let mut changed = false;
    let w = ((ui.available_width() - 4.0 - (options.len() as f32 - 1.0) * 2.0)
        / options.len() as f32)
        .max(30.0);
    Frame::default()
        .fill(INSET)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (v, label) in options {
                    let on = *value == *v;
                    let b = egui::Button::new(
                        egui::RichText::new(*label).color(if on { TEXT } else { MUTED }),
                    )
                    .fill(if on { ACCENT_SEL } else { Color32::TRANSPARENT })
                    .corner_radius(CornerRadius::same(4));
                    if ui.add_sized([w, 24.0], b).clicked() && !on {
                        *value = *v;
                        changed = true;
                    }
                }
            });
        });
    changed
}
