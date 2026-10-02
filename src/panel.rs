//! Control panel: a second window (egui) for modes, scene playlist, dancer
//! options, sync and hotkey bindings. It shares the renderer's GPU device.
//!
//! Layout (Docs/mockups.html): a top bar holding the page tabs and live
//! status (1b arrangement — the 1a console is styling reference only), then
//! one page at a time. Perform is the default: a thumbnail scene library
//! with a live inspector on the right (1b), or the Pads perform view (1c).
//! Dancer & FX, Stream, Timeline and Keys are the settings pages.

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

/// A saved-timeline row's metadata — parsed from the `.json` lazily (the
/// full doc carries per-beat analysis, too heavy to read every frame).
struct SavedMeta {
    name: String,
    song: String,
    length_s: f64,
    edited: String,
    mtime: Option<std::time::SystemTime>,
}

/// The saved-timelines list cache: rescan the directory every second, but
/// only re-parse files whose mtime moved.
#[derive(Default)]
struct SavedCache {
    rows: Vec<(PathBuf, SavedMeta)>,
    scanned: Option<Instant>,
}

impl SavedCache {
    fn refresh(&mut self) {
        let stale = self
            .scanned
            .is_none_or(|t| t.elapsed() > Duration::from_secs(1));
        if !stale {
            return;
        }
        self.scanned = Some(Instant::now());
        let paths = Timeline::list(&crate::config::timelines_dir());
        let mut rows: Vec<(PathBuf, SavedMeta)> = Vec::new();
        for p in &paths {
            let mtime = std::fs::metadata(p).and_then(|m| m.modified()).ok();
            if let Some((_, meta)) = self
                .rows
                .iter()
                .find(|(old, m)| old == p && m.mtime == mtime)
            {
                rows.push((p.clone(), SavedMeta {
                    name: meta.name.clone(),
                    song: meta.song.clone(),
                    length_s: meta.length_s,
                    edited: meta.edited.clone(),
                    mtime,
                }));
                continue;
            }
            rows.push((p.clone(), SavedMeta::parse(p, mtime)));
        }
        self.rows = rows;
    }
}

#[derive(serde::Deserialize)]
struct TlMetaClip {
    #[serde(default)]
    name: String,
    #[serde(default)]
    offset_s: f64,
    #[serde(default)]
    duration_s: f64,
}

#[derive(serde::Deserialize)]
struct TlMeta {
    #[serde(default)]
    name: String,
    #[serde(default)]
    clips: Vec<TlMetaClip>,
}

impl SavedMeta {
    fn parse(p: &PathBuf, mtime: Option<std::time::SystemTime>) -> Self {
        let meta = std::fs::read_to_string(p)
            .ok()
            .and_then(|j| serde_json::from_str::<TlMeta>(&j).ok());
        let (name, song, length_s) = match meta {
            Some(m) => {
                let end = m
                    .clips
                    .iter()
                    .map(|c| c.offset_s + c.duration_s)
                    .fold(0.0f64, f64::max);
                let song = match m.clips.len() {
                    0 => "—".to_string(),
                    1 => m.clips[0].name.clone(),
                    n => format!("{} songs", n),
                };
                let name = if m.name.is_empty() {
                    p.file_stem().unwrap_or_default().to_string_lossy().to_string()
                } else {
                    m.name
                };
                (name, song, end)
            }
            None => (
                p.file_stem().unwrap_or_default().to_string_lossy().to_string(),
                "—".to_string(),
                0.0,
            ),
        };
        let edited = mtime.map(ago_str).unwrap_or_default();
        SavedMeta {
            name,
            song,
            length_s,
            edited,
            mtime,
        }
    }
}

/// "5 m ago" / "3 h ago" / a date for anything older.
fn ago_str(t: std::time::SystemTime) -> String {
    let age = t.elapsed().unwrap_or_default().as_secs();
    if age < 90 {
        "just now".into()
    } else if age < 3600 {
        format!("{} m ago", age / 60)
    } else if age < 86_400 {
        format!("{} h ago", age / 3600)
    } else {
        format!("{} d ago", age / 86_400)
    }
}

/// Things the panel asks the app to do (beyond editing settings directly).
pub enum UiCommand {
    Do(Action),
    GoToScene(usize),
    /// Queue a scene to play at the next cut (right-click / shift-click).
    QueueNext(usize),
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
    /// Scene library + inspector — the old Show and Scenes tabs merged
    /// (mockup 1b).
    Perform,
    /// Dancer routines and whole-frame FX on one page.
    DancerFx,
    Stream,
    Timeline,
    Keys,
    /// App-wide preferences: audio source & sync, auto-pilot rules, and
    /// the AI show builder's provider/key.
    Settings,
}

/// Scene-library filter chip (mockup 1b toolbar).
#[derive(Clone, Copy, PartialEq)]
enum LibChip {
    All,
    Flat,
    Heavy,
    Seasonal,
    /// Starred scenes only (Settings.favourite_scenes).
    Fav,
}

impl Tab {
    const ALL: [Tab; 6] = [
        Tab::Perform,
        Tab::DancerFx,
        Tab::Stream,
        Tab::Timeline,
        Tab::Keys,
        Tab::Settings,
    ];

    fn label(self) -> &'static str {
        match self {
            Tab::Perform => "Perform",
            Tab::DancerFx => "Dancer & FX",
            Tab::Stream => "Stream",
            Tab::Timeline => "Timeline",
            Tab::Keys => "Keys",
            Tab::Settings => "Settings",
        }
    }
}

pub struct Panel {
    /// Clone of `win.window` — kept as a field so `p.window` keeps working.
    pub window: Arc<Window>,
    win: crate::egui_win::EguiWin,
    /// Waiting for a key press to bind to this action.
    pub rebinding: Option<Action>,
    /// Waiting for a MIDI pad press (note-on) to bind to this action —
    /// captured globally by `App::midi_note`, whichever window is focused.
    pub midi_learn: Option<Action>,
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
    /// Saved-timeline metadata for the Timeline tab's list — parsed lazily
    /// and refreshed only when the file list or mtimes change.
    saved: SavedCache,
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
            midi_learn: None,
            tab: Tab::Perform,
            scene_filter: String::new(),
            chip: LibChip::All,
            thumbs: HashMap::new(),
            want_thumbs: HashMap::new(),
            clip_thumbs: HashMap::new(),
            keys_filter: String::new(),
            saved: SavedCache::default(),
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
        midi_status: &(bool, String),
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
        let midi_learn = &mut self.midi_learn;
        let tab = &mut self.tab;
        let scene_filter = &mut self.scene_filter;
        let chip = &mut self.chip;
        let thumbs = &mut self.thumbs;
        let want_thumbs = &mut self.want_thumbs;
        let clip_thumbs = &mut self.clip_thumbs;
        let keys_filter = &mut self.keys_filter;
        let saved = &mut self.saved;
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
                midi_learn,
                midi_status,
                tab,
                scene_filter,
                chip,
                thumbs,
                want_thumbs,
                clip_thumbs,
                keys_filter,
                saved,
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
    midi_learn: &mut Option<Action>,
    midi_status: &(bool, String),
    tab: &mut Tab,
    scene_filter: &mut String,
    chip: &mut LibChip,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want_thumbs: &mut HashMap<String, Instant>,
    clip_thumbs: &mut HashMap<String, Option<egui::TextureHandle>>,
    keys_filter: &mut String,
    saved: &mut SavedCache,
    cmd: &mut Vec<UiCommand>,
) -> bool {
    let before = serde_json::to_string(s).unwrap_or_default();

    // --- Top bar: tabs on the left, live status on the right (1b) --------
    // The beat pips animate, so repaint every frame while we're up.
    ui.ctx().request_repaint();
    egui::Panel::top("topbar")
        .frame(
            egui::Frame::NONE
                .fill(crate::ui_theme::PANEL)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .stroke(egui::Stroke::new(1.0, crate::ui_theme::BORDER)),
        )
        .show(ui, |ui| topbar(ui, tab, s, st));

    // One outer margin for every page (review rule): nothing may touch the
    // window edge, so nothing can clip against it either.
    egui::Frame::NONE
        .inner_margin(egui::Margin {
            left: 12,
            right: 12,
            top: 6,
            bottom: 10,
        })
        .show(ui, |ui| match *tab {
            Tab::Perform => {
                if s.perform_pads {
                    pads_view(ui, s, st, scenes, thumbs, want_thumbs, cmd);
                } else {
                    // Inspector declared before the central grid (top → right →
                    // central) so egui shrinks the library correctly — no width
                    // fudges needed downstream.
                    egui::Panel::right("inspector")
                        .resizable(false)
                        .exact_size(292.0)
                        .frame(
                            egui::Frame::NONE
                                .fill(crate::ui_theme::PANEL)
                                .inner_margin(egui::Margin::symmetric(12, 10))
                                .stroke(egui::Stroke::new(1.0, crate::ui_theme::BORDER)),
                        )
                        .show(ui, |ui| {
                            inspector(ui, s, st, scenes, tab, thumbs, want_thumbs, cmd);
                        });
                    perform_tab(
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
                    );
                }
            }
            _ => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, true])
                    .horizontal_scroll_offset(0.0)
                    .show(ui, |ui| match *tab {
                        Tab::DancerFx => {
                            dancer_fx_tab(ui, s, st, clips, clip_thumbs, cmd)
                        }
                        Tab::Stream => stream_tab(ui, s, st, cmd),
                        Tab::Timeline => timeline_tab(ui, tl_shared, saved, cmd),
                        Tab::Keys => keys_tab(ui, s, rebinding, midi_learn, midi_status, keys_filter),
                        Tab::Settings => settings_tab(ui, s, st, cmd),
                        Tab::Perform => unreachable!(),
                    });
            }
        });

    serde_json::to_string(s).unwrap_or_default() != before
}

