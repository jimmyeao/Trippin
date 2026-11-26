//! Headless scene snapshots + timing: `trippin --snap <scenes>`.
//!
//! Renders each scene offscreen through the real scene → bloom → present
//! chain with a synthetic 126 BPM groove, saves PNGs at the requested
//! times, and times a batch of frames at the snapshot size. No window, no
//! audio device — for iterating on scenes and for GPU-tier budgeting
//! (`--snap-size 1920x1080` on the dev box vs the Mac).
//!
//! Flags: `--snap a,b,c` (or `all`), `--snap-size WxH` (default 1280x720),
//! `--snap-at 2,6` (seconds, default 6), `--snap-out dir` (default
//! `snaps/`), `--snap-bench N` (frames timed, default 60; 0 = skip),
//! `--gpu low`, `--snap-dancer <style>` (a dancer look by name or index,
//! drawn over each scene with the real dancer shader) with `--snap-clip
//! <name>` to pick the routine (default: the first clip), `--snap-burst
//! <0..1>` (hold the dancer's Disintegrate at that level) or `--snap-burst-at
//! <s>` (set one off at that time, for `--snap-at` sequences),
//! `--snap-accent <scene>` (that scene as the accent layer over each one,
//! at `--snap-accent-amt`, default 0.6), `--snap-echo <0..1>` (the echo
//! post effect at that amount), `--snap-logo <png>` (the branding logo in
//! the top-right corner, bursting with `--snap-burst`/`--snap-burst-at`).
//! `--snap-track <audio>` drives the scenes with that track run through the
//! real analyser instead of the synthetic groove (from `--snap-track-from
//! <s>`, default 60). `--snap-react <s>` (needs `--snap-track`) renders that
//! many seconds and prints how much each scene's picture moves with the
//! music: the change just after each beat against mid-beat, and how the
//! change follows loudness (`lvl4`) and kicks (`hits4.x`). Writes `react.tsv`.
//! `--snap-fx <mode>` renders with a post effect at full strength (the
//! `u.fx` modes in present.wgsl: 1 mirror X ... 5 kaleido 8).
//! `--snap-safe` turns the photosensitive-safe limiter on (present.wgsl
//! `fs_lum`). `--snap-flash <s>` renders that many seconds of each scene,
//! reads every frame back and prints the most flashes in any one second
//! (WCAG 2.3.1 general flashes: opposing luminance swings of 0.1 or more
//! with the darker side under 0.8); over 3 fails. Writes `flash.tsv`.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};

use crate::audio::SPECTRUM_BINS;
use crate::dancer::{self, DancerUniforms, SlotUniforms};
use crate::gfx;
use crate::palettes;
use crate::render::{self, FrameRes, Renderer, SCENE_FORMAT, Uniforms};

const OUT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

fn arg<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == k)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// A plausible house groove at `t` seconds, 126 BPM.
fn groove(t: f32, w: u32, h: u32, frame: u32, bloom: f32, tonemap: f32, calm: f32) -> Uniforms {
    let bpm = 126.0;
    let beat = t * bpm / 60.0;
    let ph = beat.fract();
    // In a breakdown there are no drums: no kick, no onsets.
    let kick = (-ph * 7.0).exp() * (1.0 - calm);
    let bar = (beat / 4.0).fract();
    let mut spectrum = [0.0f32; SPECTRUM_BINS];
    for (i, v) in spectrum.iter_mut().enumerate() {
        let x = i as f32 / SPECTRUM_BINS as f32;
        let wob = 0.5 + 0.5 * (t * (1.3 + x * 3.0) + i as f32 * 1.7).sin();
        *v = ((1.0 - x) * 0.6 * (0.5 + 0.5 * kick) + wob * 0.3 * (0.4 + x)).clamp(0.0, 1.0);
    }
    let mut waveform = [[0.0f32; 4]; 16];
    for (i, v) in waveform.iter_mut().flatten().enumerate() {
        *v = (i as f32 * 0.4 + t * 40.0).sin() * (0.3 + 0.5 * kick);
    }
    Uniforms {
        time: t,
        dt: 1.0 / 60.0,
        res_x: w as f32,
        res_y: h as f32,
        bass: 0.4 + 0.5 * kick,
        mid: 0.5,
        high: 0.35 + 0.2 * (t * 3.0).sin().abs(),
        energy: 0.6,
        onset: if ph < 0.05 { 1.0 - calm } else { 0.0 },
        kick,
        beat,
        beat_phase: ph,
        bar_phase: bar,
        bpm,
        build: 0.0,
        scene_time: t,
        intensity: 0.7 - 0.45 * calm,
        hue: 0.0,
        seed: 17.0,
        flash: 0.0,
        flow: beat,
        master: 1.0,
        fx: 0.0,
        fx_amt: 0.0,
        spectrum,
        waveform,
        bloom,
        tonemap,
        frame: (frame % 4096) as f32,
        calm,
        lvl4: [0.4 + 0.5 * kick, 0.5, 0.4, 0.35],
        hits4: [kick, if ph < 0.05 { 0.6 } else { 0.0 } * (1.0 - calm), 0.0, if (beat * 2.0).fract() < 0.05 { 0.5 } else { 0.0 }],
        pres4: [0.55 - 0.3 * calm, 0.5, 0.45, 0.4],
        clock4: [beat * (1.0 - 0.3 * calm); 4],
        misc4: [0.0; 4],
        look4: [1.0, 1.0, 0.0, 0.0],
    }
}

