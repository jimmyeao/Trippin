//! wgpu renderer: each scene is a fullscreen fragment shader that renders into
//! an offscreen HDR texture while reading the previous frame (feedback), then a
//! present pass post-processes that texture onto the window.
//!
//! The optional dancer layer (`dancer.wgsl`) is alpha-blended into the same
//! target after the scene, so scenes with feedback leave echo trails of it.
//!
//! Shaders live in `shaders/` and hot-reload on save: `common.wgsl` is
//! prepended to every scene in `shaders/scenes/*.wgsl`, `present.wgsl` and
//! `dancer.wgsl`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result, anyhow};
use winit::window::Window;

use crate::audio::SPECTRUM_BINS;
use crate::dancer::{Clip, DancerUniforms, SLOTS};
use crate::text::{TEXT_SLOTS, TextBitmap, TextUniforms};

const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub time: f32,
    pub dt: f32,
    pub res_x: f32,
    pub res_y: f32,
    pub bass: f32,
    pub mid: f32,
    pub high: f32,
    pub energy: f32,
    pub onset: f32,
    pub kick: f32,
    /// Beat position (wrapped to keep f32 precision); fract = beat phase.
    pub beat: f32,
    pub beat_phase: f32,
    /// 0..1 through the current bar.
    pub bar_phase: f32,
    pub bpm: f32,
    pub build: f32,
    pub scene_time: f32,
    /// 0..1 overall drive, set by the director.
    pub intensity: f32,
    pub hue: f32,
    pub seed: f32,
    /// 1 on a scene cut / drop, decays to 0.
    pub flash: f32,
    /// Smooth beat clock for camera motion: advances at the tempo but, unlike
    /// `beat`, never jumps when the beat tracker corrects its phase.
    pub flow: f32,
    /// Overall brightness 0..1 (blackout fades this to 0).
    pub master: f32,
    /// Post effect mode (`Fx::index`), consumed by `present.wgsl`.
    pub fx: f32,
    /// Post effect strength 0..1 (uv blend in `present.wgsl`).
    pub fx_amt: f32,
    pub spectrum: [f32; SPECTRUM_BINS],
    /// Time-domain trace: 64 samples (16×vec4), consumed by scope scenes.
    /// (`[f32; 64]` isn't `Pod`, so it's packed as vec4s to match WGSL.)
    pub waveform: [[f32; 4]; 16],
}

#[derive(Default, Clone, Copy, PartialEq)]
enum Kind {
    #[default]
    Scene,
    Present,
    Dancer,
    /// Text overlay — drawn over the surface in the present pass.
    Text,
}

#[derive(Default)]
struct Scene {
    kind: Kind,
    name: String,
    path: PathBuf,
    mtime: Option<SystemTime>,
    pipeline: Option<wgpu::RenderPipeline>,
}

/// GPU handles shared with the control panel window.
#[derive(Clone)]
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Bumped by the uncaptured-error callback. A failed `configure` reports
    /// only through the callback (the call returns `()`), leaving the surface
    /// unconfigured — and `get_current_texture` *panics* on that. Each surface
    /// owner compares against the epoch it last saw and reconfigures when it
    /// advances; a counter (not a flag) so both surfaces observe every error.
    pub surface_epoch: Arc<AtomicU64>,
    /// Serializes queue submission and present with surface `configure`.
    /// `configure` waits for the device to go idle, then requires the queue to
    /// be empty at that instant — if another thread keeps submitting
    /// (the render loop submits every vsync), the wait can never satisfy that
    /// and fails with `GpuWaitTimeout` forever. Holding this gate around
    /// submit/present/configure makes the wait reliable.
    pub submit_gate: Arc<Mutex<()>>,
}

