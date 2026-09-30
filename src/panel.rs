//! Control panel: a second window (egui) for modes, scene playlist, dancer
//! options, sync and hotkey bindings. It shares the renderer's GPU device.
//!
//! Layout (Docs/mockups.html): a live-status header, a tab strip — Perform,
//! Dancer & FX, Stream, Timeline, Keys — then one page at a time. Perform is
//! the default: a thumbnail scene library on the left and a live inspector
//! on the right (mockup 1b).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Icon, Window};

use crate::config::{Action, Fx, Mode, Seasonal, Settings, Tristate, in_season, today};
use crate::dancer::STYLES;
use crate::render::Gpu;
use crate::timeline::{CueKind, PlayMode, SongCtl, Timeline};

/// A fully-tessellated egui frame, ready to be drawn without the settings lock.
pub type PanelFrame = crate::egui_win::Frame;

/// Things the panel asks the app to do (beyond editing settings directly).
pub enum UiCommand {
    Do(Action),
    GoToScene(usize),
    ShowClip(usize),
    /// Decode an audio file and append it as a clip region on the timeline.
    AddSong(PathBuf),
    /// Open a saved timeline `.json`.
    LoadTimeline(PathBuf),
    /// Open the dedicated timeline editor window.
    OpenEditor,
    /// Save the current timeline under `timelines/<name>.json`.
    SaveTimeline,
    /// Fire a cue's effect immediately (editor preview).
    FireCue(CueKind),
    /// Transport control for the song player.
    Song(SongCtl),
    /// Ask the render thread to produce a scene thumbnail.
    Thumb(String),
}

/// Live state shown in the panel's status area, written by the render thread.
#[derive(Clone, Default)]
pub struct Status {
    pub bpm: f32,
    pub confidence: f32,
    pub beat_in_bar: u64,
    pub silent: bool,
    pub fps: f32,
    pub device: String,
    pub scene: usize,
    /// Predicted next scene in ordered mode (None = random or unknown).
    pub next_scene: Option<usize>,
    /// "bar N of M" toward the next auto cut (0/0 outside Auto).
    pub bar_in_scene: u32,
    pub bars_total: u32,
    pub clip: Option<String>,
    pub blackout: bool,
    pub fullscreen: bool,
    /// The post effect actually on screen (the auto-pilot's pick in auto mode).
    pub fx: Fx,
    /// External output status line (NDI receiver count / error) — Some while
    /// output is enabled.
    pub output: Option<String>,
    /// Groove 0..1 and breakdown state 0..1 (see audio.rs `Features`).
    pub groove: f32,
    pub calm: f32,
    /// Now playing: the track on screen, and one status line per source.
    pub np_track: Option<String>,
    pub np_status: Vec<(String, String)>,
    /// Clip recorder state (None = not running) / why it can't run.
    pub rec: Option<crate::rec::Status>,
    pub rec_err: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    /// The console: now/next previews, director, palette, sync (mockup 1a).
    Show,
    /// Scene library + inspector — the old Show and Scenes tabs merged
    /// (mockup 1b).
    Perform,
    /// Dancer routines and whole-frame FX on one page.
    DancerFx,
    Stream,
    Timeline,
    Keys,
}

/// Scene-library filter chip (mockup 1b toolbar).
#[derive(Clone, Copy, PartialEq)]
enum LibChip {
    All,
    Flat,
    Heavy,
    Seasonal,
}

impl Tab {
    const ALL: [Tab; 6] = [
        Tab::Show,
        Tab::Perform,
        Tab::DancerFx,
        Tab::Stream,
        Tab::Timeline,
        Tab::Keys,
    ];

    fn label(self) -> &'static str {
        match self {
            Tab::Show => "Show",
            Tab::Perform => "Perform",
            Tab::DancerFx => "Dancer & FX",
            Tab::Stream => "Stream",
            Tab::Timeline => "Timeline",
            Tab::Keys => "Keys",
        }
    }
}

pub struct Panel {
    /// Clone of `win.window` — kept as a field so `p.window` keeps working.
    pub window: Arc<Window>,
    win: crate::egui_win::EguiWin,
    /// Waiting for a key press to bind to this action.
    pub rebinding: Option<Action>,
    tab: Tab,
    /// Text filter for the scene list.
    scene_filter: String,
    /// Library filter chip (All / 2D / 3D / Seasonal).
    chip: LibChip,
    /// Uploaded scene thumbnails, keyed "scene:<name>".
    thumbs: HashMap<String, egui::TextureHandle>,
    /// Thumbnails we've asked the render thread for (Instant = last request,
    /// for re-requesting a thumb that never came back).
    want_thumbs: HashMap<String, Instant>,
    /// Dancer mid-frame thumbs, keyed by clip name — decoded straight from
    /// the clip's frames on disk (no render thread involved).
    clip_thumbs: HashMap<String, Option<egui::TextureHandle>>,
    /// Text filter on the Keys page.
    keys_filter: String,
}

impl Panel {
    pub fn new(
        event_loop: &ActiveEventLoop,
        gpu: Gpu,
        icon: Option<Icon>,
        anchor: Option<&Window>,
    ) -> anyhow::Result<Self> {
        let win = crate::egui_win::EguiWin::new(
            event_loop,
            gpu,
            icon,
            "Trippin — control",
            winit::dpi::PhysicalSize::new(980, 640),
            anchor,
            true, // floats above a fullscreen visuals window
        )?;
        // Library grid + inspector need room; below this it gets cramped.
        win.window
            .set_min_inner_size(Some(winit::dpi::PhysicalSize::new(720, 520)));
        let window = win.window.clone();
        Ok(Self {
            window,
            win,
            rebinding: None,
            tab: Tab::Show,
            scene_filter: String::new(),
            chip: LibChip::All,
            thumbs: HashMap::new(),
            want_thumbs: HashMap::new(),
            clip_thumbs: HashMap::new(),
            keys_filter: String::new(),
        })
    }

    /// Feed a window event to egui. Returns true if egui used it.
    pub fn on_event(&mut self, event: &WindowEvent) -> bool {
        self.win.on_event(event)
    }

    pub fn wants_keyboard(&self) -> bool {
        self.win.wants_keyboard()
    }

    /// Run the egui UI. Called while the settings lock is held — must not do
    /// any GPU work that can block (a stalled surface acquire here would
    /// freeze the render thread via the lock).
    pub fn run_ui(
        &mut self,
        settings: &mut Settings,
        status: &Status,
        scenes: &[String],
        scene_heavy: &[bool],
        heavy_ok: bool,
        clips: &[String],
        tl_shared: &crate::timeline::Shared,
        thumb_store: &Mutex<HashMap<String, (u32, u32, Vec<u8>)>>,
    ) -> (Vec<UiCommand>, bool, PanelFrame) {
        let mut commands = Vec::new();
        let mut changed = false;

        // Collect finished thumbs we asked the render thread for. The store
        // is shared with the editor window — we only take keys we requested.
        let mut got: Vec<(String, u32, u32, Vec<u8>)> = Vec::new();
        {
            let store = thumb_store.lock().unwrap_or_else(|e| e.into_inner());
            for k in self.want_thumbs.keys() {
                if self.thumbs.contains_key(k) {
                    continue;
                }
                if let Some((w, h, px)) = store.get(k) {
                    got.push((k.clone(), *w, *h, px.clone()));
                }
            }
        }

        let rebinding = &mut self.rebinding;
        let tab = &mut self.tab;
        let scene_filter = &mut self.scene_filter;
        let chip = &mut self.chip;
        let thumbs = &mut self.thumbs;
        let want_thumbs = &mut self.want_thumbs;
        let clip_thumbs = &mut self.clip_thumbs;
        let keys_filter = &mut self.keys_filter;
        let frame = self.win.frame(|ui| {
            ui.add_space(6.0);
            for (key, w, h, px) in &got {
                let img = egui::ColorImage::from_rgba_unmultiplied(
                    [*w as usize, *h as usize],
                    px,
                );
                let tex = ui.ctx().load_texture(
                    format!("panel_thumb:{key}"),
                    img,
                    egui::TextureOptions::LINEAR,
                );
                thumbs.insert(key.clone(), tex);
                cache_thumb_png(key, *w, *h, px);
            }
            changed |= build_ui(
                ui,
                settings,
                status,
                scenes,
                scene_heavy,
                heavy_ok,
                clips,
                tl_shared,
                rebinding,
                tab,
                scene_filter,
                chip,
                thumbs,
                want_thumbs,
                clip_thumbs,
                keys_filter,
                &mut commands,
            );
        });
        (commands, changed, frame)
    }

    /// Upload textures, acquire a surface frame and present — all without
    /// holding the settings lock.
    pub fn present(&mut self, frame: PanelFrame) {
        self.win.present(frame);
    }
}

/// The panel layout. Returns true when settings changed (so they get saved).
fn build_ui(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    scenes: &[String],
    scene_heavy: &[bool],
    heavy_ok: bool,
    clips: &[String],
    tl_shared: &crate::timeline::Shared,
    rebinding: &mut Option<Action>,
    tab: &mut Tab,
    scene_filter: &mut String,
    chip: &mut LibChip,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want_thumbs: &mut HashMap<String, Instant>,
    clip_thumbs: &mut HashMap<String, Option<egui::TextureHandle>>,
    keys_filter: &mut String,
    cmd: &mut Vec<UiCommand>,
) -> bool {
    let before = serde_json::to_string(s).unwrap_or_default();

    // --- Status header ---------------------------------------------------
    // The beat pips animate, so repaint every frame while we're up.
    ui.ctx().request_repaint();
    egui::Frame::NONE
        .fill(crate::ui_theme::PANEL)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| header(ui, st, scenes));

    // --- Tabs -------------------------------------------------------------
    ui.horizontal(|ui| {
        for t in Tab::ALL {
            ui.selectable_value(tab, t, t.label());
        }
    });
    ui.separator();

    match *tab {
        Tab::Perform => perform_tab(
            ui,
            s,
            st,
            scenes,
            scene_heavy,
            heavy_ok,
            scene_filter,
            chip,
            thumbs,
            want_thumbs,
            cmd,
        ),
        _ => {
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .horizontal_scroll_offset(0.0)
                .show(ui, |ui| match *tab {
                    Tab::Show => {
                        show_tab(ui, s, st, scenes, scene_heavy, thumbs, want_thumbs, cmd)
                    }
                    Tab::DancerFx => {
                        dancer_fx_tab(ui, s, st, clips, clip_thumbs, cmd)
                    }
                    Tab::Stream => stream_tab(ui, s, st, cmd),
                    Tab::Timeline => timeline_tab(ui, tl_shared, cmd),
                    Tab::Keys => keys_tab(ui, s, rebinding, keys_filter),
                    Tab::Perform => unreachable!(),
                });
        }
    }

    serde_json::to_string(s).unwrap_or_default() != before
}

