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
use winit::window::{Icon, Window, WindowLevel};

use crate::config::{in_season, today, Action, Fx, Mode, Seasonal, Settings, Tristate};
use crate::dancer::STYLES;
use crate::render::Gpu;
use crate::timeline::{Cue, CueKind, PlayMode, SongCtl, Timeline};

/// A fully-tessellated egui frame, ready to be drawn without the settings lock.
pub struct PanelFrame {
    prims: Vec<egui::ClippedPrimitive>,
    textures_delta: egui::TexturesDelta,
    ppp: f32,
}

/// Things the panel asks the app to do (beyond editing settings directly).
pub enum UiCommand {
    Do(Action),
    GoToScene(usize),
    ShowClip(usize),
    /// Decode an audio file and build a fresh timeline for it.
    LoadSong(PathBuf),
    /// Open a saved timeline `.json`.
    LoadTimeline(PathBuf),
    /// Save the current timeline under `timelines/<name>.json`.
    SaveTimeline,
    /// Transport control for the song player.
    Song(SongCtl),
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
    const ALL: [Tab; 6] = [Tab::Show, Tab::Scenes, Tab::Dancer, Tab::Effects, Tab::Timeline, Tab::Keys];

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
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    ctx: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    /// Waiting for a key press to bind to this action.
    pub rebinding: Option<Action>,
    tab: Tab,
    /// Text filter for the scene list.
    scene_filter: String,
    /// Selected cue index in the timeline tab.
    tl_sel: Option<usize>,
    /// Cue kind chosen in the "add cue" picker (keeps its last params).
    tl_add: CueKind,
    /// Text field for an explicit song path (drag-drop is the fast path).
    song_path: String,
    /// Throttles configure retries after a failure.
    last_configure: std::time::Instant,
    /// False after an acquire failure — retry configure before acquiring again.
    surface_ok: bool,
    /// The error epoch this surface has already reconfigured for — see
    /// `Gpu::surface_epoch`.
    seen_epoch: u64,
}

impl Panel {
    pub fn new(event_loop: &ActiveEventLoop, gpu: Gpu, icon: Option<Icon>, anchor: Option<&Window>) -> anyhow::Result<Self> {
        let mut attrs = Window::default_attributes()
            .with_title("Trippin — control")
            .with_inner_size(winit::dpi::LogicalSize::new(480, 620))
            .with_window_icon(icon)
            // A floating console must never hide behind a fullscreen frame.
            .with_window_level(WindowLevel::AlwaysOnTop);
        // Spawn on the same monitor as the visuals window — the OS default
        // position can land on another screen or behind the fullscreen window.
        if let Some(main) = anchor {
            if let Some(mon) = main.current_monitor() {
                let mp = mon.position();
                let ms = mon.size();
                attrs = attrs.with_position(winit::dpi::PhysicalPosition::new(
                    mp.x + ms.width as i32 - 520,
                    mp.y + 24,
                ));
            }
        }
        // Deliberately NOT focused: the visuals keep keyboard focus so hotkeys
        // (incl. F1 itself) behave identically whether the panel is up or not.
        // Clicking the panel focuses it as usual.
        let window = Arc::new(event_loop.create_window(attrs)?);
        let surface = gpu.instance.create_surface(window.clone())?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| anyhow::anyhow!("panel surface unsupported"))?;
        // egui outputs gamma-space colours: prefer a non-sRGB target.
        let caps = surface.get_capabilities(&gpu.adapter);
        if let Some(f) = caps.formats.iter().find(|f| !f.is_srgb()) {
            config.format = *f;
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        {
            // The render thread submits continuously; configure must own the
            // gate so its wait-for-idle can see an empty queue.
            let _g = gpu.submit_gate.lock().unwrap_or_else(|e| e.into_inner());
            surface.configure(&gpu.device, &config);
        }

        let ctx = egui::Context::default();
        ctx.set_visuals(egui::Visuals::dark());
        let state = egui_winit::State::new(ctx.clone(), egui::ViewportId::ROOT, &window, None, None, None);
        let renderer = egui_wgpu::Renderer::new(&gpu.device, config.format, egui_wgpu::RendererOptions::default());
        Ok(Self {
            window,
            surface,
            config,
            gpu,
            ctx,
            state,
            renderer,
            rebinding: None,
            tab: Tab::Show,
            scene_filter: String::new(),
            tl_sel: None,
            tl_add: CueKind::NextScene,
            song_path: String::new(),
            last_configure: std::time::Instant::now(),
            surface_ok: true,
            seen_epoch: 0,
        })
    }