/// The top bar (mockup 1b): page tabs on the left — with the Perform
/// Library/Pads toggle beside them when Perform is active — and live
/// status on the right: fps · BPM · beat pips · pills.
fn topbar(ui: &mut egui::Ui, tab: &mut Tab, s: &mut Settings, st: &Status) {
    use crate::ui_theme::*;
    ui.horizontal(|ui| {
        for t in Tab::ALL {
            let sel = *tab == t;
            let b = egui::Button::new(
                egui::RichText::new(t.label())
                    .size(12.5)
                    .color(if sel { TEXT } else { MUTED }),
            )
            .fill(if sel { ACCENT_SEL } else { egui::Color32::TRANSPARENT })
            .stroke(if sel {
                egui::Stroke::new(1.0, ACCENT)
            } else {
                egui::Stroke::NONE
            })
            .corner_radius(egui::CornerRadius::same(5));
            if ui.add(b).clicked() {
                *tab = t;
            }
        }
        // Budget the right cluster against what the tabs left over — items
        // that don't fit are dropped (fps first, then pills) instead of
        // clipping the tab strip (A1).
        let room = ui.available_width();
        let pads_on = *tab == Tab::Perform && s.perform_pads;
        let mut need = 0.0;
        if *tab == Tab::Perform {
            need += 126.0; // Library/Pads segmented
        }
        if !pads_on {
            need += 118.0; // BPM + beat pips
        }
        let show_fps = room > need + 72.0;
        let mut pill_room = room - need - if show_fps { 72.0 } else { 0.0 };
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Rightmost first: the Library/Pads toggle is pinned to the
            // right edge so variable-width readouts (BPM digits, pills
            // appearing) can't push the buttons out from under the cursor.
            if *tab == Tab::Perform {
                segmented(ui, &mut s.perform_pads, &[(false, "Library"), (true, "Pads")]);
                ui.add_space(6.0);
            }
            // The Pads view has the big BPM readout — one readout per
            // screen, so the header's BPM + pips hide while Pads is up (B3).
            if !pads_on {
                // Four beat pips: the live one is accent.
                let (r, _) =
                    ui.allocate_exact_size(egui::vec2(42.0, 10.0), egui::Sense::hover());
                for i in 0..4u64 {
                    let c = egui::pos2(r.min.x + 5.0 + i as f32 * 11.0, r.center().y);
                    ui.painter().circle_filled(
                        c,
                        4.0,
                        if i == st.beat_in_bar % 4 { ACCENT } else { BORDER },
                    );
                }
                ui.label(
                    egui::RichText::new(format!("{:>5}", format!("{:.1}", st.bpm)))
                        .monospace()
                        .size(16.0)
                        .strong(),
                );
                ui.label(egui::RichText::new("BPM").size(10.0).color(FAINT));
            }
            if show_fps {
                ui.label(
                    egui::RichText::new(format!("{:>3.0} fps", st.fps))
                        .color(MUTED)
                        .size(11.0),
                );
            }
            // Pills come last (cluster's left edge) — appearing/disappearing
            // extends into empty space instead of moving the readouts.
            if st.calm > 0.5 && pill_room > 96.0 {
                pill(ui, "breakdown", BREAKDOWN, BREAKDOWN_BG);
                pill_room -= 96.0;
            }
            if st.silent && pill_room > 88.0 {
                pill(ui, "no signal", WARN, WARN_BG);
            }
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

/// One row inside an `egui::Grid` form: muted left-aligned label, then the
/// control(s). Keeps every card's label column the same width (review A4).
fn grow(ui: &mut egui::Ui, label: &str, body: impl FnOnce(&mut egui::Ui)) {
    if label.is_empty() {
        ui.label("");
    } else {
        ui.label(
            egui::RichText::new(label)
                .size(12.0)
                .color(crate::ui_theme::MUTED),
        );
    }
    // The body is ONE cell: a horizontal child ui so multi-widget rows
    // (field + button) don't each claim their own grid column and push
    // the grid wider than the card (the 720px overflow, A1).
    ui.scope_builder(
        egui::UiBuilder::new().layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| body(ui),
    );
    ui.end_row();
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
                LibChip::Fav => s.favourite_scenes.iter().any(|f| f == n),
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
    // horizontal_wrapped: a non-wrapping row that overflows its allocation
    // re-expands the parent's cursor and the grid would slide back under
    // the inspector at narrow widths.
    ui.horizontal_wrapped(|ui| {
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
            (LibChip::Fav, "fav"),
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
        // Rotation rules live beside the filters they affect (mockup 1b).
        let rules = ui.add(
            egui::Button::new(egui::RichText::new("rules").size(11.5).color(MUTED))
                .fill(RAISED)
                .corner_radius(egui::CornerRadius::same(9)),
        );
        rules.clone().on_hover_text("Which scene sets may rotate in");
        egui::Popup::menu(&rules).show(|ui| {
            ui.set_min_width(230.0);
            ui.label(
                egui::RichText::new("rotation rules")
                    .size(10.0)
                    .color(crate::ui_theme::FAINT),
            );
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Seasonal").size(11.5).color(MUTED));
            segmented_wide(
                ui,
                &mut s.seasonal,
                &[
                    (Seasonal::Auto, "auto"),
                    (Seasonal::Always, "always"),
                    (Seasonal::Off, "off"),
                ],
            );
            ui.label(egui::RichText::new("3D (raymarched)").size(11.5).color(MUTED));
            segmented_wide(
                ui,
                &mut s.heavy_scenes,
                &[
                    (Tristate::Auto, "auto"),
                    (Tristate::On, "on"),
                    (Tristate::Off, "off"),
                ],
            );
            ui.label(egui::RichText::new("2D").size(11.5).color(MUTED));
            segmented_wide(ui, &mut s.flat_scenes, &[(true, "on"), (false, "off")]);
            ui.label(
                egui::RichText::new(if heavy_ok {
                    "GPU is fine with 3D scenes"
                } else {
                    "auto: 3D scenes are off — weak GPU"
                })
                .size(10.0)
                .color(crate::ui_theme::FAINT),
            );
        });
    });
    // Stats line: counts left, all on/off at the library's right edge.
    // allocate_ui_with_layout pins the right edge to the real content width
    // (a panel doesn't shrink the parent's max_rect — right_to_left on the
    // parent would anchor into the inspector's gutter).
    // Wrapped for the same reason as the toolbar — at narrow widths the
    // counts + buttons won't fit on one row, and overflowing would re-widen
    // the parent cursor under the inspector.
    ui.horizontal_wrapped(|ui| {
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
        let rem = (ui.available_width() - 6.0).max(40.0);
        ui.allocate_ui_with_layout(
            egui::vec2(rem, 16.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
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
            },
        );
    });
    ui.add_space(2.0);

    // Thumbnail grid — fluid: columns fill the available width, tiles
    // stretch to fit. The scrollbar appears when needed; the x-offset is
    // pinned because a stray horizontal offset has no bar to undo it.
    const GAP: f32 = 10.0;
    const MIN_TILE: f32 = 140.0;
    // Non-floating bar so it reserves width instead of overlaying the last
    // column; a small outer margin keeps it off the inspector edge. (Set on
    // the parent — inside the closure it's too late for the ScrollArea.)
    ui.spacing_mut().scroll.floating = false;
    ui.spacing_mut().scroll.bar_outer_margin = 4.0;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .horizontal_scroll_offset(0.0)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(GAP, GAP);
            let avail = ui.available_width();
            let cols = ((avail + GAP) / MIN_TILE).floor().max(2.0) as usize;
            let tile_w = (avail - GAP * (cols - 1) as f32) / cols as f32;
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
                            tile_w,
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

/// A 16:9 scene thumbnail (or striped placeholder) painted through a
/// rect-clipped painter so nothing bleeds past the frame. Shared by the
/// library tiles, inspector preview, and Pads now/next cards.
fn scene_thumb(
    ui: &egui::Ui,
    rect: egui::Rect,
    key: &str,
    dim: f32,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    let p = ui.painter_at(rect);
    if let Some(tex) = thumb_tex(ui, thumbs, want, key, cmd) {
        p.image(
            tex,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE.gamma_multiply(dim),
        );
    } else {
        // Diagonal stripes as a stand-in until the thumbnail lands.
        p.rect_filled(rect, 4.0, RAISED);
        let mut x = rect.min.x - rect.height();
        while x < rect.max.x {
            p.line_segment(
                [
                    egui::pos2(x, rect.max.y),
                    egui::pos2(x + rect.height(), rect.min.y),
                ],
                egui::Stroke::new(10.0, HOVER.gamma_multiply(0.6 * dim)),
            );
            x += 26.0;
        }
    }
    p.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0, BORDER),
        egui::StrokeKind::Inside,
    );
}

/// Truncate `text` to `max_w` with an ellipsis, measured with real
/// galleys and safe for multi-byte characters (never slices mid-char).
fn ellipsize(p: &egui::Painter, text: &str, font: &egui::FontId, max_w: f32) -> String {
    let measure = |s: String| {
        p.layout_no_wrap(s, font.clone(), egui::Color32::WHITE)
            .size()
            .x
    };
    if measure(text.to_string()) <= max_w {
        return text.to_string();
    }
    let mut out = String::new();
    for ch in text.chars() {
        if measure(format!("{out}{ch}…")) > max_w {
            break;
        }
        out.push(ch);
    }
    format!("{out}…")
}

/// Elide `text` from the middle — head and tail both survive, so a
/// meaningful suffix (e.g. `_mir`) isn't the part that gets lost.
fn elide_mid(p: &egui::Painter, text: &str, font: &egui::FontId, max_w: f32) -> String {
    let measure = |s: &str| {
        p.layout_no_wrap(s.to_string(), font.clone(), egui::Color32::WHITE)
            .size()
            .x
    };
    if measure(text) <= max_w {
        return text.to_string();
    }
    let mut head = String::new();
    for ch in text.chars() {
        if measure(&format!("{head}{ch}…")) > max_w {
            break;
        }
        head.push(ch);
    }
    let mut tail = String::new();
    for ch in text.chars().rev() {
        if measure(&format!("{head}…{ch}{tail}")) > max_w {
            break;
        }
        tail.insert(0, ch);
    }
    format!("{head}…{tail}")
}

/// Elide `text` from the left — keeps the tail (filename) visible, drops
/// leading directories behind a leading ellipsis.
fn elide_left(p: &egui::Painter, text: &str, font: &egui::FontId, max_w: f32) -> String {
    let measure = |s: &str| {
        p.layout_no_wrap(s.to_string(), font.clone(), egui::Color32::WHITE)
            .size()
            .x
    };
    if measure(text) <= max_w {
        return text.to_string();
    }
    let mut tail = String::new();
    for ch in text.chars().rev() {
        if measure(&format!("…{ch}{tail}")) > max_w {
            break;
        }
        tail.insert(0, ch);
    }
    format!("…{tail}")
}

/// Width of `text` in `font`, measured with a real galley.
fn text_w(p: &egui::Painter, text: &str, font: &egui::FontId) -> f32 {
    p.layout_no_wrap(text.to_string(), font.clone(), egui::Color32::WHITE)
        .size()
        .x
}

/// One scene-library tile: thumbnail, type tag, rotation checkbox, name.
/// Click cuts to the scene; right-click or shift-click queues it as the
/// next scene (mockup 1b).
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
    w: f32,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    let img_h = (w - 10.0) * 9.0 / 16.0;
    let h = 5.0 + img_h + 5.0 + 15.0 + 4.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    let p = ui.painter();
    let live = i == st.scene;
    let queued = st.next_scene == Some(i);
    let on = !s.disabled_scenes.iter().any(|d| d == name);
    let seasonal = in_season(name, date).is_some();
    // Can't join rotation at all — dimmed and click does nothing.
    let blocked = (is_heavy && !heavy_on) || (!is_heavy && !s.flat_scenes);
    let dim = if blocked || !on { 0.45 } else { 1.0 };

    p.rect_filled(rect, 6.0, CARD);
    let img_r = egui::Rect::from_min_size(
        rect.min + egui::vec2(5.0, 5.0),
        egui::vec2(w - 10.0, img_h),
    );
    let key = format!("scene:{name}");
    scene_thumb(ui, img_r, &key, dim, thumbs, want, cmd);

    // Type tag, top-left over the thumbnail. Plain ASCII-ish labels — the
    // bundled font has no ❄ glyph.
    let tag = if seasonal {
        "season"
    } else if is_heavy {
        "3D"
    } else {
        "2D"
    };
    let tag_font = egui::FontId::monospace(9.0);
    let tag_w = text_w(p, tag, &tag_font) + 8.0;
    let tag_r =
        egui::Rect::from_min_size(img_r.min + egui::vec2(4.0, 4.0), egui::vec2(tag_w, 13.0));
    p.rect_filled(tag_r, 3.0, INSET.gamma_multiply(0.85));
    p.text(
        tag_r.center(),
        egui::Align2::CENTER_CENTER,
        tag,
        tag_font,
        if seasonal { LANE_FX } else { MUTED },
    );

    // Favourite star, left of the checkbox — drawn, not a glyph (the
    // bundled font has no ★). Filled amber when starred.
    let fav = s.favourite_scenes.iter().any(|f| f == name);
    let star_r = egui::Rect::from_center_size(
        egui::pos2(img_r.max.x - 32.0, img_r.min.y + 12.0),
        egui::vec2(15.0, 15.0),
    );
    let star = ui.interact(star_r, resp.id.with("fav"), egui::Sense::click());
    let star_clicked = star.clicked();
    let pts: Vec<egui::Pos2> = (0..10)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            let r = if i % 2 == 0 { 6.5 } else { 2.9 };
            star_r.center() + egui::vec2(a.cos() * r, a.sin() * r)
        })
        .collect();
    if fav {
        p.add(egui::Shape::convex_polygon(pts, WARN, egui::Stroke::NONE));
    } else {
        p.add(egui::Shape::closed_line(
            pts,
            egui::Stroke::new(
                1.0,
                if star.hovered() { MUTED } else { FAINT.gamma_multiply(0.7) },
            ),
        ));
    }
    if star_clicked {
        if fav {
            s.favourite_scenes.retain(|f| f != name);
        } else {
            s.favourite_scenes.push(name.to_string());
        }
    }
    star.on_hover_text(if fav {
        "Starred — click to unfavourite"
    } else {
        "Favourite — shows under the fav filter"
    });

    // Rotation checkbox, top-right — register the interact after the tile so
    // it wins the click inside its rect.
    let cb_r = egui::Rect::from_min_size(
        egui::pos2(img_r.max.x - 20.0, img_r.min.y + 4.0),
        egui::vec2(16.0, 16.0),
    );
    let cb = ui.interact(cb_r, resp.id.with("rot"), egui::Sense::click());
    let cb_clicked = cb.clicked();
    paint_check(p, cb_r, on);
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

    // Name row under the thumbnail — ellipsized with real galley metrics.
    let name_font = egui::FontId::monospace(11.0);
    let live_w = if live || queued {
        text_w(p, "LIVE", &egui::FontId::proportional(9.0)) + 10.0
    } else {
        0.0
    };
    let shown = ellipsize(p, name, &name_font, w - 12.0 - live_w);
    p.text(
        egui::pos2(rect.min.x + 6.0, img_r.max.y + 9.0),
        egui::Align2::LEFT_TOP,
        shown,
        name_font,
        if live {
            ACCENT
        } else {
            TEXT.gamma_multiply(dim)
        },
    );
    if live {
        p.text(
            egui::pos2(rect.max.x - 6.0, img_r.max.y + 9.0),
            egui::Align2::RIGHT_TOP,
            "LIVE",
            egui::FontId::proportional(9.0),
            ACCENT,
        );
    } else if queued {
        p.text(
            egui::pos2(rect.max.x - 6.0, img_r.max.y + 9.0),
            egui::Align2::RIGHT_TOP,
            "NEXT",
            egui::FontId::proportional(9.0),
            BREAKDOWN,
        );
    }
    p.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(
            if live { 1.5 } else { 1.0 },
            if live {
                ACCENT
            } else if queued {
                BREAKDOWN
            } else {
                BORDER
            },
        ),
        egui::StrokeKind::Inside,
    );
    let queued_click =
        resp.secondary_clicked() || (resp.clicked() && ui.input(|i| i.modifiers.shift));
    if queued_click && !blocked {
        cmd.push(UiCommand::QueueNext(i));
    } else if resp.clicked() && !cb_clicked && !star_clicked && !blocked {
        cmd.push(UiCommand::GoToScene(i));
    }
    resp.on_hover_text(if blocked {
        format!("{name} — out of rotation (scene set off)")
    } else {
        format!("{name} — click to cut · right-click to play next")
    });
}