pub fn run(args: &[String]) -> Result<()> {
    let which = arg(args, "--snap").unwrap_or("all");
    let (w, h) = arg(args, "--snap-size")
        .and_then(|s| s.split_once('x'))
        .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
        .unwrap_or((1280u32, 720u32));
    // `--snap-energy`: rate every scene's visual energy (motion, brightness,
    // colour) from frame pairs 1/30 s apart, written to
    // shaders/scene_energy.json for the AI show builder.
    let energy_mode = args.iter().any(|a| a == "--snap-energy");
    let (w, h) = if energy_mode && arg(args, "--snap-size").is_none() {
        (320, 180)
    } else {
        (w, h)
    };
    let times: Vec<f32> = if energy_mode {
        vec![3.0, 3.034, 5.0, 5.034, 7.0, 7.034]
    } else {
        arg(args, "--snap-at")
            .map(|s| s.split(',').filter_map(|v| v.parse().ok()).collect())
            .unwrap_or_else(|| vec![6.0])
    };
    let bench: u32 = if energy_mode {
        0
    } else {
        arg(args, "--snap-bench")
            .and_then(|s| s.parse().ok())
            .unwrap_or(60)
    };
    let mut energy_rows: Vec<(String, f32, f32, f32)> = Vec::new();
    let out_dir = PathBuf::from(arg(args, "--snap-out").unwrap_or("snaps"));
    std::fs::create_dir_all(&out_dir)?;
    let low = arg(args, "--gpu") == Some("low");
    // `--snap-calm 1` previews breakdown mode (no drums).
    let calm: f32 = arg(args, "--snap-calm").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let burst_fixed: Option<f32> = arg(args, "--snap-burst").and_then(|s| s.parse().ok());
    let burst_at: Option<f32> = arg(args, "--snap-burst-at").and_then(|s| s.parse().ok());

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: if low {
            wgpu::PowerPreference::LowPower
        } else {
            wgpu::PowerPreference::HighPerformance
        },
        compatible_surface: None,
        force_fallback_adapter: false,
        ..Default::default()
    }))
    .context("no GPU adapter")?;
    let info = adapter.get_info();
    println!("GPU: {} ({:?}) — {w}x{h}", info.name, info.backend);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("snap"),
        ..Default::default()
    }))?;

    let layout = render::frame_layout(&device);
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("snap"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let sim_kit = crate::sim::SimKit::new(&device, &layout);
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::MirrorRepeat,
        address_mode_v: wgpu::AddressMode::MirrorRepeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let ubuf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("snap uniforms"),
        size: std::mem::size_of::<Uniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let pal = device.create_texture(&wgpu::TextureDescriptor {
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
    // `--snap-art <audio or image file>`: render with that cover's palette,
    // and write the cover beside its palette strip (`art_<file>.png`).
    let mut pal_name = arg(args, "--palette").unwrap_or("rainbow");
    if let Some(f) = arg(args, "--snap-art") {
        let p = std::path::Path::new(f);
        let bytes = crate::albumart::embedded(p)
            .or_else(|| std::fs::read(p).ok())
            .ok_or_else(|| anyhow::anyhow!("--snap-art: no cover in {f}"))?;
        let a = crate::albumart::build(&bytes, f.to_string()).ok_or_else(|| anyhow::anyhow!("--snap-art: {f} didn't decode"))?;
        palettes::set_art(Some((1, a.lut.clone())));
        pal_name = palettes::ART;
        let mut sheet = image::RgbaImage::new(128 + 256, 128);
        let small = image::imageops::resize(&*a.thumb, 128, 128, image::imageops::FilterType::Triangle);
        image::imageops::overlay(&mut sheet, &small, 0, 0);
        for x in 0..256u32 {
            let c = &a.lut[x as usize * 4..x as usize * 4 + 4];
            for y in 0..128 {
                sheet.put_pixel(128 + x, y, image::Rgba([c[0], c[1], c[2], 255]));
            }
        }
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        sheet.save(out_dir.join(format!("art_{stem}.png")))?;
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &pal,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &palettes::lut(pal_name),
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
    let pal_view = pal.create_view(&Default::default());
    let statics = gfx::Statics::new(&device, &queue);
    // Type scenes: a sample now-playing line (`--snap-title` to change it).
    let title_frac = gfx::title_bitmap(arg(args, "--snap-title").unwrap_or("Daft Punk - One More Time"))
        .map_or(0.0, |b| statics.upload_title(&queue, &b));
    let bloom = gfx::Bloom::new(&device, w, h);
    let targets = Renderer::make_targets(&device, w, h);
    let bgs = Renderer::make_bind_groups(
        &device,
        &layout,
        &ubuf,
        &sampler,
        &targets,
        &FrameRes {
            pal: &pal_view,
            noise3: &statics.noise3_view,
            blue: &statics.blue_view,
            repeat: &statics.repeat,
            bloom: bloom.view(),
            ext: &statics.ext_view,
            vid: &statics.vid_view,
            title: &statics.title_view,
            lum: &statics.lum,
        },
    );
    let out = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("snap out"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OUT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let out_view = out.create_view(&Default::default());

    let dir = render::find_shader_dir()?;
    let common = std::fs::read_to_string(dir.join("common.wgsl"))?;
    let present = render::compile_pipeline(&device, &common, &dir.join("present.wgsl"), &pl, OUT, None)
        .map_err(|e| anyhow!("present: {e}"))?;
    // `--snap-safe`: the photosensitive-safe limiter, as the app runs it.
    let safe = args.iter().any(|a| a == "--snap-safe");
    let flash_s: f32 = arg(args, "--snap-flash").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let mut flash_rows: Vec<(String, u32)> = Vec::new();
    let track = match arg(args, "--snap-track") {
        Some(p) => {
            let (n, fr) = crate::audio::track_uniforms(std::path::Path::new(p))?;
            println!("track: {n} ({} frames)", fr.len());
            Some(fr)
        }
        None => None,
    };
    let track_from: f32 = arg(args, "--snap-track-from").and_then(|s| s.parse().ok()).unwrap_or(60.0);
    let react_s: f32 = arg(args, "--snap-react").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    // `--snap-fx <mode>`: a post effect at full strength (1 mirror X ... 5 kaleido 8).
    let snap_fx: f32 = arg(args, "--snap-fx").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let mut react_rows: Vec<(String, f32, f32, f32, f32)> = Vec::new();
    // The track's uniforms at snapshot time t (None: the synthetic groove).
    let track_at = |t: f32| -> Option<Uniforms> {
        let tr = track.as_ref()?;
        let i = (((track_from + t) * 60.0) as usize).min(tr.len().saturating_sub(1));
        tr.get(i).copied()
    };
    let lum_pipe = if safe {
        Some(
            render::compile_pipeline_entries(
                &device,
                &common,
                &dir.join("present.wgsl"),
                &pl,
                wgpu::TextureFormat::Rgba8Unorm,
                None,
                ("vs_main", "fs_lum"),
            )
            .map_err(|e| anyhow!("fs_lum: {e}"))?,
        )
    } else {
        None
    };
    let lum_view = device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("safe lum"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default());

    // `--snap-dancer <style>`: draw a dancer clip over every scene with the
    // real dancer shader (same mask texture array + bind group as the app).
    struct DancerSnap {
        pipeline: wgpu::RenderPipeline,
        particles: wgpu::RenderPipeline,
        bind_group: wgpu::BindGroup,
        buf: wgpu::Buffer,
        frames: f32,
        duration: f32,
        aspect: f32,
        style: f32,
    }
    let dancer_snap: Option<DancerSnap> = match arg(args, "--snap-dancer") {
        None => None,
        Some(want) => {
            let style = want
                .parse::<usize>()
                .ok()
                .or_else(|| dancer::STYLES.iter().position(|n| *n == want))
                .ok_or_else(|| anyhow!("unknown dancer style {want} (try {:?})", dancer::STYLES))?;
            let clips_dir = dancer::find_dancer_dir().ok_or_else(|| anyhow!("no dancers/ folder"))?;
            let entries = dancer::list_clips(&clips_dir);
            let entry = match arg(args, "--snap-clip") {
                Some(n) => entries.iter().find(|e| e.name == n),
                None => entries.first(),
            }
            .ok_or_else(|| anyhow!("no such dancer clip"))?;
            let clip = dancer::load_clip(&entry.path)?;
            println!("dancer: {} ({} frames) style {}", clip.name, clip.frames.len(), dancer::STYLES[style]);
            let size = wgpu::Extent3d { width: clip.width, height: clip.height, depth_or_array_layers: 1 };
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("snap dancer masks"),
                size: wgpu::Extent3d { depth_or_array_layers: clip.frames.len() as u32, ..size },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            for (i, frame) in clip.frames.iter().enumerate() {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &tex,
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
            let view = tex.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let tex_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: render::VIS,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            };
            let dl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("snap dancer"),
                entries: &[
                    tex_entry(0),
                    tex_entry(1),
                    tex_entry(2),
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: render::VIS,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
            let dpl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("snap dancer"),
                bind_group_layouts: &[Some(&layout), Some(&dl)],
                immediate_size: 0,
            });
            let pipeline = render::compile_pipeline(
                &device,
                &common,
                &dir.join("dancer.wgsl"),
                &dpl,
                SCENE_FORMAT,
                Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            )
            .map_err(|e| anyhow!("dancer: {e}"))?;
            let particles = render::compile_pipeline_entries(
                &device,
                &common,
                &dir.join("dancer.wgsl"),
                &dpl,
                SCENE_FORMAT,
                Some(render::GLOW_BLEND),
                ("vs_particles", "fs_particles"),
            )
            .map_err(|e| anyhow!("dancer particles: {e}"))?;
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("snap dancer uniforms"),
                size: std::mem::size_of::<DancerUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("snap dancer"),
                layout: &dl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 3, resource: buf.as_entire_binding() },
                ],
            });
            Some(DancerSnap {
                pipeline,
                particles,
                bind_group,
                buf,
                frames: clip.frames.len() as f32,
                duration: clip.duration.max(0.1),
                aspect: clip.width as f32 / clip.height as f32,
                style: style as f32,
            })
        }
    };

    // `--snap-accent <scene>`: the accent layer, as the app draws it — its
    // own feedback pair, added into the scene target by accent.wgsl.
    struct AccentSnap {
        pipeline: wgpu::RenderPipeline,
        mix: wgpu::RenderPipeline,
        bgs: [wgpu::BindGroup; 2],
        mix_bgs: [[wgpu::BindGroup; 2]; 2],
        targets: [wgpu::Texture; 2],
        amt: f32,
    }
    let targets_main = &targets;
    let accent_snap: Option<AccentSnap> = match arg(args, "--snap-accent") {
        None => None,
        Some(name) => {
            // An accent overlay (shaders/accents/), or any scene.
            let path = render::accent_files(&dir)
                .into_iter()
                .chain(crate::packs::scene_files_all(&dir))
                .find(|p| p.file_stem().is_some_and(|s| s == name))
                .ok_or_else(|| anyhow!("no accent scene {name}"))?;
            let pipeline = render::compile_pipeline(&device, &common, &path, &pl, SCENE_FORMAT, None)
                .map_err(|e| anyhow!("accent {name}: {e}"))?;
            let mix = render::compile_pipeline(
                &device,
                &common,
                &dir.join("accent.wgsl"),
                &pl,
                SCENE_FORMAT,
                Some(render::ADD_BLEND),
            )
            .map_err(|e| anyhow!("accent mix: {e}"))?;
            let targets = Renderer::make_targets(&device, w, h);
            let bgs = Renderer::make_bind_groups(
                &device,
                &layout,
                &ubuf,
                &sampler,
                &targets,
                &FrameRes {
                    pal: &pal_view,
                    noise3: &statics.noise3_view,
                    blue: &statics.blue_view,
                    repeat: &statics.repeat,
                    bloom: bloom.view(),
                    ext: &statics.ext_view,
                    vid: &statics.vid_view,
                    title: &statics.title_view,
                    lum: &statics.lum,
                },
            );
            let mix_bgs = Renderer::make_mix_bind_groups(
                &device,
                &layout,
                &ubuf,
                &sampler,
                &targets_main,
                &targets,
                &FrameRes {
                    pal: &pal_view,
                    noise3: &statics.noise3_view,
                    blue: &statics.blue_view,
                    repeat: &statics.repeat,
                    bloom: bloom.view(),
                    ext: &statics.ext_view,
                    vid: &statics.vid_view,
                    title: &statics.title_view,
                    lum: &statics.lum,
                },
            );
            let amt = arg(args, "--snap-accent-amt").and_then(|s| s.parse().ok()).unwrap_or(0.6);
            Some(AccentSnap { pipeline, mix, targets, bgs, mix_bgs, amt })
        }
    };
    let acc_cur = std::cell::Cell::new(0usize);

    // `--snap-echo <amount>`: the echo post effect, as the app runs it — its
    // own feedback pair; present reads the echo.
    struct EchoSnap {
        pipeline: wgpu::RenderPipeline,
        targets: [wgpu::Texture; 2],
        bgs: [wgpu::BindGroup; 2],
        pass_bgs: [[wgpu::BindGroup; 2]; 2],
        amt: f32,
    }
    let echo_snap: Option<EchoSnap> = match arg(args, "--snap-echo").and_then(|s| s.parse::<f32>().ok()) {
        None => None,
        Some(amt) => {
            let pipeline = render::compile_pipeline(&device, &common, &dir.join("echo.wgsl"), &pl, SCENE_FORMAT, None)
                .map_err(|e| anyhow!("echo: {e}"))?;
            let etargets = Renderer::make_targets(&device, w, h);
            let res = FrameRes {
                pal: &pal_view,
                noise3: &statics.noise3_view,
                blue: &statics.blue_view,
                repeat: &statics.repeat,
                bloom: bloom.view(),
                ext: &statics.ext_view,
                vid: &statics.vid_view,
                title: &statics.title_view,
                lum: &statics.lum,
            };
            let bgs = Renderer::make_bind_groups(&device, &layout, &ubuf, &sampler, &etargets, &res);
            let pass_bgs = Renderer::make_mix_bind_groups(&device, &layout, &ubuf, &sampler, &etargets, targets_main, &res);
            Some(EchoSnap { pipeline, targets: etargets, bgs, pass_bgs, amt })
        }
    };
    let echo_cur = std::cell::Cell::new(0usize);

    // `--snap-logo <png>`: the branding logo overlay (layer 1) as the app
    // draws it in the present pass, plus its Disintegrate dust.
    struct LogoSnap {
        overlay: wgpu::RenderPipeline,
        particles: wgpu::RenderPipeline,
        bg: wgpu::BindGroup,
        buf: wgpu::Buffer,
        quad: [f32; 4],
    }
    let logo_snap: Option<LogoSnap> = match arg(args, "--snap-logo") {
        None => None,
        Some(path) => {
            let img = image::open(path).map_err(|e| anyhow!("logo {path}: {e}"))?.to_rgba8();
            let brand = crate::overlay::render_brand(Some(&img), "", "", [0.9, 0.3, 1.0])
                .ok_or_else(|| anyhow!("logo {path}: nothing to draw"))?;
            let (_t, view) = render::rgba_texture(&device, &queue, brand.w, brand.h, &brand.px);
            let (_b, blank) = render::rgba_texture(&device, &queue, 1, 1, &[0, 0, 0, 0]);
            let tex = |binding: u32| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: render::VIS,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            };
            let ol = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("snap overlay"),
                entries: &[
                    tex(0),
                    tex(1),
                    tex(2),
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: render::VIS,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    tex(4),
                ],
            });
            let opl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("snap overlay"),
                bind_group_layouts: &[Some(&layout), Some(&ol)],
                immediate_size: 0,
            });
            let ovp = dir.join("overlay.wgsl");
            let overlay = render::compile_pipeline(
                &device,
                &common,
                &ovp,
                &opl,
                OUT,
                Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            )
            .map_err(|e| anyhow!("overlay: {e}"))?;
            let particles = render::compile_pipeline_entries(
                &device,
                &common,
                &ovp,
                &opl,
                OUT,
                Some(render::GLOW_BLEND),
                ("vs_logo", "fs_logo"),
            )
            .map_err(|e| anyhow!("logo particles: {e}"))?;
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("snap overlay uniforms"),
                size: std::mem::size_of::<crate::overlay::OvUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("snap overlay"),
                layout: &ol,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&blank) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&blank) },
                    wgpu::BindGroupEntry { binding: 3, resource: buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&blank) },
                ],
            });
            // Top-right corner, 0.36 of the screen height tall.
            let hh = 0.18;
            let hw = hh * brand.w as f32 / brand.h.max(1) as f32;
            let asp = w as f32 / h as f32;
            let quad = [asp - hw - 0.08, -1.0 + hh + 0.08, hw, hh];
            Some(LogoSnap { overlay, particles, bg, buf, quad })
        }
    };

    // Built in plus the effects packs' scenes (packs.rs).
    let mut scenes: Vec<PathBuf> = crate::packs::scene_files_all(&dir);
    if which != "all" {
        let want: Vec<&str> = which.split(',').collect();
        scenes.retain(|p| {
            want.contains(&p.file_stem().unwrap().to_string_lossy().as_ref())
        });
    }
    if scenes.is_empty() {
        return Err(anyhow!("no scenes matched {which}"));
    }

    let mut report = Vec::new();
    for path in &scenes {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let body = std::fs::read_to_string(path)?;
        let (_, bloom_amt, tonemap) = render::header_tags(&body);
        let pipe = match render::compile_pipeline(&device, &common, path, &pl, SCENE_FORMAT, None) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("FAIL {name}:\n{e}");
                continue;
            }
        };
        // Particle scenes (`// @particles N`): their instanced dots too.
        let npart = render::header_particles(&body);
        let parts = if npart > 0 {
            match render::compile_particles(&device, &common, path, &pl) {
                Ok(p) => Some(p),
                Err(e) => {
                    eprintln!("FAIL {name} (particles):\n{e}");
                    continue;
                }
            }
        } else {
            None
        };
        // Sim scenes (`// @sim N`): their step and draw, and the particles
        // (spawned on the first frame).
        let nsim = crate::sim::header_sim(&body);
        let sim_pipes = if nsim > 0 {
            let cs = crate::sim::compile_compute(&device, &common, path, &sim_kit);
            let draw = render::compile_pipeline_entries(
                &device,
                &common,
                path,
                &sim_kit.draw_pl,
                SCENE_FORMAT,
                Some(render::GLOW_BLEND),
                ("vs_particles", "fs_particles"),
            );
            match (cs, draw) {
                (Ok(c), Ok(d)) => Some((c, d)),
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("FAIL {name} (sim):\n{e}");
                    continue;
                }
            }
        } else {
            None
        };
        let sim_buf = std::cell::RefCell::new(sim_pipes.as_ref().map(|_| crate::sim::SimBuf::new(&device, &sim_kit, nsim)));
        let sim_fresh = std::cell::Cell::new(true);
        // Clear both feedback targets so scenes start from black.
        {
            let mut enc = device.create_command_encoder(&Default::default());
            for t in &targets {
                let v = t.create_view(&Default::default());
                enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &v,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
            }
            queue.submit([enc.finish()]);
        }
        let mut cur = 0usize;
        let mut frame = 0u32;
        // Each scene starts the limiter afresh, as switching it on does.
        queue.write_buffer(&statics.lum, 0, bytemuck::cast_slice(&gfx::lum_init(safe)));
        // video_* scenes: the clip plays at 1x on the snapshot clock, or on
        // the groove's beat grid for bar-locked scenes (video::snap_time).
        let vclip = name
            .strip_prefix("video_")
            .and_then(crate::video::clip_path)
            .and_then(|p| crate::video::Clip::open(&p).ok());
        if name.starts_with("video_") && vclip.is_none() {
            eprintln!("{name}: no clip on disk (videos/ or TRIPPIN_VIDEOS) — it renders black");
        }
        // One full frame: scene → bloom → present.
        let step = |t: f32, frame: u32, cur: &mut usize| {
            let mut u = groove(t, w, h, frame, bloom_amt, tonemap, calm);
            if let Some(x) = track_at(t) {
                // The music from the track; the frame's own state from the groove.
                let g = u;
                u = x;
                (u.time, u.dt, u.res_x, u.res_y, u.frame) = (g.time, g.dt, g.res_x, g.res_y, g.frame);
                (u.bloom, u.tonemap, u.scene_time, u.hue, u.seed) = (g.bloom, g.tonemap, g.scene_time, g.hue, g.seed);
                (u.misc4, u.look4, u.fx, u.fx_amt) = (g.misc4, g.look4, g.fx, g.fx_amt);
                u.intensity = g.intensity.max(u.intensity);
            }
            u.look4[3] = title_frac;
            if snap_fx > 0.0 {
                (u.fx, u.fx_amt) = (snap_fx, 1.0);
            }
            if let Some(c) = &vclip {
                let len = c.frames() as f64 / c.fps as f64;
                let beat = t as f64 * 126.0 / 60.0;
                let vb = crate::video::Beat {
                    pos: beat,
                    bpm: 126.0,
                    beat_in_bar: (beat.floor() as u64) % 4,
                    calm,
                    ..Default::default()
                };
                let ct = crate::video::snap_time(&name, len, &vb).unwrap_or(t as f64);
                let fpos = ct * c.fps as f64;
                let f0 = fpos.floor() as usize;
                for (slot, f) in [(0, f0), (1, f0 + 1)] {
                    if let Ok(px) = c.decode(f) {
                        render::upload_video(&queue, &statics.vid, slot, &px);
                    }
                }
                u.misc4[1] = fpos.fract() as f32;
            }
            if let Some(a) = &accent_snap {
                u.misc4[2] = a.amt;
            }
            if let Some(e) = &echo_snap {
                u.misc4[3] = e.amt;
            }
            queue.write_buffer(&ubuf, 0, bytemuck::bytes_of(&u));
            let next = 1 - *cur;
            let mut enc = device.create_command_encoder(&Default::default());
            if let (Some((cs, _)), Some(b)) = (&sim_pipes, sim_buf.borrow_mut().as_mut()) {
                b.step(&queue, &mut enc, cs, &bgs[*cur], &u, sim_fresh.replace(false));
            }
            if let Some(a) = &accent_snap {
                let an = 1 - acc_cur.get();
                let v = a.targets[an].create_view(&Default::default());
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("accent"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &v,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                pass.set_pipeline(&a.pipeline);
                pass.set_bind_group(0, &a.bgs[acc_cur.get()], &[]);
                pass.draw(0..3, 0..1);
                drop(pass);
                acc_cur.set(an);
            }
            let view = targets[next].create_view(&Default::default());
            {
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
                pass.set_pipeline(&pipe);
                pass.set_bind_group(0, &bgs[*cur], &[]);
                pass.draw(0..3, 0..1);
                if let Some(pp) = &parts {
                    pass.set_pipeline(pp);
                    pass.draw(0..3, 0..npart);
                }
                if let (Some((_, dp)), Some(b)) = (&sim_pipes, sim_buf.borrow().as_ref()) {
                    pass.set_pipeline(dp);
                    pass.set_bind_group(1, b.draw_group(), &[]);
                    pass.draw(0..3, 0..nsim);
                }
                if let Some(a) = &accent_snap {
                    pass.set_pipeline(&a.mix);
                    pass.set_bind_group(0, &a.mix_bgs[acc_cur.get()][*cur], &[]);
                    pass.draw(0..3, 0..1);
                }
            }
            // `--snap-burst <level>` holds Disintegrate at a level;
            // `--snap-burst-at <s>` sets one off at that time (126 BPM).
            let burst_level = match (burst_fixed, burst_at) {
                (Some(l), _) => l,
                (None, Some(t0)) => dancer::Burst::level_at((t - t0) * 126.0 / 60.0),
                _ => 0.0,
            };
            if let Some(ls) = &logo_snap {
                let mut ou = crate::overlay::OvUniforms::default();
                ou.layers[1] = crate::overlay::OvLayerU { quad: ls.quad, uvx: [1.0, 0.0, 1.0, 0.0] };
                ou.burst = [burst_level, 0.3, 0.0, 0.0];
                queue.write_buffer(&ls.buf, 0, bytemuck::bytes_of(&ou));
            }
            if let Some(ds) = &dancer_snap {
                let frame_f = (t / ds.duration).rem_euclid(1.0) * ds.frames;
                let mut slots = [SlotUniforms::default(); dancer::SLOTS];
                slots[0] = SlotUniforms { frame: frame_f, frames: ds.frames, aspect: ds.aspect, _pad: 0.0 };
                let du = DancerUniforms {
                    slots,
                    opacity: 1.0,
                    style: ds.style,
                    count: 1.0,
                    scale: 0.8,
                    trail: 0.0,
                    canon_fade: 0.0,
                    burst: burst_level,
                    burst_seed: 0.3,
                };
                queue.write_buffer(&ds.buf, 0, bytemuck::bytes_of(&du));
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("dancer"),
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
                pass.set_pipeline(&ds.pipeline);
                pass.set_bind_group(0, &bgs[*cur], &[]);
                pass.set_bind_group(1, &ds.bind_group, &[]);
                pass.draw(0..3, 0..1);
                if burst_level > 0.0 {
                    pass.set_pipeline(&ds.particles);
                    pass.draw(0..6, 0..render::BURST_PARTICLES);
                }
            }
            if bloom_amt > 0.0 || (u.fx - crate::config::Fx::Anamorphic.index()).abs() < 0.5 {
                bloom.encode(&device, &mut enc, &view);
            }
            let mut present_bg = &bgs[next];
            if let Some(e) = &echo_snap {
                let en = 1 - echo_cur.get();
                let v = e.targets[en].create_view(&Default::default());
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("echo"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &v,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                pass.set_pipeline(&e.pipeline);
                pass.set_bind_group(0, &e.pass_bgs[next][echo_cur.get()], &[]);
                pass.draw(0..3, 0..1);
                drop(pass);
                echo_cur.set(en);
                present_bg = &e.bgs[en];
            }
            if let Some(lp) = &lum_pipe {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("safe lum"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &lum_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Discard,
                        },
                    })],
                    ..Default::default()
                });
                pass.set_pipeline(lp);
                pass.set_bind_group(0, present_bg, &[]);
                pass.draw(0..3, 0..1);
            }
            {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("present"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &out_view,
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
                pass.set_bind_group(0, present_bg, &[]);
                pass.draw(0..3, 0..1);
                if let Some(ls) = &logo_snap {
                    pass.set_pipeline(&ls.overlay);
                    pass.set_bind_group(1, &ls.bg, &[]);
                    pass.draw(0..3, 0..1);
                    if burst_level > 0.0 {
                        pass.set_pipeline(&ls.particles);
                        pass.draw(0..6, 0..render::LOGO_PARTICLES);
                    }
                }
            }
            queue.submit([enc.finish()]);
            *cur = next;
        };
        let mut t = 0.0f32;
        if react_s > 0.0 && track.is_some() {
            // Per-frame picture change against the music driving it.
            let luma = |px: &[u8]| -> Vec<f32> {
                px.chunks(4).map(|p| (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0).collect()
            };
            // A second of warm-up so feedback scenes settle.
            while t < 1.0 {
                step(t, frame, &mut cur);
                frame += 1;
                t += 1.0 / 60.0;
            }
            let mut prev = luma(&read_pixels(&device, &queue, &out, w, h)?);
            let (mut d, mut ph, mut lv, mut hk) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            while t < 1.0 + react_s {
                step(t, frame, &mut cur);
                frame += 1;
                let x = track_at(t).unwrap();
                t += 1.0 / 60.0;
                let cur_l = luma(&read_pixels(&device, &queue, &out, w, h)?);
                let diff = cur_l.iter().zip(&prev).map(|(a, b)| (a - b).abs()).sum::<f32>() / cur_l.len().max(1) as f32;
                prev = cur_l;
                d.push(diff);
                ph.push(x.beat_phase);
                lv.push(x.lvl4[0] * 0.45 + x.lvl4[1] * 0.3 + x.lvl4[2] * 0.15 + x.lvl4[3] * 0.1);
                hk.push(x.hits4[0]);
            }
            let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
            let on: Vec<f32> = d.iter().zip(&ph).filter(|(_, p)| **p < 0.25).map(|(x, _)| *x).collect();
            let off: Vec<f32> = d.iter().zip(&ph).filter(|(_, p)| (0.5..0.95).contains(*p)).map(|(x, _)| *x).collect();
            let beat = mean(&on) / mean(&off).max(1e-6);
            let (cl, ch, m) = (pearson(&d, &lv), pearson(&d, &hk), mean(&d));
            println!("{name:<24} motion {m:.4}  beat x{beat:.2}  loud r={cl:+.2}  kick r={ch:+.2}");
            react_rows.push((name, m, beat, cl, ch));
            continue;
        }
        if flash_s > 0.0 {
            // Every frame's mean displayed luminance (the target is sRGB:
            // decode before averaging).
            let lin = |v: u8| {
                let c = v as f32 / 255.0;
                if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
            };
            let mut series = Vec::new();
            while t < flash_s {
                step(t, frame, &mut cur);
                frame += 1;
                t += 1.0 / 60.0;
                let px = read_pixels(&device, &queue, &out, w, h)?;
                let n = (px.len() / 4).max(1) as f32;
                let sum: f32 = px
                    .chunks(4)
                    .map(|p| 0.2126 * lin(p[0]) + 0.7152 * lin(p[1]) + 0.0722 * lin(p[2]))
                    .sum();
                series.push(sum / n);
            }
            let worst = max_flashes_per_s(&series, 60);
            println!("{name:<22} {worst} flashes/s{}", if worst > 3 { "  FAIL" } else { "" });
            flash_rows.push((name, worst));
            continue;
        }
        let mut shots = times.clone();
        shots.sort_by(f32::total_cmp);
        let mut grabs: Vec<Vec<u8>> = Vec::new();
        for (k, &at) in shots.iter().enumerate() {
            while t < at {
                step(t, frame, &mut cur);
                frame += 1;
                t += 1.0 / 60.0;
                if frame % 30 == 0 {
                    let _ = device.poll(wgpu::PollType::wait_indefinitely());
                }
            }
            if energy_mode {
                grabs.push(read_pixels(&device, &queue, &out, w, h)?);
                continue;
            }
            let file = if shots.len() > 1 {
                out_dir.join(format!("{name}_{k}.png"))
            } else {
                out_dir.join(format!("{name}.png"))
            };
            save(&device, &queue, &out, w, h, &file)?;
        }
        if energy_mode {
            let (m, b, c) = frame_stats(&grabs);
            println!("{name:<18} motion {m:.4} bright {b:.3} colour {c:.3}");
            // unity_* shows only draw with the engine running: a snap is
            // black, and a 0.0 rating would mark them as dead calm.
            if !name.starts_with("unity_") {
                energy_rows.push((name, m, b, c));
            }
            continue;
        }
        if bench > 0 {
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
            let t0 = Instant::now();
            for _ in 0..bench {
                step(t, frame, &mut cur);
                frame += 1;
                t += 1.0 / 60.0;
            }
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
            let ms = t0.elapsed().as_secs_f64() * 1000.0 / bench as f64;
            println!("{name:<18} {ms:6.2} ms/frame");
            report.push((name, ms));
        } else {
            println!("{name}");
        }
    }
    if energy_mode && !energy_rows.is_empty() {
        // Rank-based so one outlier can't squash the scale: motion counts
        // double (motion, not brightness, is what reads as energy).
        let pct = |col: &dyn Fn(&(String, f32, f32, f32)) -> f32, v: f32| -> f32 {
            let below = energy_rows.iter().filter(|r| col(r) < v).count();
            below as f32 / (energy_rows.len() - 1).max(1) as f32
        };
        // Ranked over every scene, then each written where it lives: the
        // built-in file, or its pack's own (the AI builder merges them).
        let mut files: std::collections::BTreeMap<PathBuf, serde_json::Map<String, serde_json::Value>> =
            Default::default();
        for r in &energy_rows {
            let e = 0.5 * pct(&|x| x.1, r.1) + 0.25 * pct(&|x| x.2, r.2) + 0.25 * pct(&|x| x.3, r.3);
            let home = crate::packs::owner(&r.0).map_or_else(|| dir.clone(), |p| p.dir.clone());
            files
                .entry(home.join("scene_energy.json"))
                .or_default()
                .insert(r.0.clone(), serde_json::json!((e as f64 * 100.0).round() / 100.0));
        }
        for (file, out) in files {
            std::fs::write(&file, serde_json::to_string_pretty(&out)? + "\n")?;
            println!("wrote {}", file.display());
        }
    }
    if !react_rows.is_empty() {
        let lines: Vec<String> = react_rows
            .iter()
            .map(|(n, m, b, cl, ch)| format!("{n}\t{m:.4}\t{b:.2}\t{cl:.2}\t{ch:.2}"))
            .collect();
        std::fs::write(out_dir.join("react.tsv"), "scene\tmotion\tbeat\tloud_r\tkick_r\n".to_string() + &lines.join("\n") + "\n")?;
    }
    if !flash_rows.is_empty() {
        flash_rows.sort_by(|a, b| b.1.cmp(&a.1));
        let lines: Vec<String> = flash_rows.iter().map(|(n, f)| format!("{n}\t{f}")).collect();
        std::fs::write(out_dir.join("flash.tsv"), lines.join("\n") + "\n")?;
        let fails = flash_rows.iter().filter(|r| r.1 > 3).count();
        println!("{fails} of {} scenes over 3 flashes/s", flash_rows.len());
    }
    if !report.is_empty() {
        report.sort_by(|a, b| b.1.total_cmp(&a.1));
        let lines: Vec<String> = report.iter().map(|(n, ms)| format!("{n}\t{ms:.3}")).collect();
        std::fs::write(out_dir.join("bench.tsv"), lines.join("\n") + "\n")?;
    }
    Ok(())
}