/// The live-status strip: current + next scene, BPM, beat pips, bar
/// progress and state pills (Docs/mockups.html §2).
fn header(ui: &mut egui::Ui, st: &Status, scenes: &[String]) {
    use crate::ui_theme::*;
    ui.horizontal(|ui| {
        // Left: what's on screen.
        ui.vertical(|ui| {
            section_label(ui, "on screen");
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(
                        scenes.get(st.scene).map(String::as_str).unwrap_or("—"),
                    )
                    .monospace()
                    .size(18.0)
                    .strong(),
                );
                if let Some(nx) = st.next_scene {
                    ui.label(
                        egui::RichText::new(format!(
                            "next → {}",
                            scenes.get(nx).map(String::as_str).unwrap_or("?")
                        ))
                        .size(11.0)
                        .color(MUTED)
                        .monospace(),
                    );
                }
            });
        });
        // Right: fps · BPM · pips · pills.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format!("{:.0} fps", st.fps)).color(MUTED).size(11.0));
            if st.calm > 0.5 {
                pill(ui, "breakdown", BREAKDOWN, BREAKDOWN_BG);
            }
            if st.silent {
                pill(ui, "no signal", WARN, WARN_BG);
            }
            // Four beat pips: the live one is accent.
            let (r, _) = ui.allocate_exact_size(egui::vec2(42.0, 10.0), egui::Sense::hover());
            for i in 0..4u64 {
                let c = egui::pos2(r.min.x + 5.0 + i as f32 * 11.0, r.center().y);
                ui.painter().circle_filled(
                    c,
                    4.0,
                    if i == st.beat_in_bar % 4 { ACCENT } else { BORDER },
                );
            }
            ui.label(
                egui::RichText::new(format!("{:.1}", st.bpm))
                    .monospace()
                    .size(16.0)
                    .strong(),
            );
            ui.label(egui::RichText::new("BPM").size(10.0).color(FAINT));
        });
    });
    // Second line: bar progress to the next cut + dancer + device.
    ui.horizontal(|ui| {
        if st.bars_total > 0 {
            let frac = st.bar_in_scene as f32 / st.bars_total as f32;
            ui.add(
                egui::ProgressBar::new(frac)
                    .desired_width(90.0)
                    .desired_height(4.0)
                    .fill(ACCENT)
                    .corner_radius(2.0)
                    .show_percentage(),
            );
            ui.label(
                egui::RichText::new(bar_progress_line(st))
                    .size(11.0)
                    .color(MUTED),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(&st.device).size(10.0).color(FAINT));
            ui.add(
                egui::ProgressBar::new(st.groove)
                    .desired_width(40.0)
                    .desired_height(4.0)
                    .fill(BREAKDOWN)
                    .corner_radius(2.0),
            )
            .on_hover_text("Groove: how steadily kicks land. Sustained low = breakdown.");
            ui.label(
                egui::RichText::new(format!(
                    "dancer {} · {:.0}% conf",
                    st.clip.as_deref().unwrap_or("off"),
                    st.confidence * 100.0
                ))
                .size(11.0)
                .color(MUTED)
                .monospace(),
            );
        });
    });
}

/// "bar N of M · cut in K" — reads "cut now" on the phrase's last bar.
fn bar_progress_line(st: &Status) -> String {
    let rem = st.bars_total.saturating_sub(st.bar_in_scene);
    if rem == 0 {
        format!("bar {} of {} · cut now", st.bar_in_scene, st.bars_total)
    } else {
        format!("bar {} of {} · cut in {}", st.bar_in_scene, st.bars_total, rem)
    }
}

/// A labelled row of content with the label kept a fixed width.
fn row(ui: &mut egui::Ui, label: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized([92.0, 20.0], egui::Label::new(label));
        body(ui);
    });
}

// ---------------------------------------------------------------------------
// Show tab (mockup 1a): now/next previews, director, palette, sync.
// ---------------------------------------------------------------------------

/// A framed scene preview: thumbnail with a caption bar baked into its
/// lower-left corner, like the NOW / UP NEXT cards in mockup 1a.
#[allow(clippy::too_many_arguments)]
fn preview_card(
    ui: &mut egui::Ui,
    name: &str,
    caption: &str,
    accent_edge: bool,
    w: f32,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    let h = w * 9.0 / 16.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    let p = ui.painter();
    let key = format!("scene:{name}");
    if let Some(tex) = thumb_tex(ui, thumbs, want, &key, cmd) {
        p.image(
            tex,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        // Diagonal stripes as a stand-in until the thumbnail lands.
        p.rect_filled(rect, 6.0, RAISED);
        let mut x = rect.min.x - rect.height();
        while x < rect.max.x {
            p.line_segment(
                [
                    egui::pos2(x, rect.max.y),
                    egui::pos2(x + rect.height(), rect.min.y),
                ],
                egui::Stroke::new(10.0, HOVER.gamma_multiply(0.6)),
            );
            x += 26.0;
        }
    }
    p.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(
            if accent_edge { 1.5 } else { 1.0 },
            if accent_edge { ACCENT } else { BORDER_HI },
        ),
        egui::StrokeKind::Inside,
    );
    let cap_w = (caption.len() as f32 * 7.0 + 16.0).clamp(48.0, w - 12.0);
    let cap_r = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + 8.0, rect.max.y - 26.0),
        egui::pos2(rect.min.x + 8.0 + cap_w, rect.max.y - 8.0),
    );
    p.rect_filled(cap_r, 4.0, INSET.gamma_multiply(0.85));
    p.text(
        cap_r.center(),
        egui::Align2::CENTER_CENTER,
        caption,
        egui::FontId::monospace(11.0),
        if accent_edge { TEXT } else { MUTED },
    );
}

/// One palette swatch — a small gradient chip that selects the palette.
fn palette_swatch(ui: &mut egui::Ui, s: &mut Settings, name: &str, w: f32, h: f32) {
    use crate::ui_theme::*;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    let lut = crate::palettes::lut(name);
    let n = 24usize;
    for i in 0..n {
        let c = &lut[i * (crate::palettes::LUT_SIZE / n) * 4..];
        ui.painter().rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.min.x + w * i as f32 / n as f32, rect.min.y),
                egui::pos2(rect.min.x + w * (i + 1) as f32 / n as f32 + 1.0, rect.max.y),
            ),
            0.0,
            egui::Color32::from_rgb(c[0], c[1], c[2]),
        );
    }
    let sel = s.palette == name;
    ui.painter().rect_stroke(
        rect,
        3.0,
        egui::Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { TEXT } else { BORDER }),
        egui::StrokeKind::Inside,
    );
    if resp.clicked() {
        s.palette = name.to_string();
    }
    resp.on_hover_text(name);
}

/// Mockup 1a: the console. Now/up-next previews over Director, Palette and
/// Sync cards, with Blackout + Fullscreen at the foot.
#[allow(clippy::too_many_arguments)]
fn show_tab(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    scenes: &[String],
    heavy: &[bool],
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    let now = scenes.get(st.scene).cloned().unwrap_or_else(|| "—".into());
    let next = st.next_scene.map(|i| scenes.get(i).cloned().unwrap_or_else(|| "?".into()));

    // Row 1: NOW card | prev/next column | UP NEXT card.
    let avail = ui.available_width();
    let btn_w = 150.0;
    let gap = ui.spacing().item_spacing.x * 2.0;
    let card_w = ((avail - btn_w - gap) / 2.0).max(120.0);
    let card_h = card_w * 9.0 / 16.0;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            section_label(ui, "now");
            preview_card(
                ui, &now, &format!("{now} · live"), true, card_w, thumbs, want, cmd,
            );
        });
        ui.allocate_ui(egui::vec2(btn_w, card_h + 18.0), |ui| {
            ui.vertical(|ui| {
                ui.add_space(14.0);
                let bw = ui.available_width();
                if ui
                    .add_sized([bw, 30.0], egui::Button::new("◀ Prev"))
                    .clicked()
                {
                    cmd.push(UiCommand::Do(Action::PrevScene));
                }
                ui.add_space(6.0);
                if ui
                    .add_sized(
                        [bw, 30.0],
                        egui::Button::new("Next ▶").fill(ACCENT_SEL),
                    )
                    .clicked()
                {
                    cmd.push(UiCommand::Do(Action::NextScene));
                }
                ui.add_space(6.0);
                let cut = if st.bars_total > 0 {
                    let rem = st.bars_total.saturating_sub(st.bar_in_scene);
                    if rem == 0 { "cut now".into() } else { format!("cut in {rem} bars") }
                } else {
                    "held".into()
                };
                ui.label(egui::RichText::new(cut).size(11.0).color(MUTED).monospace());
            });
        });
        ui.vertical(|ui| {
            section_label(
                ui,
                if s.random_order { "up next · random" } else { "up next" },
            );
            match &next {
                Some(n) => {
                    let tag = if heavy.get(st.next_scene.unwrap()).copied().unwrap_or(false) {
                        "3D"
                    } else {
                        "2D"
                    };
                    preview_card(
                        ui, n, &format!("{n} ({tag})"), false, card_w, thumbs, want, cmd,
                    );
                }
                None => {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 6.0, CARD);
                    ui.painter().rect_stroke(
                        rect,
                        6.0,
                        egui::Stroke::new(1.0, BORDER),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "manual — nothing queued",
                        egui::FontId::monospace(11.0),
                        FAINT,
                    );
                }
            }
        });
    });

    ui.add_space(8.0);

    // Row 2: Director card left; Palette + Sync stacked right.
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Director").size(14.0).strong());
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Mode").size(11.0).color(MUTED));
            segmented_wide(
                ui,
                &mut s.mode,
                &[(Mode::Auto, "Auto"), (Mode::Static, "Static"), (Mode::Manual, "Manual")],
            );
            ui.small(match s.mode {
                Mode::Auto => "Cuts on phrases and drops; the dancer follows the track.",
                Mode::Static => "Holds the current scene until you change it.",
                Mode::Manual => "Nothing changes by itself — you drive every cut.",
            });
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Scene length (bars)").size(11.0).color(MUTED));
            segmented_wide(
                ui,
                &mut s.phrase_bars,
                &[(4u32, "4"), (8, "8"), (16, "16"), (32, "32")],
            );
            ui.add_space(6.0);
            ui.checkbox(&mut s.breakdown_mode, "Detect breakdowns");
            ui.checkbox(&mut s.cut_on_drops, "Cut early when a drop lands");
            ui.checkbox(&mut s.random_order, "Random order");
        });

        let ui = &mut cols[1];
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Palette").size(14.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(&s.palette).size(11.0).color(ACCENT).monospace(),
                    );
                });
            });
            ui.add_space(4.0);
            let names: Vec<&str> = crate::palettes::names().collect();
            let n_per_row = 7usize;
            let sw_w = ((ui.available_width() - (n_per_row as f32 - 1.0) * 6.0)
                / n_per_row as f32)
                .max(28.0);
            for row_names in names.chunks(n_per_row) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for name in row_names {
                        palette_swatch(ui, s, name, sw_w, 18.0);
                    }
                });
            }
        });
        ui.add_space(6.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Sync").size(14.0).strong());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Latency").size(11.0).color(MUTED));
                ui.add(
                    egui::Slider::new(&mut s.latency_ms, -100.0..=200.0)
                        .step_by(5.0)
                        .show_value(false),
                );
                ui.label(
                    egui::RichText::new(format!("{:+.0} ms", s.latency_ms))
                        .size(11.0)
                        .color(MUTED)
                        .monospace(),
                );
            });
            if ui.button("Mark this beat as the downbeat (the \"one\")").clicked() {
                cmd.push(UiCommand::Do(Action::MarkDownbeat));
            }
        });
    });

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let bw = ((ui.available_width() - ui.spacing().item_spacing.x - 56.0) / 2.0).max(40.0);
        let bo = if st.blackout { "Blackout: ON" } else { "Blackout" };
        if ui
            .add_sized(
                [bw, 28.0],
                egui::Button::new(egui::RichText::new(bo).color(if st.blackout {
                    TEXT
                } else {
                    DANGER
                }))
                .fill(if st.blackout { DANGER } else { DANGER_BG }),
            )
            .clicked()
        {
            cmd.push(UiCommand::Do(Action::Blackout));
        }
        key_badge(ui, &key_short(s.keys.get(&Action::Blackout).map_or("", String::as_str)));
        let fs = if st.fullscreen { "Leave fullscreen" } else { "Fullscreen" };
        if ui.add_sized([bw, 28.0], egui::Button::new(fs)).clicked() {
            cmd.push(UiCommand::Do(Action::Fullscreen));
        }
        key_badge(ui, &key_short(s.keys.get(&Action::Fullscreen).map_or("", String::as_str)));
    });
}