/// Mockup 1b right column: live preview, transport, summary rows that
/// navigate to the owning settings tab, and a collapsible director &amp;
/// sync section — Blackout / Fullscreen pinned to the foot.
#[allow(clippy::too_many_arguments)]
fn inspector(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    scenes: &[String],
    tab: &mut Tab,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    // Pinned foot: reserve the button row, scroll the rest above it.
    let foot_h = 38.0;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(ui.available_height() - foot_h)
        .horizontal_scroll_offset(0.0)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            ui.spacing_mut().scroll.floating = false;
            inspector_body(ui, s, st, scenes, tab, thumbs, want, cmd);
        });

    // Blackout + Fullscreen, always reachable (1b foot).
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let bw = ((ui.available_width() - ui.spacing().item_spacing.x) / 2.0).max(40.0);
        let bo = egui::Button::new(
            egui::RichText::new("Blackout").size(12.0).color(if st.blackout {
                DANGER
            } else {
                TEXT
            }),
        )
        .fill(if st.blackout { DANGER_BG } else { RAISED })
        .corner_radius(egui::CornerRadius::same(5));
        if ui.add_sized([bw, 30.0], bo).clicked() {
            cmd.push(UiCommand::Do(Action::Blackout));
        }
        let fs = egui::Button::new(egui::RichText::new("Fullscreen").size(12.0).color(TEXT))
            .fill(RAISED)
            .corner_radius(egui::CornerRadius::same(5));
        if ui.add_sized([bw, 30.0], fs).clicked() {
            cmd.push(UiCommand::Do(Action::Fullscreen));
        }
    });
}

/// Everything in the inspector above the pinned foot buttons.
#[allow(clippy::too_many_arguments)]
fn inspector_body(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    scenes: &[String],
    tab: &mut Tab,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    // Live preview with the accent frame + a "live output" caption baked
    // into its lower-left corner (mockup 1b).
    let name = scenes.get(st.scene).cloned().unwrap_or_default();
    let w = ui.available_width();
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(w, w * 9.0 / 16.0), egui::Sense::hover());
    let key = format!("scene:{name}");
    scene_thumb(ui, rect, &key, 1.0, thumbs, want, cmd);
    ui.painter().rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(1.5, ACCENT),
        egui::StrokeKind::Inside,
    );
    let p = ui.painter();
    let cap = "live output";
    let cap_font = egui::FontId::monospace(10.0);
    let cap_w = text_w(p, cap, &cap_font) + 14.0;
    let cap_r = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + 6.0, rect.max.y - 22.0),
        egui::pos2(rect.min.x + 6.0 + cap_w, rect.max.y - 6.0),
    );
    p.rect_filled(cap_r, 4.0, INSET.gamma_multiply(0.85));
    p.text(cap_r.center(), egui::Align2::CENTER_CENTER, cap, cap_font, MUTED);

    // Name left, "bar N of M" right — then the thin cut-progress bar.
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&name).monospace().size(16.0).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if st.bars_total > 0 {
                ui.label(
                    egui::RichText::new(format!("bar {} of {}", st.bar_in_scene, st.bars_total))
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
            egui::Rect::from_min_size(br.min, egui::vec2(br.width() * frac, br.height())),
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
            let raw = format!("next → {}", scenes.get(nx).map(String::as_str).unwrap_or("?"));
            let font = egui::FontId::monospace(11.0);
            let shown = ellipsize(ui.painter(), &raw, &font, ui.available_width() - 60.0);
            ui.label(egui::RichText::new(shown).size(11.0).color(ACCENT).monospace())
                .on_hover_text(raw);
        } else {
            ui.label(
                egui::RichText::new(bar_progress_line(st)).size(11.0).color(FAINT),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if st.calm > 0.5 {
                pill(ui, "breakdown", BREAKDOWN, BREAKDOWN_BG);
            }
        });
    });
    ui.add_space(4.0);

    // Prev / Next — Next carries the accent like the mockup.
    ui.horizontal(|ui| {
        let bw = ((ui.available_width() - ui.spacing().item_spacing.x) / 2.0).max(40.0);
        if ui
            .add_sized([bw, 28.0], egui::Button::new("< Prev"))
            .clicked()
        {
            cmd.push(UiCommand::Do(Action::PrevScene));
        }
        if ui
            .add_sized([bw, 28.0], egui::Button::new("Next >").fill(ACCENT_SEL))
            .clicked()
        {
            cmd.push(UiCommand::Do(Action::NextScene));
        }
    });

    ui.add_space(8.0);
    section_label(ui, "controls");
    card().show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(egui::RichText::new("Mode").size(12.0).color(MUTED));
        segmented_wide(
            ui,
            &mut s.mode,
            &[
                (Mode::Auto, "Auto"),
                (Mode::Static, "Static"),
                (Mode::Manual, "Manual"),
            ],
        );
        ui.label(egui::RichText::new("Bars").size(12.0).color(MUTED));
        segmented_wide(
            ui,
            &mut s.phrase_bars,
            &[(4u32, "4"), (8, "8"), (16, "16"), (32, "32")],
        );
        ctl_row(ui, "Dancer", |ui| {
            // Button first via right-to-left so a long clip name can't push
            // it past the panel edge; the value gets the leftover width.
            let bw = 44.0;
            let vw = (ui.available_width() - bw - 8.0).max(40.0);
            ui.allocate_ui_with_layout(
                egui::vec2(vw, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let look = s.dancer_style.map(|i| STYLES[i]).unwrap_or("auto");
                    let val = format!(
                        "{} · {look} · {:.0}%",
                        st.clip.as_deref().unwrap_or("off"),
                        st.confidence * 100.0
                    );
                    let shown = ellipsize(
                        ui.painter(),
                        &val,
                        &egui::FontId::monospace(12.0),
                        vw - 4.0,
                    );
                    ui.label(egui::RichText::new(shown).monospace().size(12.0))
                        .on_hover_text(format!("{val} — routine · look · beat confidence"));
                },
            );
            if ui.button("next").clicked() {
                cmd.push(UiCommand::Do(Action::NextClip));
            }
        });
        // Look / Effect / Palette summarise and navigate to the owning
        // settings page — the inspector isn't a second settings panel.
        nav_row(
            ui,
            "Look",
            s.dancer_style.map(|i| STYLES[i]).unwrap_or("auto").to_string(),
            || {
                *tab = Tab::DancerFx;
            },
        );
        nav_row(
            ui,
            "Effect",
            if s.fx_auto {
                format!("{} (auto)", s.fx.label())
            } else {
                s.fx.label().to_string()
            },
            || {
                *tab = Tab::DancerFx;
            },
        );
        // Palette row: a clickable gradient strip opening a swatch popover.
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Palette").size(12.0).color(MUTED));
            let w = ui.available_width();
            let (rect, resp) =
                ui.allocate_exact_size(egui::vec2(w, 18.0), egui::Sense::click());
            palette_fill(ui.painter_at(rect), rect, &s.palette);
            ui.painter().rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, BORDER_HI),
                egui::StrokeKind::Inside,
            );
            let popup = egui::Popup::menu(&resp);
            resp.on_hover_text(format!("{} — click to pick a palette", s.palette));
            popup.show(|ui| {
                ui.set_min_width(280.0);
                let names: Vec<_> = crate::palettes::names().collect();
                for chunk in names.chunks(2) {
                    ui.horizontal(|ui| {
                        for name in chunk {
                            palette_swatch(ui, s, name, 132.0, 20.0);
                        }
                    });
                }
            });
        });
    });

}

