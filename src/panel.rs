//! Control panel: a second window (egui) for modes, scene playlist, dancer
//! options, sync and hotkey bindings. It shares the renderer's GPU device.

use std::sync::Arc;

use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::config::{in_season, today, Action, Mode, Seasonal, Settings, Tristate};
use crate::dancer::STYLES;
use crate::render::Gpu;

/// Things the panel asks the app to do (beyond editing settings directly).
pub enum UiCommand {
    Do(Action),
    GoToScene(usize),
    ShowClip(usize),
}

/// Live state shown in the panel's status area.
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
}

impl Panel {
    pub fn new(event_loop: &ActiveEventLoop, gpu: Gpu) -> anyhow::Result<Self> {
        let attrs = Window::default_attributes()
            .with_title("Trippin — control")
            .with_inner_size(winit::dpi::LogicalSize::new(560, 860));
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
        surface.configure(&gpu.device, &config);

        let ctx = egui::Context::default();
        ctx.set_visuals(egui::Visuals::dark());
        let state = egui_winit::State::new(ctx.clone(), egui::ViewportId::ROOT, &window, None, None, None);
        let renderer = egui_wgpu::Renderer::new(&gpu.device, config.format, egui_wgpu::RendererOptions::default());
        Ok(Self { window, surface, config, gpu, ctx, state, renderer, rebinding: None })
    }

    /// Feed a window event to egui. Returns true if egui used it.
    pub fn on_event(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::Resized(size) = event {
            if size.width > 0 && size.height > 0 {
                self.config.width = size.width;
                self.config.height = size.height;
                self.surface.configure(&self.gpu.device, &self.config);
            }
        }
        let r = self.state.on_window_event(&self.window, event);
        if r.repaint {
            self.window.request_redraw();
        }
        r.consumed
    }

    pub fn wants_keyboard(&self) -> bool {
        self.ctx.egui_wants_keyboard_input()
    }

    pub fn redraw(
        &mut self,
        settings: &mut Settings,
        status: &Status,
        scenes: &[String],
        clips: &[String],
    ) -> (Vec<UiCommand>, bool) {
        let mut commands = Vec::new();
        let mut changed = false;
        let raw = self.state.take_egui_input(&self.window);
        let rebinding = &mut self.rebinding;
        let out = self.ctx.run_ui(raw, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(6.0);
                changed |= build_ui(ui, settings, status, scenes, clips, rebinding, &mut commands);
            });
        });
        self.state.handle_platform_output(&self.window, out.platform_output);
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas {
                self.renderer.update_texture(&self.gpu.device, &self.gpu.queue, *id, delta);
            }
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
                return (commands, changed);
            }
            _ => return (commands, changed),
        };
        let view = frame.texture.create_view(&Default::default());
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: out.pixels_per_point,
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
        self.gpu.queue.submit(extra.into_iter().chain([enc.finish()]));
        self.window.pre_present_notify();
        self.gpu.queue.present(frame);
        for id in &out.textures_delta.free {
            self.renderer.free_texture(id);
        }
        (commands, changed)
    }
}

fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(title).strong().size(15.0))
        .default_open(true)
        .show(ui, body);
}

