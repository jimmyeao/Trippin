//! Shared GPU resources behind the "2026" scene tier:
//!
//! - `Statics`: a tileable 64³ RGBA8 3D noise volume (Perlin fbm + Worley
//!   octaves, baked at startup) and a 64² void-and-cluster blue-noise tile,
//!   plus a Repeat sampler for them. Bound at group(0) bindings 4-6 — see
//!   `noise3()` / `blue()` in common.wgsl. One texture fetch replaces ~8 hash
//!   noise evaluations, which is what makes volumetrics affordable on an M2.
//! - `Bloom`: a dual-filter down/up chain (Jimenez "next-gen post" style:
//!   13-tap downsample with a Karis average on the first pass, 3×3 tent
//!   upsample added back up the chain). Its half-res result is bound at
//!   binding 7 for present.wgsl. Scenes opt in with `// @bloom <amount>` in
//!   their header, so older scenes render exactly as before.

pub const NOISE3_SIZE: u32 = 64;
pub const BLUE_SIZE: u32 = 64;
const BLOOM_LEVELS: usize = 6;

pub struct Statics {
    _noise3: wgpu::Texture,
    pub noise3_view: wgpu::TextureView,
    _blue: wgpu::Texture,
    pub blue_view: wgpu::TextureView,
    pub repeat: wgpu::Sampler,
}

impl Statics {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let t0 = std::time::Instant::now();
        let n = NOISE3_SIZE;
        let noise3 = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("noise3"),
            size: wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: n,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &noise3,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bake_noise3(n as usize),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(n * 4),
                rows_per_image: Some(n),
            },
            wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: n,
            },
        );
        let b = BLUE_SIZE;
        let blue = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("blue noise"),
            size: wgpu::Extent3d {
                width: b,
                height: b,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &blue,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bake_blue(b as usize),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(b),
                rows_per_image: Some(b),
            },
            wgpu::Extent3d {
                width: b,
                height: b,
                depth_or_array_layers: 1,
            },
        );
        let repeat = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("repeat"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        println!("Noise volumes baked in {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
        Self {
            noise3_view: noise3.create_view(&Default::default()),
            _noise3: noise3,
            blue_view: blue.create_view(&Default::default()),
            _blue: blue,
            repeat,
        }
    }
}

// ---- 3D noise bake -------------------------------------------------------

fn hash3(x: i32, y: i32, z: i32, salt: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f)
        ^ salt.wrapping_mul(0x1656_67b1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^ (h >> 15)
}

fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Tileable gradient (Perlin) noise with period `per` lattice cells, ~-1..1.
fn perlin(p: [f32; 3], per: i32, salt: u32) -> f32 {
    let i = [p[0].floor(), p[1].floor(), p[2].floor()];
    let f = [p[0] - i[0], p[1] - i[1], p[2] - i[2]];
    let w = f.map(|t| t * t * t * (t * (t * 6.0 - 15.0) + 10.0));
    let mut acc = [0.0f32; 8];
    for (k, a) in acc.iter_mut().enumerate() {
        let o = [(k & 1) as i32, ((k >> 1) & 1) as i32, ((k >> 2) & 1) as i32];
        let c = [
            (i[0] as i32 + o[0]).rem_euclid(per),
            (i[1] as i32 + o[1]).rem_euclid(per),
            (i[2] as i32 + o[2]).rem_euclid(per),
        ];
        let h = hash3(c[0], c[1], c[2], salt);
        // 12 edge gradients of a cube.
        let g: [f32; 3] = match h % 12 {
            0 => [1.0, 1.0, 0.0],
            1 => [-1.0, 1.0, 0.0],
            2 => [1.0, -1.0, 0.0],
            3 => [-1.0, -1.0, 0.0],
            4 => [1.0, 0.0, 1.0],
            5 => [-1.0, 0.0, 1.0],
            6 => [1.0, 0.0, -1.0],
            7 => [-1.0, 0.0, -1.0],
            8 => [0.0, 1.0, 1.0],
            9 => [0.0, -1.0, 1.0],
            10 => [0.0, 1.0, -1.0],
            _ => [0.0, -1.0, -1.0],
        };
        let d = [f[0] - o[0] as f32, f[1] - o[1] as f32, f[2] - o[2] as f32];
        *a = g[0] * d[0] + g[1] * d[1] + g[2] * d[2];
    }
    let lx = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lx(acc[0], acc[1], w[0]);
    let x10 = lx(acc[2], acc[3], w[0]);
    let x01 = lx(acc[4], acc[5], w[0]);
    let x11 = lx(acc[6], acc[7], w[0]);
    lx(lx(x00, x10, w[1]), lx(x01, x11, w[1]), w[2])
}