/// A summary row — label left, monospaced value right, chevron — that
/// navigates to the tab owning the setting.
fn nav_row(ui: &mut egui::Ui, label: &str, value: String, go: impl FnOnce()) {
    use crate::ui_theme::*;
    let h = 24.0;
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, HOVER.gamma_multiply(0.5));
    }
    let p = ui.painter();
    p.text(
        egui::pos2(rect.min.x + 2.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        MUTED,
    );
    let val_font = egui::FontId::monospace(12.0);
    let vw = text_w(p, &value, &val_font);
    p.text(
        egui::pos2(rect.max.x - 18.0 - vw, rect.center().y),
        egui::Align2::LEFT_CENTER,
        &value,
        val_font,
        TEXT,
    );
    p.text(
        egui::pos2(rect.max.x - 6.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        ">",
        egui::FontId::monospace(12.0),
        ACCENT,
    );
    if resp.clicked() {
        go();
    }
}

/// Paint a palette's gradient into `rect` through a clipped painter.
fn palette_fill(p: egui::Painter, rect: egui::Rect, palette: &str) {
    let lut = crate::palettes::lut(palette);
    let n = 32usize;
    for i in 0..n {
        let c = &lut[i * (crate::palettes::LUT_SIZE / n) * 4..];
        p.rect_filled(
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
    // Palette column on the right — fixed width; resizable panels draw a
    // grab pill on the border.
    egui::Panel::right("palettes")
        .resizable(false)
        .exact_size(174.0)
        .frame(
            egui::Frame::NONE
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .stroke(egui::Stroke::new(1.0, BORDER)),
        )
        .show(ui, |ui| palette_column(ui, s));

    // BPM at ~40px mono, four beat blocks stretched across the row, then
    // pills + "cut in N" at the right end (review B4 — no void). The
    // pills/blocks are painted inside a rect bounded by the shrunk cursor:
    // a nested right_to_left would re-anchor to the window's full max_rect
    // and overflow under the palette panel (the A1 cursor bug).
    let beat_right = ui.cursor().min.x + ui.available_width();
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(format!("{:.0}", st.bpm))
                .monospace()
                .size(40.0)
                .strong(),
        );
        ui.label(egui::RichText::new("BPM").size(11.0).color(FAINT));
        ui.add_space(10.0);

        let (r, _) = ui.allocate_exact_size(
            egui::vec2((beat_right - ui.cursor().min.x).max(40.0), 30.0),
            egui::Sense::hover(),
        );
        let p = ui.painter().clone();
        let cy = r.center().y;
        // Right-aligned cluster: "cut in N" text then pills, measured.
        let mut x = r.max.x;
        if st.bars_total > 0 {
            let rem = st.bars_total.saturating_sub(st.bar_in_scene);
            let txt = if rem == 0 {
                "cut now".to_string()
            } else {
                format!("cut in {rem} bar{}", if rem == 1 { "" } else { "s" })
            };
            let g = p.layout_no_wrap(txt, egui::FontId::monospace(11.0), MUTED);
            x -= g.size().x;
            p.galley(egui::pos2(x, cy - g.size().y * 0.5), g, MUTED);
            x -= 8.0;
        }
        for (txt, fg, bg) in [
            (st.calm > 0.5, "breakdown", BREAKDOWN, BREAKDOWN_BG),
            (st.silent, "no signal", WARN, WARN_BG),
        ]
        .into_iter()
        .filter_map(|(on, t, f, b)| on.then_some((t, f, b)))
        {
            let g = p.layout_no_wrap(txt.to_string(), egui::FontId::proportional(10.5), fg);
            let pw = g.size().x + 14.0;
            p.rect_filled(
                egui::Rect::from_min_max(egui::pos2(x - pw, r.min.y + 3.0), egui::pos2(x, r.max.y - 3.0)),
                10.0,
                bg,
            );
            p.galley(
                egui::pos2(x - pw + 7.0, cy - g.size().y * 0.5),
                g,
                fg,
            );
            x -= pw + 6.0;
        }
        // Beat blocks fill what remains.
        let bw = ((x - 6.0 - (r.min.x + 4.0)) - 3.0 * 8.0) / 4.0;
        if bw > 4.0 {
            for i in 0..4u64 {
                let c = egui::Rect::from_min_size(
                    egui::pos2(r.min.x + 4.0 + i as f32 * (bw + 8.0), r.min.y + 1.0),
                    egui::vec2(bw, 28.0),
                );
                p.rect_filled(c, 5.0, if i == st.beat_in_bar % 4 { ACCENT } else { BORDER });
            }
        }
    });
    ui.add_space(6.0);

    // NOW / UP NEXT — thumbnails fill their cards at 16:9, ~40% of the
    // content height so they read across the room (review B2). Width is
    // captured at parent scope — inside ui.horizontal the child's
    // max_rect is the window's, not the shrunk cursor's.
    let preview_h = (ui.available_height() * 0.40).max(120.0);
    let preview_w = (ui.available_width() - 10.0) / 2.0;
    ui.horizontal(|ui| {
        let w = preview_w;
        pad_preview(ui, thumbs, want, cmd, w, preview_h, "NOW", scenes.get(st.scene), {
            if st.bars_total > 0 {
                bar_progress_line(st)
            } else {
                String::new()
            }
        });
        pad_preview(
            ui,
            thumbs,
            want,
            cmd,
            w,
            preview_h,
            "UP NEXT",
            st.next_scene.and_then(|i| scenes.get(i)),
            String::new(),
        );
    });
    ui.add_space(8.0);

    // Action pads — the same Actions the hotkeys fire, with the current
    // binding badged top-right and a live sub-state under the label (1c).
    section_label(ui, "pads");
    let canon_now = match s.canon {
        Tristate::Auto => "auto",
        Tristate::On => "on",
        Tristate::Off => "off",
    };
    let holding = s.mode == Mode::Static;
    // Lit states for the overlay pads mirror what the render thread
    // actually shows (master switch + piece flag + content present).
    let logo_live =
        s.brand_on && s.brand_logo_on && !s.brand_logo.trim().is_empty();
    let name_live = s.brand_on
        && s.brand_name_on
        && !(s.brand_name.trim().is_empty() && s.brand_handles.trim().is_empty());
    let ticker_live = s.ticker_on && !s.ticker_text.trim().is_empty();
    let pads: [(Action, &str, String, PadKind, bool); 12] = [
        (
            Action::NextScene,
            "next scene",
            st.next_scene
                .and_then(|i| scenes.get(i))
                .map_or("cut now".to_string(), |n| n.clone()),
            PadKind::Go,
            false,
        ),
        (
            Action::PrevScene,
            "prev scene",
            "back one".to_string(),
            PadKind::Neutral,
            false,
        ),
        // Hold is a toggle: lit while static, tap again to release to Auto.
        (
            if holding { Action::ModeAuto } else { Action::ModeStatic },
            "hold / static",
            if holding {
                "on — tap to release".into()
            } else {
                match s.mode {
                    Mode::Auto => "mode: auto".into(),
                    Mode::Static => unreachable!(),
                    Mode::Manual => "mode: manual".into(),
                }
            },
            PadKind::Hold,
            holding,
        ),
        (
            Action::Blackout,
            "blackout",
            if st.blackout { "on — tap to lift" } else { "off" }.to_string(),
            PadKind::Danger,
            st.blackout,
        ),
        (
            Action::MarkPhrase,
            "mark section",
            format!("this beat = bar 1 of {}", s.phrase_bars),
            PadKind::Warn,
            false,
        ),
        (
            Action::CycleFx,
            "next fx",
            st.fx.label().to_string(),
            PadKind::Neutral,
            false,
        ),
        (
            Action::NextStyle,
            "dancer look",
            s.dancer_style.map(|i| STYLES[i]).unwrap_or("auto").to_string(),
            PadKind::Dancer,
            false,
        ),
        (
            Action::CycleCanon,
            "canon",
            canon_now.to_string(),
            PadKind::Dancer,
            false,
        ),
        (
            Action::ToggleDancer,
            "dancer",
            if s.dancer_enabled { "on" } else { "off" }.to_string(),
            PadKind::Dancer,
            s.dancer_enabled,
        ),
        (
            Action::ToggleLogo,
            "logo",
            if s.brand_logo.trim().is_empty() {
                "no image set".to_string()
            } else if logo_live {
                "on".to_string()
            } else {
                "off".to_string()
            },
            PadKind::Neutral,
            logo_live,
        ),
        (
            Action::ToggleName,
            "dj name",
            if s.brand_name.trim().is_empty() && s.brand_handles.trim().is_empty() {
                "no name set".to_string()
            } else if name_live {
                "on".to_string()
            } else {
                "off".to_string()
            },
            PadKind::Neutral,
            name_live,
        ),
        (
            Action::ToggleTicker,
            "scroll text",
            if s.ticker_text.trim().is_empty() {
                "no text set".to_string()
            } else if ticker_live {
                "on".to_string()
            } else {
                "off".to_string()
            },
            PadKind::Neutral,
            ticker_live,
        ),
    ];
    let pad_w = ((ui.available_width() - 3.0 * 10.0) / 4.0).max(100.0);
    // Fixed 88px pads — leftover height belongs to the previews (B1).
    let pad_h = 88.0;
    for row in pads.chunks(4) {
        ui.horizontal(|ui| {
            for (a, label, sub, kind, active) in row {
                // The hold pad fires ModeAuto to release, but its badge
                // stays the hold key.
                let key_of = if *a == Action::ModeAuto {
                    &Action::ModeStatic
                } else {
                    a
                };
                let key = key_short(s.keys.get(key_of).map_or("", String::as_str));
                // No key bound? Show the MIDI note instead ("n36").
                let key = if key.is_empty() {
                    s.midi_notes
                        .get(key_of)
                        .map(|n| format!("n{n}"))
                        .unwrap_or_default()
                } else {
                    key
                };
                if pad_button(ui, pad_w, pad_h, label, sub, &key, *kind, *active) {
                    cmd.push(UiCommand::Do(*a));
                }
            }
        });
    }
}

/// Pad colour by meaning. Only three are coloured (review B5): Go = the
/// primary accent fill, Danger = blackout red, everything else is CARD —
/// Hold/Warn/Dancer/Neutral differ only in what they do, not in colour.
#[derive(Clone, Copy)]
enum PadKind {
    Go,
    Neutral,
    Hold,
    Danger,
    Warn,
    Dancer,
}