pub struct Renderer {
    pub window: Arc<Window>,
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    uniform_buf: wgpu::Buffer,
    /// Ping-pong targets; `bind_groups[i]` samples `targets[i]`.
    targets: [wgpu::Texture; 2],
    bind_groups: [wgpu::BindGroup; 2],
    current: usize,
    /// Scenes render at this fraction of the window size and are upscaled by
    /// the present pass (lighter GPUs).
    scale: f32,
    shader_dir: PathBuf,
    common_mtime: Option<SystemTime>,
    scenes: Vec<Scene>,
    present: Scene,
    dancer: Scene,
    dancer_layout: wgpu::BindGroupLayout,
    dancer_pipeline_layout: wgpu::PipelineLayout,
    dancer_buf: wgpu::Buffer,
    /// Mask texture array per dancer slot (main + canon companions).
    dancer_masks: [Option<(wgpu::Texture, wgpu::TextureView)>; SLOTS],
    dancer_bg: Option<wgpu::BindGroup>,
    /// Text overlay — one coverage mask per text lane.
    text: Scene,
    text_layout: wgpu::BindGroupLayout,
    text_pipeline_layout: wgpu::PipelineLayout,
    text_buf: wgpu::Buffer,
    text_masks: [Option<(wgpu::Texture, wgpu::TextureView)>; TEXT_SLOTS],
    text_bg: Option<wgpu::BindGroup>,
    /// A size seen but not yet applied — settled before the surface is
    /// reconfigured, so resize bursts can't churn the swapchain.
    pending_size: Option<(u32, u32, Instant)>,
    /// Throttles configure retries after a failure.
    last_configure: Instant,
    /// False after an acquire failure — retry configure before acquiring again.
    surface_ok: bool,
    /// Shared with `Gpu` / the error callback — see `Gpu::surface_epoch`.
    surface_epoch: Arc<AtomicU64>,
    /// The error epoch this surface has already reconfigured for.
    seen_epoch: u64,
    /// Shared submit/configure gate — see `Gpu::submit_gate`.
    submit_gate: Arc<Mutex<()>>,
}

pub fn find_shader_dir() -> Result<PathBuf> {
    let mut candidates = vec![PathBuf::from("shaders")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("shaders"));
            // Inside a .app bundle: Contents/MacOS/trippin → Contents/Resources.
            if let Some(contents) = dir.parent() {
                candidates.push(contents.join("Resources/shaders"));
            }
        }
    }
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders"));
    candidates
        .into_iter()
        .find(|p| p.join("common.wgsl").exists())
        .ok_or_else(|| anyhow!("could not find a shaders/ directory containing common.wgsl"))
}

fn scaled(w: u32, h: u32, scale: f32) -> (u32, u32) {
    (
        ((w as f32 * scale) as u32).max(1),
        ((h as f32 * scale) as u32).max(1),
    )
}

fn dancer_tex_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        },
        count: None,
    }
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