// ---------------------------------------------------------------------------
// Perform tab (mockup 1b): scene library grid left, inspector on the right.
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn perform_tab(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    scenes: &[String],
    heavy: &[bool],
    heavy_ok: bool,
    filter: &mut String,
    chip: &mut LibChip,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    // Library / Pads switch (persisted) — then the view's own toolbar bits.
    ui.horizontal(|ui| {
        segmented(
            ui,
            &mut s.perform_pads,
            &[(false, "Library"), (true, "Pads")],
        );
    });

    if s.perform_pads {
        pads_view(ui, s, st, scenes, thumbs, want, cmd);
        return;
    }

    // Right inspector — fixed width; blackout/fullscreen pinned at its foot.
    egui::Panel::right("inspector")
        .exact_size(292.0)
        .frame(
            egui::Frame::NONE
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .stroke(egui::Stroke::new(1.0, BORDER)),
        )
        .show(ui, |ui| {
            inspector(ui, s, st, scenes, heavy_ok, thumbs, want, cmd);
        });

    let date = today();
    let heavy_on = match s.heavy_scenes {
        Tristate::Auto => heavy_ok,
        Tristate::On => true,
        Tristate::Off => false,
    };
    let q = filter.to_lowercase();
    let shown: Vec<usize> = (0..scenes.len())
        .filter(|&i| {
            let n = &scenes[i];
            if !q.is_empty() && !n.to_lowercase().contains(&q) {
                return false;
            }
            let is_heavy = heavy.get(i).copied().unwrap_or(false);
            match *chip {
                LibChip::All => true,
                LibChip::Flat => !is_heavy,
                LibChip::Heavy => is_heavy,
                LibChip::Seasonal => in_season(n, date).is_some(),
            }
        })
        .collect();
    let in_rotation = (0..scenes.len())
        .filter(|&i| {
            !s.disabled_scenes.contains(&scenes[i])
                && if heavy.get(i).copied().unwrap_or(false) {
                    heavy_on
                } else {
                    s.flat_scenes
                }
        })
        .count();

    // Toolbar: search field, filter pills (active one is bright, like the
    // mockup), then the stats line with all on/off at its right edge.
    ui.horizontal(|ui| {
        ui.add_space(2.0);
        ui.add(
            egui::TextEdit::singleline(filter)
                .desired_width((ui.available_width() * 0.5).min(320.0))
                .hint_text(format!("filter {} scenes…", scenes.len())),
        );
        for (c, l) in [
            (LibChip::All, "All"),
            (LibChip::Flat, "2D"),
            (LibChip::Heavy, "3D"),
            (LibChip::Seasonal, "Seasonal"),
        ] {
            let on = *chip == c;
            let b = egui::Button::new(
                egui::RichText::new(l)
                    .size(11.5)
                    .color(if on { INSET } else { MUTED }),
            )
            .fill(if on { TEXT } else { RAISED })
            .corner_radius(egui::CornerRadius::same(9));
            if ui.add(b).clicked() {
                *chip = c;
            }
        }
    });
    // Keep the right-aligned controls inside the library column — the
    // central panel underlaps the inspector by ~15px, so unconstrained
    // right alignment would paint into the gutter.
    let stat_w = (ui.available_width() - 24.0).max(80.0);
    ui.allocate_ui(egui::vec2(stat_w, 18.0), |ui| {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} shown · {} of {} in rotation · click to preview · toggle to add/remove from rotation",
                    shown.len(),
                    in_rotation,
                    scenes.len()
                ))
                .size(10.5)
                .color(FAINT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("all off").size(11.0).color(ACCENT),
                    ).fill(egui::Color32::TRANSPARENT).frame(false))
                    .clicked()
                {
                    s.disabled_scenes = scenes.to_vec();
                }
                ui.label(egui::RichText::new("·").size(11.0).color(FAINT));
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("all on").size(11.0).color(ACCENT),
                    ).fill(egui::Color32::TRANSPARENT).frame(false))
                    .clicked()
                {
                    s.disabled_scenes.clear();
                }
            });
        });
    });
    ui.add_space(2.0);

    // Thumbnail grid — reflows with the column width. The scrollbar takes
    // real layout space (non-floating) and `max_width` leaves a gap before
    // the inspector so the handle can't sit on its border. x-offset is
    // pinned: a stray horizontal offset has no scrollbar to undo it.
    ui.spacing_mut().scroll.floating = false;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_width(ui.available_width() - 16.0)
        .horizontal_scroll_offset(0.0)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            let avail = ui.available_width() - 4.0;
            let cols = ((avail + 10.0) / 160.0).floor().max(2.0) as usize;
            for chunk in shown.chunks(cols) {
                ui.horizontal(|ui| {
                    for &i in chunk {
                        scene_tile(
                            ui,
                            s,
                            st,
                            i,
                            scenes[i].as_str(),
                            heavy.get(i).copied().unwrap_or(false),
                            heavy_on,
                            date,
                            thumbs,
                            want,
                            cmd,
                        );
                    }
                });
            }
            ui.add_space(4.0);
        });
}

/// One scene-library tile: thumbnail, type tag, rotation checkbox, name.
/// Click the tile to cut to the scene (mockup 1b).
#[allow(clippy::too_many_arguments)]
fn scene_tile(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    i: usize,
    name: &str,
    is_heavy: bool,
    heavy_on: bool,
    date: (u32, u32),
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    const TW: f32 = 150.0;
    const TH: f32 = 108.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(TW, TH), egui::Sense::click());
    let p = ui.painter();
    let live = i == st.scene;
    let on = !s.disabled_scenes.iter().any(|d| d == name);
    let seasonal = in_season(name, date).is_some();
    // Can't join rotation at all — dimmed and click does nothing.
    let blocked = (is_heavy && !heavy_on) || (!is_heavy && !s.flat_scenes);
    let dim = if blocked || !on { 0.45 } else { 1.0 };

    p.rect_filled(rect, 6.0, CARD);
    let img_r = egui::Rect::from_min_size(
        rect.min + egui::vec2(5.0, 5.0),
        egui::vec2(TW - 10.0, 78.0),
    );
    let key = format!("scene:{name}");
    if let Some(tex) = thumb_tex(ui, thumbs, want, &key, cmd) {
        p.image(
            tex,
            img_r,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE.gamma_multiply(dim),
        );
    } else {
        p.rect_filled(img_r, 4.0, RAISED);
        p.text(
            img_r.center(),
            egui::Align2::CENTER_CENTER,
            name.get(..2).unwrap_or(name),
            egui::FontId::monospace(13.0),
            FAINT,
        );
    }
    p.rect_stroke(
        img_r,
        4.0,
        egui::Stroke::new(1.0, BORDER),
        egui::StrokeKind::Inside,
    );

    // Type tag, top-left over the thumbnail.
    let tag = if seasonal {
        "❄"
    } else if is_heavy {
        "3D"
    } else {
        "2D"
    };
    let tag_r = egui::Rect::from_min_size(img_r.min + egui::vec2(4.0, 4.0), egui::vec2(22.0, 13.0));
    p.rect_filled(tag_r, 3.0, INSET.gamma_multiply(0.85));
    p.text(
        tag_r.center(),
        egui::Align2::CENTER_CENTER,
        tag,
        egui::FontId::monospace(9.0),
        if seasonal { LANE_FX } else { MUTED },
    );

    // Rotation checkbox, top-right — register the interact after the tile so
    // it wins the click inside its rect.
    let cb_r = egui::Rect::from_min_size(
        egui::pos2(img_r.max.x - 20.0, img_r.min.y + 4.0),
        egui::vec2(16.0, 16.0),
    );
    let cb = ui.interact(cb_r, resp.id.with("rot"), egui::Sense::click());
    let cb_clicked = cb.clicked();
    p.rect_filled(
        cb_r,
        3.0,
        if on {
            ACCENT
        } else {
            INSET.gamma_multiply(0.9)
        },
    );
    p.rect_stroke(
        cb_r,
        3.0,
        egui::Stroke::new(1.0, BORDER_HI),
        egui::StrokeKind::Inside,
    );
    if on {
        p.text(
            cb_r.center(),
            egui::Align2::CENTER_CENTER,
            "✓",
            egui::FontId::proportional(11.0),
            TEXT,
        );
    }
    if cb_clicked {
        if on {
            s.disabled_scenes.push(name.to_string());
        } else {
            s.disabled_scenes.retain(|d| d != name);
        }
    }
    cb.on_hover_text(if on {
        "In rotation — click to leave it out"
    } else {
        "Out of rotation — click to include it"
    });

    // Name row under the thumbnail.
    let shown = if name.len() > 20 {
        format!("{}…", &name[..19])
    } else {
        name.to_string()
    };
    p.text(
        egui::pos2(rect.min.x + 6.0, img_r.max.y + 5.0),
        egui::Align2::LEFT_TOP,
        shown,
        egui::FontId::monospace(11.0),
        if live {
            ACCENT
        } else {
            TEXT.gamma_multiply(dim)
        },
    );
    if live {
        p.text(
            egui::pos2(rect.max.x - 6.0, img_r.max.y + 5.0),
            egui::Align2::RIGHT_TOP,
            "LIVE",
            egui::FontId::proportional(9.0),
            ACCENT,
        );
    }
    p.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(if live { 1.5 } else { 1.0 }, if live { ACCENT } else { BORDER }),
        egui::StrokeKind::Inside,
    );
    if resp.clicked() && !cb_clicked && !blocked {
        cmd.push(UiCommand::GoToScene(i));
    }
    resp.on_hover_text(if blocked {
        format!("{name} — out of rotation (scene set off)")
    } else {
        name.to_string()
    });
}