/// One big action pad: label + sub-state bottom-left, key badge top-right.
/// `active` lights the pad — brighter fill, colour-keyed border, status dot.
fn pad_button(
    ui: &mut egui::Ui,
    w: f32,
    h: f32,
    label: &str,
    sub: &str,
    key: &str,
    kind: PadKind,
    active: bool,
) -> bool {
    use crate::ui_theme::*;
    // Three colours only (review B5): Go = ACCENT_SEL, Danger = DANGER_BG,
    // everything else CARD — active state is always the one accent.
    let (fill, fg, act) = match kind {
        PadKind::Go => (ACCENT_SEL, TEXT, ACCENT),
        PadKind::Danger => (DANGER_BG, DANGER, DANGER),
        _ => (CARD, TEXT, ACCENT),
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    let p = ui.painter();
    let fill = if active {
        fill.gamma_multiply(1.6)
    } else if resp.hovered() {
        fill.gamma_multiply(1.35)
    } else {
        fill
    };
    p.rect_filled(rect, 8.0, fill);
    p.rect_stroke(
        rect,
        8.0,
        if active {
            egui::Stroke::new(1.5, act)
        } else {
            egui::Stroke::new(1.0, if resp.hovered() { BORDER_HI } else { BORDER })
        },
        egui::StrokeKind::Inside,
    );
    if active {
        // Live-status dot, top-left — same language as the tab pills.
        p.circle_filled(egui::pos2(rect.min.x + 13.0, rect.min.y + 13.0), 4.0, act);
    }
    // Label + sub-line vertically centred on the left (B1).
    p.text(
        egui::pos2(rect.min.x + 12.0, rect.center().y - 16.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(14.0),
        fg,
    );
    p.text(
        egui::pos2(rect.min.x + 12.0, rect.center().y + 1.0),
        egui::Align2::LEFT_TOP,
        ellipsize(p, sub, &egui::FontId::monospace(10.5), w - 24.0),
        egui::FontId::monospace(10.5),
        MUTED,
    );
    // Key badge, top-right (same chips as ui_theme::key_badge).
    if !key.is_empty() {
        let kf = egui::FontId::monospace(11.0);
        let kw = text_w(p, key, &kf) + 12.0;
        let kr = egui::Rect::from_min_max(
            egui::pos2(rect.max.x - 8.0 - kw, rect.min.y + 8.0),
            egui::pos2(rect.max.x - 8.0, rect.min.y + 8.0 + 18.0),
        );
        p.rect_filled(kr, 4.0, INSET);
        p.rect_stroke(kr, 4.0, egui::Stroke::new(1.0, BORDER_HI), egui::StrokeKind::Inside);
        p.text(kr.center(), egui::Align2::CENTER_CENTER, key, kf, MUTED);
    }
    resp.clicked()
}

/// A NOW / UP NEXT card (review B2): the label sits in a left gutter
/// inside the card; the thumbnail fills the rest at 16:9, with the scene
/// name baked into its lower-left corner.
#[allow(clippy::too_many_arguments)]
fn pad_preview(
    ui: &mut egui::Ui,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
    w: f32,
    h: f32,
    title: &str,
    scene: Option<&String>,
    line: String,
) {
    use crate::ui_theme::*;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 8.0, CARD);
    p.rect_stroke(
        rect,
        8.0,
        egui::Stroke::new(1.0, BORDER),
        egui::StrokeKind::Inside,
    );

    // Left gutter: the label reads bottom-to-top (rotated 90°).
    let gut = 22.0;
    let galley = p.layout_no_wrap(title.to_string(), egui::FontId::monospace(10.0), MUTED);
    let gc = egui::pos2(rect.min.x + gut / 2.0, rect.center().y);
    let mut ts = egui::epaint::TextShape::new(gc, galley, MUTED);
    ts.angle = std::f32::consts::FRAC_PI_2;
    p.add(egui::Shape::Text(ts));
    p.line_segment(
        [
            egui::pos2(rect.min.x + gut, rect.min.y + 6.0),
            egui::pos2(rect.min.x + gut, rect.max.y - 6.0),
        ],
        egui::Stroke::new(1.0, BORDER),
    );

    // Thumbnail fills the card (minus gutter) at 16:9.
    let avail = egui::vec2(rect.width() - gut - 12.0, rect.height() - 12.0);
    let ih = avail.y.min(avail.x * 9.0 / 16.0);
    let iw = avail.x.min(ih * 16.0 / 9.0);
    let tr = egui::Rect::from_center_size(
        egui::pos2(rect.min.x + gut + 6.0 + avail.x / 2.0, rect.center().y),
        egui::vec2(iw, ih),
    );
    if let Some(name) = scene {
        let key = format!("scene:{name}");
        scene_thumb(ui, tr, &key, 1.0, thumbs, want, cmd);
    } else {
        p.rect_filled(tr, 4.0, RAISED);
        p.rect_stroke(
            tr,
            4.0,
            egui::Stroke::new(1.0, BORDER),
            egui::StrokeKind::Inside,
        );
    }
    // Name + timing caption inside the frame, lower-left.
    let caption = match scene {
        Some(n) if line.is_empty() => n.clone(),
        Some(n) => format!("{n} · {line}"),
        None => "—".to_string(),
    };
    let cf = egui::FontId::monospace(11.0);
    let cw = text_w(p, &caption, &cf) + 14.0;
    let cr = egui::Rect::from_min_max(
        egui::pos2(tr.min.x + 6.0, tr.max.y - 24.0),
        egui::pos2(tr.min.x + 6.0 + cw.min(tr.width() - 12.0), tr.max.y - 6.0),
    );
    p.rect_filled(cr, 4.0, INSET.gamma_multiply(0.85));
    p.text(
        cr.center(),
        egui::Align2::CENTER_CENTER,
        ellipsize(p, &caption, &cf, cr.width() - 10.0),
        cf,
        if title == "NOW" { TEXT } else { MUTED },
    );
}

/// The scrollable palette column on the pads view's right edge — a plain
/// list, not cards (review B6): name over a gradient strip, 6px gaps.
fn palette_column(ui: &mut egui::Ui, s: &mut Settings) {
    use crate::ui_theme::*;
    section_label(ui, "palette");
    ui.add_space(2.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .horizontal_scroll_offset(0.0)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            for name in crate::palettes::names() {
                let sel = s.palette == name;
                let w = ui.available_width();
                let (rect, resp) = ui.allocate_exact_size(
                    egui::vec2(w, 30.0),
                    egui::Sense::click(),
                );
                let p = ui.painter();
                p.text(
                    rect.min,
                    egui::Align2::LEFT_TOP,
                    name,
                    egui::FontId::monospace(11.0),
                    if sel { TEXT } else { MUTED },
                );
                // Gradient strip under the name — the swatch itself.
                let lut = crate::palettes::lut(name);
                let strip = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x, rect.min.y + 15.0),
                    egui::vec2(w, 14.0),
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
                // Selected: 2px accent outline on the strip.
                if sel {
                    p.rect_stroke(
                        strip,
                        2.0,
                        egui::Stroke::new(2.0, ACCENT),
                        egui::StrokeKind::Inside,
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
            [60.0, 26.0],
            egui::Label::new(
                egui::RichText::new(label)
                    .size(12.0)
                    .color(crate::ui_theme::MUTED),
            ),
        );
        body(ui);
    });
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

/// Compact key name for the badges under buttons — plain words only; the
/// bundled fonts lack the arrow/space glyphs (review: no tofu).
fn key_short(k: &str) -> String {
    match k {
        "ArrowRight" => "Right".into(),
        "ArrowLeft" => "Left".into(),
        "ArrowUp" => "Up".into(),
        "ArrowDown" => "Down".into(),
        "Space" => "Space".into(),
        other => other.to_string(),
    }
}

/// Paint a rotation checkbox: filled rounded rect + a two-segment tick.
/// No font glyphs — the bundled fonts lack "✓" (review rule).
fn paint_check(p: &egui::Painter, r: egui::Rect, on: bool) {
    use crate::ui_theme::*;
    p.rect_filled(r, 3.0, if on { ACCENT } else { INSET.gamma_multiply(0.9) });
    p.rect_stroke(
        r,
        3.0,
        egui::Stroke::new(1.0, BORDER_HI),
        egui::StrokeKind::Inside,
    );
    if on {
        let tick = egui::Stroke::new(1.8, egui::Color32::from_rgb(0x0B, 0x0D, 0x10));
        let pt = |fx: f32, fy: f32| {
            egui::pos2(
                r.min.x + r.width() * fx,
                r.min.y + r.height() * fy,
            )
        };
        p.line_segment([pt(0.24, 0.52), pt(0.44, 0.74)], tick);
        p.line_segment([pt(0.44, 0.74), pt(0.78, 0.28)], tick);
    }
}

/// Coloured status dot + line at the top of a Stream card. The text is
/// elided — a long track name can't wrap inside a horizontal row and
/// would push the card wider than its column.
fn status_dot(ui: &mut egui::Ui, ok: bool, text: &str) {
    // Measure before entering the horizontal — inside it, available_width
    // can read the unshrunk child max_rect.
    let outer_w = ui.available_width();
    ui.horizontal(|ui| {
        let (r, _) =
            ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
        ui.painter().circle_filled(
            r.center(),
            3.5,
            if ok { crate::ui_theme::GOOD } else { crate::ui_theme::FAINT },
        );
        let f = egui::FontId::proportional(11.0);
        let shown = ellipsize(ui.painter(), text, &f, (outer_w - 18.0).max(60.0));
        ui.label(
            egui::RichText::new(shown)
                .size(11.0)
                .color(crate::ui_theme::MUTED),
        )
        .on_hover_text(text);
    });
}

fn stream_tab(ui: &mut egui::Ui, s: &mut Settings, st: &Status, cmd: &mut Vec<UiCommand>) {
    use crate::nowplaying::NpSource;
    use crate::ui_theme::*;
    // 12px between cards on both axes (review D4).
    ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
    // Two bounded column uis — `ui.columns` inside a ScrollArea lets card
    // content bleed under the neighbour card at narrow widths (A1). Each
    // column gets an explicit rect so nothing can overflow.
    let row_w = ui.available_width();
    let col_w = ((row_w - 12.0) / 2.0).max(220.0);
    let top = ui.cursor().min;
    let mut mk_col = |x: f32, w: f32| {
        ui.new_child(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_size(
                    egui::pos2(top.x + x, top.y),
                    egui::vec2(w, 4000.0),
                ))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        )
    };
    // Right column first — its height is what the Now Playing card
    // stretches to, so the column bottoms line up.
    let right_h = {
        let ui = &mut mk_col(col_w + 12.0, col_w);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, "branding");
            let brand_live = s.brand_on
                && ((s.brand_logo_on && !s.brand_logo.trim().is_empty())
                    || (s.brand_name_on
                        && !(s.brand_name.trim().is_empty()
                            && s.brand_handles.trim().is_empty())));
            status_dot(ui, brand_live, if brand_live { "showing" } else { "off" });
        ui.horizontal(|ui| {
            ui.checkbox(&mut s.brand_on, "Show");
            ui.checkbox(&mut s.brand_logo_on, "logo");
            ui.checkbox(&mut s.brand_name_on, "name");
        });
        egui::Grid::new("brand_grid")
            .num_columns(2)
            .min_col_width(96.0)
            .spacing(egui::vec2(8.0, 8.0))
            .show(ui, |ui| {
            grow(ui, "DJ name", |ui| {
                ui.add(egui::TextEdit::singleline(&mut s.brand_name).desired_width(180.0));
            });
            grow(ui, "Handles", |ui| {
                ui.add(egui::TextEdit::singleline(&mut s.brand_handles).desired_width(180.0).hint_text("@you · twitch.tv/you"));
            });
            grow(ui, "Logo", |ui| {
                ui.add(egui::TextEdit::singleline(&mut s.brand_logo).desired_width(150.0).hint_text("PNG, optional"));
                if ui.button("…").clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("PNG", &["png"]).pick_file() {
                        s.brand_logo = p.display().to_string();
                    }
                }
            });
            grow(ui, "Corner", |ui| {
                // Corner arrows are tofu in the bundled fonts — words,
                // wrapped 2x2 so BR never clips the card edge.
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                    for (i, l) in ["TL", "TR", "BL", "BR"].iter().enumerate() {
                        ui.selectable_value(&mut s.brand_corner, i as u8, *l);
                    }
                });
            });
            grow(ui, "Size", |ui| {
                ui.add(egui::Slider::new(&mut s.brand_size, 0.5..=2.0));
            });
            grow(ui, "Opacity", |ui| {
                ui.add(egui::Slider::new(&mut s.brand_opacity, 0.1..=1.0));
            });
            grow(ui, "Accent", |ui| {
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
        });
        ui.add_space(12.0);
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
        egui::Grid::new("ticker_grid")
            .num_columns(2)
            .min_col_width(96.0)
            .spacing(egui::vec2(8.0, 8.0))
            .show(ui, |ui| {
            grow(ui, "Speed", |ui| {
                ui.add(egui::Slider::new(&mut s.ticker_speed, 0.3..=3.0));
            });
            });
        });
        ui.min_rect().height()
    };
    let left_h = {
        let ui = &mut mk_col(0.0, col_w);
        card().show(ui, |ui| {
            // Stretch to the right column's height so the bottoms align.
            ui.set_min_height((right_h - 30.0).max(60.0));
            ui.set_width(ui.available_width());
            section_label(ui, "now playing");
            status_dot(
                ui,
                st.np_track.is_some(),
                &st.np_track.clone().unwrap_or_else(|| "no track".into()),
            );
        ui.small("Auto follows whichever source changed last — media players, Serato, VirtualDJ, rekordbox, Mixxx, or a text file.");
        egui::Grid::new("np_grid")
            .num_columns(2)
            .min_col_width(96.0)
            .spacing(egui::vec2(8.0, 8.0))
            .show(ui, |ui| {
            grow(ui, "Source", |ui| {
                egui::ComboBox::from_id_salt("np_src")
                    .width(130.0)
                    .selected_text(s.np_source.label())
                    .show_ui(ui, |ui| {
                        for src in NpSource::ALL {
                            ui.selectable_value(&mut s.np_source, src, src.label());
                        }
                    });
            });
            if s.np_source == NpSource::File || s.np_source == NpSource::Auto {
                grow(ui, "Text file", |ui| {
                    ui.add(egui::TextEdit::singleline(&mut s.np_file).desired_width(150.0).hint_text("optional"));
                    if ui.button("…").clicked() {
                        if let Some(p) = rfd::FileDialog::new().add_filter("text", &["txt"]).pick_file() {
                            s.np_file = p.display().to_string();
                        }
                    }
                });
            }
            grow(ui, "Delay", |ui| {
                ui.add(egui::Slider::new(&mut s.np_delay_s, 0.0..=60.0).step_by(1.0).suffix(" s"));
            });
            grow(ui, "Card", |ui| {
                ui.checkbox(&mut s.np_card, "Show on screen");
                if ui.button("Show again").clicked() {
                    cmd.push(UiCommand::Do(Action::ShowNowPlaying));
                }
            });
            grow(ui, "Card time", |ui| {
                ui.add(egui::Slider::new(&mut s.np_hold_s, 0.0..=60.0).step_by(1.0).suffix(" s"));
            });
            grow(ui, "Card size", |ui| {
                ui.add(egui::Slider::new(&mut s.np_size, 0.5..=2.0));
            });
            });
        // Per-source status: compact dot + text grid under a collapse
        // (review D2). Green = has a track, grey = idle / not found.
        if !st.np_status.is_empty() {
            egui::CollapsingHeader::new(
                egui::RichText::new("Sources").size(11.0).color(MUTED),
            )
            .id_salt("np_sources")
            .default_open(false)
            .show(ui, |ui| {
                egui::Grid::new("np_src_grid")
                    .num_columns(2)
                    .min_col_width(90.0)
                    .spacing(egui::vec2(8.0, 4.0))
                    .show(ui, |ui| {
                        for (name, line) in &st.np_status {
                            let live_src = !(line == "idle"
                                || line.starts_with("no ")
                                || line.starts_with("not ")
                                || line.contains("found"));
                            ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(
                                    egui::vec2(10.0, 10.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().circle_filled(
                                    r.center(),
                                    3.0,
                                    if live_src { GOOD } else { FAINT },
                                );
                                ui.label(
                                    egui::RichText::new(name).size(11.0).color(MUTED),
                                );
                            });
                            ui.label(
                                egui::RichText::new(ellipsize(
                                    ui.painter(),
                                    line,
                                    &egui::FontId::monospace(10.5),
                                    160.0,
                                ))
                                .monospace()
                                .size(10.5)
                                .color(if live_src { TEXT } else { FAINT }),
                            );
                            ui.end_row();
                        }
                    });
            });
        }
        // The OBS path: a normal left-aligned label + mono path + Copy —
        // justified text was rendering as letter-spaced mush (review D1).
        ui.label(
            egui::RichText::new("Add a Text source in OBS reading:")
                .size(11.0)
                .color(FAINT),
        );
        // Elide the LEFT of the path against the card's inner width — a
        // long path otherwise pushes the card wider than its column (A1).
        let path_outer_w = ui.available_width();
        ui.horizontal(|ui| {
            let path = crate::config::data_dir()
                .join("nowplaying.txt")
                .display()
                .to_string();
            let mono = egui::FontId::monospace(10.5);
            // frame margins (14) + spacing + Copy button (~46)
            let max_w = (path_outer_w - 72.0).max(80.0);
            let shown = elide_left(ui.painter(), &path, &mono, max_w);
            egui::Frame::NONE
                .fill(INSET)
                .stroke(egui::Stroke::new(1.0, BORDER_HI))
                .corner_radius(egui::CornerRadius::same(4))
                .inner_margin(egui::Margin::symmetric(6, 3))
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(shown).font(mono).color(MUTED),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&path);
                });
            if ui.small_button("Copy").clicked() {
                ui.ctx().copy_text(path);
            }
        });
        });
        ui.min_rect().height()
    };
    // The child uis painted without touching the parent's cursor — claim
    // the row's height so the next row starts below both columns.
    ui.add_space(right_h.max(left_h));
    ui.add_space(12.0);
    let top2 = ui.cursor().min;
    let mut mk_col2 = |x: f32, w: f32| {
        ui.new_child(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_size(
                    egui::pos2(top2.x + x, top2.y),
                    egui::vec2(w, 4000.0),
                ))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        )
    };
    let rec_h = {
        let ui = &mut mk_col2(0.0, col_w);
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
        egui::Grid::new("rec_grid")
            .num_columns(2)
            .min_col_width(96.0)
            .spacing(egui::vec2(8.0, 8.0))
            .show(ui, |ui| {
            grow(ui, "Replay", |ui| {
                ui.checkbox(&mut s.rec_buffer, "Keep the last");
                ui.add(egui::DragValue::new(&mut s.rec_keep_s).range(10..=600).suffix(" s"));
            });
            grow(ui, "Format", |ui| {
                for l in crate::rec::Layout::ALL {
                    ui.selectable_value(&mut s.rec_layout, l, l.label());
                }
            });
            grow(ui, "", |ui| {
                let can = st.rec.is_some();
                if ui.add_enabled(can, egui::Button::new("Save clip")).on_hover_text("Hotkey K").clicked() {
                    cmd.push(UiCommand::Do(Action::SaveClip));
                }
                let rolling = st.rec.as_ref().and_then(|r| r.set_since);
                let label = match rolling {
                    Some(t) => {
                        let e = t.elapsed().as_secs();
                        format!("Stop set ({}:{:02}:{:02})", e / 3600, e / 60 % 60, e % 60)
                    }
                    None => "Record set".into(),
                };
                if ui.button(label).on_hover_text("Hotkey J — records until you stop it").clicked() {
                    cmd.push(UiCommand::Do(Action::RecordSet));
                }
            });
            grow(ui, "Folder", |ui| {
                ui.add(egui::TextEdit::singleline(&mut s.rec_dir).desired_width(150.0).hint_text("Videos/Trippin"));
                if ui.button("…").clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        s.rec_dir = p.display().to_string();
                    }
                }
            });
            grow(ui, "ffmpeg", |ui| {
                ui.add(egui::TextEdit::singleline(&mut s.ffmpeg_path).desired_width(150.0).hint_text("auto"));
                if ui.button("…").clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_file() {
                        s.ffmpeg_path = p.display().to_string();
                    }
                }
            });
            });
        if let Some(e) = &st.rec_err {
            ui.colored_label(WARN, e);
        }
        if let Some(r) = &st.rec {
            let mut line = format!("{} · {} s buffered", r.encoder, r.buffered_s);
            if r.saving {
                line.push_str(" · saving…");
            }
            ui.small(line);
            if let Some(e) = &r.err {
                ui.colored_label(DANGER, e);
            } else if let Some(p) = &r.last {
                ui.small(format!("Saved {p}"));
            }
        }
        ui.small("Clips include the overlays and the audio. Size/fps follow the video output settings.");

        });
        ui.min_rect().height()
    };
    let out_h = {
        let ui = &mut mk_col2(col_w + 12.0, col_w);
        card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section_label(ui, "video output");
        status_dot(
            ui,
            st.output.is_some(),
            st.output.as_deref().unwrap_or("off — enable Spout or NDI"),
        );
        egui::Grid::new("out_grid")
            .num_columns(2)
            .min_col_width(96.0)
            .spacing(egui::vec2(8.0, 8.0))
            .show(ui, |ui| {
            grow(ui, "Name", |ui| {
                ui.add(egui::TextEdit::singleline(&mut s.ndi_name).desired_width(140.0));
            });
            grow(ui, "Send", |ui| {
                if cfg!(windows) {
                    ui.checkbox(&mut s.spout_enabled, "Spout")
                        .on_hover_text("Same PC: OBS → Add source → Spout2 Capture (needs the free Spout2 OBS plugin).");
                }
                ui.checkbox(&mut s.ndi_enabled, "NDI")
                    .on_hover_text("Over the network: OBS → NDI Source (needs the free NDI runtime / DistroAV plugin).");
            });
            grow(ui, "Background", |ui| {
                ui.selectable_value(&mut s.out_transparent, false, "Scenes");
                ui.selectable_value(&mut s.out_transparent, true, "Transparent");
            });
            grow(ui, "Size", |ui| {
                for h in [720u32, 1080, 2160] {
                    ui.selectable_value(&mut s.ndi_height, h, format!("{h}p"));
                }
                ui.label("at");
                for f in [30u32, 60] {
                    ui.selectable_value(&mut s.ndi_fps, f, format!("{f} fps"));
                }
            });
            });
        if s.out_transparent {
            ui.small("Scenes off: only the dancer, glow, overlays and text go out, with alpha — layer them over your camera in OBS.");
        }
        if st.output.is_none() {
            ui.small(
                "Sends the finished frame (overlays included) to OBS or another display. \
                 Spout for OBS on this PC, NDI across the network.",
            );
        }
        });
        ui.min_rect().height()
    };
    ui.add_space(rec_h.max(out_h));
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
    // Bounded column uis — `ui.columns` lets card content bleed under the
    // neighbour card inside a ScrollArea at narrow widths (A1).
    let row_w = ui.available_width();
    let col_w = ((row_w - 12.0) / 2.0).max(220.0);
    let top = ui.cursor().min;
    let mut mk_col = |x: f32| {
        ui.new_child(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_size(
                    egui::pos2(top.x + x, top.y),
                    egui::vec2(col_w, 4000.0),
                ))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        )
    };
    let h1 = {
        let ui = &mut mk_col(0.0);
        let r1 = card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, "dancer");
            ui.small("Silhouette layer, look and movement — one line only.");
            ui.add_space(4.0);
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
            ctl_row(ui, "Size", |ui| {
                ui.add(egui::Slider::new(&mut s.dancer_size, 0.4..=1.0));
            });
            ui.checkbox(&mut s.dancer_trails, "Motion trails");
        });
        r1.response.rect.height()
    };
    let h2 = {
        // Equal heights: the FX card stretches to the Dancer card's height.
        let ui = &mut mk_col(col_w + 12.0);
        let r2 = card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height((h1 - 26.0).max(60.0));
            section_label(ui, "fx");
            ui.small("Whole-frame transforms — your pick is the live preview.");
            ui.add_space(4.0);
            // 3x2 segmented grid — fixed cells, nothing wraps (review C1).
            egui::Grid::new("fx_grid")
                .num_columns(3)
                .spacing(egui::vec2(6.0, 6.0))
                .show(ui, |ui| {
                    let cw = ((ui.available_width() - 12.0) / 3.0).max(60.0);
                    for (i, f) in Fx::ALL.iter().enumerate() {
                        let on = s.fx == *f && !s.fx_auto;
                        let r = ui.add_sized(
                            [cw, 28.0],
                            egui::Button::new(f.label()).selected(on),
                        );
                        if r.clicked() {
                            s.fx = *f;
                            s.fx_auto = false;
                        }
                        if i % 3 == 2 {
                            ui.end_row();
                        }
                    }
                });
            ui.add_space(4.0);
            ctl_row(ui, "Strength", |ui| {
                ui.add_enabled(
                    s.fx != Fx::Off,
                    egui::Slider::new(&mut s.fx_amt, 0.0..=1.0),
                );
            });
            ui.checkbox(&mut s.fx_auto, "Auto — a fresh effect on every scene cut");
            if s.fx_auto {
                ui.small(format!("On screen now: {}", st.fx.label()));
            }
        });
        r2.response.rect.height()
    };
    // The child uis didn't move the parent cursor — claim the row height.
    ui.add_space(h1.max(h2));
    ui.add_space(6.0);

    // Routine library: one tile per routine — `_mir` twins ride on their
    // base tile as a "mir" badge, tick to keep in rotation, click to preview.
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section_label(ui, "routines");
        ui.small("Auto-pilot picks among the ticked routines — click a tile to preview it live.");
        ui.add_space(4.0);
        let avail = ui.available_width();
        let cols = ((avail + 8.0) / 108.0).floor().max(3.0) as usize;
        let bases: Vec<usize> = (0..clips.len())
            .filter(|&i| !clips[i].ends_with("_mir"))
            .collect();
        for chunk in bases.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                for &i in chunk {
                    let name = &clips[i];
                    let has_mir = clips.iter().any(|c| c == &format!("{name}_mir"));
                    clip_tile(ui, s, st, i, name, has_mir, clip_thumbs, cmd);
                }
            });
        }
    });
}