impl Renderer {
    pub async fn new(
        window: Arc<Window>,
        low_power: bool,
        scale: Option<f32>,
        vsync: bool,
    ) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: if low_power {
                    wgpu::PowerPreference::LowPower
                } else {
                    wgpu::PowerPreference::HighPerformance
                },
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                ..Default::default()
            })
            .await
            .context("no suitable GPU adapter")?;
        let info = adapter.get_info();
        let integrated = info.device_type == wgpu::DeviceType::IntegratedGpu;
        let scale = scale
            .unwrap_or(if integrated { 0.75 } else { 1.0 })
            .clamp(0.25, 1.0);
        println!(
            "GPU: {} ({:?}), render scale {:.0}%",
            info.name,
            info.backend,
            scale * 100.0
        );
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("trippin"),
                ..Default::default()
            })
            .await?;
        // Surface real errors — wgpu otherwise fails silently and later calls
        // panic on invalid resources, hiding the original cause. A failed
        // configure also marks surfaces dirty so they get reconfigured before
        // the next acquire (which would panic on an unconfigured surface).
        let surface_epoch = Arc::new(AtomicU64::new(0));
        let epoch = surface_epoch.clone();
        device.on_uncaptured_error(Arc::new(move |e| {
            eprintln!("wgpu error: {e}");
            epoch.fetch_add(1, Ordering::Relaxed);
        }));
        device.set_device_lost_callback(|reason, msg| {
            eprintln!("GPU device lost ({reason:?}): {msg}")
        });
        let submit_gate = Arc::new(Mutex::new(()));

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .context("surface not supported by adapter")?;
        // Metal drawables only return to the pool once the WindowServer has
        // composited them; vsync-gated (Fifo) presents then quantize to every
        // other frame whenever the compositor is loaded (a fullscreen app on
        // another display is enough) — a sticky ~30 fps. Immediate removes the
        // vsync quantisation: the acquire still self-paces to the compositor's
        // drain rate, and windowed content can't tear anyway.
        let caps = surface.get_capabilities(&adapter);
        config.present_mode = if cfg!(target_os = "macos")
            && !vsync
            && caps.present_modes.contains(&wgpu::PresentMode::Immediate)
        {
            wgpu::PresentMode::Immediate
        } else {
            wgpu::PresentMode::AutoVsync
        };
        // Metal lists Bgra8Unorm first — but the present shader relies on the
        // hardware sRGB encode, so prefer an sRGB surface format.
        if let Some(f) = caps.formats.iter().copied().find(|f| f.is_srgb()) {
            config.format = f;
        }
        {
            let _g = submit_gate.lock().unwrap_or_else(|e| e.into_inner());
            surface.configure(&device, &config);
        }
        println!("Surface: {:?}, {:?}", config.present_mode, config.format);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frame"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::MirrorRepeat,
            address_mode_v: wgpu::AddressMode::MirrorRepeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dancer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("dancer"),
            entries: &[
                dancer_tex_entry(0),
                dancer_tex_entry(1),
                dancer_tex_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let dancer_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("dancer"),
                bind_group_layouts: &[Some(&layout), Some(&dancer_layout)],
                immediate_size: 0,
            });
        let dancer_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dancer uniforms"),
            size: std::mem::size_of::<DancerUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let text_tex_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let text_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text"),
            entries: &[
                text_tex_entry(0),
                text_tex_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let text_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text"),
            bind_group_layouts: &[Some(&layout), Some(&text_layout)],
            immediate_size: 0,
        });
        let text_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text uniforms"),
            size: std::mem::size_of::<TextUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let (tw, th) = scaled(config.width, config.height, scale);
        let targets = Self::make_targets(&device, tw, th);
        let bind_groups =
            Self::make_bind_groups(&device, &layout, &uniform_buf, &sampler, &targets);

        let shader_dir = find_shader_dir()?;
        println!("Shaders: {}", shader_dir.display());
        let present_path = shader_dir.join("present.wgsl");
        let dancer_path = shader_dir.join("dancer.wgsl");
        let text_path = shader_dir.join("text.wgsl");
        let mut r = Self {
            window,
            instance,
            adapter,
            surface,
            device,
            queue,
            config,
            layout,
            pipeline_layout,
            sampler,
            uniform_buf,
            targets,
            bind_groups,
            current: 0,
            scale,
            shader_dir,
            common_mtime: None,
            scenes: Vec::new(),
            present: Scene {
                kind: Kind::Present,
                name: "present".into(),
                path: present_path,
                ..Default::default()
            },
            dancer: Scene {
                kind: Kind::Dancer,
                name: "dancer".into(),
                path: dancer_path,
                ..Default::default()
            },
            dancer_layout,
            dancer_pipeline_layout,
            dancer_buf,
            dancer_masks: Default::default(),
            dancer_bg: None,
            text: Scene {
                kind: Kind::Text,
                name: "text".into(),
                path: text_path,
                ..Default::default()
            },
            text_layout,
            text_pipeline_layout,
            text_buf,
            text_masks: Default::default(),
            text_bg: None,
            pending_size: None,
            last_configure: Instant::now(),
            surface_ok: true,
            surface_epoch,
            seen_epoch: 0,
            submit_gate,
        };
        r.reload_shaders(true);
        if r.present.pipeline.is_none() {
            return Err(anyhow!("present.wgsl failed to compile"));
        }
        if !r.scenes.iter().any(|s| s.pipeline.is_some()) {
            return Err(anyhow!("no scene shaders compiled"));
        }
        Ok(r)
    }

    fn make_targets(device: &wgpu::Device, w: u32, h: u32) -> [wgpu::Texture; 2] {
        let make = |label| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w.max(1),
                    height: h.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: SCENE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        [make("frame a"), make("frame b")]
    }

    fn make_bind_groups(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buf: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        targets: &[wgpu::Texture; 2],
    ) -> [wgpu::BindGroup; 2] {
        let make = |t: &wgpu::Texture| {
            let view = t.create_view(&Default::default());
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        };
        [make(&targets[0]), make(&targets[1])]
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.last_configure = Instant::now();
        {
            let _g = self.submit_gate.lock().unwrap_or_else(|e| e.into_inner());
            self.surface.configure(&self.device, &self.config);
        }
        self.surface_ok = true;
        let (tw, th) = scaled(w, h, self.scale);
        self.targets = Self::make_targets(&self.device, tw, th);
        self.bind_groups = Self::make_bind_groups(
            &self.device,
            &self.layout,
            &self.uniform_buf,
            &self.sampler,
            &self.targets,
        );
    }

    /// The size the surface was last configured with.
    pub fn surface_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// A resize event hint — applied once the size settles (see `sync_size`).
    pub fn note_size(&mut self, w: u32, h: u32) {
        if w > 0 && h > 0 && (w, h) != self.surface_size() {
            self.pending_size = Some((w, h, Instant::now()));
        }
    }

    /// Reconfigure the surface if the window's real size doesn't match the
    /// configured size and has been stable for a moment. Fullscreen
    /// transitions emit a burst of sizes; configuring on every one churns the
    /// swapchain mid-present, which can take the whole device down. Covers
    /// resize events lost or reordered during transitions.
    pub fn sync_size(&mut self) {
        let size = self.window.inner_size();
        let wanted = (size.width, size.height);
        if size.width == 0 || size.height == 0 || wanted == self.surface_size() {
            self.pending_size = None;
            return;
        }
        match self.pending_size {
            Some((w, h, since)) if (w, h) == wanted => {
                if since.elapsed() > std::time::Duration::from_millis(150) {
                    self.pending_size = None;
                    self.resize(w, h);
                }
            }
            _ => self.pending_size = Some((wanted.0, wanted.1, std::time::Instant::now())),
        }
    }

    /// Time between frames the render loop should hold to when the present
    /// mode doesn't pace itself — on macOS `Immediate` free-runs once the
    /// drawable pool stops back-pressuring, so we cap at the display's
    /// refresh instead of burning the GPU on frames nobody sees. `None`
    /// where vsync throttles already.
    pub fn frame_interval(&self) -> Option<Duration> {
        if self.config.present_mode != wgpu::PresentMode::Immediate {
            return None;
        }
        let hz = self
            .window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .map(|mhz| mhz as f64 / 1000.0)
            .filter(|h| *h > 1.0)
            .unwrap_or(60.0);
        Some(Duration::from_secs_f64(1.0 / hz))
    }

    /// Size of the scene render targets (what shaders see as the resolution).
    pub fn gpu(&self) -> Gpu {
        Gpu {
            instance: self.instance.clone(),
            adapter: self.adapter.clone(),
            device: self.device.clone(),
            queue: self.queue.clone(),
            surface_epoch: self.surface_epoch.clone(),
            submit_gate: self.submit_gate.clone(),
        }
    }

    pub fn size(&self) -> (u32, u32) {
        scaled(self.config.width, self.config.height, self.scale)
    }

    pub fn scene_names(&self) -> Vec<String> {
        self.scenes.iter().map(|s| s.name.clone()).collect()
    }

    /// Indices of scenes that currently compile.
    pub fn usable_scenes(&self) -> Vec<usize> {
        (0..self.scenes.len())
            .filter(|&i| self.scenes[i].pipeline.is_some())
            .collect()
    }

    /// Recompile anything whose file changed (or everything if `force`).
    pub fn reload_shaders(&mut self, force: bool) {
        let common_path = self.shader_dir.join("common.wgsl");
        let common_m = mtime(&common_path);
        let common_changed = force || common_m != self.common_mtime;
        self.common_mtime = common_m;
        let Ok(common) = std::fs::read_to_string(&common_path) else {
            eprintln!("cannot read {}", common_path.display());
            return;
        };

        // Pick up newly added scene files.
        let mut paths: Vec<PathBuf> = std::fs::read_dir(self.shader_dir.join("scenes"))
            .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default();
        paths.retain(|p| p.extension().is_some_and(|e| e == "wgsl"));
        paths.sort();
        for p in paths {
            if !self.scenes.iter().any(|s| s.path == p) {
                let name = p.file_stem().unwrap().to_string_lossy().into_owned();
                self.scenes.push(Scene {
                    name,
                    path: p,
                    ..Default::default()
                });
            }
        }

        let mut scenes = std::mem::take(&mut self.scenes);
        let mut present = std::mem::take(&mut self.present);
        let mut dancer = std::mem::take(&mut self.dancer);
        let mut text = std::mem::take(&mut self.text);
        for s in scenes
            .iter_mut()
            .chain([&mut present, &mut dancer, &mut text])
        {
            let m = mtime(&s.path);
            if !common_changed && m == s.mtime {
                continue;
            }
            s.mtime = m;
            match self.compile(&common, &s.path, s.kind) {
                Ok(p) => {
                    if !force {
                        println!("reloaded {}", s.name);
                    }
                    s.pipeline = Some(p);
                }
                // Keep the last good pipeline so a typo never blanks the screen.
                Err(e) => eprintln!("shader {} failed:\n{e}", s.name),
            }
        }
        self.scenes = scenes;
        self.present = present;
        self.dancer = dancer;
        self.text = text;
    }

    fn compile(&self, common: &str, path: &Path, kind: Kind) -> Result<wgpu::RenderPipeline> {
        let (format, layout, blend) = match kind {
            Kind::Scene => (SCENE_FORMAT, &self.pipeline_layout, None),
            Kind::Present => (self.config.format, &self.pipeline_layout, None),
            Kind::Dancer => (
                SCENE_FORMAT,
                &self.dancer_pipeline_layout,
                Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            ),
            // Over the finished frame on the surface — post FX leave it alone.
            Kind::Text => (
                self.config.format,
                &self.text_pipeline_layout,
                Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            ),
        };
        let body = std::fs::read_to_string(path)?;
        let src = format!("{common}\n{body}");
        // Validate with naga first to get readable errors instead of a panic.
        let module = wgpu::naga::front::wgsl::parse_str(&src)
            .map_err(|e| anyhow!(e.emit_to_string(&src)))?;
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .map_err(|e| anyhow!(e.emit_to_string(&src)))?;

        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: path.to_str(),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            });
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: path.to_str(),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(anyhow!("{e}"));
        }
        Ok(pipeline)
    }

    /// Upload a dancer clip's masks as a texture array into `slot`
    /// (0 = main dancer, 1-2 = canon companions), replacing what was there.
    pub fn set_dancer_clip(&mut self, slot: usize, clip: &Clip) {
        let size = wgpu::Extent3d {
            width: clip.width,
            height: clip.height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("dancer masks"),
            size: wgpu::Extent3d {
                depth_or_array_layers: clip.frames.len() as u32,
                ..size
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (i, frame) in clip.frames.iter().enumerate() {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: i as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                frame,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(clip.width),
                    rows_per_image: Some(clip.height),
                },
                size,
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        self.dancer_masks[slot] = Some((texture, view));

        // Rebuild the bind group; empty slots borrow any loaded one (the
        // shader skips slots whose frame count is 0).
        let Some(fallback) = self
            .dancer_masks
            .iter()
            .flatten()
            .next()
            .map(|(_, v)| v.clone())
        else {
            return;
        };
        let views: Vec<wgpu::TextureView> = self
            .dancer_masks
            .iter()
            .map(|m| m.as_ref().map_or(fallback.clone(), |(_, v)| v.clone()))
            .collect();
        self.dancer_bg = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dancer"),
            layout: &self.dancer_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&views[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&views[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.dancer_buf.as_entire_binding(),
                },
            ],
        }));
    }

    /// Upload a rasterised text mask into `slot` (0/1 — the two text lanes).
    pub fn set_text_bitmap(&mut self, slot: usize, bmp: &TextBitmap) {
        if slot >= TEXT_SLOTS {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("text mask"),
            size: wgpu::Extent3d {
                width: bmp.width.max(1),
                height: bmp.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bmp.mask,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bmp.width.max(1)),
                rows_per_image: Some(bmp.height.max(1)),
            },
            wgpu::Extent3d {
                width: bmp.width.max(1),
                height: bmp.height.max(1),
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        self.text_masks[slot] = Some((texture, view));
        self.rebuild_text_bg();
    }

    fn rebuild_text_bg(&mut self) {
        // Empty slots borrow any loaded mask — the shader skips them on
        // `aspect < 0.01`, so the borrow is never sampled.
        let Some(fallback) = self
            .text_masks
            .iter()
            .flatten()
            .next()
            .map(|(_, v)| v.clone())
        else {
            return;
        };
        let views: Vec<wgpu::TextureView> = self
            .text_masks
            .iter()
            .map(|m| m.as_ref().map_or(fallback.clone(), |(_, v)| v.clone()))
            .collect();
        self.text_bg = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text"),
            layout: &self.text_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&views[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.text_buf.as_entire_binding(),
                },
            ],
        }));
    }

    /// Render `scene` (plus the dancer layer, if given) into the next
    /// ping-pong target, then present it.
    pub fn render(
        &mut self,
        scene: usize,
        u: &Uniforms,
        dancer: Option<&DancerUniforms>,
        text: Option<&TextUniforms>,
    ) -> Result<()> {
        self.queue
            .write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(u));
        if let Some(d) = dancer {
            self.queue
                .write_buffer(&self.dancer_buf, 0, bytemuck::bytes_of(d));
        }
        if let Some(t) = text {
            self.queue
                .write_buffer(&self.text_buf, 0, bytemuck::bytes_of(t));
        }

        // A failed configure is reported via the device's error callback —
        // asynchronously, and only drained by poll/submit. Polling here forces
        // the callback to run before we touch the surface: if configure failed,
        // the epoch has advanced and we reconfigure instead of calling
        // get_current_texture on an unconfigured surface (which panics).
        // catch_unwind below remains only as a last-resort guard.
        let _ = self.device.poll(wgpu::PollType::Poll);
        let epoch = self.surface_epoch.load(Ordering::Relaxed);
        if epoch != self.seen_epoch {
            self.seen_epoch = epoch;
            self.surface_ok = false;
        }
        if !self.surface_ok {
            self.sync_size();
            if self.pending_size.is_none()
                && self.last_configure.elapsed() > Duration::from_millis(250)
            {
                self.last_configure = Instant::now();
                {
                    let _g = self.submit_gate.lock().unwrap_or_else(|e| e.into_inner());
                    self.surface.configure(&self.device, &self.config);
                }
                self.surface_ok = true;
            }
            // Back off while the surface is unhealthy — reconfigure runs at
            // 4 Hz, so there's no work worth thousands of spins a second.
            std::thread::sleep(Duration::from_millis(4));
            return Ok(());
        }
        let acquired = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.surface.get_current_texture()
        }));
        let frame = match acquired {
            Ok(wgpu::CurrentSurfaceTexture::Success(f))
            | Ok(wgpu::CurrentSurfaceTexture::Suboptimal(f)) => f,
            Ok(wgpu::CurrentSurfaceTexture::Timeout) => return Ok(()),
            // Occluded (covered, minimised, off-screen) is transient — skip the
            // frame rather than churning the surface, and nap so the loop
            // doesn't spin on an invisible window.
            Ok(wgpu::CurrentSurfaceTexture::Occluded) => {
                std::thread::sleep(Duration::from_millis(16));
                return Ok(());
            }
            other => {
                eprintln!("acquire: {other:?}");
                self.surface_ok = false;
                return Ok(());
            }
        };
        let prev = self.current;
        let next = 1 - prev;
        let mut enc = self.device.create_command_encoder(&Default::default());

        if let Some(pipeline) = self.scenes.get(scene).and_then(|s| s.pipeline.as_ref()) {
            let view = self.targets[next].create_view(&Default::default());
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.bind_groups[prev], &[]);
            pass.draw(0..3, 0..1);
            if let (Some(_), Some(dancer), Some(dancer_bg)) = (
                dancer,
                self.dancer.pipeline.as_ref(),
                self.dancer_bg.as_ref(),
            ) {
                pass.set_pipeline(dancer);
                pass.set_bind_group(1, dancer_bg, &[]);
                pass.draw(0..3, 0..1);
            }
            drop(pass);
            self.current = next;
        }

        let view = frame.texture.create_view(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(self.present.pipeline.as_ref().unwrap());
            pass.set_bind_group(0, &self.bind_groups[self.current], &[]);
            pass.draw(0..3, 0..1);
            // Text rides on top of the finished frame — post FX can't distort it.
            if let (Some(_), Some(text), Some(text_bg)) =
                (text, self.text.pipeline.as_ref(), self.text_bg.as_ref())
            {
                pass.set_pipeline(text);
                pass.set_bind_group(0, &self.bind_groups[self.current], &[]);
                pass.set_bind_group(1, text_bg, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        // Notify before presenting, but outside the gate: on macOS this call
        // dispatches synchronously to the main thread, and the main thread
        // can be waiting on `submit_gate` in the panel — holding it here
        // deadlocks.
        self.window.pre_present_notify();
        {
            // The gate keeps the queue still while a surface configure on the
            // other thread waits for it to go idle — see Gpu::submit_gate.
            let _g = self.submit_gate.lock().unwrap_or_else(|e| e.into_inner());
            self.queue.submit([enc.finish()]);
            self.queue.present(frame);
        }
        Ok(())
    }

    /// Render one scene once into a small RGBA8 image — the editor's cue
    /// block thumbnails. Reuses the real scene+present pipelines on tiny
    /// offscreen targets with a lively canned frame of uniforms.
    pub fn thumbnail(&mut self, scene: usize, w: u32, h: u32) -> Option<Vec<u8>> {
        let pipeline = self.scenes.get(scene)?.pipeline.clone()?;
        let present = self.present.pipeline.clone()?;
        let mut spectrum = [0.35f32; SPECTRUM_BINS];
        for (i, v) in spectrum.iter_mut().enumerate() {
            *v = (0.6 - i as f32 * 0.004).max(0.1);
        }
        let mut waveform = [[0.0f32; 4]; 16];
        for (i, v) in waveform.iter_mut().enumerate() {
            let x = i as f32 / 16.0 * std::f32::consts::TAU;
            *v = [x.sin(), x.sin() * 0.7, 0.0, 0.0];
        }
        let u = Uniforms {
            time: 3.0,
            dt: 0.016,
            res_x: w as f32,
            res_y: h as f32,
            bass: 0.7,
            mid: 0.55,
            high: 0.45,
            energy: 0.7,
            onset: 1.0,
            kick: 1.0,
            beat: 8.0,
            beat_phase: 0.3,
            bar_phase: 0.6,
            bpm: 128.0,
            build: 0.5,
            scene_time: 3.0,
            intensity: 0.75,
            hue: 0.0,
            seed: scene as f32 * 37.7 % 100.0,
            flash: 0.0,
            flow: 8.0,
            master: 1.0,
            fx: 0.0,
            fx_amt: 1.0,
            spectrum,
            waveform,
        };
        self.queue
            .write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&u));

        let extent = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        let mk = |format| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("thumb"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        // Two tiny scene targets so the ping-pong/feedback bind groups work
        // exactly like the live path (prev starts black).
        let targets = [mk(SCENE_FORMAT), mk(SCENE_FORMAT)];
        let bgs = Self::make_bind_groups(
            &self.device,
            &self.layout,
            &self.uniform_buf,
            &self.sampler,
            &targets,
        );
        // The present pipeline is built for the surface format (usually
        // Bgra8) — match it and swizzle on readback.
        let out_fmt = self.config.format;
        let out = mk(out_fmt);

        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let view = targets[1].create_view(&Default::default());
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("thumb_scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bgs[0], &[]);
            pass.draw(0..3, 0..1);
        }
        {
            let view = out.create_view(&Default::default());
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("thumb_present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&present);
            pass.set_bind_group(0, &bgs[1], &[]);
            pass.draw(0..3, 0..1);
        }

        let bpr = ((w * 4 + 255) / 256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("thumb_rb"),
            size: (bpr * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &out,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(h),
                },
            },
            extent,
        );
        {
            let _g = self.submit_gate.lock().unwrap_or_else(|e| e.into_inner());
            self.queue.submit([enc.finish()]);
        }
        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv().ok()?.ok()?;
        let bgra = matches!(
            out_fmt,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let data = slice.get_mapped_range().ok()?;
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let s = (y * bpr) as usize;
            let row = &data[s..s + (w * 4) as usize];
            if bgra {
                for px4 in row.chunks_exact(4) {
                    px.extend_from_slice(&[px4[2], px4[1], px4[0], px4[3]]);
                }
            } else {
                px.extend_from_slice(row);
            }
        }
        Some(px)
    }
}