/// Tileable Worley: 1 - distance to the nearest feature point, 0..1.
fn worley(p: [f32; 3], per: i32, salt: u32) -> f32 {
    let i = [p[0].floor(), p[1].floor(), p[2].floor()];
    let mut best = 9.0f32;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let c = [i[0] as i32 + dx, i[1] as i32 + dy, i[2] as i32 + dz];
                let h = [
                    c[0].rem_euclid(per),
                    c[1].rem_euclid(per),
                    c[2].rem_euclid(per),
                ];
                let fp = [
                    c[0] as f32 + unit(hash3(h[0], h[1], h[2], salt)),
                    c[1] as f32 + unit(hash3(h[0], h[1], h[2], salt ^ 0x9e37)),
                    c[2] as f32 + unit(hash3(h[0], h[1], h[2], salt ^ 0x7f4a)),
                ];
                let d = [p[0] - fp[0], p[1] - fp[1], p[2] - fp[2]];
                best = best.min(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
            }
        }
    }
    (1.0 - best.sqrt()).clamp(0.0, 1.0)
}

fn perlin_fbm(p: [f32; 3], per: i32, octaves: i32, salt: u32) -> f32 {
    let (mut v, mut a, mut f, mut norm) = (0.0, 1.0, 1.0f32, 0.0);
    for o in 0..octaves {
        v += a * perlin(p.map(|x| x * f), per * f as i32, salt + o as u32);
        norm += a;
        a *= 0.5;
        f *= 2.0;
    }
    v / norm
}

fn worley_fbm(p: [f32; 3], per: i32, salt: u32) -> f32 {
    worley(p, per, salt) * 0.625
        + worley(p.map(|x| x * 2.0), per * 2, salt + 1) * 0.25
        + worley(p.map(|x| x * 4.0), per * 4, salt + 2) * 0.125
}

/// RGBA8 volume, all channels tile with period 1 in uvw:
/// R = Perlin-Worley (billowy cloud shapes), G = Worley fbm at 4 cells,
/// B = Perlin fbm at 4 cells (smooth, 0.5-centred), A = Worley fbm at 8 cells.
fn bake_noise3(n: usize) -> Vec<u8> {
    let mut out = vec![0u8; n * n * n * 4];
    let threads = std::thread::available_parallelism().map_or(4, |c| c.get()).min(16);
    let slab = n.div_ceil(threads);
    std::thread::scope(|s| {
        for (t, chunk) in out.chunks_mut(slab * n * n * 4).enumerate() {
            s.spawn(move || {
                for (k, px) in chunk.chunks_mut(4).enumerate() {
                    let idx = t * slab * n * n + k;
                    let (x, y, z) = (idx % n, (idx / n) % n, idx / (n * n));
                    let p = [
                        (x as f32 + 0.5) / n as f32,
                        (y as f32 + 0.5) / n as f32,
                        (z as f32 + 0.5) / n as f32,
                    ];
                    let p4 = p.map(|v| v * 4.0);
                    let p8 = p.map(|v| v * 8.0);
                    let pf = perlin_fbm(p4, 4, 4, 11) * 0.5 + 0.5;
                    let w4 = worley_fbm(p4, 4, 101);
                    let w8 = worley_fbm(p8, 8, 202);
                    // Perlin-Worley: remap Perlin by Worley (Schneider 2015).
                    let pw = ((pf - (1.0 - w4)) / (1.0 - (1.0 - w4)).max(1e-3)).clamp(0.0, 1.0);
                    let pw = pw * 0.6 + pf * 0.4;
                    let smooth = perlin_fbm(p4, 4, 3, 303) * 0.5 + 0.5;
                    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    px.copy_from_slice(&[q(pw), q(w4), q(smooth), q(w8)]);
                }
            });
        }
    });
    out
}

// ---- Blue noise (void and cluster, Ulichney 1993) -------------------------

fn bake_blue(n: usize) -> Vec<u8> {
    let len = n * n;
    let sigma = 1.9f32;
    // Toroidal Gaussian energy kernel, indexed by wrapped (dx, dy).
    let mut kern = vec![0.0f32; len];
    for dy in 0..n {
        for dx in 0..n {
            let wx = dx.min(n - dx) as f32;
            let wy = dy.min(n - dy) as f32;
            kern[dy * n + dx] = (-(wx * wx + wy * wy) / (2.0 * sigma * sigma)).exp();
        }
    }
    let mut energy = vec![0.0f32; len];
    let splat = |energy: &mut [f32], i: usize, sign: f32| {
        let (ix, iy) = (i % n, i / n);
        for y in 0..n {
            let ky = (y + n - iy) % n;
            for x in 0..n {
                let kx = (x + n - ix) % n;
                energy[y * n + x] += sign * kern[ky * n + kx];
            }
        }
    };
    let mut on = vec![false; len];
    // Initial pattern: ~10% random points.
    let mut rng = 0x1234_5678u32;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        rng
    };
    let initial = len / 10;
    let mut count = 0;
    while count < initial {
        let i = next() as usize % len;
        if !on[i] {
            on[i] = true;
            splat(&mut energy, i, 1.0);
            count += 1;
        }
    }
    let tightest = |on: &[bool], energy: &[f32]| {
        (0..len)
            .filter(|&i| on[i])
            .max_by(|&a, &b| energy[a].total_cmp(&energy[b]))
            .unwrap()
    };
    let largest_void = |on: &[bool], energy: &[f32]| {
        (0..len)
            .filter(|&i| !on[i])
            .min_by(|&a, &b| energy[a].total_cmp(&energy[b]))
            .unwrap()
    };
    // Relax: move the tightest cluster into the largest void until stable.
    for _ in 0..len {
        let c = tightest(&on, &energy);
        on[c] = false;
        splat(&mut energy, c, -1.0);
        let v = largest_void(&on, &energy);
        on[v] = true;
        splat(&mut energy, v, 1.0);
        if v == c {
            break;
        }
    }
    let mut rank = vec![0usize; len];
    // Phase 1: rank the initial points by removing tightest clusters.
    {
        let mut on1 = on.clone();
        let mut e1 = energy.clone();
        for r in (0..initial).rev() {
            let c = tightest(&on1, &e1);
            on1[c] = false;
            splat(&mut e1, c, -1.0);
            rank[c] = r;
        }
    }
    // Phases 2+3: fill the largest voids.
    for r in initial..len {
        let v = largest_void(&on, &energy);
        on[v] = true;
        splat(&mut energy, v, 1.0);
        rank[v] = r;
    }
    rank.iter()
        .map(|&r| ((r as f32 + 0.5) / len as f32 * 255.0) as u8)
        .collect()
}

