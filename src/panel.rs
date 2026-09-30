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
    const ALL: [Tab; 5] = [
        Tab::Perform,
        Tab::DancerFx,
        Tab::Stream,
        Tab::Timeline,
        Tab::Keys,
    ];

    fn label(self) -> &'static str {
        match self {
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
            tab: Tab::Perform,
            scene_filter: String::new(),
            chip: LibChip::All,
            thumbs: HashMap::new(),
            want_thumbs: HashMap::new(),
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
            clips,
            scene_filter,
            chip,
            thumbs,
            want_thumbs,
            cmd,
        ),
        _ => {
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| match *tab {
                    Tab::DancerFx => {
                        dancer_tab(ui, s, st, clips, cmd);
                        ui.separator();
                        effects_tab(ui, s, st);
                    }
                    Tab::Stream => stream_tab(ui, s, st, cmd),
                    Tab::Timeline => timeline_tab(ui, tl_shared, cmd),
                    Tab::Keys => keys_tab(ui, s, rebinding),
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
    clips: &[String],
    filter: &mut String,
    chip: &mut LibChip,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
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
            inspector(ui, s, st, scenes, clips, heavy_ok, thumbs, want, cmd);
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

    // Toolbar: search, filter chips, all on/off.
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(filter)
                .desired_width(150.0)
                .hint_text("search scenes"),
        );
        for (c, l) in [
            (LibChip::All, "All"),
            (LibChip::Flat, "2D"),
            (LibChip::Heavy, "3D"),
            (LibChip::Seasonal, "Seasonal"),
        ] {
            ui.selectable_value(chip, c, l);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("all off").clicked() {
                s.disabled_scenes = scenes.to_vec();
            }
            if ui.small_button("all on").clicked() {
                s.disabled_scenes.clear();
            }
        });
    });
    ui.label(
        egui::RichText::new(format!(
            "{} shown · {} of {} in rotation",
            shown.len(),
            in_rotation,
            scenes.len()
        ))
        .size(11.0)
        .color(MUTED),
    );

    // Thumbnail grid — reflows with the column width.
    let avail = ui.available_width();
    let cols = ((avail + 10.0) / 160.0).floor().max(2.0) as usize;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
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
    clips: &[String],
    heavy_ok: bool,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want: &mut HashMap<String, Instant>,
    cmd: &mut Vec<UiCommand>,
) {
    use crate::ui_theme::*;
    let _ = clips;
    let scroll_h = (ui.available_height() - 44.0).max(80.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(scroll_h)
        .show(ui, |ui| {
            // Live scene preview.
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
                p.rect_filled(rect, 4.0, RAISED);
            }
            p.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, BORDER),
                egui::StrokeKind::Inside,
            );

            ui.add_space(4.0);
            ui.label(egui::RichText::new(&name).monospace().size(16.0).strong());
            if st.bars_total > 0 {
                ui.label(
                    egui::RichText::new(bar_progress_line(st))
                        .size(11.0)
                        .color(MUTED),
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
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("◀ prev").clicked() {
                    cmd.push(UiCommand::Do(Action::PrevScene));
                }
                if ui.add(egui::Button::new("next ▶").fill(ACCENT_SEL)).clicked() {
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
                    ui.label(
                        egui::RichText::new(st.clip.as_deref().unwrap_or("off"))
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

    ui.separator();
    ui.horizontal(|ui| {
        let bo = if st.blackout { "Blackout: ON" } else { "Blackout" };
        if ui.add_sized([110.0, 26.0], egui::Button::new(bo)).clicked() {
            cmd.push(UiCommand::Do(Action::Blackout));
        }
        key_badge(ui, &key_short(s.keys.get(&Action::Blackout).map_or("", String::as_str)));
        let fs = if st.fullscreen {
            "Leave fullscreen"
        } else {
            "Fullscreen"
        };
        if ui.add_sized([112.0, 26.0], egui::Button::new(fs)).clicked() {
            cmd.push(UiCommand::Do(Action::Fullscreen));
        }
        key_badge(ui, &key_short(s.keys.get(&Action::Fullscreen).map_or("", String::as_str)));
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

fn stream_tab(ui: &mut egui::Ui, s: &mut Settings, st: &Status, cmd: &mut Vec<UiCommand>) {
    use crate::nowplaying::NpSource;
    ui.label(egui::RichText::new("Now playing").strong());
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
    match &st.np_track {
        Some(t) => ui.label(format!("♪ {t}")),
        None => ui.weak("No track yet"),
    };
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

    ui.separator();
    ui.label(egui::RichText::new("Recording").strong());
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

    ui.separator();
    ui.label(egui::RichText::new("Branding").strong());
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

    ui.separator();
    ui.label(egui::RichText::new("Ticker").strong());
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

    ui.separator();
    ui.label(egui::RichText::new("Video output (OBS)").strong());
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
    match &st.output {
        Some(line) => {
            ui.small(line);
        }
        None => {
            ui.small(
                "Sends the finished frame (overlays included) to OBS or another display. \
                 Spout for OBS on this PC, NDI across the network.",
            );
        }
    }
}


fn dancer_tab(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    clips: &[String],
    cmd: &mut Vec<UiCommand>,
) {
    ui.checkbox(&mut s.dancer_enabled, "Dancer layer on");
    row(ui, "Look", |ui| {
        ui.selectable_value(&mut s.dancer_style, None, "Auto");
        for (i, name) in STYLES.iter().enumerate() {
            ui.selectable_value(&mut s.dancer_style, Some(i), *name);
        }
    });
    row(ui, "Canon", |ui| {
        ui.selectable_value(&mut s.canon, Tristate::Auto, "Auto");
        ui.selectable_value(&mut s.canon, Tristate::On, "On");
        ui.selectable_value(&mut s.canon, Tristate::Off, "Off");
        ui.small("three dancers");
    });
    row(ui, "Size", |ui| {
        ui.add(egui::Slider::new(&mut s.dancer_size, 0.4..=1.0));
    });
    ui.checkbox(
        &mut s.dancer_trails,
        "Motion trails (echoes behind the dancer)",
    );
    ui.separator();
    ui.small("Auto-pilot picks among the ticked routines:");
    egui::Grid::new("clips")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            for (i, name) in clips.iter().enumerate() {
                let mut on = !s.disabled_clips.contains(name);
                if ui.checkbox(&mut on, name).changed() {
                    if on {
                        s.disabled_clips.retain(|n| n != name);
                    } else {
                        s.disabled_clips.push(name.clone());
                    }
                }
                let showing = st.clip.as_deref() == Some(name.as_str());
                if ui
                    .add_enabled(
                        !showing,
                        egui::Button::new(if showing { "▶" } else { "show" }).small(),
                    )
                    .clicked()
                {
                    cmd.push(UiCommand::ShowClip(i));
                }
                ui.end_row();
            }
        });
}

fn effects_tab(ui: &mut egui::Ui, s: &mut Settings, st: &Status) {
    ui.small("Whole-frame transforms — they apply live, so what you pick here is the preview.");
    ui.add_space(4.0);
    egui::Grid::new("fx").num_columns(3).show(ui, |ui| {
        for (i, f) in Fx::ALL.iter().enumerate() {
            if ui.selectable_value(&mut s.fx, *f, f.label()).clicked() {
                s.fx_auto = false;
            }
            if i % 3 == 2 {
                ui.end_row();
            }
        }
    });
    row(ui, "Strength", |ui| {
        ui.add_enabled(s.fx != Fx::Off, egui::Slider::new(&mut s.fx_amt, 0.0..=1.0));
    });
    ui.checkbox(&mut s.fx_auto, "Auto — a fresh effect on every scene cut");
    if s.fx_auto {
        ui.small(format!("On screen now: {}", st.fx.label()));
    } else if s.fx == Fx::MirrorY {
        ui.small("Mirror Y flips the frame top-to-bottom — dancers end up upside-down; it's manual-only, auto never picks it.");
    }
}

fn keys_tab(ui: &mut egui::Ui, s: &mut Settings, rebinding: &mut Option<Action>) {
    ui.small("Keys work in both windows. Click Rebind, then press the new key.");
    ui.add_space(4.0);
    egui::Grid::new("keys")
        .num_columns(3)
        .striped(true)
        .show(ui, |ui| {
            for a in Action::ALL {
                ui.label(a.label());
                let key = s.keys.get(&a).cloned().unwrap_or_default();
                if *rebinding == Some(a) {
                    ui.colored_label(egui::Color32::YELLOW, "press a key…");
                    if ui.button("Cancel").clicked() {
                        *rebinding = None;
                    }
                } else {
                    ui.monospace(if key.is_empty() {
                        "—".to_string()
                    } else {
                        key
                    });
                    if ui.button("Rebind").clicked() {
                        *rebinding = Some(a);
                    }
                }
                ui.end_row();
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
    use egui::Color32;
    match k {
        CueKind::Scene(_) | CueKind::NextScene | CueKind::PrevScene => {
            Color32::from_rgb(160, 95, 250)
        }
        CueKind::Fx(_) | CueKind::FxAuto(_) => Color32::from_rgb(70, 200, 220),
        CueKind::Dancer(_)
        | CueKind::Clip(_)
        | CueKind::NextClip
        | CueKind::NextLook
        | CueKind::Look(_)
        | CueKind::Trails(_) => Color32::from_rgb(90, 210, 130),
        CueKind::Canon(_) => Color32::from_rgb(150, 220, 90),
        CueKind::Blackout(_) => Color32::from_rgb(240, 90, 90),
        CueKind::Mode(_) => Color32::from_rgb(240, 175, 70),
        CueKind::Palette(_) => Color32::from_rgb(235, 60, 160),
        CueKind::Text(_) | CueKind::TextOff(_) => Color32::from_rgb(240, 140, 200),
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

    ui.horizontal(|ui| {
        if ui.button("Open timeline editor").clicked() {
            cmd.push(UiCommand::OpenEditor);
        }
        if let Some(doc) = doc_opt.as_mut() {
            ui.label(egui::RichText::new(&doc.name).strong());
            ui.small(format!(
                "{} song{} · {} · {} cues{}",
                doc.clips.len(),
                if doc.clips.len() == 1 { "" } else { "s" },
                fmt_time(doc.end_s()),
                doc.cues.len(),
                if *dirty { " · unsaved" } else { "" }
            ));
        }
    });

    // --- Transport ----------------------------------------------------------
    ui.horizontal(|ui| {
        let play_label = if *mode == PlayMode::Playing {
            "⏸ Pause"
        } else {
            "▶ Play"
        };
        if ui
            .add_enabled(doc_opt.is_some() && !*busy, egui::Button::new(play_label))
            .clicked()
        {
            cmd.push(UiCommand::Song(SongCtl::Toggle));
        }
        if ui
            .add_enabled(*mode != PlayMode::Stopped, egui::Button::new("⏹"))
            .clicked()
        {
            cmd.push(UiCommand::Song(SongCtl::Stop));
        }
        ui.label(format!("{} {}", fmt_time(*pos_s), mode_str(*mode)));
        if *recording {
            ui.colored_label(egui::Color32::from_rgb(255, 80, 80), "● REC");
        }
        if *autosync {
            ui.label(if *live_locked {
                "live: LOCKED"
            } else {
                "live: listening…"
            });
            ui.small(format!("{:.2}", *live_score));
        }
    });

    // --- Songs on the timeline ----------------------------------------------
    if let Some(doc) = doc_opt.as_mut() {
        if !doc.clips.is_empty() {
            ui.add_space(4.0);
            ui.label("Songs:");
        }
        for c in &doc.clips {
            ui.horizontal(|ui| {
                ui.small(format!("@{}", fmt_time(c.offset_s)));
                ui.label(&c.name);
                ui.small(format!("{} · {:.1} BPM", fmt_time(c.duration_s), c.bpm));
            });
        }
    }

    // --- Save / open ---------------------------------------------------------
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if let Some(doc) = doc_opt.as_mut() {
            ui.label("name");
            ui.add(egui::TextEdit::singleline(&mut doc.name).desired_width(110.0));
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
    ui.small("Add tracks and lay out cues in the editor — or drop an audio file / timeline .json on any window.");
    if *busy {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("decoding — a few seconds");
        });
    }
    if !message.is_empty() {
        ui.small(message.as_str());
    }
}

fn mode_str(m: PlayMode) -> &'static str {
    match m {
        PlayMode::Playing => "playing",
        PlayMode::Paused => "paused",
        PlayMode::Stopped => "stopped",
    }
}