    /// Feed a window event to egui. Returns true if egui used it.
    pub fn on_event(&mut self, event: &WindowEvent) -> bool {
        let r = self.state.on_window_event(&self.window, event);
        if r.repaint {
            self.window.request_redraw();
        }
        r.consumed
    }

    pub fn wants_keyboard(&self) -> bool {
        self.ctx.egui_wants_keyboard_input()
    }

    /// Run the egui UI. Called while the settings lock is held — must not do
    /// any GPU work that can block (a stalled surface acquire here would
    /// freeze the render thread via the lock).
    pub fn run_ui(
        &mut self,
        settings: &mut Settings,
        status: &Status,
        scenes: &[String],
        clips: &[String],
        tl_shared: &crate::timeline::Shared,
    ) -> (Vec<UiCommand>, bool, PanelFrame) {
        let mut commands = Vec::new();
        let mut changed = false;

        let raw = self.state.take_egui_input(&self.window);
        let rebinding = &mut self.rebinding;
        let tab = &mut self.tab;
        let scene_filter = &mut self.scene_filter;
        let tl_sel = &mut self.tl_sel;
        let tl_add = &mut self.tl_add;
        let song_path = &mut self.song_path;
        let mut out = self.ctx.run_ui(raw, |ui| {
            ui.add_space(6.0);
            changed |= build_ui(
                ui, settings, status, scenes, clips, tl_shared, rebinding, tab, scene_filter,
                tl_sel, tl_add, song_path, &mut commands,
            );
        });
        self.state.handle_platform_output(&self.window, out.platform_output);
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        // TexturesDelta must be emptied before it's dropped (debug_assert).
        let textures_delta = std::mem::take(&mut out.textures_delta);
        (commands, changed, PanelFrame { prims, textures_delta, ppp: out.pixels_per_point })
    }

    /// Upload textures, acquire a surface frame and present — all without
    /// holding the settings lock.
    pub fn present(&mut self, frame_data: PanelFrame) {
        let PanelFrame { prims, mut textures_delta, ppp } = frame_data;

        // The surface must match the window's real size — inner_size() at
        // creation can return the requested logical size before the window is
        // realized, and a mismatch makes egui's scissors fail validation.
        let size = self.window.inner_size();
        if size.width > 0 && size.height > 0 && (size.width, size.height) != (self.config.width, self.config.height) {
            self.config.width = size.width;
            self.config.height = size.height;
            self.last_configure = std::time::Instant::now();
            {
                let _g = self.gpu.submit_gate.lock().unwrap_or_else(|e| e.into_inner());
                self.surface.configure(&self.gpu.device, &self.config);
            }
            self.surface_ok = true;
        }

        for (id, deltas) in textures_delta.set.drain() {
            for delta in deltas {
                self.renderer.update_texture(&self.gpu.device, &self.gpu.queue, id, &delta);
            }
        }
        // Drained up-front so early returns below never drop a non-empty delta.
        let freed: Vec<egui::TextureId> = textures_delta.free.drain().collect();

        // A failed configure is reported via the device's error callback —
        // asynchronously, and only drained by poll/submit. Polling here forces
        // the callback to run before we touch the surface: if configure failed,
        // the epoch has advanced and we reconfigure instead of calling
        // get_current_texture on an unconfigured surface (which panics).
        // catch_unwind below remains only as a last-resort guard.
        let _ = self.gpu.device.poll(wgpu::PollType::Poll);
        let epoch = self.gpu.surface_epoch.load(std::sync::atomic::Ordering::Relaxed);
        if epoch != self.seen_epoch {
            self.seen_epoch = epoch;
            self.surface_ok = false;
        }
        if !self.surface_ok {
            if self.last_configure.elapsed() > std::time::Duration::from_millis(250) {
                self.last_configure = std::time::Instant::now();
                {
                    let _g = self.gpu.submit_gate.lock().unwrap_or_else(|e| e.into_inner());
                    self.surface.configure(&self.gpu.device, &self.config);
                }
                self.surface_ok = true;
            }
            return;
        }
        let acquired = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.surface.get_current_texture()));
        let frame = match acquired {
            Ok(wgpu::CurrentSurfaceTexture::Success(f)) | Ok(wgpu::CurrentSurfaceTexture::Suboptimal(f)) => f,
            _ => {
                self.surface_ok = false;
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        // Size the screen from the acquired frame itself — the egui scissors
        // can never exceed the render target this way.
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [frame.texture.width(), frame.texture.height()],
            pixels_per_point: ppp,
        };
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        let extra = self.renderer.update_buffers(&self.gpu.device, &self.gpu.queue, &mut enc, &prims, &screen);
        {
            let pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("panel"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.05, b: 0.06, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.renderer.render(&mut pass.forget_lifetime(), &prims, &screen);
        }
        {
            // Serialize submit/present with surface configure — see
            // Gpu::submit_gate. Without it a configure can starve forever.
            let _g = self.gpu.submit_gate.lock().unwrap_or_else(|e| e.into_inner());
            self.gpu.queue.submit(extra.into_iter().chain([enc.finish()]));
            self.window.pre_present_notify();
            self.gpu.queue.present(frame);
        }
        for id in freed {
            self.renderer.free_texture(&id);
        }
    }
}