/// Mockup 1b right column: live preview, transport, look controls, and the
/// advanced knobs folded under collapsing headers.
#[allow(clippy::too_many_arguments)]
fn inspector(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    scenes: &[String],
    heavy_ok: bool,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    let scroll_h = (ui.available_height() - 44.0).max(80.0);
    ui.spacing_mut().scroll.floating = false;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(scroll_h)
        .horizontal_scroll_offset(0.0)
        .show(ui, |ui| {
            // Live preview with the accent frame + a "live output" caption
            // baked into its lower-left corner (mockup 1b).
            let name = scenes.get(st.scene).cloned().unwrap_or_default();
            let w = ui.available_width();
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(w, w * 9.0 / 16.0), egui::Sense::hover());
            let p = ui.painter();
            let key = format!("scene:{name}");
            if let Some(tex) = thumb_tex(ui, thumbs, want, &key, cmd) {
                p.image(
                    tex,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            } else {
                p.rect_filled(rect, 6.0, RAISED);
            }
            p.rect_stroke(
                rect,
                6.0,
                egui::Stroke::new(1.5, ACCENT),
                egui::StrokeKind::Inside,
            );
            let cap_r = egui::Rect::from_min_max(
                egui::pos2(rect.min.x + 6.0, rect.max.y - 22.0),
                egui::pos2(rect.min.x + 92.0, rect.max.y - 6.0),
            );
            p.rect_filled(cap_r, 4.0, INSET.gamma_multiply(0.85));
            p.text(
                cap_r.center(),
                egui::Align2::CENTER_CENTER,
                "live output",
                egui::FontId::monospace(10.0),
                MUTED,
            );

            // Name left, "bar N of M" right — then the thin cut-progress bar.
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&name).monospace().size(16.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if st.bars_total > 0 {
                        ui.label(
                            egui::RichText::new(format!(
                                "bar {} of {}",
                                st.bar_in_scene, st.bars_total
                            ))
                            .monospace()
                            .size(11.0)
                            .color(MUTED),
                        );
                    }
                });
            });
            if st.bars_total > 0 {
                let frac = (st.bar_in_scene as f32 / st.bars_total.max(1) as f32).clamp(0.0, 1.0);
                let (br, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 4.0),
                    egui::Sense::hover(),
                );
                let pp = ui.painter();
                pp.rect_filled(br, 2.0, INSET);
                pp.rect_filled(
                    egui::Rect::from_min_size(
                        br.min,
                        egui::vec2(br.width() * frac, br.height()),
                    ),
                    2.0,
                    ACCENT,
                );
            } else {
                ui.label(
                    egui::RichText::new(match s.mode {
                        Mode::Static => "static — held until you change it",
                        Mode::Manual => "manual — nothing changes by itself",
                        Mode::Auto => "",
                    })
                    .size(11.0)
                    .color(MUTED),
                );
            }

            // Next-scene line left; a breakdown pill sits at the right edge.
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if let Some(nx) = st.next_scene {
                    ui.label(
                        egui::RichText::new(format!(
                            "next → {}",
                            scenes.get(nx).map(String::as_str).unwrap_or("?")
                        ))
                        .size(11.0)
                        .color(ACCENT)
                        .monospace(),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(bar_progress_line(st))
                            .size(11.0)
                            .color(FAINT),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if st.calm > 0.5 {
                        pill(ui, "● breakdown", BREAKDOWN, BREAKDOWN_BG);
                    }
                });
            });
            ui.add_space(4.0);

            // Prev / Next — Next carries the accent like the mockup.
            ui.horizontal(|ui| {
                let bw = ((ui.available_width() - ui.spacing().item_spacing.x) / 2.0).max(40.0);
                if ui
                    .add_sized([bw, 28.0], egui::Button::new("◀ Prev"))
                    .clicked()
                {
                    cmd.push(UiCommand::Do(Action::PrevScene));
                }
                if ui
                    .add_sized(
                        [bw, 28.0],
                        egui::Button::new("Next ▶").fill(ACCENT_SEL),
                    )
                    .clicked()
                {
                    cmd.push(UiCommand::Do(Action::NextScene));
                }
            });

            ui.add_space(8.0);
            section_label(ui, "controls");
            card().show(ui, |ui| {
                ctl_row(ui, "Mode", |ui| {
                    segmented(
                        ui,
                        &mut s.mode,
                        &[
                            (Mode::Auto, "Auto"),
                            (Mode::Static, "Static"),
                            (Mode::Manual, "Manual"),
                        ],
                    );
                });
                ctl_row(ui, "Bars", |ui| {
                    segmented(
                        ui,
                        &mut s.phrase_bars,
                        &[(4u32, "4"), (8, "8"), (16, "16"), (32, "32")],
                    );
                });
                ctl_row(ui, "Dancer", |ui| {
                    let look = s.dancer_style.map(|i| STYLES[i]).unwrap_or("auto");
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {look}",
                            st.clip.as_deref().unwrap_or("off")
                        ))
                        .monospace()
                        .size(11.0),
                    );
                    if ui.small_button("next").clicked() {
                        cmd.push(UiCommand::Do(Action::NextClip));
                    }
                });
                ctl_row(ui, "Look", |ui| {
                    egui::ComboBox::from_id_salt("look")
                        .width(120.0)
                        .selected_text(s.dancer_style.map(|i| STYLES[i]).unwrap_or("auto"))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut s.dancer_style, None, "auto");
                            for (i, name) in STYLES.iter().enumerate() {
                                ui.selectable_value(&mut s.dancer_style, Some(i), *name);
                            }
                        });
                });
                ctl_row(ui, "Effect", |ui| {
                    egui::ComboBox::from_id_salt("fx")
                        .width(88.0)
                        .selected_text(s.fx.label())
                        .show_ui(ui, |ui| {
                            for f in Fx::ALL {
                                if ui
                                    .selectable_value(&mut s.fx, f, f.label())
                                    .clicked()
                                {
                                    s.fx_auto = false;
                                }
                            }
                        });
                    ui.checkbox(&mut s.fx_auto, "")
                        .on_hover_text("Auto — a fresh effect on every scene cut");
                });
                ctl_row(ui, "Palette", |ui| {
                    egui::ComboBox::from_id_salt("pal")
                        .width(110.0)
                        .selected_text(s.palette.as_str())
                        .show_ui(ui, |ui| {
                            for name in crate::palettes::names() {
                                ui.selectable_value(&mut s.palette, name.to_string(), name);
                            }
                        });
                });
                palette_strip(ui, &s.palette);
            });

            ui.add_space(8.0);
            egui::CollapsingHeader::new(
                egui::RichText::new("director & sync").size(11.0).color(MUTED),
            )
            .default_open(false)
            .show(ui, |ui| {
                ctl_row(ui, "Breakdowns", |ui| {
                    segmented(
                        ui,
                        &mut s.breakdown_mode,
                        &[(true, "Detect"), (false, "Off")],
                    );
                });
                ui.small(if s.breakdown_mode {
                    "When the drums drop out, visuals calm down."
                } else {
                    "Always react as if the beat is playing."
                });
                ui.checkbox(&mut s.cut_on_drops, "Cut early when a drop lands");
                ui.checkbox(&mut s.random_order, "Random scene order");
                ctl_row(ui, "Latency", |ui| {
                    ui.add(
                        egui::Slider::new(&mut s.latency_ms, -100.0..=200.0)
                            .step_by(5.0)
                            .suffix(" ms"),
                    );
                });
                ui.small(
                    "Raise it if the visuals land after the beat, lower it if they land before.",
                );
                if ui
                    .button("Mark this beat as the downbeat (the \"one\")")
                    .clicked()
                {
                    cmd.push(UiCommand::Do(Action::MarkDownbeat));
                }
            });
            egui::CollapsingHeader::new(
                egui::RichText::new("scene sets").size(11.0).color(MUTED),
            )
            .default_open(false)
            .show(ui, |ui| {
                ctl_row(ui, "Seasonal", |ui| {
                    segmented(
                        ui,
                        &mut s.seasonal,
                        &[
                            (Seasonal::Auto, "Auto"),
                            (Seasonal::Always, "Always"),
                            (Seasonal::Off, "Off"),
                        ],
                    );
                });
                ctl_row(ui, "3D scenes", |ui| {
                    segmented(
                        ui,
                        &mut s.heavy_scenes,
                        &[
                            (Tristate::Auto, "Auto"),
                            (Tristate::On, "On"),
                            (Tristate::Off, "Off"),
                        ],
                    );
                });
                ui.small(match s.heavy_scenes {
                    Tristate::Auto if heavy_ok => {
                        "Raymarched scenes are in rotation — this GPU can handle them."
                    }
                    Tristate::Auto => {
                        "Raymarched scenes are off — this GPU can't keep up. Force them with On."
                    }
                    Tristate::On => {
                        "Raymarched scenes forced on — may drop frames on a weak GPU."
                    }
                    Tristate::Off => "Raymarched scenes are off.",
                });
                ctl_row(ui, "2D scenes", |ui| {
                    segmented(ui, &mut s.flat_scenes, &[(true, "On"), (false, "Off")]);
                });
                let in_now: Vec<&str> = scenes
                    .iter()
                    .map(String::as_str)
                    .filter(|n| in_season(n, today()) == Some(true))
                    .collect();
                ui.small(if in_now.is_empty() {
                    "Nothing seasonal today.".to_string()
                } else {
                    format!("In season: {}", in_now.join(", "))
                });
            });
        });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let bw = ((ui.available_width() - ui.spacing().item_spacing.x - 56.0) / 2.0).max(40.0);
        let bo = if st.blackout {
            "Blackout: ON"
        } else {
            "Blackout"
        };
        if ui
            .add_sized(
                [bw, 28.0],
                egui::Button::new(egui::RichText::new(bo).color(if st.blackout {
                    TEXT
                } else {
                    DANGER
                }))
                .fill(if st.blackout { DANGER } else { DANGER_BG }),
            )
            .clicked()
        {
            cmd.push(UiCommand::Do(Action::Blackout));
        }
        key_badge(ui, &key_short(s.keys.get(&Action::Blackout).map_or("", String::as_str)));
        let fs = if st.fullscreen {
            "Leave fullscreen"
        } else {
            "Fullscreen"
        };
        if ui.add_sized([bw, 28.0], egui::Button::new(fs)).clicked() {
            cmd.push(UiCommand::Do(Action::Fullscreen));
        }
        key_badge(ui, &key_short(s.keys.get(&Action::Fullscreen).map_or("", String::as_str)));
    });
}

