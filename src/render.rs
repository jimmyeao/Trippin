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
use crate::gfx;
use crate::output::{self, OUT_FORMAT};
use crate::palettes;
use crate::overlay::{OV_LAYERS, OvUniforms};
use crate::text::{TEXT_SLOTS, TextBitmap, TextUniforms};

pub const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

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
    /// Bloom strength — set by the renderer from the scene's `// @bloom`.
    pub bloom: f32,
    /// 0 ACES, 1 AgX — set by the renderer from `// @tonemap agx`.
    pub tonemap: f32,
    /// Frame counter (wraps) — animates blue-noise dither.
    pub frame: f32,
    /// Breakdown state 0 = beats, 1 = breakdown (no drums) — see audio.rs.
    pub calm: f32,
    /// Four-band vocabulary (bass, mid, mid-high, high) — see audio.rs.
    pub lvl4: [f32; 4],
    pub hits4: [f32; 4],
    pub pres4: [f32; 4],
    /// Energy clocks (beats): (whole mix, bass, mid, high) — advance faster
    /// the louder the band. Integrated in main.rs from smoothed levels.
    pub clock4: [f32; 4],
    /// x: 1 = transparent background (the renderer sets it — scenes are
    /// skipped and present writes real alpha for OBS/NDI/Spout); yzw spare.
    pub misc4: [f32; 4],
}

#[derive(Default, Clone, Copy, PartialEq)]
enum Kind {
    #[default]
    Scene,
    Present,
    Dancer,
    /// Text overlay — drawn over the surface in the present pass.
    Text,
    /// Stream overlays (now playing, branding, ticker) — same pass.
    Overlay,
}

#[derive(Default)]
struct Scene {
    kind: Kind,
    name: String,
    path: PathBuf,
    mtime: Option<SystemTime>,
    pipeline: Option<wgpu::RenderPipeline>,
    /// `// @heavy` in the file header marks a raymarched scene that's only in
    /// rotation when the GPU tier allows (or the user forces it).
    heavy: bool,
    /// `// @bloom <amount>` in the header (0 = no bloom passes at all).
    bloom: f32,
    /// `// @tonemap agx` → 1.0, else ACES (0.0).
    tonemap: f32,
    /// `// @title <display name>` — human label for pickers; the id (file
    /// stem) stays the key everywhere else.
    title: Option<String>,
    /// `// @no-dancer` in the header: the scene draws its own people (a
    /// crowd), so the dancer overlay stays off it.
    no_dancer: bool,
}

/// Parse `// @title <rest of line>` — the display-name header directive.
/// Unlike the `value` tags it's the whole line tail, not one token.
pub fn header_title(body: &str) -> Option<String> {
    body.lines().take(8).find_map(|l| {
        let i = l.find("@title")?;
        let t = l[i + "@title".len()..].trim();
        (!t.is_empty()).then(|| t.to_string())
    })
}

/// `// @no-dancer` in the header: the scene has its own crowd.
pub fn header_no_dancer(body: &str) -> bool {
    body.lines().take(8).any(|l| l.contains("@no-dancer"))
}

/// Parse the `// @tag value` header directives of a scene file.
pub fn header_tags(body: &str) -> (bool, f32, f32) {
    let head: Vec<&str> = body.lines().take(8).collect();
    let heavy = head.iter().any(|l| l.contains("@heavy"));
    let value = |tag: &str| {
        head.iter().find_map(|l| {
            let i = l.find(tag)?;
            l[i + tag.len()..].split_whitespace().next().map(str::to_string)
        })
    };
    let bloom = value("@bloom")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.0)
        .clamp(0.0, 4.0);
    let tonemap = if value("@tonemap").is_some_and(|v| v.starts_with("agx")) {
        1.0
    } else {
        0.0
    };
    (heavy, bloom, tonemap)
}

/// Everything group(0) binds besides the uniforms and the feedback texture.
pub struct FrameRes<'a> {
    pub pal: &'a wgpu::TextureView,
    pub noise3: &'a wgpu::TextureView,
    pub blue: &'a wgpu::TextureView,
    pub repeat: &'a wgpu::Sampler,
    pub bloom: &'a wgpu::TextureView,
    pub ext: &'a wgpu::TextureView,
}

impl FrameRes<'_> {
    /// Bind group entries 3..=8 (after uniforms, feedback and sampler).
    pub fn entries(&self) -> [wgpu::BindGroupEntry<'_>; 6] {
        [
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(self.pal),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(self.noise3),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(self.blue),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::Sampler(self.repeat),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(self.bloom),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::TextureView(self.ext),
            },
        ]
    }
}