/// The panel layout. Returns true when settings changed (so they get saved).
fn build_ui(
    ui: &mut egui::Ui,
    s: &mut Settings,
    st: &Status,
    scenes: &[String],
    clips: &[String],
    rebinding: &mut Option<Action>,
    cmd: &mut Vec<UiCommand>,
) -> bool {
    let before = serde_json::to_string(s).unwrap_or_default();

    ui.horizontal(|ui| {
        ui.heading("Trippin");
        ui.label(format!("{:.0} fps", st.fps));
    });
    ui.label(format!(
        "{:.1} BPM  ·  confidence {:.0}%  ·  beat {}/4{}",
        st.bpm,
        st.confidence * 100.0,
        st.beat_in_bar + 1,
        if st.silent { "  ·  no signal" } else { "" }
    ));
    ui.label(format!(
        "Scene: {}   Dancer: {}",
        scenes.get(st.scene).map(String::as_str).unwrap_or("?"),
        st.clip.as_deref().unwrap_or("—")
    ));
    ui.small(format!("Audio: {}", st.device));
    ui.separator();

    section(ui, "Show", |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut s.mode, Mode::Auto, "Auto");
            ui.selectable_value(&mut s.mode, Mode::Static, "Static");
            ui.selectable_value(&mut s.mode, Mode::Manual, "Manual");
        });
        ui.small(match s.mode {
            Mode::Auto => "Cuts scenes on phrases (and drops); the dancer follows the track.",
            Mode::Static => "Holds the current scene; the dancer still changes with the phrases.",
            Mode::Manual => "Nothing changes unless you change it.",
        });
        ui.horizontal(|ui| {
            if ui.button("◀ Previous").clicked() {
                cmd.push(UiCommand::Do(Action::PrevScene));
            }
            if ui.button("Next ▶").clicked() {
                cmd.push(UiCommand::Do(Action::NextScene));
            }
            ui.checkbox(&mut s.random_order, "Random order");
        });
        ui.horizontal(|ui| {
            ui.label("Scene length");
            for bars in [4, 8, 16, 32] {
                ui.selectable_value(&mut s.phrase_bars, bars, format!("{bars} bars"));
            }
        });
        ui.checkbox(&mut s.cut_on_drops, "Cut early when a drop lands");
        ui.horizontal(|ui| {
            let bo = if st.blackout { "Blackout: ON" } else { "Blackout" };
            if ui.button(bo).clicked() {
                cmd.push(UiCommand::Do(Action::Blackout));
            }
            let fs = if st.fullscreen { "Leave fullscreen" } else { "Fullscreen" };
            if ui.button(fs).clicked() {
                cmd.push(UiCommand::Do(Action::Fullscreen));
            }
        });
    });

    section(ui, "Scenes in rotation", |ui| {
        ui.horizontal(|ui| {
            ui.label("Seasonal scenes");
            ui.selectable_value(&mut s.seasonal, Seasonal::Auto, "Auto (in season)");
            ui.selectable_value(&mut s.seasonal, Seasonal::Always, "Always");
            ui.selectable_value(&mut s.seasonal, Seasonal::Off, "Off");
        });
        let date = today();
        let in_now: Vec<&str> = scenes.iter().map(String::as_str).filter(|n| in_season(n, date) == Some(true)).collect();
        ui.small(if in_now.is_empty() {
            "Nothing seasonal today (Halloween: Oct, Christmas: Dec, fireworks: Bonfire Night and New Year).".to_string()
        } else {
            format!("In season today: {}", in_now.join(", "))
        });
        egui::Grid::new("scenes").num_columns(2).striped(true).show(ui, |ui| {
            for (i, name) in scenes.iter().enumerate() {
                let mut on = !s.disabled_scenes.contains(name);
                let label = match in_season(name, date) {
                    Some(true) => format!("{name} (seasonal, in season)"),
                    Some(false) => format!("{name} (seasonal)"),
                    None => name.clone(),
                };
                if ui.checkbox(&mut on, label).changed() {
                    if on {
                        s.disabled_scenes.retain(|n| n != name);
                    } else {
                        s.disabled_scenes.push(name.clone());
                    }
                }
                let label = if i == st.scene { "▶ showing" } else { "Show now" };
                if ui.add_enabled(i != st.scene, egui::Button::new(label)).clicked() {
                    cmd.push(UiCommand::GoToScene(i));
                }
                ui.end_row();
            }
        });
    });

    section(ui, "Dancer", |ui| {
        ui.checkbox(&mut s.dancer_enabled, "Dancer layer on");
        ui.horizontal(|ui| {
            ui.label("Look");
            ui.selectable_value(&mut s.dancer_style, None, "Auto");
            for (i, name) in STYLES.iter().enumerate() {
                ui.selectable_value(&mut s.dancer_style, Some(i), *name);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Canon (3 dancers)");
            ui.selectable_value(&mut s.canon, Tristate::Auto, "Auto");
            ui.selectable_value(&mut s.canon, Tristate::On, "On");
            ui.selectable_value(&mut s.canon, Tristate::Off, "Off");
        });
        ui.add(egui::Slider::new(&mut s.dancer_size, 0.4..=1.0).text("Size"));
        ui.label("Routines (auto-pilot picks among the ticked ones):");
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
                if ui.add_enabled(!showing, egui::Button::new(if showing { "▶ showing" } else { "Show now" })).clicked() {
                    cmd.push(UiCommand::ShowClip(i));
                }
                ui.end_row();
            }
        });
    });

    section(ui, "Sync", |ui| {
        ui.add(egui::Slider::new(&mut s.latency_ms, -100.0..=200.0).step_by(5.0).text("Latency (ms)"));
        ui.small("Raise it if the visuals land after the beat, lower it if they land before.");
        if ui.button("Mark this beat as the downbeat (the \"one\")").clicked() {
            cmd.push(UiCommand::Do(Action::MarkDownbeat));
        }
    });

    section(ui, "Hotkeys", |ui| {
        ui.small("Keys work in both windows. Click Rebind, then press the new key.");
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
        if ui.button("Reset all keys to defaults").clicked() {
            s.keys = Settings::default().keys;
        }
    });

    serde_json::to_string(s).unwrap_or_default() != before
}