// ---------------------------------------------------------------------------
// Pads view (mockup 1c): big BPM + beat blocks, NOW / UP NEXT, action pads,
// palette column on the right.
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn pads_view(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    scenes: &[String],
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    // Palette column on the right.
    egui::Panel::right("palettes")
        .exact_size(196.0)
        .frame(
            egui::Frame::NONE
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .stroke(egui::Stroke::new(1.0, BORDER)),
        )
        .show(ui, |ui| palette_column(ui, s));

    // BPM + the four beat blocks.
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(format!("{:.0}", st.bpm))
                .monospace()
                .size(34.0)
                .strong(),
        );
        ui.label(egui::RichText::new("BPM").size(11.0).color(FAINT));
        ui.add_space(10.0);
        let (r, _) = ui.allocate_exact_size(egui::vec2(96.0, 22.0), egui::Sense::hover());
        for i in 0..4u64 {
            let c = egui::Rect::from_min_size(
                egui::pos2(r.min.x + i as f32 * 26.0, r.min.y),
                egui::vec2(20.0, 20.0),
            );
            ui.painter().rect_filled(
                c,
                4.0,
                if i == st.beat_in_bar % 4 { ACCENT } else { BORDER },
            );
        }
        if st.silent {
            pill(ui, "no signal", WARN, WARN_BG);
        }
    });
    ui.add_space(6.0);

    // NOW / UP NEXT.
    ui.horizontal(|ui| {
        let w = (ui.available_width() - 10.0) / 2.0;
        pad_preview(ui, thumbs, want, cmd, w, "NOW", scenes.get(st.scene), {
            if st.bars_total > 0 {
                bar_progress_line(st)
            } else {
                String::new()
            }
        });
        let next_line = if st.bars_total > 0 {
            let rem = st.bars_total.saturating_sub(st.bar_in_scene);
            format!("in {rem} bar{}", if rem == 1 { "" } else { "s" })
        } else {
            String::new()
        };
        pad_preview(
            ui,
            thumbs,
            want,
            cmd,
            w,
            "UP NEXT",
            st.next_scene.and_then(|i| scenes.get(i)),
            next_line,
        );
    });
    ui.add_space(8.0);

    // Action pads — same Actions as the hotkeys, badge shows the binding.
    section_label(ui, "pads");
    let pads: [(Action, &str); 8] = [
        (Action::NextScene, "next scene"),
        (Action::PrevScene, "prev scene"),
        (Action::ModeStatic, "hold / static"),
        (Action::Blackout, "blackout"),
        (Action::MarkDownbeat, "mark the one"),
        (Action::CycleFx, "next fx"),
        (Action::NextStyle, "dancer look"),
        (Action::CycleCanon, "canon"),
    ];
    let pad_w = ((ui.available_width() - 3.0 * 10.0) / 4.0).max(100.0);
    for row in pads.chunks(4) {
        ui.horizontal(|ui| {
            for (a, label) in row {
                let key =
                    key_short(s.keys.get(a).map_or("", String::as_str));
                let mut job = egui::text::LayoutJob::default();
                job.append(
                    label,
                    0.0,
                    egui::TextFormat::simple(
                        egui::FontId::proportional(13.0),
                        TEXT,
                    ),
                );
                job.append(
                    &format!("\n{key}"),
                    0.0,
                    egui::TextFormat::simple(egui::FontId::monospace(10.0), MUTED),
                );
                if ui
                    .add_sized(
                        [pad_w, 52.0],
                        egui::Button::new(job).fill(CARD),
                    )
                    .clicked()
                {
                    cmd.push(UiCommand::Do(*a));
                }
            }
        });
    }
}

/// A NOW / UP NEXT card: thumbnail + name + timing line.
#[allow(clippy::too_many_arguments)]
fn pad_preview(
    ui: &mut egui::Ui,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
    w: f32,
    title: &str,
    scene: Option<&String>,
    line: String,
) {
    use crate::ui_theme::*;
    egui::Frame::NONE
        .fill(CARD)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(w - 20.0);
            section_label(ui, title);
            ui.horizontal(|ui| {
                let (rect, _) = ui
                    .allocate_exact_size(egui::vec2(96.0, 54.0), egui::Sense::hover());
                let p = ui.painter();
                if let Some(name) = scene {
                    let key = format!("scene:{name}");
                    if let Some(tex) = thumb_tex(ui, thumbs, want, &key, cmd) {
                        p.image(
                            tex,
                            rect,
                            egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2(1.0, 1.0),
                            ),
                            egui::Color32::WHITE,
                        );
                    } else {
                        p.rect_filled(rect, 4.0, RAISED);
                    }
                } else {
                    p.rect_filled(rect, 4.0, RAISED);
                }
                p.rect_stroke(
                    rect,
                    4.0,
                    egui::Stroke::new(1.0, BORDER),
                    egui::StrokeKind::Inside,
                );
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(scene.map_or("—", String::as_str))
                            .monospace()
                            .size(14.0)
                            .strong(),
                    );
                    if !line.is_empty() {
                        ui.label(egui::RichText::new(line).size(11.0).color(MUTED));
                    }
                });
            });
        });
}

/// The scrollable palette column on the pads view's right edge.
fn palette_column(ui: &mut egui::Ui, s: &mut Settings) {
    use crate::ui_theme::*;
    section_label(ui, "palette");
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .horizontal_scroll_offset(0.0)
        .show(ui, |ui| {
            for name in crate::palettes::names() {
                let sel = s.palette == name;
                let w = ui.available_width();
                let (rect, resp) = ui.allocate_exact_size(
                    egui::vec2(w, 34.0),
                    egui::Sense::click(),
                );
                let p = ui.painter();
                p.rect_filled(rect, 5.0, CARD);
                p.rect_stroke(
                    rect,
                    5.0,
                    egui::Stroke::new(1.0, if sel { ACCENT } else { BORDER }),
                    egui::StrokeKind::Inside,
                );
                p.text(
                    rect.min + egui::vec2(8.0, 5.0),
                    egui::Align2::LEFT_TOP,
                    name,
                    egui::FontId::monospace(10.0),
                    if sel { ACCENT } else { TEXT },
                );
                // mini gradient strip along the bottom of the row
                let lut = crate::palettes::lut(name);
                let strip = egui::Rect::from_min_max(
                    egui::pos2(rect.min.x + 6.0, rect.max.y - 9.0),
                    egui::pos2(rect.max.x - 6.0, rect.max.y - 4.0),
                );
                let n = 48usize;
                for i in 0..n {
                    let c = &lut[i * (crate::palettes::LUT_SIZE / n) * 4..];
                    p.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(
                                strip.min.x + strip.width() * i as f32 / n as f32,
                                strip.min.y,
                            ),
                            egui::pos2(
                                strip.min.x
                                    + strip.width() * (i + 1) as f32 / n as f32
                                    + 1.0,
                                strip.max.y,
                            ),
                        ),
                        0.0,
                        egui::Color32::from_rgb(c[0], c[1], c[2]),
                    );
                }
                if resp.clicked() {
                    s.palette = name.to_string();
                }
            }
        });
}

/// A labelled control row for the inspector — tighter than `row`.
fn ctl_row(ui: &mut egui::Ui, label: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [56.0, 20.0],
            egui::Label::new(
                egui::RichText::new(label)
                    .size(11.0)
                    .color(crate::ui_theme::MUTED),
            ),
        );
        body(ui);
    });
}

/// The palette gradient strip — the actual LUT the GPU gets.
fn palette_strip(ui: &mut egui::Ui, palette: &str) {
    let lut = crate::palettes::lut(palette);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 8.0),
        egui::Sense::hover(),
    );
    let n = 64usize;
    for i in 0..n {
        let c = &lut[i * (crate::palettes::LUT_SIZE / n) * 4..];
        ui.painter().rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.min.x + rect.width() * i as f32 / n as f32, rect.min.y),
                egui::pos2(
                    rect.min.x + rect.width() * (i + 1) as f32 / n as f32 + 1.0,
                    rect.max.y,
                ),
            ),
            0.0,
            egui::Color32::from_rgb(c[0], c[1], c[2]),
        );
    }
}

/// Texture id for a "scene:<name>" thumbnail — serves the memory map, then
/// the disk cache, and finally queues a render (re-requesting every few
/// seconds while it stays missing).
fn thumb_tex(
    ui: &egui::Ui,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    key: &str,
    cmd: &mut Vec<UiCommand>,
) -> Option<egui::TextureId> {
    if let Some(t) = thumbs.get(key) {
        return Some(t.id());
    }
    if !want.contains_key(key) {
        // First time we've seen this key this run — try the disk cache
        // before paying for a GPU render.
        if let Some(t) = cached_thumb(ui.ctx(), key) {
            let id = t.id();
            thumbs.insert(key.to_string(), t);
            return Some(id);
        }
    }
    let stale = match want.get(key) {
        None => true,
        Some(t) => t.elapsed() > Duration::from_secs(3),
    };
    if stale {
        want.insert(key.to_string(), Instant::now());
        cmd.push(UiCommand::Thumb(key.to_string()));
    }
    None
}