fn tex_entry(binding: u32, dim: wgpu::TextureViewDimension) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: dim,
            multisampled: false,
        },
        count: None,
    }
}

/// The group(0) layout every scene / present / dancer / text shader shares.
pub fn frame_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    use wgpu::TextureViewDimension::{D2, D3};
    let sampler = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            tex_entry(1, D2),
            sampler(2),
            // 256×1 palette LUT — see palettes.rs / common.wgsl palette().
            tex_entry(3, D2),
            // Noise volume, blue noise, repeat sampler — see gfx.rs.
            tex_entry(4, D3),
            tex_entry(5, D2),
            sampler(6),
            // Half-res bloom result.
            tex_entry(7, D2),
            // External frame (Spout in) — gfx::Statics::ext.
            tex_entry(8, D2),
        ],
    })
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
    /// Stream overlays — one RGBA image per layer (see overlay.rs).
    overlay: Scene,
    ov_layout: wgpu::BindGroupLayout,
    ov_pipeline_layout: wgpu::PipelineLayout,
    ov_buf: wgpu::Buffer,
    ov_tex: [Option<(wgpu::Texture, wgpu::TextureView)>; OV_LAYERS],
    /// 1×1 transparent stand-in for layers never uploaded.
    ov_blank: wgpu::TextureView,
    ov_bg: Option<wgpu::BindGroup>,
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
    /// The adapter can comfortably raymarch (`@heavy` scenes join rotation).
    heavy_ok: bool,
    /// Global palette LUT + the name of the palette currently uploaded.
    pal_tex: wgpu::Texture,
    pal_view: wgpu::TextureView,
    pal_name: String,
    /// Noise volumes (gfx.rs) and the bloom chain.
    statics: gfx::Statics,
    bloom: gfx::Bloom,
    /// Eased bloom strength — scene cuts don't pop the glow on/off.
    bloom_amt: f32,
    frame: u32,
    /// External output (NDI) — Some(conf) mirrors the panel setting;
    /// `out` is Some only once the runtime loads and resources are built.
    out_conf: Option<output::Conf>,
    out: Option<output::Output>,
    /// Last init failure + when — a missing runtime is re-probed every 5s,
    /// not every frame.
    out_err: Option<(output::Conf, Instant, String)>,
    /// present/text recompiled for the output texture format.
    out_present: Option<wgpu::RenderPipeline>,
    out_text: Option<wgpu::RenderPipeline>,
    out_overlay: Option<wgpu::RenderPipeline>,
    /// Transparent-background mode: no scene, alpha out (see Uniforms::misc4).
    pub transparent: bool,
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
        // `@heavy` scenes raymarch every pixel — keep them off iGPUs that
        // would drop frames, but Apple Silicon reports "integrated" while
        // being plenty fast. `--gpu low` always opts out.
        let heavy_ok = !low_power
            && match info.device_type {
                wgpu::DeviceType::DiscreteGpu => true,
                // Vendor 0x106b = Apple.
                wgpu::DeviceType::IntegratedGpu => info.vendor == 0x106b,
                _ => false,
            };
        println!(
            "GPU: {} ({:?}), render scale {:.0}%, heavy scenes {}",
            info.name,
            info.backend,
            scale * 100.0,
            if heavy_ok { "on" } else { "off" }
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
        let layout = frame_layout(&device);
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

        let ov_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("overlay"),
            entries: &[
                text_tex_entry(0),
                text_tex_entry(1),
                text_tex_entry(2),
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
                text_tex_entry(4),
            ],
        });
        let ov_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("overlay"),
            bind_group_layouts: &[Some(&layout), Some(&ov_layout)],
            immediate_size: 0,
        });
        let ov_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("overlay uniforms"),
            size: std::mem::size_of::<OvUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ov_blank = rgba_texture(&device, &queue, 1, 1, &[0, 0, 0, 0]).1;

        // Palette LUT: a 256×1 gradient the user can swap globally.
        let pal_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("palette"),
            size: wgpu::Extent3d {
                width: palettes::LUT_SIZE as u32,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &pal_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &palettes::lut("rainbow"),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((palettes::LUT_SIZE * 4) as u32),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: palettes::LUT_SIZE as u32,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let pal_view = pal_tex.create_view(&Default::default());

        let (tw, th) = scaled(config.width, config.height, scale);
        let targets = Self::make_targets(&device, tw, th);
        let statics = gfx::Statics::new(&device, &queue);
        let bloom = gfx::Bloom::new(&device, tw, th);
        let bind_groups = Self::make_bind_groups(
            &device,
            &layout,
            &uniform_buf,
            &sampler,
            &targets,
            &FrameRes {
                pal: &pal_view,
                noise3: &statics.noise3_view,
                blue: &statics.blue_view,
                repeat: &statics.repeat,
                bloom: bloom.view(),
                ext: &statics.ext_view,
            },
        );

        let shader_dir = find_shader_dir()?;
        println!("Shaders: {}", shader_dir.display());
        let present_path = shader_dir.join("present.wgsl");
        let dancer_path = shader_dir.join("dancer.wgsl");
        let text_path = shader_dir.join("text.wgsl");
        let overlay_path = shader_dir.join("overlay.wgsl");
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
            overlay: Scene {
                kind: Kind::Overlay,
                name: "overlay".into(),
                path: overlay_path,
                ..Default::default()
            },
            ov_layout,
            ov_pipeline_layout,
            ov_buf,
            ov_tex: Default::default(),
            ov_blank,
            ov_bg: None,
            pending_size: None,
            last_configure: Instant::now(),
            surface_ok: true,
            surface_epoch,
            seen_epoch: 0,
            submit_gate,
            heavy_ok,
            pal_tex,
            pal_view,
            pal_name: "rainbow".to_string(),
            statics,
            bloom,
            bloom_amt: 0.0,
            frame: 0,
            out_conf: None,
            out: None,
            out_err: None,
            out_present: None,
            out_text: None,
            out_overlay: None,
            transparent: false,
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

    pub fn make_targets(device: &wgpu::Device, w: u32, h: u32) -> [wgpu::Texture; 2] {
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

    pub fn make_bind_groups(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buf: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        targets: &[wgpu::Texture; 2],
        res: &FrameRes,
    ) -> [wgpu::BindGroup; 2] {
        let make = |t: &wgpu::Texture| {
            let view = t.create_view(&Default::default());
            let [e3, e4, e5, e6, e7, e8] = res.entries();
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
                    e3,
                    e4,
                    e5,
                    e6,
                    e7,
                    e8,
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
        self.bloom.resize(&self.device, tw, th);
        let res = FrameRes {
            pal: &self.pal_view,
            noise3: &self.statics.noise3_view,
            blue: &self.statics.blue_view,
            repeat: &self.statics.repeat,
            bloom: self.bloom.view(),
            ext: &self.statics.ext_view,
        };
        self.bind_groups = Self::make_bind_groups(
            &self.device,
            &self.layout,
            &self.uniform_buf,
            &self.sampler,
            &self.targets,
            &res,
        );
        if let Some(o) = self.out.as_mut() {
            o.rebind(&self.device, &self.layout, &self.sampler, &self.targets, &res);
        }
    }

    /// Swap the global palette — rewrites the LUT in place so every bound
    /// shader sees it next frame; no pipeline or bind-group churn needed.
    pub fn set_palette(&mut self, name: &str) {
        if name.is_empty() || name == self.pal_name {
            return;
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.pal_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &palettes::lut(name),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((palettes::LUT_SIZE * 4) as u32),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: palettes::LUT_SIZE as u32,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        self.pal_name = name.to_string();
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

    /// One scene's name without cloning the whole list.
    pub fn scene_name(&self, i: usize) -> &str {
        self.scenes.get(i).map_or("", |s| s.name.as_str())
    }

    /// Upload an external frame (RGBA8, EXT_W x EXT_H) for `ext_tex`.
    pub fn upload_external(&self, rgba: &[u8]) {
        use crate::gfx::{EXT_H, EXT_W};
        if rgba.len() != (EXT_W * EXT_H * 4) as usize {
            return;
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.statics.ext,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(EXT_W * 4),
                rows_per_image: Some(EXT_H),
            },
            wgpu::Extent3d {
                width: EXT_W,
                height: EXT_H,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Indices of scenes that currently compile.
    pub fn usable_scenes(&self) -> Vec<usize> {
        (0..self.scenes.len())
            .filter(|&i| self.scenes[i].pipeline.is_some())
            .collect()
    }

    /// The adapter can comfortably raymarch (`@heavy` scenes). `false` on
    /// non-Apple iGPUs and under `--gpu low`.
    pub fn heavy_ok(&self) -> bool {
        self.heavy_ok
    }

    /// The dancer overlay stays off this scene: it has its own figures —
    /// robots and androids (tagged `character`) or a crowd (`@no-dancer`).
    pub fn scene_no_dancer(&self, i: usize) -> bool {
        self.scenes.get(i).is_some_and(|s| {
            s.no_dancer || crate::styles::catalog().tags.has(&s.name, "character")
        })
    }

    /// Per-scene `@heavy` flags, parallel with `scene_names`.
    pub fn scene_heavy(&self) -> Vec<bool> {
        self.scenes.iter().map(|s| s.heavy).collect()
    }

    /// Display titles, parallel with `scene_names`: the `// @title` header
    /// when set, else the id title-cased. For pickers and the remote —
    /// `scene_names` stays the stable key everywhere.
    pub fn scene_titles(&self) -> Vec<String> {
        self.scenes
            .iter()
            .map(|s| s.title.clone().unwrap_or_else(|| crate::config::titleize(&s.name)))
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
        let mut overlay = std::mem::take(&mut self.overlay);
        for s in scenes
            .iter_mut()
            .chain([&mut present, &mut dancer, &mut text, &mut overlay])
        {
            let m = mtime(&s.path);
            if !common_changed && m == s.mtime {
                continue;
            }
            s.mtime = m;
            // `// @heavy` in the file header gates the scene to stronger GPUs;
            // `@bloom` / `@tonemap` pick its post settings.
            if let Ok(body) = std::fs::read_to_string(&s.path) {
                (s.heavy, s.bloom, s.tonemap) = header_tags(&body);
                s.title = header_title(&body);
                s.no_dancer = header_no_dancer(&body);
            }
            match self.compile(&common, &s.path, s.kind) {
                Ok(p) => {
                    if !force {
                        println!("reloaded {}", s.name);
                    }
                    s.pipeline = Some(p);
                    // The output tap re-runs present/text into a BGRA target —
                    // keep sibling pipelines at OUT_FORMAT alongside.
                    let (layout, blend) = match s.kind {
                        Kind::Present => (&self.pipeline_layout, None),
                        Kind::Text => (
                            &self.text_pipeline_layout,
                            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        ),
                        Kind::Overlay => (
                            &self.ov_pipeline_layout,
                            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        ),
                        _ => continue,
                    };
                    match self.compile_into(&common, &s.path, layout, OUT_FORMAT, blend) {
                        Ok(p) => match s.kind {
                            Kind::Present => self.out_present = Some(p),
                            Kind::Text => self.out_text = Some(p),
                            Kind::Overlay => self.out_overlay = Some(p),
                            _ => {}
                        },
                        Err(e) => eprintln!("shader {} (output) failed:\n{e}", s.name),
                    }
                }
                // Keep the last good pipeline so a typo never blanks the screen.
                Err(e) => eprintln!("shader {} failed:\n{e}", s.name),
            }
        }
        self.scenes = scenes;
        self.present = present;
        self.dancer = dancer;
        self.text = text;
        self.overlay = overlay;
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
            Kind::Overlay => (
                self.config.format,
                &self.ov_pipeline_layout,
                Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            ),
        };
        self.compile_into(common, path, layout, format, blend)
    }

    /// Compile `path` for an explicit target format — used for the output
    /// tap's sibling present/text pipelines at `output::OUT_FORMAT`.
    fn compile_into(
        &self,
        common: &str,
        path: &Path,
        layout: &wgpu::PipelineLayout,
        format: wgpu::TextureFormat,
        blend: Option<wgpu::BlendState>,
    ) -> Result<wgpu::RenderPipeline> {
        compile_pipeline(&self.device, common, path, layout, format, blend)
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

    /// Upload a stream-overlay image into `layer` (see overlay.rs).
    pub fn set_overlay_image(&mut self, layer: usize, img: &crate::overlay::Image) {
        if layer >= OV_LAYERS {
            return;
        }
        self.ov_tex[layer] = Some(rgba_texture(&self.device, &self.queue, img.w, img.h, &img.px));
        let view = |i: usize| {
            self.ov_tex[i]
                .as_ref()
                .map_or(&self.ov_blank, |(_, v)| v)
        };
        self.ov_bg = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("overlay"),
            layout: &self.ov_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view(0)),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view(1)),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(view(2)),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.ov_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(view(3)),
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
        overlay: Option<&OvUniforms>,
    ) -> Result<()> {
        let (want_bloom, tonemap) = self
            .scenes
            .get(scene)
            .map_or((0.0, 0.0), |s| (s.bloom, s.tonemap));
        // Ease toward the scene's bloom so cuts cross-fade the glow.
        let k = (u.dt * 4.0).clamp(0.0, 1.0);
        self.bloom_amt += (want_bloom - self.bloom_amt) * k;
        if want_bloom == 0.0 && self.bloom_amt < 0.01 {
            self.bloom_amt = 0.0;
        }
        self.frame = self.frame.wrapping_add(1);
        let mut uu = *u;
        uu.bloom = self.bloom_amt;
        uu.tonemap = tonemap;
        uu.frame = (self.frame % 4096) as f32;
        uu.misc4[0] = if self.transparent { 1.0 } else { 0.0 };
        let u = &uu;
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
        if let Some(o) = overlay {
            self.queue
                .write_buffer(&self.ov_buf, 0, bytemuck::bytes_of(o));
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

        let scene_pipe = self.scenes.get(scene).and_then(|s| s.pipeline.as_ref());
        if scene_pipe.is_some() || self.transparent {
            let view = self.targets[next].create_view(&Default::default());
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Transparent mode: no scene, just the dancer on
                        // a cleared (alpha 0) target.
                        load: if self.transparent {
                            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_bind_group(0, &self.bind_groups[prev], &[]);
            if let (Some(pipeline), false) = (scene_pipe, self.transparent) {
                pass.set_pipeline(pipeline);
                pass.draw(0..3, 0..1);
            }
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
            if self.bloom_amt > 0.0 {
                let src = self.targets[next].create_view(&Default::default());
                self.bloom.encode(&self.device, &mut enc, &src);
            }
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
            if let (Some(_), Some(op), Some(obg)) =
                (overlay, self.overlay.pipeline.as_ref(), self.ov_bg.as_ref())
            {
                pass.set_pipeline(op);
                pass.set_bind_group(0, &self.bind_groups[self.current], &[]);
                pass.set_bind_group(1, obg, &[]);
                pass.draw(0..3, 0..1);
            }
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

        // External output (NDI): re-run present (+text) into the BGRA tap
        // texture and queue an async readback — same submit, so the render
        // thread never waits on the network or the map.
        let out_i = if let Some(o) = self.out.as_mut() {
            if o.due() { o.free_staging() } else { None }
        } else {
            None
        };
        if let (Some(o), Some(i)) = (self.out.as_ref(), out_i) {
            let mut ou = *u;
            ou.res_x = o.width as f32;
            ou.res_y = o.height as f32;
            self.queue
                .write_buffer(&o.uniform_buf, 0, bytemuck::bytes_of(&ou));
            if let Some(pipe) = self.out_present.as_ref() {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("output"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &o.view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                pass.set_pipeline(pipe);
                pass.set_bind_group(0, &o.bind_groups[self.current], &[]);
                pass.draw(0..3, 0..1);
                if let (Some(_), Some(op), Some(obg)) =
                    (overlay, self.out_overlay.as_ref(), self.ov_bg.as_ref())
                {
                    pass.set_pipeline(op);
                    pass.set_bind_group(0, &o.bind_groups[self.current], &[]);
                    pass.set_bind_group(1, obg, &[]);
                    pass.draw(0..3, 0..1);
                }
                if let (Some(_), Some(tp), Some(tbg)) =
                    (text, self.out_text.as_ref(), self.text_bg.as_ref())
                {
                    pass.set_pipeline(tp);
                    pass.set_bind_group(0, &o.bind_groups[self.current], &[]);
                    pass.set_bind_group(1, tbg, &[]);
                    pass.draw(0..3, 0..1);
                }
            }
            enc.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &o.tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &o.staging[i],
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(o.row_bytes),
                        rows_per_image: Some(o.height),
                    },
                },
                wgpu::Extent3d {
                    width: o.width,
                    height: o.height,
                    depth_or_array_layers: 1,
                },
            );
            self.out.as_mut().unwrap().mark_pending(i);
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
        // Kick the async readback maps now the submit has landed; the
        // callbacks fire on the next `device.poll` at the top of render().
        if let Some(o) = self.out.as_mut() {
            o.map_pending();
        }
        Ok(())
    }

    /// Enable/disable/reconfigure the external output. Cheap to call every
    /// frame — rebuilds only on a conf change or a throttled error retry.
    pub fn set_output(&mut self, conf: Option<output::Conf>) {
        if conf == self.out_conf {
            let settled = self.out.is_some() == conf.is_some();
            let retry_due = self
                .out_err
                .as_ref()
                .is_some_and(|(_, at, _)| at.elapsed() > Duration::from_secs(5));
            if settled && !retry_due {
                return;
            }
        }
        self.out_conf = conf.clone();
        self.out = None;
        let Some(c) = conf else {
            self.out_err = None;
            return;
        };
        match output::Output::new(
            &self.device,
            &self.layout,
            &self.sampler,
            &self.targets,
            &FrameRes {
                pal: &self.pal_view,
                noise3: &self.statics.noise3_view,
                blue: &self.statics.blue_view,
                repeat: &self.statics.repeat,
                bloom: self.bloom.view(),
                ext: &self.statics.ext_view,
            },
            c.clone(),
        ) {
            Ok(o) => {
                println!("output: {}", o.status());
                self.out = Some(o);
                self.out_err = None;
            }
            Err(e) => {
                let msg = format!("{e:#}");
                // One line per distinct failure — the retry loop would
                // otherwise spam the log every 5s.
                if self.out_err.as_ref().map(|(_, _, m)| m) != Some(&msg) {
                    eprintln!("output: {msg}");
                }
                self.out_err = Some((c, Instant::now(), msg));
            }
        }
    }

    /// Panel status line — None while output is off.
    pub fn output_status(&self) -> Option<String> {
        self.out_conf.as_ref()?;
        if let Some(o) = &self.out {
            return Some(o.status());
        }
        self.out_err
            .as_ref()
            .map(|(_, _, e)| e.clone())
            .or_else(|| Some("Output: starting…".into()))
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
            bloom: 0.0,
            tonemap: self.scenes[scene].tonemap,
            frame: 0.0,
            calm: 0.0,
            lvl4: [0.6, 0.5, 0.45, 0.4],
            hits4: [1.0, 0.5, 0.3, 0.3],
            pres4: [0.6, 0.5, 0.45, 0.4],
            clock4: [8.0, 8.0, 8.0, 8.0],
            misc4: [0.0; 4],
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
            &FrameRes {
                pal: &self.pal_view,
                noise3: &self.statics.noise3_view,
                blue: &self.statics.blue_view,
                repeat: &self.statics.repeat,
                bloom: self.bloom.view(),
                ext: &self.statics.ext_view,
            },
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

/// An sRGB RGBA8 texture filled with `px` (straight alpha).
fn rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    w: u32,
    h: u32,
    px: &[u8],
) -> (wgpu::Texture, wgpu::TextureView) {
    let size = wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("overlay"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        px,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: Some(h),
        },
        size,
    );
    let view = texture.create_view(&Default::default());
    (texture, view)
}

/// Compile `common + path` into a fullscreen-triangle pipeline for `format`.
pub fn compile_pipeline(
device: &wgpu::Device,
    common: &str,
    path: &Path,
    layout: &wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> Result<wgpu::RenderPipeline> {
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

    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: path.to_str(),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `// @title` in the first eight lines wins the display name; the rest
    /// of the header tags still parse alongside it.
    #[test]
    fn crowd_scenes_keep_the_dancer_off() {
        for name in ["stage_rig", "laser_show"] {
            let body = std::fs::read_to_string(format!("shaders/scenes/{name}.wgsl")).unwrap();
            assert!(header_no_dancer(&body), "{name} draws a crowd: needs @no-dancer");
            assert_eq!(header_tags(&body).1 > 0.0, true, "{name}: the tag broke @bloom parsing");
        }
    }

    #[test]
    fn header_title_parses_with_other_tags() {
        let body = "// @title Neon Alley\n// @heavy\n// @bloom 0.7\n// @tonemap agx\n\n@fragment\n";
        assert_eq!(header_title(body).as_deref(), Some("Neon Alley"));
        let (heavy, bloom, tonemap) = header_tags(body);
        assert!(heavy);
        assert!((bloom - 0.7).abs() < 1e-6);
        assert!((tonemap - 1.0).abs() < 1e-6);
    }

    #[test]
    fn header_title_ignored_past_the_header() {
        // Only the first eight lines count — a stray mention deeper in the
        // file must not become the scene's name.
        let mut body = String::from("// comment\n\n\n\n\n\n\n\n\nfn f() {}\n");
        body.push_str("// @title Not A Title\n");
        assert_eq!(header_title(&body), None);
        assert_eq!(header_title("// @title\n// nothing after the tag\n"), None);
    }
}
