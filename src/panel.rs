//! Control panel: a second window (egui) for modes, scene playlist, dancer
//! options, sync and hotkey bindings. It shares the renderer's GPU device.
//!
//! Layout: a slim status header, a tab strip, then one tab of content at a
//! time — Show, Scenes, Dancer, Effects, Keys — so it stays tidy even with
//! 50+ scenes.

use std::sync::Arc;

use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Icon, Window, WindowLevel};

use crate::config::{in_season, today, Action, Fx, Mode, Seasonal, Settings, Tristate};
use crate::dancer::STYLES;
use crate::render::Gpu;

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
    Keys,
}

impl Tab {
    const ALL: [Tab; 5] = [Tab::Show, Tab::Scenes, Tab::Dancer, Tab::Effects, Tab::Keys];

    fn label(self) -> &'static str {
        match self {
            Tab::Show => "Show",
            Tab::Scenes => "Scenes",
            Tab::Dancer => "Dancer",
            Tab::Effects => "Effects",
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
    ) -> (Vec<UiCommand>, bool, PanelFrame) {
        let mut commands = Vec::new();
        let mut changed = false;

        let raw = self.state.take_egui_input(&self.window);
        let rebinding = &mut self.rebinding;
        let tab = &mut self.tab;
        let scene_filter = &mut self.scene_filter;
        let mut out = self.ctx.run_ui(raw, |ui| {
            ui.add_space(6.0);
            changed |= build_ui(ui, settings, status, scenes, clips, rebinding, tab, scene_filter, &mut commands);
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
            Tab::Scenes => scenes_tab(ui, s, st, scenes, scene_filter, cmd),
            Tab::Dancer => dancer_tab(ui, s, st, clips, cmd),
            Tab::Effects => effects_tab(ui, s, st),
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