/// On-disk thumbnail cache — scene looks barely change, so render once.
fn thumbs_dir() -> PathBuf {
    crate::config::data_dir().join("thumbs")
}

fn thumb_file(key: &str) -> PathBuf {
    thumbs_dir().join(format!("{}.png", key.replace(':', "_")))
}

fn cached_thumb(ctx: &egui::Context, key: &str) -> Option<egui::TextureHandle> {
    let img = image::open(thumb_file(key)).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(ctx.load_texture(
        format!("panel_thumb:{key}"),
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &img),
        egui::TextureOptions::LINEAR,
    ))
}

fn cache_thumb_png(key: &str, w: u32, h: u32, px: &[u8]) {
    let dir = thumbs_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let _ = image::save_buffer(thumb_file(key), px, w, h, image::ColorType::Rgba8);
}

/// Compact key name for the badges under buttons.
fn key_short(k: &str) -> String {
    match k {
        "ArrowRight" => "→".into(),
        "ArrowLeft" => "←".into(),
        "ArrowUp" => "↑".into(),
        "ArrowDown" => "↓".into(),
        "Space" => "␣".into(),
        other => other.to_string(),
    }
}

/// Coloured status dot + line at the top of a Stream card.
fn status_dot(ui: &mut egui::Ui, ok: bool, text: &str) {
    ui.horizontal(|ui| {
        let (r, _) =
            ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().circle_filled(
            r.center(),
            3.5,
            if ok { crate::ui_theme::GOOD } else { crate::ui_theme::FAINT },
        );
        ui.label(
            egui::RichText::new(text)
                .size(11.0)
                .color(crate::ui_theme::MUTED),
        );
    });
}

fn stream_tab(ui: &mut egui::Ui, s: &mut Settings, st: &Status, cmd: &mut Vec<UiCommand>) {
    use crate::nowplaying::NpSource;
    use crate::ui_theme::*;
    // Two columns like the Dancer & FX page — input-ish cards on the left,
    // on-screen dressing on the right, output full width at the bottom.
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, "now playing");
            status_dot(
                ui,
                st.np_track.is_some(),
                &st.np_track.clone().unwrap_or_else(|| "no track".into()),
            );
        row(ui, "Source", |ui| {
            egui::ComboBox::from_id_salt("np_src")
                .width(130.0)
                .selected_text(s.np_source.label())
                .show_ui(ui, |ui| {
                    for src in NpSource::ALL {
                        ui.selectable_value(&mut s.np_source, src, src.label());
                    }
                });
        });
        ui.small(
            "Auto follows whichever source changed last: Spotify / Apple Music / any media \
             player, Serato, VirtualDJ, rekordbox, Mixxx, or a text file.",
        );
        if s.np_source == NpSource::File || s.np_source == NpSource::Auto {
            row(ui, "Text file", |ui| {
                ui.add(egui::TextEdit::singleline(&mut s.np_file).desired_width(150.0).hint_text("optional"));
                if ui.button("…").clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("text", &["txt"]).pick_file() {
                        s.np_file = p.display().to_string();
                    }
                }
            });
        }
        row(ui, "Delay", |ui| {
            ui.add(egui::Slider::new(&mut s.np_delay_s, 0.0..=60.0).step_by(1.0).suffix(" s"));
        });
        ui.small("A new track must stay loaded this long before it's shown — skips headphone cue-ups.");
        for (name, line) in &st.np_status {
            ui.small(format!("{name}: {line}"));
        }
        ui.small(format!(
            "OBS: add a Text source reading {}",
            crate::config::data_dir().join("nowplaying.txt").display()
        ));
        row(ui, "Card", |ui| {
            ui.checkbox(&mut s.np_card, "Show on screen");
            if ui.button("Show again").clicked() {
                cmd.push(UiCommand::Do(Action::ShowNowPlaying));
            }
        });
        row(ui, "Card time", |ui| {
            ui.add(egui::Slider::new(&mut s.np_hold_s, 0.0..=60.0).step_by(1.0).suffix(" s"));
            ui.small("0 = always");
        });
        row(ui, "Card size", |ui| {
            ui.add(egui::Slider::new(&mut s.np_size, 0.5..=2.0));
        });

        });
        ui.add_space(6.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, "recording");
            status_dot(
                ui,
                st.rec.is_some() && st.rec_err.is_none(),
                &match (&st.rec_err, &st.rec) {
                    (Some(e), _) => format!("{e}"),
                    (None, Some(r)) => format!("{} · {} s buffered", r.encoder, r.buffered_s),
                    (None, None) => "recorder off".to_string(),
                },
            );
        row(ui, "Replay", |ui| {
            ui.checkbox(&mut s.rec_buffer, "Keep the last");
            ui.add(egui::DragValue::new(&mut s.rec_keep_s).range(10..=600).suffix(" s"));
        });
        row(ui, "Format", |ui| {
            for l in crate::rec::Layout::ALL {
                ui.selectable_value(&mut s.rec_layout, l, l.label());
            }
        });
        row(ui, "", |ui| {
            let can = st.rec.is_some();
            if ui.add_enabled(can, egui::Button::new("💾 Save clip")).on_hover_text("Hotkey K").clicked() {
                cmd.push(UiCommand::Do(Action::SaveClip));
            }
            let rolling = st.rec.as_ref().and_then(|r| r.set_since);
            let label = match rolling {
                Some(t) => {
                    let e = t.elapsed().as_secs();
                    format!("⏹ Stop set ({}:{:02}:{:02})", e / 3600, e / 60 % 60, e % 60)
                }
                None => "⏺ Record set".into(),
            };
            if ui.button(label).on_hover_text("Hotkey J — records until you stop it").clicked() {
                cmd.push(UiCommand::Do(Action::RecordSet));
            }
        });
        if let Some(e) = &st.rec_err {
            ui.colored_label(egui::Color32::from_rgb(255, 160, 60), e);
        }
        if let Some(r) = &st.rec {
            let mut line = format!("{} · {} s buffered", r.encoder, r.buffered_s);
            if r.saving {
                line.push_str(" · saving…");
            }
            ui.small(line);
            if let Some(e) = &r.err {
                ui.colored_label(egui::Color32::from_rgb(255, 120, 120), e);
            } else if let Some(p) = &r.last {
                ui.small(format!("Saved {p}"));
            }
        }
        row(ui, "Folder", |ui| {
            ui.add(egui::TextEdit::singleline(&mut s.rec_dir).desired_width(150.0).hint_text("Videos/Trippin"));
            if ui.button("…").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    s.rec_dir = p.display().to_string();
                }
            }
        });
        row(ui, "ffmpeg", |ui| {
            ui.add(egui::TextEdit::singleline(&mut s.ffmpeg_path).desired_width(150.0).hint_text("auto"));
            if ui.button("…").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_file() {
                    s.ffmpeg_path = p.display().to_string();
                }
            }
        });
        ui.small("Clips include the overlays and the audio. Size/fps follow the video output settings below.");

        });
        let ui = &mut cols[1];
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, "branding");
            status_dot(ui, s.brand_on, if s.brand_on { "showing" } else { "off" });
        ui.checkbox(&mut s.brand_on, "Show logo / name");
        row(ui, "DJ name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut s.brand_name).desired_width(180.0));
        });
        row(ui, "Handles", |ui| {
            ui.add(egui::TextEdit::singleline(&mut s.brand_handles).desired_width(180.0).hint_text("@you · twitch.tv/you"));
        });
        row(ui, "Logo", |ui| {
            ui.add(egui::TextEdit::singleline(&mut s.brand_logo).desired_width(150.0).hint_text("PNG, optional"));
            if ui.button("…").clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("PNG", &["png"]).pick_file() {
                    s.brand_logo = p.display().to_string();
                }
            }
        });
        row(ui, "Corner", |ui| {
            for (i, l) in ["↖", "↗", "↙", "↘"].iter().enumerate() {
                ui.selectable_value(&mut s.brand_corner, i as u8, *l);
            }
        });
        row(ui, "Size", |ui| {
            ui.add(egui::Slider::new(&mut s.brand_size, 0.5..=2.0));
        });
        row(ui, "Opacity", |ui| {
            ui.add(egui::Slider::new(&mut s.brand_opacity, 0.1..=1.0));
        });
        row(ui, "Accent", |ui| {
            let mut c = crate::overlay::parse_hex(&s.brand_color, [0.25, 0.85, 1.0]);
            if ui.color_edit_button_rgb(&mut c).changed() {
                s.brand_color = format!(
                    "#{:02x}{:02x}{:02x}",
                    (c[0] * 255.0) as u8,
                    (c[1] * 255.0) as u8,
                    (c[2] * 255.0) as u8
                );
            }
        });

        });
        ui.add_space(6.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, "ticker");
            status_dot(ui, s.ticker_on, if s.ticker_on { "scrolling" } else { "off" });
        ui.checkbox(&mut s.ticker_on, "Scroll a message along the bottom");
        ui.add(
            egui::TextEdit::multiline(&mut s.ticker_text)
                .desired_rows(2)
                .desired_width(280.0)
                .hint_text("Requests in chat · follow for the next set"),
        );
        row(ui, "Speed", |ui| {
            ui.add(egui::Slider::new(&mut s.ticker_speed, 0.3..=3.0));
        });

        });
    });
    ui.add_space(6.0);
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section_label(ui, "video output");
        status_dot(
            ui,
            st.output.is_some(),
            st.output.as_deref().unwrap_or("off — enable Spout or NDI"),
        );
    row(ui, "Name", |ui| {
        ui.add(egui::TextEdit::singleline(&mut s.ndi_name).desired_width(140.0));
    });
    row(ui, "Send", |ui| {
        if cfg!(windows) {
            ui.checkbox(&mut s.spout_enabled, "Spout")
                .on_hover_text("Same PC: OBS → Add source → Spout2 Capture (needs the free Spout2 OBS plugin).");
        }
        ui.checkbox(&mut s.ndi_enabled, "NDI")
            .on_hover_text("Over the network: OBS → NDI Source (needs the free NDI runtime / DistroAV plugin).");
    });
    row(ui, "Background", |ui| {
        ui.selectable_value(&mut s.out_transparent, false, "Scenes");
        ui.selectable_value(&mut s.out_transparent, true, "Transparent");
    });
    if s.out_transparent {
        ui.small("Scenes off: only the dancer, glow, overlays and text go out, with alpha — layer them over your camera in OBS.");
    }
    row(ui, "Size", |ui| {
        for h in [720u32, 1080, 2160] {
            ui.selectable_value(&mut s.ndi_height, h, format!("{h}p"));
        }
        ui.label("at");
        for f in [30u32, 60] {
            ui.selectable_value(&mut s.ndi_fps, f, format!("{f} fps"));
        }
    });
    if st.output.is_none() {
        ui.small(
            "Sends the finished frame (overlays included) to OBS or another display. \
             Spout for OBS on this PC, NDI across the network.",
        );
    }
    });
}


