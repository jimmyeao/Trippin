//! Control panel: a second window (egui) for modes, scene playlist, dancer
//! options, sync and hotkey bindings. It shares the renderer's GPU device.
//!
//! Layout: a slim status header, a tab strip, then one tab of content at a
//! time — Show, Scenes, Dancer, Effects, Keys — so it stays tidy even with
//! 50+ scenes.

use std::path::PathBuf;
use std::sync::Arc;

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
    pub clip: Option<String>,
    pub blackout: bool,
    pub fullscreen: bool,
    /// The post effect actually on screen (the auto-pilot's pick in auto mode).
    pub fx: Fx,
    /// External output status line (NDI receiver count / error) — Some while
    /// output is enabled.
    pub output: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Show,
    Scenes,
    Dancer,
    Effects,
    Timeline,
    Keys,
}

impl Tab {
    const ALL: [Tab; 6] = [
        Tab::Show,
        Tab::Scenes,
        Tab::Dancer,
        Tab::Effects,
        Tab::Timeline,
        Tab::Keys,
    ];

    fn label(self) -> &'static str {
        match self {
            Tab::Show => "Show",
            Tab::Scenes => "Scenes",
            Tab::Dancer => "Dancer",
            Tab::Effects => "Effects",
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
            winit::dpi::PhysicalSize::new(480, 620),
            anchor,
            true, // floats above a fullscreen visuals window
        )?;
        let window = win.window.clone();
        Ok(Self {
            window,
            win,
            rebinding: None,
            tab: Tab::Show,
            scene_filter: String::new(),
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
    ) -> (Vec<UiCommand>, bool, PanelFrame) {
        let mut commands = Vec::new();
        let mut changed = false;

        let rebinding = &mut self.rebinding;
        let tab = &mut self.tab;
        let scene_filter = &mut self.scene_filter;
        let frame = self.win.frame(|ui| {
            ui.add_space(6.0);
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
    cmd: &mut Vec<UiCommand>,
) -> bool {
    let before = serde_json::to_string(s).unwrap_or_default();

    // --- Status header ---------------------------------------------------
    ui.horizontal(|ui| {
        ui.heading("Trippin");
        ui.separator();
        ui.label(format!("{:.0} fps", st.fps));
        ui.separator();
        ui.label(format!("{:.1} BPM", st.bpm));
        ui.separator();
        ui.label(format!("beat {}/4", st.beat_in_bar + 1));
        if st.silent {
            ui.colored_label(egui::Color32::from_rgb(255, 160, 60), "no signal");
        }
    });
    ui.horizontal(|ui| {
        ui.monospace(scenes.get(st.scene).map(String::as_str).unwrap_or("?"));
        ui.separator();
        ui.small(format!(
            "dancer {}  ·  {:.0}% conf  ·  {}",
            st.clip.as_deref().unwrap_or("off"),
            st.confidence * 100.0,
            st.device
        ));
    });
    ui.add_space(2.0);
    ui.separator();

    // --- Tabs -------------------------------------------------------------
    ui.horizontal(|ui| {
        for t in Tab::ALL {
            ui.selectable_value(tab, t, t.label());
        }
    });
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| match *tab {
            Tab::Show => show_tab(ui, s, st, cmd),
            Tab::Scenes => scenes_tab(ui, s, st, scenes, scene_heavy, heavy_ok, scene_filter, cmd),
            Tab::Dancer => dancer_tab(ui, s, st, clips, cmd),
            Tab::Effects => effects_tab(ui, s, st),
            Tab::Timeline => timeline_tab(ui, tl_shared, cmd),
            Tab::Keys => keys_tab(ui, s, rebinding),
        });

    serde_json::to_string(s).unwrap_or_default() != before
}

/// A labelled row of content with the label kept a fixed width.
fn row(ui: &mut egui::Ui, label: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized([92.0, 20.0], egui::Label::new(label));
        body(ui);
    });
}

fn show_tab(ui: &mut egui::Ui, s: &mut Settings, st: &Status, cmd: &mut Vec<UiCommand>) {
    row(ui, "Mode", |ui| {
        ui.selectable_value(&mut s.mode, Mode::Auto, "Auto");
        ui.selectable_value(&mut s.mode, Mode::Static, "Static");
        ui.selectable_value(&mut s.mode, Mode::Manual, "Manual");
    });
    ui.small(match s.mode {
        Mode::Auto => "Cuts scenes on phrases (and drops); the dancer follows the track.",
        Mode::Static => "Holds the current scene; the dancer still changes with the phrases.",
        Mode::Manual => "Nothing changes unless you change it.",
    });
    row(ui, "Scene", |ui| {
        if ui.button("◀ Prev").clicked() {
            cmd.push(UiCommand::Do(Action::PrevScene));
        }
        if ui.button("Next ▶").clicked() {
            cmd.push(UiCommand::Do(Action::NextScene));
        }
        ui.checkbox(&mut s.random_order, "Random order");
    });
    row(ui, "Length", |ui| {
        for bars in [4, 8, 16, 32] {
            ui.selectable_value(&mut s.phrase_bars, bars, format!("{bars}"));
        }
        ui.label("bars");
    });
    ui.checkbox(&mut s.cut_on_drops, "Cut early when a drop lands");
    row(ui, "Output", |ui| {
        let bo = if st.blackout {
            "Blackout: ON"
        } else {
            "Blackout"
        };
        if ui.button(bo).clicked() {
            cmd.push(UiCommand::Do(Action::Blackout));
        }
        let fs = if st.fullscreen {
            "Leave fullscreen"
        } else {
            "Fullscreen"
        };
        if ui.button(fs).clicked() {
            cmd.push(UiCommand::Do(Action::Fullscreen));
        }
    });
    ui.separator();
    ui.label(egui::RichText::new("Sync").strong());
    row(ui, "Latency", |ui| {
        ui.add(
            egui::Slider::new(&mut s.latency_ms, -100.0..=200.0)
                .step_by(5.0)
                .suffix(" ms"),
        );
    });
    ui.small("Raise it if the visuals land after the beat, lower it if they land before.");
    if ui
        .button("Mark this beat as the downbeat (the \"one\")")
        .clicked()
    {
        cmd.push(UiCommand::Do(Action::MarkDownbeat));
    }
    ui.separator();
    ui.label(egui::RichText::new("Network output").strong());
    row(ui, "NDI", |ui| {
        ui.checkbox(&mut s.ndi_enabled, "Send");
        ui.label("as");
        ui.add(egui::TextEdit::singleline(&mut s.ndi_name).desired_width(110.0));
    });
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
                "Sends the composited frame to OBS / other displays. \
                 Needs the free NDI runtime installed (NDI Tools).",
            );
        }
    }
}