/// Mean per-pixel luma change between frame pairs (0/1, 2/3, …), mean
/// luma, and mean colourfulness (max - min channel), all 0..1.
fn frame_stats(frames: &[Vec<u8>]) -> (f32, f32, f32) {
    let luma = |p: &[u8]| (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0;
    let (mut motion, mut pairs) = (0.0f32, 0u32);
    for pair in frames.chunks(2) {
        if let [a, b] = pair {
            let n = a.len() / 4;
            let d: f32 = a.chunks(4).zip(b.chunks(4)).map(|(x, y)| (luma(x) - luma(y)).abs()).sum();
            motion += d / n as f32;
            pairs += 1;
        }
    }
    let (mut bright, mut colour, mut n) = (0.0f32, 0.0f32, 0usize);
    for f in frames {
        for p in f.chunks(4) {
            bright += luma(p);
            let mx = p[0].max(p[1]).max(p[2]) as f32;
            let mn = p[0].min(p[1]).min(p[2]) as f32;
            colour += (mx - mn) / 255.0;
            n += 1;
        }
    }
    (
        motion / pairs.max(1) as f32,
        bright / n.max(1) as f32,
        colour / n.max(1) as f32,
    )
}

fn save(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tex: &wgpu::Texture,
    w: u32,
    h: u32,
    path: &std::path::Path,
) -> Result<()> {
    let px = read_pixels(device, queue, tex, w, h)?;
    image::save_buffer(path, &px, w, h, image::ColorType::Rgba8)?;
    Ok(())
}

/// Read the output texture back as tightly packed RGBA8.
/// Pearson correlation of two equal-length series (0 when either is flat).
fn pearson(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len()) as f32;
    if n < 2.0 {
        return 0.0;
    }
    let (ma, mb) = (a.iter().sum::<f32>() / n, b.iter().sum::<f32>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
    }
    if saa < 1e-12 || sbb < 1e-12 {
        return 0.0;
    }
    sab / (saa * sbb).sqrt()
}