/// Dancer & FX page (mockup 1a styling): two control cards side by side,
/// then the routine thumbnail library underneath.
fn dancer_fx_tab(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    clips: &[String],
    clip_thumbs: &mut HashMap<String, Option<egui::TextureHandle>>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    ui.columns(2, |cols| {
        card().show(&mut cols[0], |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, "dancer");
            ui.checkbox(&mut s.dancer_enabled, "Dancer layer on");
            ctl_row(ui, "Look", |ui| {
                let mut opts: Vec<(Option<usize>, &str)> = vec![(None, "auto")];
                for (i, name) in STYLES.iter().enumerate() {
                    opts.push((Some(i), *name));
                }
                segmented(ui, &mut s.dancer_style, &opts);
            });
            ctl_row(ui, "Canon", |ui| {
                segmented(
                    ui,
                    &mut s.canon,
                    &[
                        (Tristate::Auto, "auto"),
                        (Tristate::On, "on"),
                        (Tristate::Off, "off"),
                    ],
                );
            });
            ui.small("Canon: three dancers");
            ctl_row(ui, "Size", |ui| {
                ui.add(egui::Slider::new(&mut s.dancer_size, 0.4..=1.0));
            });
            ui.checkbox(&mut s.dancer_trails, "Motion trails");
            ui.small("Ghost echoes trail the dancer's movement.");
        });
        card().show(&mut cols[1], |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, "fx");
            ui.small("Whole-frame transforms — live, so your pick is the preview.");
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for f in Fx::ALL {
                    let on = s.fx == f && !s.fx_auto;
                    if ui.selectable_label(on, f.label()).clicked() {
                        s.fx = f;
                        s.fx_auto = false;
                    }
                }
            });
            ctl_row(ui, "Strength", |ui| {
                ui.add_enabled(
                    s.fx != Fx::Off,
                    egui::Slider::new(&mut s.fx_amt, 0.0..=1.0),
                );
            });
            ui.checkbox(&mut s.fx_auto, "Auto — a fresh effect on every scene cut");
            if s.fx_auto {
                ui.small(format!("On screen now: {}", st.fx.label()));
            } else if s.fx == Fx::MirrorY {
                ui.small("Mirror Y flips top-to-bottom — dancers end up upside-down; auto never picks it.");
            }
        });
    });
    ui.add_space(6.0);

    // Routine library: thumbnail tiles, tick to keep in rotation, click to
    // preview live.
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section_label(ui, "routines");
        ui.small("Auto-pilot picks among the ticked routines — click a tile to preview it live.");
        ui.add_space(4.0);
        let avail = ui.available_width();
        let cols = ((avail + 8.0) / 108.0).floor().max(3.0) as usize;
        for (row, chunk) in clips.chunks(cols).enumerate() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                for (ci, name) in chunk.iter().enumerate() {
                    clip_tile(ui, s, st, row * cols + ci, name, clip_thumbs, cmd);
                }
            });
        }
    });
}

/// One routine tile in the Dancer & FX library.
fn clip_tile(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    i: usize,
    name: &str,
    clip_thumbs: &mut HashMap<String, Option<egui::TextureHandle>>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    const TW: f32 = 100.0;
    const TH: f32 = 116.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(TW, TH), egui::Sense::click());
    let p = ui.painter();
    let live = st.clip.as_deref() == Some(name);
    let on = !s.disabled_clips.iter().any(|d| d == name);
    let dim = if on { 1.0 } else { 0.45 };

    p.rect_filled(rect, 6.0, CARD);
    let img_r = egui::Rect::from_min_size(
        rect.min + egui::vec2(5.0, 5.0),
        egui::vec2(TW - 10.0, 84.0),
    );
    match clip_tex(ui, clip_thumbs, name) {
        Some((tex, sz)) => {
            // Letterbox: clips are portrait, the box isn't.
            let sc = (img_r.width() / sz.x).min(img_r.height() / sz.y);
            let fit = egui::vec2(sz.x * sc, sz.y * sc);
            let fr = egui::Rect::from_center_size(img_r.center(), fit);
            p.rect_filled(img_r, 4.0, INSET);
            p.image(
                tex,
                fr,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE.gamma_multiply(dim),
            );
        }
        None => {
            p.rect_filled(img_r, 4.0, RAISED);
            p.text(
                img_r.center(),
                egui::Align2::CENTER_CENTER,
                name.get(..2).unwrap_or(name),
                egui::FontId::monospace(13.0),
                FAINT,
            );
        }
    }
    p.rect_stroke(
        img_r,
        4.0,
        egui::Stroke::new(1.0, BORDER),
        egui::StrokeKind::Inside,
    );

    // Rotation checkbox top-right (registered last so it wins its clicks).
    let cb_r = egui::Rect::from_min_size(
        egui::pos2(img_r.max.x - 18.0, img_r.min.y + 3.0),
        egui::vec2(15.0, 15.0),
    );
    let cb = ui.interact(cb_r, resp.id.with("rot"), egui::Sense::click());
    let cb_clicked = cb.clicked();
    p.rect_filled(cb_r, 3.0, if on { LANE_DANCER } else { INSET.gamma_multiply(0.9) });
    p.rect_stroke(cb_r, 3.0, egui::Stroke::new(1.0, BORDER_HI), egui::StrokeKind::Inside);
    if on {
        p.text(
            cb_r.center(),
            egui::Align2::CENTER_CENTER,
            "✓",
            egui::FontId::proportional(10.0),
            TEXT,
        );
    }
    if cb_clicked {
        if on {
            s.disabled_clips.push(name.to_string());
        } else {
            s.disabled_clips.retain(|d| d != name);
        }
    }

    let shown = if name.len() > 13 {
        format!("{}…", &name[..12])
    } else {
        name.to_string()
    };
    p.text(
        egui::pos2(rect.min.x + 6.0, img_r.max.y + 5.0),
        egui::Align2::LEFT_TOP,
        shown,
        egui::FontId::monospace(10.0),
        if live { LANE_DANCER } else { TEXT.gamma_multiply(dim) },
    );
    p.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(
            if live { 1.5 } else { 1.0 },
            if live { LANE_DANCER } else { BORDER },
        ),
        egui::StrokeKind::Inside,
    );
    if resp.clicked() && !cb_clicked {
        cmd.push(UiCommand::ShowClip(i));
    }
    resp.on_hover_text(name);
}

/// Mid-frame texture for a dancer clip, decoded once and cached.
fn clip_tex(
    ui: &egui::Ui,
    clip_thumbs: &mut HashMap<String, Option<egui::TextureHandle>>,
    name: &str,
) -> Option<(egui::TextureId, egui::Vec2)> {
    match clip_thumbs.entry(name.to_string()).or_insert_with(|| {
        crate::editor::load_clip_thumb(ui.ctx(), name)
    }) {
        Some(t) => Some((t.id(), t.size_vec2())),
        None => None,
    }
}

/// Keys page (mockup 1a card): filter, key badges, rebind, conflict warn.
fn keys_tab(
    ui: &mut egui::Ui,
    s: &mut Settings,
    rebinding: &mut Option<Action>,
    filter: &mut String,
) {
    use crate::ui_theme::*;
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(filter)
                .desired_width(160.0)
                .hint_text("filter actions"),
        );
        ui.label(
            egui::RichText::new("keys work in both windows — rebind, then press the key")
                .size(11.0)
                .color(FAINT),
        );
    });
    // A key bound to two actions fires both — flag it.
    let mut bound: HashMap<&str, u32> = HashMap::new();
    for k in s.keys.values() {
        if !k.is_empty() {
            *bound.entry(k.as_str()).or_default() += 1;
        }
    }
    let q = filter.to_lowercase();
    let acts: Vec<Action> = Action::ALL
        .iter()
        .copied()
        .filter(|a| q.is_empty() || a.label().to_lowercase().contains(&q))
        .collect();
    // Two cards side by side — the window is wide, a single list wastes it.
    let mid = acts.len().div_ceil(2);
    ui.columns(2, |cols| {
        for (ci, col) in cols.iter_mut().enumerate() {
            card().show(col, |ui| {
                ui.set_width(ui.available_width());
                let lbl_w = (ui.available_width() - 158.0).max(110.0);
                for &a in acts.iter().skip(ci * mid).take(mid) {
                    let key = s.keys.get(&a).cloned().unwrap_or_default();
                    let conflict = bound.get(key.as_str()).copied().unwrap_or(0) > 1;
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [lbl_w, 20.0],
                            egui::Label::new(
                                egui::RichText::new(a.label()).size(12.0),
                            )
                            .truncate(),
                        );
                        egui::Frame::NONE
                            .fill(INSET)
                            .stroke(egui::Stroke::new(
                                1.0,
                                if conflict { WARN } else { BORDER_HI },
                            ))
                            .corner_radius(egui::CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(6, 2))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(if key.is_empty() {
                                        "—"
                                    } else {
                                        &key
                                    })
                                    .monospace()
                                    .size(11.0)
                                    .color(if conflict { WARN } else { MUTED }),
                                );
                            });
                        if *rebinding == Some(a) {
                            ui.colored_label(WARN, "press a key…");
                            if ui.small_button("cancel").clicked() {
                                *rebinding = None;
                            }
                        } else if ui.small_button("rebind").clicked() {
                            *rebinding = Some(a);
                        }
                        if conflict {
                            ui.label(
                                egui::RichText::new("conflict").size(10.0).color(WARN),
                            );
                        }
                    });
                }
            });
        }
    });
    ui.add_space(4.0);
    if ui.button("Reset all keys to defaults").clicked() {
        s.keys = Settings::default().keys;
    }
}

// ---------------------------------------------------------------------------
// Timeline tab — load a track, drop cues on its beat grid, play or follow.
// ---------------------------------------------------------------------------