/// One routine tile in the Dancer & FX library. `_mir` twins are folded
/// into their base clip: a small "mir" badge toggles the twin's rotation
/// membership (review — 22 tiles, not 44).
fn clip_tile(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    i: usize,
    name: &str,
    has_mir: bool,
    clip_thumbs: &mut HashMap<String, Option<egui::TextureHandle>>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    const TW: f32 = 100.0;
    const TH: f32 = 116.0;
    let twin = format!("{name}_mir");
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(TW, TH), egui::Sense::click());
    let p = ui.painter();
    let live = st.clip.as_deref() == Some(name) || st.clip.as_deref() == Some(twin.as_str());
    let on = !s.disabled_clips.iter().any(|d| d == name);
    let mir_on = !s.disabled_clips.iter().any(|d| d == &twin);
    let dim = if on { 1.0 } else { 0.45 };

    p.rect_filled(rect, 6.0, CARD);
    let img_r = egui::Rect::from_min_size(
        rect.min + egui::vec2(5.0, 5.0),
        egui::vec2(TW - 10.0, 84.0),
    );
    match clip_tex(ui, clip_thumbs, name) {
        Some((tex, sz)) => {
            // Letterbox: clips are portrait, the box isn't. The thumb is a
            // white silhouette with luminance alpha — no black box.
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
            p.rect_filled(img_r, 4.0, INSET);
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

    // Mirror badge, top-left: whether the _mir twin is in rotation.
    if has_mir {
        let mb_r = egui::Rect::from_min_size(
            egui::pos2(img_r.min.x + 3.0, img_r.min.y + 3.0),
            egui::vec2(24.0, 15.0),
        );
        let mb = ui.interact(mb_r, resp.id.with("mir"), egui::Sense::click());
        p.rect_filled(
            mb_r,
            3.0,
            if mir_on { ACCENT_SEL } else { INSET.gamma_multiply(0.9) },
        );
        p.rect_stroke(
            mb_r,
            3.0,
            egui::Stroke::new(1.0, if mir_on { ACCENT } else { BORDER_HI }),
            egui::StrokeKind::Inside,
        );
        p.text(
            mb_r.center(),
            egui::Align2::CENTER_CENTER,
            "mir",
            egui::FontId::monospace(8.5),
            if mir_on { ACCENT } else { FAINT },
        );
        if mb.clicked() {
            if mir_on {
                s.disabled_clips.push(twin.clone());
            } else {
                s.disabled_clips.retain(|d| d != &twin);
            }
        }
        mb.on_hover_text(format!("mirror variant {}", if mir_on { "in rotation" } else { "off" }));
    }

    // Rotation checkbox top-right (registered last so it wins its clicks).
    let cb_r = egui::Rect::from_min_size(
        egui::pos2(img_r.max.x - 18.0, img_r.min.y + 3.0),
        egui::vec2(15.0, 15.0),
    );
    let cb = ui.interact(cb_r, resp.id.with("rot"), egui::Sense::click());
    let cb_clicked = cb.clicked();
    paint_check(p, cb_r, on);
    if cb_clicked {
        if on {
            s.disabled_clips.push(name.to_string());
        } else {
            s.disabled_clips.retain(|d| d != name);
        }
    }

    // Middle-elide: suffixes (like _mir) carry meaning, don't truncate them.
    let shown = elide_mid(
        p,
        name,
        &egui::FontId::monospace(10.0),
        TW - 12.0,
    );
    p.text(
        egui::pos2(rect.min.x + 6.0, img_r.max.y + 5.0),
        egui::Align2::LEFT_TOP,
        shown,
        egui::FontId::monospace(10.0),
        if live { ACCENT } else { TEXT.gamma_multiply(dim) },
    );
    p.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(
            if live { 1.5 } else { 1.0 },
            if live { ACCENT } else { BORDER },
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

/// One `action | key badge | rebind | midi` row inside a keys-page Grid
/// (must emit exactly four cells + `end_row`).
fn key_row(
    ui: &mut egui::Ui,
    s: &mut Settings,
    rebinding: &mut Option<Action>,
    midi_learn: &mut Option<Action>,
    bound: &HashMap<String, u32>,
    a: Action,
) {
    use crate::ui_theme::*;
    let key = s.keys.get(&a).cloned().unwrap_or_default();
    let conflict = !key.is_empty() && bound.get(key.as_str()).copied().unwrap_or(0) > 1;
    // Column 1: action label, left-aligned — fixed generous width so
    // full names show (truncate() alone let the grid squeeze the column).
    ui.add_sized(
        [200.0, 18.0],
        egui::Label::new(egui::RichText::new(a.label()).size(12.0).color(TEXT)).truncate(),
    );
    // Column 2: key badge, fixed 90px.
    ui.scope(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(90.0, 20.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(r, 4.0, INSET);
        p.rect_stroke(
            r,
            4.0,
            egui::Stroke::new(1.0, if conflict { WARN } else { BORDER_HI }),
            egui::StrokeKind::Inside,
        );
        let shown = key_short(&key);
        p.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            if shown.is_empty() { "—".to_string() } else { shown },
            egui::FontId::monospace(11.0),
            if conflict { WARN } else { MUTED },
        );
    });
    // Column 3: rebind, fixed 70px.
    ui.scope(|ui| {
        if *rebinding == Some(a) {
            ui.label(
                egui::RichText::new("press a key…")
                    .size(11.0)
                    .color(WARN),
            );
            if ui.small_button("cancel").clicked() {
                *rebinding = None;
            }
        } else {
            let r = ui.add_sized(
                [70.0, 24.0],
                egui::Button::new(egui::RichText::new("rebind").size(12.0)),
            );
            if r.clicked() {
                *rebinding = Some(a);
            }
        }
    });
    // Column 4: MIDI pad binding — click to learn the next note-on,
    // right-click a bound note to clear it.
    ui.scope(|ui| {
        if *midi_learn == Some(a) {
            ui.label(
                egui::RichText::new("hit a pad…")
                    .size(11.0)
                    .color(WARN),
            );
            if ui.small_button("cancel").clicked() {
                *midi_learn = None;
            }
        } else {
            let note = s.midi_notes.get(&a).copied();
            let txt = note.map_or_else(|| "midi".to_string(), |n| n.to_string());
            let mut b = ui.add_sized(
                [56.0, 24.0],
                egui::Button::new(egui::RichText::new(txt).size(11.0)),
            );
            if let Some(n) = note {
                b = b.on_hover_text(format!("note {n} = {}", crate::midi::note_name(n)));
            }
            if b.clicked() {
                *midi_learn = Some(a);
            } else if b.secondary_clicked() {
                s.midi_notes.remove(&a);
            }
        }
    });
    ui.end_row();
    if conflict {
        ui.label("");
        ui.label(
            egui::RichText::new("conflict — this key fires two actions")
                .size(10.0)
                .color(WARN),
        );
        ui.label("");
        ui.label("");
        ui.end_row();
    }
}

/// Keys page: filter, grouped action/key/rebind rows across columns.
fn keys_tab(
    ui: &mut egui::Ui,
    s: &mut Settings,
    rebinding: &mut Option<Action>,
    midi_learn: &mut Option<Action>,
    midi_status: &(bool, String),
    filter: &mut String,
) {
    use crate::ui_theme::*;
    // MIDI input: pick the controller port, then the per-action "midi"
    // buttons learn whatever pad is hit next. Ports are listed only while
    // the dropdown is open — enumeration creates a fresh client each call.
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("midi input")
                .size(11.0)
                .color(MUTED),
        );
        egui::ComboBox::from_id_salt("midi_in")
            .width(200.0)
            .selected_text(if s.midi_in.is_empty() {
                "off".to_string()
            } else {
                s.midi_in.clone()
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut s.midi_in, String::new(), "off");
                for name in crate::midi::ports() {
                    ui.selectable_value(&mut s.midi_in, name.clone(), name);
                }
            });
        status_dot(
            ui,
            midi_status.0,
            if s.midi_in.is_empty() {
                "off"
            } else {
                midi_status.1.as_str()
            },
        );
    });
    // Filter styled like the library search, aligned with the card's edge.
    egui::Frame::NONE
        .fill(INSET)
        .stroke(egui::Stroke::new(1.0, BORDER_HI))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(filter)
                    .desired_width(220.0)
                    .frame(egui::Frame::NONE)
                    .hint_text("Filter actions…"),
            );
        });
    ui.label(
        egui::RichText::new("keys work in both windows — rebind, then press the key")
            .size(11.0)
            .color(FAINT),
    );
    ui.add_space(4.0);

    // A key bound to two actions fires both — flag it. Owned strings: a
    // &str map would borrow s.keys for the rest of the function and block
    // the &mut s the rebind buttons need.
    let mut bound: HashMap<String, u32> = HashMap::new();
    for k in s.keys.values() {
        if !k.is_empty() {
            *bound.entry(k.clone()).or_default() += 1;
        }
    }
    let q = filter.to_lowercase();

    // Grouped sections (review F) — far easier to scan than 25 flat rows.
    const GROUPS: &[(&str, &[Action])] = &[
        ("scenes", &[Action::NextScene, Action::PrevScene, Action::ToggleRandom]),
        ("mode", &[Action::ModeAuto, Action::ModeStatic, Action::ModeManual]),
        (
            "dancer",
            &[
                Action::ToggleDancer,
                Action::NextClip,
                Action::NextStyle,
                Action::CycleCanon,
            ],
        ),
        (
            "sync",
            &[
                Action::MarkPhrase,
                Action::MarkDownbeat,
                Action::LatencyDown,
                Action::LatencyUp,
            ],
        ),
        (
            "output",
            &[
                Action::CycleFx,
                Action::Blackout,
                Action::Fullscreen,
                Action::LeaveFullscreen,
                Action::TogglePanel,
                Action::ShowNowPlaying,
                Action::ReloadShaders,
                Action::ToggleLogo,
                Action::ToggleName,
                Action::ToggleTicker,
            ],
        ),
        (
            "timeline",
            &[
                Action::ToggleEditor,
                Action::TimelinePlay,
                Action::TimelineRecord,
            ],
        ),
        ("recording", &[Action::SaveClip, Action::RecordSet]),
    ];

    // Filter groups, then split them across columns — one card each — so
    // the list uses the page width instead of scrolling as one tall
    // column. Bounded child uis, not ui.columns (that bleeds under
    // neighbours inside a ScrollArea — A1).
    let groups: Vec<(&str, Vec<Action>)> = GROUPS
        .iter()
        .filter_map(|(g, acts)| {
            let acts: Vec<Action> = acts
                .iter()
                .copied()
                .filter(|a| q.is_empty() || a.label().to_lowercase().contains(&q))
                .collect();
            (!acts.is_empty()).then_some((*g, acts))
        })
        .collect();
    let avail = ui.available_width();
    // A row needs ~470px: label 200 + badge 90 + rebind 70 + midi 56 +
    // grid spacing + card margin. Two columns fit a 980px window.
    let ncol = ((avail / 470.0) as usize).clamp(1, 3).min(groups.len().max(1));
    // Greedy balance by row count — a group is never split across columns.
    let total: usize = groups.iter().map(|(_, a)| a.len() + 1).sum();
    let target = total.div_ceil(ncol);
    let mut buckets: Vec<Vec<(&str, Vec<Action>)>> = vec![Vec::new(); ncol];
    let (mut bi, mut rows) = (0usize, 0usize);
    for g in groups {
        let n = g.1.len() + 1;
        if rows > 0 && rows + n > target && bi + 1 < ncol {
            bi += 1;
            rows = 0;
        }
        rows += n;
        buckets[bi].push(g);
    }
    let col_w = (avail - 16.0 * (ncol - 1) as f32) / ncol as f32;
    let top = ui.cursor().min;
    let mut max_h = 0.0_f32;
    for (i, bucket) in buckets.iter().enumerate() {
        let mut col = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_size(
                    egui::pos2(top.x + i as f32 * (col_w + 16.0), top.y),
                    egui::vec2(col_w, 4000.0),
                ))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        card().show(&mut col, |ui| {
            ui.set_width(ui.available_width());
            for (group, acts) in bucket {
                section_label(ui, group);
                egui::Grid::new(egui::Id::new("keys_grid").with(group))
                    .num_columns(4)
                    .spacing(egui::vec2(8.0, 6.0))
                    .show(ui, |ui| {
                        for &a in acts {
                            key_row(ui, s, rebinding, midi_learn, &bound, a);
                        }
                    });
                ui.add_space(6.0);
            }
        });
        max_h = max_h.max(col.min_rect().height());
    }
    // The child uis painted without touching the parent's cursor — claim
    // the columns' height so the footer starts below them.
    ui.add_space(max_h);
    ui.add_space(8.0);

    // Footer: reset, right-aligned, danger text style (review F).
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("Reset all keys to defaults")
                            .size(12.0)
                            .color(DANGER),
                    )
                    .frame(false),
                )
                .clicked()
            {
                s.keys = Settings::default().keys;
            }
        });
    });
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
        CueKind::Blackout(_) | CueKind::Strobe(_) => t::DANGER,
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
        CueKind::Dancer(b) | CueKind::Blackout(b) | CueKind::Strobe(b) | CueKind::FxAuto(b) => {
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
            egui::ComboBox::from_id_salt(id.with("tf"))
                .width(70.0)
                .selected_text(spec.fx.label())
                .show_ui(ui, |ui| {
                    for v in crate::text::TextFx::ALL {
                        changed |= ui.selectable_value(&mut spec.fx, v, v.label()).changed();
                    }
                });
            changed |= ui.selectable_value(&mut spec.lane, 0, "lane 1").changed();
            changed |= ui.selectable_value(&mut spec.lane, 1, "lane 2").changed();
            changed
        }
        _ => false,
    }
}