/// The panel layout. Returns true when settings changed (so they get saved).
fn build_ui(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    scenes: &[String],
    clips: &[String],
    tl_shared: &crate::timeline::Shared,
    rebinding: &mut Option<Action>,
    tab: &mut Tab,
    scene_filter: &mut String,
    tl_sel: &mut Option<usize>,
    tl_add: &mut CueKind,
    song_path: &mut String,
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
            Tab::Scenes => scenes_tab(ui, s, st, scenes, scene_filter, cmd),
            Tab::Dancer => dancer_tab(ui, s, st, clips, cmd),
            Tab::Effects => effects_tab(ui, s, st),
            Tab::Timeline => timeline_tab(ui, tl_shared, scenes, clips, tl_sel, tl_add, song_path, cmd),
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
        let bo = if st.blackout { "Blackout: ON" } else { "Blackout" };
        if ui.button(bo).clicked() {
            cmd.push(UiCommand::Do(Action::Blackout));
        }
        let fs = if st.fullscreen { "Leave fullscreen" } else { "Fullscreen" };
        if ui.button(fs).clicked() {
            cmd.push(UiCommand::Do(Action::Fullscreen));
        }
    });
    ui.separator();
    ui.label(egui::RichText::new("Sync").strong());
    row(ui, "Latency", |ui| {
        ui.add(egui::Slider::new(&mut s.latency_ms, -100.0..=200.0).step_by(5.0).suffix(" ms"));
    });
    ui.small("Raise it if the visuals land after the beat, lower it if they land before.");
    if ui.button("Mark this beat as the downbeat (the \"one\")").clicked() {
        cmd.push(UiCommand::Do(Action::MarkDownbeat));
    }
}

fn scenes_tab(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    scenes: &[String],
    filter: &mut String,
    cmd: &mut Vec<UiCommand>,
) {
    row(ui, "Seasonal", |ui| {
        ui.selectable_value(&mut s.seasonal, Seasonal::Auto, "Auto");
        ui.selectable_value(&mut s.seasonal, Seasonal::Always, "Always");
        ui.selectable_value(&mut s.seasonal, Seasonal::Off, "Off");
    });
    let date = today();
    let in_now: Vec<&str> =
        scenes.iter().map(String::as_str).filter(|n| in_season(n, date) == Some(true)).collect();
    ui.small(if in_now.is_empty() {
        "Nothing seasonal today.".to_string()
    } else {
        format!("In season: {}", in_now.join(", "))
    });
    ui.horizontal(|ui| {
        ui.label("Filter");
        ui.add(egui::TextEdit::singleline(filter).desired_width(120.0).hint_text("name…"));
        if ui.small_button("all on").clicked() {
            s.disabled_scenes.clear();
        }
        if ui.small_button("all off").clicked() {
            s.disabled_scenes = scenes.to_vec();
        }
    });
    ui.small(format!("{} of {} scenes in rotation", scenes.len() - s.disabled_scenes.len().min(scenes.len()), scenes.len()));
    ui.separator();
    let q = filter.to_lowercase();
    egui::Grid::new("scenes").num_columns(2).striped(true).show(ui, |ui| {
        for (i, name) in scenes.iter().enumerate() {
            if !q.is_empty() && !name.to_lowercase().contains(&q) {
                continue;
            }
            let mut on = !s.disabled_scenes.contains(name);
            let label = match in_season(name, date) {
                Some(true) => format!("{name} (in season)"),
                Some(false) => format!("{name} (out of season)"),
                None => name.clone(),
            };
            if ui.checkbox(&mut on, label).changed() {
                if on {
                    s.disabled_scenes.retain(|n| n != name);
                } else {
                    s.disabled_scenes.push(name.clone());
                }
            }
            let label = if i == st.scene { "▶" } else { "show" };
            if ui.add_enabled(i != st.scene, egui::Button::new(label).small()).clicked() {
                cmd.push(UiCommand::GoToScene(i));
            }
            ui.end_row();
        }
    });
}