// ---- Bloom ----------------------------------------------------------------

pub struct Bloom {
    layout: wgpu::BindGroupLayout,
    down_first: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    levels: Vec<(wgpu::Texture, wgpu::TextureView)>,
    /// `down_bgs[i]` samples level i-1 (level -1 = the scene target, rebuilt
    /// per frame since the ping-pong target alternates).
    down_bgs: Vec<wgpu::BindGroup>,
    up_bgs: Vec<wgpu::BindGroup>,
}

const BLOOM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

impl Bloom {
    pub fn new(device: &wgpu::Device, w: u32, h: u32) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloom"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/bloom.wgsl").into()),
        });
        let make = |entry: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: BLOOM_FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("bloom"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mut b = Self {
            down_first: make("fs_down_first", None),
            down: make("fs_down", None),
            up: make("fs_up", Some(additive)),
            layout,
            sampler,
            levels: Vec::new(),
            down_bgs: Vec::new(),
            up_bgs: Vec::new(),
        };
        b.resize(device, w, h);
        b
    }

    pub fn resize(&mut self, device: &wgpu::Device, w: u32, h: u32) {
        self.levels.clear();
        let (mut lw, mut lh) = (w, h);
        for i in 0..BLOOM_LEVELS {
            lw = (lw / 2).max(1);
            lh = (lh / 2).max(1);
            let t = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("bloom {i}")),
                size: wgpu::Extent3d {
                    width: lw,
                    height: lh,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: BLOOM_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let v = t.create_view(&Default::default());
            self.levels.push((t, v));
        }
        // down_bgs[i] (i ≥ 1) samples level i-1; up_bgs[i] samples level i+1.
        self.down_bgs = (0..BLOOM_LEVELS)
            .map(|i| self.bg(device, &self.levels[i.saturating_sub(1)].1))
            .collect();
        self.up_bgs = (0..BLOOM_LEVELS - 1)
            .map(|i| self.bg(device, &self.levels[i + 1].1))
            .collect();
    }

    fn bg(&self, device: &wgpu::Device, view: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloom"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// The finished half-res bloom, bound at group(0) binding(7).
    pub fn view(&self) -> &wgpu::TextureView {
        &self.levels[0].1
    }

    /// Record the down/up chain reading `scene` (the frame just rendered).
    pub fn encode(
        &self,
        device: &wgpu::Device,
        enc: &mut wgpu::CommandEncoder,
        scene: &wgpu::TextureView,
    ) {
        let first_bg = self.bg(device, scene);
        let pass = |enc: &mut wgpu::CommandEncoder,
                    target: &wgpu::TextureView,
                    pipe: &wgpu::RenderPipeline,
                    bg: &wgpu::BindGroup,
                    load: wgpu::LoadOp<wgpu::Color>| {
            let mut p = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            p.set_pipeline(pipe);
            p.set_bind_group(0, bg, &[]);
            p.draw(0..3, 0..1);
        };
        let clear = wgpu::LoadOp::Clear(wgpu::Color::BLACK);
        pass(enc, &self.levels[0].1, &self.down_first, &first_bg, clear);
        for i in 1..BLOOM_LEVELS {
            pass(enc, &self.levels[i].1, &self.down, &self.down_bgs[i], clear);
        }
        for i in (0..BLOOM_LEVELS - 1).rev() {
            pass(enc, &self.levels[i].1, &self.up, &self.up_bgs[i], wgpu::LoadOp::Load);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn noise_stats() {
        let n = 32;
        let v = super::bake_noise3(n);
        for c in 0..4 {
            let vals: Vec<f32> = v.chunks(4).map(|p| p[c] as f32 / 255.0).collect();
            let mean = vals.iter().sum::<f32>() / vals.len() as f32;
            let mut s = vals.clone();
            s.sort_by(f32::total_cmp);
            println!(
                "ch{c}: mean {mean:.3} p05 {:.3} p50 {:.3} p95 {:.3}",
                s[s.len() / 20],
                s[s.len() / 2],
                s[s.len() * 19 / 20]
            );
        }
    }
}
