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
//! `--gpu low`.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};

use crate::audio::SPECTRUM_BINS;
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
    let pal_name = arg(args, "--palette").unwrap_or("rainbow");
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
    let mut scenes: Vec<PathBuf> = std::fs::read_dir(dir.join("scenes"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "wgsl"))
        .collect();
    scenes.sort();
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
        // One full frame: scene → bloom → present.
        let step = |t: f32, frame: u32, cur: &mut usize| {
            let u = groove(t, w, h, frame, bloom_amt, tonemap, calm);
            queue.write_buffer(&ubuf, 0, bytemuck::bytes_of(&u));
            let next = 1 - *cur;
            let mut enc = device.create_command_encoder(&Default::default());
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
            }
            if bloom_amt > 0.0 {
                bloom.encode(&device, &mut enc, &view);
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
                pass.set_bind_group(0, &bgs[next], &[]);
                pass.draw(0..3, 0..1);
            }
            queue.submit([enc.finish()]);
            *cur = next;
        };
        let mut t = 0.0f32;
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
            energy_rows.push((name, m, b, c));
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
        let mut out = serde_json::Map::new();
        for r in &energy_rows {
            let e = 0.5 * pct(&|x| x.1, r.1) + 0.25 * pct(&|x| x.2, r.2) + 0.25 * pct(&|x| x.3, r.3);
            out.insert(r.0.clone(), serde_json::json!((e as f64 * 100.0).round() / 100.0));
        }
        let file = dir.join("scene_energy.json");
        std::fs::write(&file, serde_json::to_string_pretty(&out)? + "\n")?;
        println!("wrote {}", file.display());
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