/// WCAG 2.3.1 general flashes: a flash is a pair of opposing changes in
/// relative luminance of 0.1 or more, where the darker side is under 0.8.
/// Returns the most flashes in any window of `fps` frames (one second).
pub fn max_flashes_per_s(series: &[f32], fps: usize) -> u32 {
    let mut trans = Vec::new();
    let (mut lo, mut hi) = match series.first() {
        Some(&l) => (l, l),
        None => return 0,
    };
    let mut dir = 0i8;
    for (i, &l) in series.iter().enumerate() {
        lo = lo.min(l);
        hi = hi.max(l);
        if dir != 1 && l - lo >= 0.1 && lo < 0.8 {
            trans.push(i);
            dir = 1;
            (lo, hi) = (l, l);
        } else if dir != -1 && hi - l >= 0.1 && l < 0.8 {
            trans.push(i);
            dir = -1;
            (lo, hi) = (l, l);
        }
    }
    let mut worst = 0;
    for (a, &start) in trans.iter().enumerate() {
        let n = trans[a..].iter().take_while(|&&j| j < start + fps).count();
        worst = worst.max(n as u32 / 2);
    }
    worst
}

fn read_pixels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tex: &wgpu::Texture,
    w: u32,
    h: u32,
) -> Result<Vec<u8>> {
    let bpr = (w * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("snap rb"),
        size: (bpr * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
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
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([enc.finish()]);
    let slice = buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    rx.recv()??;
    let data = slice.get_mapped_range().map_err(|e| anyhow!("{e:?}"))?;
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let s = (y * bpr) as usize;
        px.extend_from_slice(&data[s..s + (w * 4) as usize]);
    }
    Ok(px)
}

#[cfg(test)]
mod flash_tests {
    use super::max_flashes_per_s;

    #[test]
    fn counts_wcag_flashes() {
        // A 6 Hz full swing at 60 fps: 6 flashes a second.
        let strobe: Vec<f32> = (0..120).map(|i| if (i / 5) % 2 == 0 { 0.05 } else { 0.6 }).collect();
        assert_eq!(max_flashes_per_s(&strobe, 60), 6);
        // A beat pulse at 2 Hz passes; so does a small flicker under 0.1.
        let beat: Vec<f32> = (0..120).map(|i| if i % 30 < 4 { 0.5 } else { 0.2 }).collect();
        assert_eq!(max_flashes_per_s(&beat, 60), 2);
        let flicker: Vec<f32> = (0..120).map(|i| 0.3 + 0.04 * (i % 2) as f32).collect();
        assert_eq!(max_flashes_per_s(&flicker, 60), 0);
        // Both sides brighter than 0.8 don't count.
        let bright: Vec<f32> = (0..120).map(|i| if (i / 5) % 2 == 0 { 0.82 } else { 0.98 }).collect();
        assert_eq!(max_flashes_per_s(&bright, 60), 0);
    }
}