fn scenes_tab(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    scenes: &[String],
    heavy: &[bool],
    heavy_ok: bool,
    filter: &mut String,
    cmd: &mut Vec<UiCommand>,
) {
    row(ui, "Seasonal", |ui| {
        ui.selectable_value(&mut s.seasonal, Seasonal::Auto, "Auto");
        ui.selectable_value(&mut s.seasonal, Seasonal::Always, "Always");
        ui.selectable_value(&mut s.seasonal, Seasonal::Off, "Off");
    });
    row(ui, "3D scenes", |ui| {
        ui.selectable_value(&mut s.heavy_scenes, Tristate::Auto, "Auto");
        ui.selectable_value(&mut s.heavy_scenes, Tristate::On, "On");
        ui.selectable_value(&mut s.heavy_scenes, Tristate::Off, "Off");
    });
    ui.small(match s.heavy_scenes {
        Tristate::Auto if heavy_ok => {
            "Raymarched scenes are in rotation — this GPU can handle them."
        }
        Tristate::Auto => "Raymarched scenes are off — this GPU can't keep up. Force them with On.",
        Tristate::On => "Raymarched scenes forced on — may drop frames on a weak GPU.",
        Tristate::Off => "Raymarched scenes are off.",
    });
    let heavy_on = match s.heavy_scenes {
        Tristate::Auto => heavy_ok,
        Tristate::On => true,
        Tristate::Off => false,
    };
    let date = today();
    let in_now: Vec<&str> = scenes
        .iter()
        .map(String::as_str)
        .filter(|n| in_season(n, date) == Some(true))
        .collect();
    ui.small(if in_now.is_empty() {
        "Nothing seasonal today.".to_string()
    } else {
        format!("In season: {}", in_now.join(", "))
    });
    ui.horizontal(|ui| {
        ui.label("Filter");
        ui.add(
            egui::TextEdit::singleline(filter)
                .desired_width(120.0)
                .hint_text("name…"),
        );
        if ui.small_button("all on").clicked() {
            s.disabled_scenes.clear();
        }
        if ui.small_button("all off").clicked() {
            s.disabled_scenes = scenes.to_vec();
        }
    });
    let blocked = (0..scenes.len())
        .filter(|&i| {
            heavy.get(i).copied().unwrap_or(false)
                && !heavy_on
                && !s.disabled_scenes.contains(&scenes[i])
        })
        .count();
    ui.small(format!(
        "{} of {} scenes in rotation",
        scenes.len() - s.disabled_scenes.len().min(scenes.len()) - blocked,
        scenes.len()
    ));
    ui.separator();
    let q = filter.to_lowercase();
    egui::Grid::new("scenes")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            for (i, name) in scenes.iter().enumerate() {
                if !q.is_empty() && !name.to_lowercase().contains(&q) {
                    continue;
                }
                let is_heavy = heavy.get(i).copied().unwrap_or(false);
                let off_gpu = is_heavy && !heavy_on;
                let mut on = !s.disabled_scenes.contains(name);
                let mut label = match in_season(name, date) {
                    Some(true) => format!("{name} (in season)"),
                    Some(false) => format!("{name} (out of season)"),
                    None => name.clone(),
                };
                if is_heavy {
                    label += if off_gpu {
                        " (3D — needs dGPU)"
                    } else {
                        " (3D)"
                    };
                }
                // Greyed out when the GPU can't run it — it can't join
                // rotation anyway, and "show" would just drop frames.
                if ui
                    .add_enabled(!off_gpu, egui::Checkbox::new(&mut on, label))
                    .changed()
                {
                    if on {
                        s.disabled_scenes.retain(|n| n != name);
                    } else {
                        s.disabled_scenes.push(name.clone());
                    }
                }
                let label = if i == st.scene { "▶" } else { "show" };
                if ui
                    .add_enabled(i != st.scene && !off_gpu, egui::Button::new(label).small())
                    .clicked()
                {
                    cmd.push(UiCommand::GoToScene(i));
                }
                ui.end_row();
            }
        });
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
        CueKind::Text(spec) => {
            use crate::text::{TextPos, TextStyle};
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