/// Settings page: app-wide preferences that aren't part of performing —
/// audio source & sync, what the auto-pilot may do, and the AI show
/// builder's provider. Two bounded columns, like the Stream page (A1).
fn settings_tab(ui: &mut egui::Ui, s: &mut Settings, st: &Status, cmd: &mut Vec<UiCommand>) {
    use crate::ui_theme::*;
    ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
    let row_w = ui.available_width();
    let col_w = ((row_w - 12.0) / 2.0).max(220.0);
    let top = ui.cursor().min;
    let mut mk_col = |x: f32, w: f32| {
        ui.new_child(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_size(
                    egui::pos2(top.x + x, top.y),
                    egui::vec2(w, 4000.0),
                ))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        )
    };
    let left_h = {
        let ui = &mut mk_col(0.0, col_w);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, "audio & sync");
            status_dot(
                ui,
                !st.silent,
                &if st.silent {
                    format!("{} · no signal", st.device)
                } else {
                    st.device.clone()
                },
            );
            ui.small(
                "Pick your controller's input if the DJ software sends audio straight \
                 to it (Serato into a Rane's USB card never reaches the system mix).",
            );
            // Measured outside the grid: inside a cell available_width
            // reads the unshrunk max_rect (A1).
            let combo_w = (ui.available_width() - 112.0).max(80.0);
            egui::Grid::new("set_audio_grid")
                .num_columns(2)
                .min_col_width(96.0)
                .spacing(egui::vec2(8.0, 8.0))
                .show(ui, |ui| {
                    // Audio source — saved, restarts capture live.
                    grow(ui, "Audio in", |ui| {
                        let sel = if s.audio_in.is_empty() {
                            crate::audio::system_audio_label()
                        } else {
                            s.audio_in.as_str()
                        };
                        let shown = ellipsize(
                            ui.painter(),
                            sel,
                            &egui::FontId::proportional(12.0),
                            combo_w - 24.0,
                        );
                        egui::ComboBox::from_id_salt("audio_in")
                            .width(combo_w)
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut s.audio_in,
                                    String::new(),
                                    crate::audio::system_audio_label(),
                                );
                                // Only built while the popup is open.
                                for n in crate::audio::capture_device_names() {
                                    ui.selectable_value(&mut s.audio_in, n.clone(), n);
                                }
                            });
                    });
                    grow(ui, "Latency", |ui| {
                        ui.add(
                            egui::Slider::new(&mut s.latency_ms, 0.0..=200.0)
                                .suffix(" ms")
                                .fixed_decimals(0),
                        )
                        .on_hover_text("Delay the visuals to line up with what the crowd hears");
                    });
                    // How steadily kicks are landing (sustained low groove =
                    // the show's in a breakdown).
                    grow(ui, "Groove", |ui| {
                        let (r, _) = ui
                            .allocate_exact_size(egui::vec2(120.0, 6.0), egui::Sense::hover());
                        let pp = ui.painter();
                        pp.rect_filled(r, 3.0, INSET);
                        pp.rect_filled(
                            egui::Rect::from_min_size(
                                r.min,
                                egui::vec2(r.width() * st.groove.clamp(0.0, 1.0), r.height()),
                            ),
                            3.0,
                            BREAKDOWN,
                        );
                    });
                });
            if ui
                .button("Mark this beat as the downbeat (the \"one\")")
                .clicked()
            {
                cmd.push(UiCommand::Do(Action::MarkDownbeat));
            }
            // Neural beat tracking (Beat This!): song grids for timelines
            // and the AI builder, plus the live downbeat check.
            if ui
                .checkbox(&mut s.beat_model, "Neural beat tracking")
                .on_hover_text(
                    "Finds the tempo and the bar's \"one\" with the Beat This! model \
                     (as in BeatDis) instead of guessing from the bass — fixes bars \
                     landing early or late after drum-roll intros. Downloads ~80 MB once.",
                )
                .changed()
                && s.beat_model
            {
                crate::beats::ensure_models();
            }
            crate::beats::set_enabled(s.beat_model);
            if s.beat_model {
                use crate::beats::ModelState;
                match crate::beats::state() {
                    ModelState::Ready => {
                        ui.small("Model ready.");
                    }
                    ModelState::Downloading { received, total, .. } => {
                        ui.small(if total > 0 {
                            format!("Downloading the model… {} of {} MB", received >> 20, total >> 20)
                        } else {
                            "Downloading the model…".to_string()
                        });
                    }
                    ModelState::Failed(e) => {
                        ui.horizontal(|ui| {
                            ui.small(egui::RichText::new("Model download failed.").color(WARN))
                                .on_hover_text(e);
                            if ui.small_button("Retry").clicked() {
                                crate::beats::ensure_models();
                            }
                        });
                    }
                    ModelState::Missing | ModelState::Unknown => {
                        if ui.small_button("Download the model").clicked() {
                            crate::beats::ensure_models();
                        }
                    }
                }
            }
        });
        ui.add_space(12.0);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;
            section_label(ui, "director");
            ui.small("What the auto-pilot is allowed to do when it cuts scenes.");
            ui.checkbox(&mut s.breakdown_mode, "Detect breakdowns")
                .on_hover_text("Quiet sections switch the show into calm mode");
            ui.checkbox(&mut s.cut_on_drops, "Cut early on a drop")
                .on_hover_text("A drop lands early — cut to the next scene with it");
            ui.checkbox(&mut s.random_order, "Random order")
                .on_hover_text("Shuffle the rotation instead of playing it in order");
            // External engine (the Unity shows in unity/): its unity_* scenes
            // join the rotation while its frames are arriving.
            ui.checkbox(&mut s.unity_link, "Unity engine link")
                .on_hover_text(
                    "Send the show state to the Unity engine (UDP 127.0.0.1:9137) and show                      its Spout output as the unity_* scenes. Start unity/TrippinStage first;                      Windows only for now.",
                );
            if s.unity_link {
                ui.small(if crate::EXT_LIVE.load(std::sync::atomic::Ordering::Relaxed) {
                    "Unity frames arriving."
                } else {
                    "Waiting for the Unity engine's Spout sender…"
                });
            }
        });
        ui.min_rect().height()
    };
    let right_h = {
        let ui = &mut mk_col(col_w + 12.0, col_w);
        card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            section_label(ui, "ai show builder");
            let saved = !s.ai_key.trim().is_empty();
            let env = s
                .ai_provider
                .env_keys()
                .iter()
                .any(|k| std::env::var(k).is_ok_and(|v| !v.trim().is_empty()));
            status_dot(
                ui,
                saved || env,
                if saved {
                    "key saved"
                } else if env {
                    "key from environment"
                } else {
                    "no key"
                },
            );
            ui.small("Used by the timeline editor's AI build (F2) to design cues for a show.");
            let field_w = (ui.available_width() - 112.0).max(80.0);
            egui::Grid::new("set_ai_grid")
                .num_columns(2)
                .min_col_width(96.0)
                .spacing(egui::vec2(8.0, 8.0))
                .show(ui, |ui| {
                    grow(ui, "Provider", |ui| {
                        egui::ComboBox::from_id_salt("ai_prov")
                            .width(field_w)
                            .selected_text(s.ai_provider.label())
                            .show_ui(ui, |ui| {
                                for p in crate::ai::AiProvider::ALL {
                                    if ui
                                        .selectable_label(s.ai_provider == p, p.label())
                                        .clicked()
                                    {
                                        crate::ai::set_provider(s, p);
                                    }
                                }
                            });
                    });
                    let def_ep = s.ai_provider.default_endpoint();
                    grow(ui, "Endpoint", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut s.ai_endpoint)
                                .hint_text(def_ep)
                                .desired_width(field_w),
                        );
                    });
                    let def_model = s.ai_provider.default_model();
                    grow(ui, "Model", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut s.ai_model)
                                .hint_text(def_model)
                                .desired_width(field_w),
                        );
                    });
                    grow(ui, "API key", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut s.ai_key)
                                .password(true)
                                .hint_text("or env var")
                                .desired_width(field_w),
                        );
                    });
                });
            ui.small(format!(
                "A blank key tries {}. A saved key lives in trippin.json.",
                s.ai_provider.env_keys().join(" / ")
            ));
            ui.add_enabled_ui(s.ai_provider == crate::ai::AiProvider::Anthropic, |ui| {
                ui.checkbox(&mut s.ai_web_search, "Look up each track online first")
                    .on_hover_text(
                        "The model searches the web for each track's genre, mood and \
                         hook words before planning. Anthropic only; up to 4 searches \
                         (about a cent each) per build.",
                    );
            });
        });
        ui.min_rect().height()
    };
    // Claim the taller column so the ScrollArea knows the page height.
    ui.add_space(left_h.max(right_h));
}

