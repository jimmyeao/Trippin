//! Shared egui-over-wgpu window shell: surface, context, present loop.
//! Both the control panel and the timeline editor are one of these — the
//! surface hardening (submit gate, error epoch, acquire retry) lives here
//! once instead of being duplicated per window.

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Icon, Window, WindowLevel};

use crate::render::Gpu;

/// A fully-tessellated egui frame, ready to be drawn without any locks held.
pub struct Frame {
    prims: Vec<egui::ClippedPrimitive>,
    textures_delta: egui::TexturesDelta,
    ppp: f32,
}

pub struct EguiWin {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    pub ctx: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    /// Throttles configure retries after a failure.
    last_configure: Instant,
    /// False after an acquire failure — retry configure before acquiring again.
    surface_ok: bool,
    /// The error epoch this surface has already reconfigured for — see
    /// `Gpu::surface_epoch`.
    seen_epoch: u64,
}

impl EguiWin {
    /// `on_top` floats the window (the control panel wants this); `anchor`
    /// positions the window near another window's monitor.
    pub fn new(
        event_loop: &ActiveEventLoop,
        gpu: Gpu,
        icon: Option<Icon>,
        title: &str,
        // Logical (points), so the window is the same size on a Retina display
        // (scale 2) as on a 100% Windows monitor; physical pixels opened it at a
        // quarter of the area on Macs.
        size: winit::dpi::LogicalSize<u32>,
        anchor: Option<&Window>,
        on_top: bool,
    ) -> anyhow::Result<Self> {
        let mut attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(size)
            .with_window_icon(icon);
        if on_top {
            // A floating console must never hide behind a fullscreen frame.
            attrs = attrs.with_window_level(WindowLevel::AlwaysOnTop);
        }
        // Spawn on the same monitor as the visuals window — the OS default
        // position can land on another screen or behind the fullscreen window.
        if let Some(main) = anchor {
            if let Some(mon) = main.current_monitor() {
                let mp = mon.position();
                let ms = mon.size();
                attrs = attrs.with_position(winit::dpi::PhysicalPosition::new(
                    mp.x + ms.width as i32 - size.width as i32 - 24,
                    mp.y + 24,
                ));
            }
        }
        // Deliberately NOT focused: the visuals keep keyboard focus so hotkeys
        // behave identically whether the window is up or not. Clicking it
        // focuses it as usual.
        let window = Arc::new(event_loop.create_window(attrs)?);
        let surface = gpu.instance.create_surface(window.clone())?;
        let wsize = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, wsize.width.max(1), wsize.height.max(1))
            .ok_or_else(|| anyhow::anyhow!("surface unsupported"))?;
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
        crate::ui_theme::apply(&ctx);
        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            None,
            None,
            None,
        );
        let renderer = egui_wgpu::Renderer::new(
            &gpu.device,
            config.format,
            egui_wgpu::RendererOptions::default(),
        );
        Ok(Self {
            window,
            surface,
            config,
            gpu,
            ctx,
            state,
            renderer,
            last_configure: Instant::now(),
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

    /// True only while a TextEdit holds focus. `egui_wants_keyboard_input`
    /// reports ANY focused widget — a clicked pad/button counts too, which
    /// made the panel swallow Space (and re-trigger the focused widget)
    /// instead of firing the bound hotkey.
    pub fn wants_keyboard(&self) -> bool {
        self.ctx.text_edit_focused()
    }

    /// Run the egui UI and tessellate. Pure CPU work — safe under a lock as
    /// long as `present` (GPU acquire/present) happens outside it.
    /// `add` gets the root viewport Ui (same as `Context::run_ui`).
    pub fn frame(&mut self, add: impl FnMut(&mut egui::Ui)) -> Frame {
        let raw = self.state.take_egui_input(&self.window);
        let mut out = self.ctx.run_ui(raw, add);
        self.state
            .handle_platform_output(&self.window, out.platform_output);
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        // TexturesDelta must be emptied before it's dropped (debug_assert).
        let textures_delta = std::mem::take(&mut out.textures_delta);
        Frame {
            prims,
            textures_delta,
            ppp: out.pixels_per_point,
        }
    }

    /// Upload textures, acquire a surface frame and present — all without
    /// holding any shared locks (a stalled acquire would otherwise freeze
    /// the render thread through them).
    pub fn present(&mut self, f: Frame) {
        let Frame {
            prims,
            mut textures_delta,
            ppp,
        } = f;

        // The surface must match the window's real size — inner_size() at
        // creation can return the requested logical size before the window is
        // realized, and a mismatch makes egui's scissors fail validation.
        let size = self.window.inner_size();
        if size.width > 0
            && size.height > 0
            && (size.width, size.height) != (self.config.width, self.config.height)
        {
            self.config.width = size.width;
            self.config.height = size.height;
            self.last_configure = Instant::now();
            {
                let _g = self
                    .gpu
                    .submit_gate
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                self.surface.configure(&self.gpu.device, &self.config);
            }
            self.surface_ok = true;
        }

        for (id, deltas) in textures_delta.set.drain() {
            for delta in deltas {
                self.renderer
                    .update_texture(&self.gpu.device, &self.gpu.queue, id, &delta);
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
        let epoch = self
            .gpu
            .surface_epoch
            .load(std::sync::atomic::Ordering::Relaxed);
        if epoch != self.seen_epoch {
            self.seen_epoch = epoch;
            self.surface_ok = false;
        }
        if !self.surface_ok {
            if self.last_configure.elapsed() > Duration::from_millis(250) {
                self.last_configure = Instant::now();
                {
                    let _g = self
                        .gpu
                        .submit_gate
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    self.surface.configure(&self.gpu.device, &self.config);
                }
                self.surface_ok = true;
            }
            return;
        }
        let acquired = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.surface.get_current_texture()
        }));
        let frame = match acquired {
            Ok(wgpu::CurrentSurfaceTexture::Success(f))
            | Ok(wgpu::CurrentSurfaceTexture::Suboptimal(f)) => f,
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
        let extra = self.renderer.update_buffers(
            &self.gpu.device,
            &self.gpu.queue,
            &mut enc,
            &prims,
            &screen,
        );
        {
            let pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.06,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.renderer
                .render(&mut pass.forget_lifetime(), &prims, &screen);
        }
        {
            // Serialize submit/present with surface configure — see
            // Gpu::submit_gate. Without it a configure can starve forever.
            let _g = self
                .gpu
                .submit_gate
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            self.gpu
                .queue
                .submit(extra.into_iter().chain([enc.finish()]));
            self.window.pre_present_notify();
            self.gpu.queue.present(frame);
        }
        for id in freed {
            self.renderer.free_texture(&id);
        }
    }
}
