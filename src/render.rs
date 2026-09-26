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
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::{anyhow, Context, Result};
use winit::window::Window;

use crate::audio::SPECTRUM_BINS;
use crate::dancer::{Clip, DancerUniforms};

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
    pub _pad: [f32; 2],
    pub spectrum: [f32; SPECTRUM_BINS],
}

#[derive(Default, Clone, Copy, PartialEq)]
enum Kind {
    #[default]
    Scene,
    Present,
    Dancer,
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
    /// Mask texture array + bind group for the loaded clip.
    dancer_clip: Option<(wgpu::Texture, wgpu::BindGroup)>,
}

pub fn find_shader_dir() -> Result<PathBuf> {
    let mut candidates = vec![PathBuf::from("shaders")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("shaders"));
        }
    }
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders"));
    candidates
        .into_iter()
        .find(|p| p.join("common.wgsl").exists())
        .ok_or_else(|| anyhow!("could not find a shaders/ directory containing common.wgsl"))
}

fn scaled(w: u32, h: u32, scale: f32) -> (u32, u32) {
    (((w as f32 * scale) as u32).max(1), ((h as f32 * scale) as u32).max(1))
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

impl Renderer {
    pub async fn new(window: Arc<Window>, low_power: bool, scale: Option<f32>) -> Result<Self> {
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
        let scale = scale.unwrap_or(if integrated { 0.75 } else { 1.0 }).clamp(0.25, 1.0);
        println!("GPU: {} ({:?}), render scale {:.0}%", info.name, info.backend, scale * 100.0);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor { label: Some("trippin"), ..Default::default() })
            .await?;

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .context("surface not supported by adapter")?;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);

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
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
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
        let dancer_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
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

        let (tw, th) = scaled(config.width, config.height, scale);
        let targets = Self::make_targets(&device, tw, th);
        let bind_groups = Self::make_bind_groups(&device, &layout, &uniform_buf, &sampler, &targets);

        let shader_dir = find_shader_dir()?;
        println!("Shaders: {}", shader_dir.display());
        let present_path = shader_dir.join("present.wgsl");
        let dancer_path = shader_dir.join("dancer.wgsl");
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
            present: Scene { kind: Kind::Present, name: "present".into(), path: present_path, ..Default::default() },
            dancer: Scene { kind: Kind::Dancer, name: "dancer".into(), path: dancer_path, ..Default::default() },
            dancer_layout,
            dancer_pipeline_layout,
            dancer_buf,
            dancer_clip: None,
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
                size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
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
                    wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
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
        self.surface.configure(&self.device, &self.config);
        let (tw, th) = scaled(w, h, self.scale);
        self.targets = Self::make_targets(&self.device, tw, th);
        self.bind_groups =
            Self::make_bind_groups(&self.device, &self.layout, &self.uniform_buf, &self.sampler, &self.targets);
    }

    /// Size of the scene render targets (what shaders see as the resolution).
    pub fn gpu(&self) -> Gpu {
        Gpu {
            instance: self.instance.clone(),
            adapter: self.adapter.clone(),
            device: self.device.clone(),
            queue: self.queue.clone(),
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
        (0..self.scenes.len()).filter(|&i| self.scenes[i].pipeline.is_some()).collect()
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
                self.scenes.push(Scene { name, path: p, ..Default::default() });
            }
        }

        let mut scenes = std::mem::take(&mut self.scenes);
        let mut present = std::mem::take(&mut self.present);
        let mut dancer = std::mem::take(&mut self.dancer);
        for s in scenes.iter_mut().chain([&mut present, &mut dancer]) {
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
        };
        let body = std::fs::read_to_string(path)?;
        let src = format!("{common}\n{body}");
        // Validate with naga first to get readable errors instead of a panic.
        let module = wgpu::naga::front::wgsl::parse_str(&src).map_err(|e| anyhow!(e.emit_to_string(&src)))?;
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .map_err(|e| anyhow!(e.emit_to_string(&src)))?;

        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: path.to_str(),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(anyhow!("{e}"));
        }
        Ok(pipeline)
    }

    /// Upload a dancer clip's masks as a texture array (replacing any previous one).
    pub fn set_dancer_clip(&mut self, clip: &Clip) {
        let size = wgpu::Extent3d { width: clip.width, height: clip.height, depth_or_array_layers: 1 };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("dancer masks"),
            size: wgpu::Extent3d { depth_or_array_layers: clip.frames.len() as u32, ..size },
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
                    origin: wgpu::Origin3d { x: 0, y: 0, z: i as u32 },
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
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dancer"),
            layout: &self.dancer_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: self.dancer_buf.as_entire_binding() },
            ],
        });
        self.dancer_clip = Some((texture, bind_group));
    }

    /// Render `scene` (plus the dancer layer, if given) into the next
    /// ping-pong target, then present it.
    pub fn render(&mut self, scene: usize, u: &Uniforms, dancer: Option<&DancerUniforms>) -> Result<()> {
        self.queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(u));
        if let Some(d) = dancer {
            self.queue.write_buffer(&self.dancer_buf, 0, bytemuck::bytes_of(d));
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            _ => return Ok(()),
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
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.bind_groups[prev], &[]);
            pass.draw(0..3, 0..1);
            if let (Some(_), Some(dancer), Some((_, dancer_bg))) =
                (dancer, self.dancer.pipeline.as_ref(), self.dancer_clip.as_ref())
            {
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
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(self.present.pipeline.as_ref().unwrap());
            pass.set_bind_group(0, &self.bind_groups[self.current], &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([enc.finish()]);
        self.window.pre_present_notify();
        self.queue.present(frame);
        Ok(())
    }
}