pub(crate) fn fmt_time(t: f64) -> String {
    let t = t.max(0.0);
    format!("{}:{:04.1}", (t / 60.0) as u64, t % 60.0)
}

pub(crate) fn cue_color(k: &CueKind) -> egui::Color32 {
    use crate::ui_theme as t;
    // Colour = the lane the block lives on, so the strip reads at a glance.
    match k {
        CueKind::Scene(_) | CueKind::NextScene | CueKind::PrevScene => t::LANE_SCENE,
        CueKind::Fx(_) | CueKind::FxAuto(_) => t::LANE_FX,
        CueKind::Dancer(_)
        | CueKind::Clip(_)
        | CueKind::NextClip
        | CueKind::NextLook
        | CueKind::Look(_)
        | CueKind::Trails(_)
        | CueKind::Canon(_) => t::LANE_DANCER,
        CueKind::Blackout(_) => t::DANGER,
        CueKind::Mode(_) | CueKind::Palette(_) => t::LANE_SHOW,
        CueKind::Text(_) | CueKind::TextOff(_) => t::LANE_TEXT,
    }
}

/// Pick a string from `opts` — returns true when the value changed.
fn pick_str(ui: &mut egui::Ui, id: egui::Id, cur: &mut String, opts: &[String]) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .width(110.0)
        .selected_text(if cur.is_empty() {
            "pick…"
        } else {
            cur.as_str()
        })
        .show_ui(ui, |ui| {
            for o in opts {
                changed |= ui.selectable_value(cur, o.clone(), o).changed();
            }
        });
    changed
}

/// Param editors for a cue kind. Returns true when it changed.
pub(crate) fn cue_param_ui(
    ui: &mut egui::Ui,
    kind: &mut CueKind,
    scenes: &[String],
    clips: &[String],
    id: egui::Id,
) -> bool {
    match kind {
        CueKind::Scene(n) => pick_str(ui, id.with("sc"), n, scenes),
        CueKind::Clip(n) => pick_str(ui, id.with("cl"), n, clips),
        CueKind::Mode(m) => {
            ui.selectable_value(m, Mode::Auto, "auto").changed()
                | ui.selectable_value(m, Mode::Static, "static").changed()
                | ui.selectable_value(m, Mode::Manual, "manual").changed()
        }
        CueKind::Dancer(b) | CueKind::Blackout(b) | CueKind::FxAuto(b) => {
            ui.checkbox(b, "on").changed()
        }
        CueKind::Look(l) => {
            let mut changed = false;
            egui::ComboBox::from_id_salt(id.with("lk"))
                .width(90.0)
                .selected_text((*l).map(|i| STYLES[i]).unwrap_or("auto"))
                .show_ui(ui, |ui| {
                    changed |= ui.selectable_value(l, None, "auto").changed();
                    for (i, name) in STYLES.iter().enumerate() {
                        changed |= ui.selectable_value(l, Some(i), *name).changed();
                    }
                });
            changed
        }
        CueKind::Canon(t) => {
            ui.selectable_value(t, Tristate::Auto, "auto").changed()
                | ui.selectable_value(t, Tristate::On, "on").changed()
                | ui.selectable_value(t, Tristate::Off, "off").changed()
        }
        CueKind::Fx(f) => {
            let mut changed = false;
            egui::ComboBox::from_id_salt(id.with("fx"))
                .width(100.0)
                .selected_text(f.label())
                .show_ui(ui, |ui| {
                    for v in Fx::ALL {
                        changed |= ui.selectable_value(f, v, v.label()).changed();
                    }
                });
            changed
        }
        CueKind::Palette(n) => {
            let opts: Vec<String> = crate::palettes::names().map(String::from).collect();
            pick_str(ui, id.with("pal"), n, &opts)
        }
        CueKind::Text(spec) => {
            use crate::text::{TextAnim, TextPos, TextStyle};
            let mut changed = ui
                .add(
                    egui::TextEdit::singleline(&mut spec.text)
                        .desired_width(140.0)
                        .hint_text("text…"),
                )
                .changed();
            egui::ComboBox::from_id_salt(id.with("ts"))
                .width(80.0)
                .selected_text(spec.style.label())
                .show_ui(ui, |ui| {
                    for v in TextStyle::ALL {
                        changed |= ui.selectable_value(&mut spec.style, v, v.label()).changed();
                    }
                });
            egui::ComboBox::from_id_salt(id.with("tp"))
                .width(70.0)
                .selected_text(spec.pos.label())
                .show_ui(ui, |ui| {
                    for v in TextPos::ALL {
                        changed |= ui.selectable_value(&mut spec.pos, v, v.label()).changed();
                    }
                });
            egui::ComboBox::from_id_salt(id.with("ta"))
                .width(70.0)
                .selected_text(spec.anim.label())
                .show_ui(ui, |ui| {
                    for v in TextAnim::ALL {
                        changed |= ui.selectable_value(&mut spec.anim, v, v.label()).changed();
                    }
                });
            changed |= ui.selectable_value(&mut spec.lane, 0, "lane 1").changed();
            changed |= ui.selectable_value(&mut spec.lane, 1, "lane 2").changed();
            changed
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn timeline_tab(ui: &mut egui::Ui, tl_shared: &crate::timeline::Shared, cmd: &mut Vec<UiCommand>) {
    use crate::ui_theme as t;
    let mut guard = tl_shared.lock().unwrap_or_else(|e| e.into_inner());
    let crate::timeline::TimelineState {
        doc: doc_opt,
        mode,
        pos_s,
        recording,
        autosync,
        live_locked,
        live_score,
        dirty,
        message,
        busy,
        ..
    } = &mut *guard;

    // --- Timeline card: doc summary + transport -------------------------------
    t::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            t::section_label(ui, "timeline");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("open editor").clicked() {
                    cmd.push(UiCommand::OpenEditor);
                }
            });
        });
        ui.add_space(6.0);

        if let Some(doc) = doc_opt.as_mut() {
            ui.label(
                egui::RichText::new(&doc.name)
                    .monospace()
                    .size(15.0)
                    .color(t::TEXT),
            );
            ui.label(
                egui::RichText::new(format!(
                    "{} song{} · {} · {} cues{}",
                    doc.clips.len(),
                    if doc.clips.len() == 1 { "" } else { "s" },
                    fmt_time(doc.end_s()),
                    doc.cues.len(),
                    if *dirty { " · unsaved" } else { "" }
                ))
                .size(11.0)
                .color(t::MUTED),
            );
        }

        // Transport — same round-button language as the editor's bar.
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let playing = *mode == PlayMode::Playing;
            if ui
                .add_enabled(
                    doc_opt.is_some() && !*busy,
                    egui::Button::new(egui::RichText::new(if playing { "⏸" } else { "▶" }).size(13.0))
                        .min_size(egui::vec2(30.0, 26.0))
                        .corner_radius(egui::CornerRadius::same(13)),
                )
                .clicked()
            {
                cmd.push(UiCommand::Song(SongCtl::Toggle));
            }
            if ui
                .add_enabled(
                    *mode != PlayMode::Stopped,
                    egui::Button::new(egui::RichText::new("⏹").size(11.0))
                        .min_size(egui::vec2(30.0, 26.0))
                        .corner_radius(egui::CornerRadius::same(13)),
                )
                .clicked()
            {
                cmd.push(UiCommand::Song(SongCtl::Stop));
            }
            ui.label(
                egui::RichText::new(format!("{} {}", fmt_time(*pos_s), mode_str(*mode)))
                    .monospace()
                    .size(13.0)
                    .color(t::TEXT),
            );
            if *recording {
                ui.label(egui::RichText::new("● REC").size(11.0).color(t::DANGER));
            }
            if *autosync {
                ui.label(
                    egui::RichText::new(if *live_locked {
                        "live: LOCKED"
                    } else {
                        "live: listening…"
                    })
                    .size(11.0)
                    .color(if *live_locked { t::GOOD } else { t::MUTED }),
                );
                ui.label(
                    egui::RichText::new(format!("{:.2}", *live_score))
                        .monospace()
                        .size(10.0)
                        .color(t::FAINT),
                );
            }
        });
    });
    ui.add_space(8.0);

    // --- Songs card ------------------------------------------------------------
    if let Some(doc) = doc_opt.as_mut() {
        if !doc.clips.is_empty() {
            t::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                t::section_label(ui, "songs");
                for c in &doc.clips {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("@{}", fmt_time(c.offset_s)))
                                .monospace()
                                .size(11.0)
                                .color(t::FAINT),
                        );
                        ui.label(
                            egui::RichText::new(&c.name)
                                .monospace()
                                .size(12.0)
                                .color(t::TEXT),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{} · {:.1} BPM",
                                fmt_time(c.duration_s),
                                c.bpm
                            ))
                            .size(10.5)
                            .color(t::MUTED),
                        );
                    });
                }
            });
            ui.add_space(8.0);
        }
    }

    // --- Library card: name + save + open saved --------------------------------
    t::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        t::section_label(ui, "file");
        ui.horizontal(|ui| {
            if let Some(doc) = doc_opt.as_mut() {
                ui.add(
                    egui::TextEdit::singleline(&mut doc.name)
                        .desired_width(140.0)
                        .hint_text("timeline name"),
                );
                if ui.button("Save").clicked() {
                    cmd.push(UiCommand::SaveTimeline);
                }
            }
            let saved = Timeline::list(&crate::config::timelines_dir());
            if !saved.is_empty() {
                egui::ComboBox::from_id_salt("tl_open")
                    .selected_text("open saved…")
                    .show_ui(ui, |ui| {
                        for p in saved {
                            let stem = p
                                .file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            if ui.selectable_label(false, &stem).clicked() {
                                cmd.push(UiCommand::LoadTimeline(p.clone()));
                            }
                        }
                    });
            }
        });
        ui.label(
            egui::RichText::new(
                "add tracks and lay out cues in the editor — or drop an audio file / timeline .json on any window",
            )
            .size(10.5)
            .color(t::FAINT),
        );
        if *busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(egui::RichText::new("decoding — a few seconds").size(11.0).color(t::MUTED));
            });
        }
        if !message.is_empty() {
            ui.label(egui::RichText::new(message.as_str()).size(11.0).color(t::MUTED));
        }
    });
}

fn mode_str(m: PlayMode) -> &'static str {
    match m {
        PlayMode::Playing => "playing",
        PlayMode::Paused => "paused",
        PlayMode::Stopped => "stopped",
    }
}