fn dancer_tab(ui: &mut egui::Ui, s: &mut Settings, st: &Status, clips: &[String], cmd: &mut Vec<UiCommand>) {
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
    ui.separator();
    ui.small("Auto-pilot picks among the ticked routines:");
    egui::Grid::new("clips").num_columns(2).striped(true).show(ui, |ui| {
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
            if ui.add_enabled(!showing, egui::Button::new(if showing { "▶" } else { "show" }).small()).clicked() {
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
    egui::Grid::new("keys").num_columns(3).striped(true).show(ui, |ui| {
        for a in Action::ALL {
            ui.label(a.label());
            let key = s.keys.get(&a).cloned().unwrap_or_default();
            if *rebinding == Some(a) {
                ui.colored_label(egui::Color32::YELLOW, "press a key…");
                if ui.button("Cancel").clicked() {
                    *rebinding = None;
                }
            } else {
                ui.monospace(if key.is_empty() { "—".to_string() } else { key });
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

fn fmt_time(t: f64) -> String {
    let t = t.max(0.0);
    format!("{}:{:04.1}", (t / 60.0) as u64, t % 60.0)
}

fn cue_color(k: &CueKind) -> egui::Color32 {
    use egui::Color32;
    match k {
        CueKind::Scene(_) | CueKind::NextScene | CueKind::PrevScene => Color32::from_rgb(160, 95, 250),
        CueKind::Fx(_) | CueKind::FxAuto(_) => Color32::from_rgb(70, 200, 220),
        CueKind::Dancer(_)
        | CueKind::Clip(_)
        | CueKind::NextClip
        | CueKind::NextLook
        | CueKind::Look(_) => Color32::from_rgb(90, 210, 130),
        CueKind::Canon(_) => Color32::from_rgb(150, 220, 90),
        CueKind::Blackout(_) => Color32::from_rgb(240, 90, 90),
        CueKind::Mode(_) => Color32::from_rgb(240, 175, 70),
    }
}

/// Pick a string from `opts` — returns true when the value changed.
fn pick_str(ui: &mut egui::Ui, id: egui::Id, cur: &mut String, opts: &[String]) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .width(110.0)
        .selected_text(if cur.is_empty() { "pick…" } else { cur.as_str() })
        .show_ui(ui, |ui| {
            for o in opts {
                changed |= ui.selectable_value(cur, o.clone(), o).changed();
            }
        });
    changed
}

/// Param editors for a cue kind. Returns true when it changed.
fn cue_param_ui(ui: &mut egui::Ui, kind: &mut CueKind, scenes: &[String], clips: &[String], id: egui::Id) -> bool {
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
            egui::ComboBox::from_id_salt(id.with("fx")).width(100.0).selected_text(f.label()).show_ui(
                ui,
                |ui| {
                    for v in Fx::ALL {
                        changed |= ui.selectable_value(f, v, v.label()).changed();
                    }
                },
            );
            changed
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn timeline_tab(
    ui: &mut egui::Ui,
    tl_shared: &crate::timeline::Shared,
    scenes: &[String],
    clips: &[String],
    sel: &mut Option<usize>,
    add_kind: &mut CueKind,
    song_path: &mut String,
    cmd: &mut Vec<UiCommand>,
) {
    use egui::{pos2, vec2, Align2, Color32, FontId, Sense, Shape, Stroke};

    let mut guard = tl_shared.lock().unwrap_or_else(|e| e.into_inner());
    // Destructure so closures borrow disjoint fields.
    let crate::timeline::TimelineState {
        doc: doc_opt,
        mode,
        pos_s,
        cursor_s,
        recording,
        autosync,
        live_locked,
        live_score,
        dirty,
        snap,
        message,
        busy,
    } = &mut *guard;

    // --- Load ---------------------------------------------------------------
    ui.horizontal(|ui| {
        ui.label("Track");
        ui.add(egui::TextEdit::singleline(song_path).desired_width(150.0).hint_text("mp3 / flac / wav / m4a…"));
        if ui
            .add_enabled(!*busy, egui::Button::new("Load"))
            .on_hover_text("Decode and analyse — a few seconds")
            .clicked()
            && !song_path.trim().is_empty()
        {
            cmd.push(UiCommand::LoadSong(PathBuf::from(song_path.trim())));
        }
        let saved = Timeline::list(&crate::config::timelines_dir());
        if !saved.is_empty() {
            egui::ComboBox::from_id_salt("tl_open").selected_text("open saved…").show_ui(ui, |ui| {
                for p in saved {
                    let stem = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    if ui.selectable_label(false, &stem).clicked() {
                        cmd.push(UiCommand::LoadTimeline(p.clone()));
                    }
                }
            });
        }
    });
    ui.small("…or drop an audio file / timeline .json on either window.");
    if *busy {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("decoding — a few seconds");
        });
    }
    if !message.is_empty() {
        ui.small(message.as_str());
    }
    let Some(doc) = doc_opt.as_mut() else {
        if doc_opt.is_none() && !*busy {
            ui.add_space(6.0);
            ui.label("No timeline yet — load a track to start one.");
        }
        return;
    };

    // --- Header + transport ---------------------------------------------------
    let dur = doc.duration.max(0.001);
    let beats_total = doc.total_beats().max(1.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&doc.name).strong());
        ui.small(format!("{dur_txt} · {bpm:.1} BPM · {beats:.0} beats{uns}",
            dur_txt = fmt_time(dur),
            bpm = doc.bpm,
            beats = beats_total,
            uns = if *dirty { "  •" } else { "" },
        ));
    });
    ui.horizontal(|ui| {
        let playing = *mode == PlayMode::Playing;
        if ui
            .add_enabled(!*busy, egui::Button::new(if playing { "⏸  Pause" } else { "▶  Play" }))
            .clicked()
        {
            cmd.push(UiCommand::Song(SongCtl::Toggle));
        }
        if ui
            .add_enabled(*mode != PlayMode::Stopped, egui::Button::new("⏹"))
            .on_hover_text("Stop — live audio resumes")
            .clicked()
        {
            cmd.push(UiCommand::Song(SongCtl::Stop));
        }
        let rec_txt = egui::RichText::new(if *recording { "● REC" } else { "● rec" })
            .color(if *recording { Color32::from_rgb(255, 80, 80) } else { Color32::GRAY });
        if ui.toggle_value(recording, rec_txt)
            .on_hover_text("Record your hotkeys/panel clicks as cues while it plays")
            .changed()
            && *recording
            && *mode != PlayMode::Playing
        {
            ui.ctx().request_repaint();
        }
        ui.checkbox(snap, "snap ¼");
        ui.checkbox(autosync, "follow live")
            .on_hover_text("When stopped: recognise this track in the room and cue it live");
        if *autosync && *mode != PlayMode::Playing {
            if *live_locked {
                ui.colored_label(Color32::from_rgb(120, 230, 130), format!("locked {:.0}%", *live_score * 100.0));
            } else {
                ui.colored_label(Color32::GRAY, "listening…");
            }
        }
    });
    {
        let beat = (doc.bpm / 60.0) * (*pos_s - doc.first_beat);
        let tag = match *mode {
            PlayMode::Playing => "playing",
            PlayMode::Paused => "paused",
            PlayMode::Stopped if *live_locked => "live match",
            PlayMode::Stopped => "",
        };
        ui.monospace(format!("{}  ·  beat {:.2}  {}", fmt_time(*pos_s), beat.max(0.0), tag));
    }

    // --- The strip ------------------------------------------------------------
    let strip_h = 132.0f32;
    let lane_h = 14.0f32;
    let axis_h = 14.0f32;
    let (rect, strip_resp) =
        ui.allocate_exact_size(vec2(ui.available_width().max(80.0), strip_h), Sense::click_and_drag());
    let p = ui.painter_at(rect);
    let wave_mid = rect.top() + lane_h + (strip_h - lane_h - axis_h) * 0.5;
    let wave_half = (strip_h - lane_h - axis_h) * 0.47;
    // Beat↔time as plain closures over copies — keeps `doc` free for editing.
    let (fb, bps) = (doc.first_beat, doc.beats_per_sec());
    let dur_f = dur;
    let x_at = |t: f64| rect.left() + (t / dur_f) as f32 * rect.width();
    let beat_at = |t: f64| (t - fb) * bps;
    let time_at = |b: f64| fb + b / bps;
    let b_at_x = |px: f32| beat_at(((px - rect.left()) as f64 / rect.width() as f64 * dur_f).clamp(0.0, dur_f));
    p.rect_filled(rect, 3.0, Color32::from_gray(18));

    if !doc.overview.is_empty() {
        let n = doc.overview.len();
        for (i, &a) in doc.overview.iter().enumerate() {
            let x = x_at(i as f64 / n as f64 * dur);
            let a = a.min(1.0) * wave_half;
            p.line_segment(
                [pos2(x, wave_mid - a), pos2(x, wave_mid + a)],
                Stroke::new(1.0, Color32::from_gray(85)),
            );
        }
    }
    // Bar lines every 4 beats, stronger each 4 bars, numbered.
    for bar in 0..=(beats_total / 4.0).ceil() as i64 {
        let t = time_at(bar as f64 * 4.0);
        if t > dur {
            break;
        }
        let x = x_at(t);
        let strong = bar % 4 == 0;
        p.line_segment(
            [pos2(x, rect.top() + lane_h), pos2(x, rect.bottom() - axis_h)],
            Stroke::new(1.0, Color32::from_gray(if strong { 70 } else { 40 })),
        );
        if strong {
            p.text(
                pos2(x + 3.0, rect.bottom() - axis_h + 1.0),
                Align2::LEFT_TOP,
                format!("{}", bar + 1),
                FontId::proportional(9.0),
                Color32::from_gray(115),
            );
        }
    }

    // Cue markers — clickable/draggable triangles in the top lane.
    let mut to_delete: Option<usize> = None;
    // The dragged cue gets resorted after the loop; remember which it was.
    let mut resort_sel: Option<(f64, &'static str)> = None;
    for (i, cue) in doc.cues.iter_mut().enumerate() {
        let x = x_at(time_at(cue.beat));
        let hit = egui::Rect::from_min_size(pos2(x - 6.0, rect.top() - 1.0), vec2(12.0, lane_h + 8.0));
        let r = ui.interact(hit, egui::Id::new(("tlcue", i)), Sense::click_and_drag());
        if r.dragged() {
            let db = r.drag_delta().x as f64 * beats_total / rect.width() as f64;
            cue.beat = (cue.beat + db).clamp(0.0, beats_total);
            *dirty = true;
            *sel = Some(i);
        }
        if r.drag_stopped() {
            if *snap {
                cue.beat = (cue.beat * 4.0).round() / 4.0;
            }
            resort_sel = Some((cue.beat, cue.kind.label()));
        }
        if r.clicked() {
            *sel = Some(i);
            *cursor_s = time_at(cue.beat);
        }
        if r.secondary_clicked() {
            to_delete = Some(i);
        }
        let mut col = cue_color(&cue.kind);
        if *sel == Some(i) || r.hovered() {
            col = Color32::WHITE;
            p.text(
                pos2(x, rect.top() + lane_h + 2.0),
                Align2::CENTER_TOP,
                format!("{} {}", cue.kind.label(), cue.kind.detail()),
                FontId::proportional(9.0),
                Color32::WHITE,
            );
        }
        p.add(Shape::convex_polygon(
            vec![pos2(x - 5.0, rect.top()), pos2(x + 5.0, rect.top()), pos2(x, rect.top() + 7.0)],
            col,
            Stroke::NONE,
        ));
        p.line_segment(
            [pos2(x, rect.top() + 7.0), pos2(x, rect.bottom() - axis_h)],
            Stroke::new(1.0, cue_color(&cue.kind).gamma_multiply(0.55)),
        );
    }
    if let Some((beat, label)) = resort_sel {
        doc.sort_cues();
        *sel = doc
            .cues
            .iter()
            .position(|c| c.beat == beat && c.kind.label() == label)
            .or(*sel);
    }
    if let Some(i) = to_delete {
        doc.cues.remove(i);
        *dirty = true;
        if *sel == Some(i) {
            *sel = None;
        }
    }

    // Clicking/dragging empty strip seeks: the playhead while running, the
    // edit cursor while stopped.
    if (strip_resp.clicked() || strip_resp.dragged()) && to_delete.is_none() {
        if let Some(pos) = strip_resp.interact_pointer_pos() {
            let mut b = b_at_x(pos.x);
            if *snap {
                b = (b * 4.0).round() / 4.0;
            }
            let t = time_at(b).clamp(0.0, dur);
            *cursor_s = t;
            if *mode != PlayMode::Stopped {
                cmd.push(UiCommand::Song(SongCtl::Seek(t)));
            } else {
                *pos_s = t;
            }
            *sel = None;
        }
    }

    // Playhead: the transport position while playing/paused, live-match
    // position while locked, else the edit cursor.
    let head_s = if *mode != PlayMode::Stopped || *live_locked { *pos_s } else { *cursor_s };
    let hx = x_at(head_s.clamp(0.0, dur));
    p.line_segment(
        [pos2(hx, rect.top()), pos2(hx, rect.bottom() - axis_h)],
        Stroke::new(1.5, Color32::from_rgb(230, 70, 70)),
    );

    // --- Add / edit ----------------------------------------------------------
    ui.horizontal(|ui| {
        ui.label("add");
        egui::ComboBox::from_id_salt("tl_add_kind")
            .width(96.0)
            .selected_text(add_kind.label())
            .show_ui(ui, |ui| {
                for k in CueKind::picker() {
                    if ui
                        .selectable_label(
                            std::mem::discriminant(&k) == std::mem::discriminant(add_kind),
                            k.label(),
                        )
                        .clicked()
                    {
                        *add_kind = k;
                    }
                }
            });
        cue_param_ui(ui, add_kind, scenes, clips, egui::Id::new("tl_add_param"));
        if ui.button("+ at cursor").clicked() {
            let mut kind = add_kind.clone();
            match &mut kind {
                CueKind::Scene(n) if n.is_empty() => *n = scenes.first().cloned().unwrap_or_default(),
                CueKind::Clip(n) if n.is_empty() => *n = clips.first().cloned().unwrap_or_default(),
                _ => {}
            }
            let mut b = doc.beat_at(*cursor_s).max(0.0);
            if *snap {
                b = (b * 4.0).round() / 4.0;
            }
            doc.cues.push(Cue { beat: b, kind });
            doc.sort_cues();
            *dirty = true;
        }
    });

    if let Some(i) = *sel {
        if i < doc.cues.len() {
            let cue = &mut doc.cues[i];
            ui.horizontal(|ui| {
                ui.label("cue");
                if ui
                    .add(egui::DragValue::new(&mut cue.beat).speed(0.25).range(0.0..=beats_total))
                    .changed()
                {
                    *dirty = true;
                }
                if cue_param_ui(ui, &mut cue.kind, scenes, clips, egui::Id::new("tl_sel_param")) {
                    *dirty = true;
                }
                if ui.small_button("✕").on_hover_text("delete cue").clicked() {
                    to_delete = Some(i);
                }
            });
        } else {
            *sel = None;
        }
    }
    if let Some(i) = to_delete.filter(|i| *i < doc.cues.len()) {
        doc.cues.remove(i);
        *dirty = true;
        *sel = None;
    }

    // --- Save + cue list ------------------------------------------------------
    ui.horizontal(|ui| {
        ui.label("name");
        ui.add(egui::TextEdit::singleline(&mut doc.name).desired_width(130.0));
        if ui.button("Save").on_hover_text("timelines/<name>.json").clicked() {
            cmd.push(UiCommand::SaveTimeline);
        }
        ui.small(format!("{} cues", doc.cues.len()));
    });
    ui.separator();
    let mut row_del: Option<usize> = None;
    egui::Grid::new("tl_cues").num_columns(4).striped(true).show(ui, |ui| {
        for (i, c) in doc.cues.iter().enumerate() {
            let r = ui.selectable_label(
                *sel == Some(i),
                egui::RichText::new(format!("{:>7.2}", c.beat)).monospace(),
            );
            if r.clicked() {
                *sel = Some(i);
                *cursor_s = doc.time_at(c.beat);
            }
            ui.colored_label(cue_color(&c.kind), c.kind.label());
            ui.small(c.kind.detail());
            if ui.small_button("✕").clicked() {
                row_del = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = row_del {
        doc.cues.remove(i);
        *dirty = true;
        if *sel == Some(i) {
            *sel = None;
        }
    }
}