#[allow(clippy::too_many_arguments)]
fn timeline_tab(
    ui: &mut egui::Ui,
    tl_shared: &crate::timeline::Shared,
    saved: &mut SavedCache,
    cmd: &mut Vec<UiCommand>,
) {
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

    saved.refresh();

    // One card (review E): open-editor primary, transport, then saved list.
    t::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        t::section_label(ui, "timeline");

        // Primary: full-width Open button, 36px.
        let open = egui::Button::new(
            egui::RichText::new("Open timeline editor").size(13.0),
        )
        .fill(t::ACCENT_SEL)
        .stroke(egui::Stroke::new(1.0, t::ACCENT))
        .corner_radius(egui::CornerRadius::same(6))
        .min_size(egui::vec2(ui.available_width(), 36.0));
        if ui.add(open).clicked() {
            cmd.push(UiCommand::OpenEditor);
        }
        ui.add_space(4.0);

        if let Some(doc) = doc_opt.as_ref() {
            ui.label(
                egui::RichText::new(format!(
                    "{} — {} song{} · {} · {} cues{}",
                    doc.name,
                    doc.clips.len(),
                    if doc.clips.len() == 1 { "" } else { "s" },
                    fmt_time(doc.end_s()),
                    doc.cues.len(),
                    if *dirty { " · unsaved" } else { "" }
                ))
                .size(11.0)
                .color(t::MUTED),
            );
        } else {
            ui.label(
                egui::RichText::new("no timeline loaded")
                    .size(11.0)
                    .color(t::FAINT),
            );
        }
        ui.add_space(6.0);

        // Transport — painted round buttons (no font glyphs, review A2).
        ui.horizontal(|ui| {
            let playing = *mode == PlayMode::Playing;
            let can_play = doc_opt.is_some() && !*busy;
            if t::tr_btn(
                ui,
                if playing {
                    t::TrIcon::Pause
                } else {
                    t::TrIcon::Play
                },
                can_play,
            ) {
                cmd.push(UiCommand::Song(SongCtl::Toggle));
            }
            if t::tr_btn(ui, t::TrIcon::Stop, *mode != PlayMode::Stopped) {
                cmd.push(UiCommand::Song(SongCtl::Stop));
            }
            ui.label(
                egui::RichText::new(format!("{} {}", fmt_time(*pos_s), mode_str(*mode)))
                    .monospace()
                    .size(13.0)
                    .color(t::TEXT),
            );
            if *recording {
                let (r, _) = ui
                    .allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().circle_filled(r.center(), 4.0, t::DANGER);
                ui.label(egui::RichText::new("REC").size(11.0).color(t::DANGER));
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
            if *busy {
                ui.spinner();
            }
        });
        if !message.is_empty() {
            ui.label(egui::RichText::new(message.as_str()).size(11.0).color(t::MUTED));
        }

        // Saved timelines: name · song · length · last edited. Click loads,
        // double-click loads + opens the editor.
        ui.add_space(8.0);
        t::section_label(ui, "saved");
        if saved.rows.is_empty() {
            ui.label(
                egui::RichText::new(
                    "nothing saved yet — add tracks and lay out cues in the editor",
                )
                .size(11.0)
                .color(t::FAINT),
            );
        }
        let rows: Vec<(PathBuf, String, String, f64, String)> = saved
            .rows
            .iter()
            .map(|(p, m)| {
                (
                    p.clone(),
                    m.name.clone(),
                    m.song.clone(),
                    m.length_s,
                    m.edited.clone(),
                )
            })
            .collect();
        for (path, name, song, len, edited) in rows {
            let w = ui.available_width();
            let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 24.0), egui::Sense::click());
            let p = ui.painter();
            if resp.hovered() {
                p.rect_filled(r, 4.0, t::RAISED);
            }
            let cy = r.center().y;
            p.text(
                egui::pos2(r.min.x + 4.0, cy),
                egui::Align2::LEFT_CENTER,
                ellipsize(p, &name, &egui::FontId::monospace(12.0), w * 0.34),
                egui::FontId::monospace(12.0),
                t::TEXT,
            );
            p.text(
                egui::pos2(r.min.x + w * 0.38, cy),
                egui::Align2::LEFT_CENTER,
                ellipsize(p, &song, &egui::FontId::proportional(11.0), w * 0.26),
                egui::FontId::proportional(11.0),
                t::MUTED,
            );
            p.text(
                egui::pos2(r.min.x + w * 0.70, cy),
                egui::Align2::LEFT_CENTER,
                fmt_time(len),
                egui::FontId::monospace(10.5),
                t::FAINT,
            );
            p.text(
                egui::pos2(r.max.x - 6.0, cy),
                egui::Align2::RIGHT_CENTER,
                edited,
                egui::FontId::proportional(10.5),
                t::FAINT,
            );
            if resp.double_clicked() {
                cmd.push(UiCommand::LoadTimeline(path.clone()));
                cmd.push(UiCommand::OpenEditor);
            } else if resp.clicked() {
                cmd.push(UiCommand::LoadTimeline(path));
            }
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
