//! Stateful GPU particles: scenes with `// @sim <count>` in the header.
//!
//! A stateless `@particles` scene recomputes every dot from its seed and the
//! clocks each frame, so the music can only reshape the whole form at once.
//! A sim scene keeps each particle's position and velocity in a storage
//! buffer: a compute pass (`cs_sim` in `shaders/sim.wgsl`, calling the
//! scene's `sim_spawn` / `sim_step`) moves them every frame before the scene
//! draws, so kicks can push them, flows can carry them and they keep their
//! momentum. `vs_particles` then reads `sim_p[instance_index]`.
//!
//! Group 1 holds the particles: the compute pass binds them read-write
//! (binding 0), the draw read-only (binding 2); both see `SimCtl`
//! (binding 1). One buffer per scene, made on first use.

use anyhow::{anyhow, Result};
use std::path::Path;

/// `shaders/sim.wgsl`, added after common.wgsl to every `@sim` scene.
pub const SIM_WGSL: &str = include_str!("../shaders/sim.wgsl");
/// Bytes per particle (two vec4s).
const STRIDE: u64 = 32;
/// Most particles a scene may ask for.
pub const MAX: u32 = 400_000;
const WG: u32 = 64;

/// `// @sim <count>` in the first eight lines (0 = not a sim scene).
pub fn header_sim(body: &str) -> u32 {
    body.lines()
        .take(8)
        .find_map(|l| {
            let i = l.find("@sim")?;
            l[i + "@sim".len()..].split_whitespace().next()?.parse::<u32>().ok()
        })
        .unwrap_or(0)
        .min(MAX)
}

/// The source a scene compiles from: common.wgsl, sim.wgsl for sim scenes,
/// then the scene.
pub fn scene_src(common: &str, body: &str) -> String {
    if header_sim(body) > 0 {
        format!("{common}\n{SIM_WGSL}\n{body}")
    } else {
        format!("{common}\n{body}")
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SimCtl {
    count: u32,
    reset: u32,
    dt: f32,
    frame: u32,
    /// Per band, how far `hits4` jumped this frame (0 otherwise): a hit's
    /// onset, for impulses that land on the beat (see sim.wgsl).
    onset: [f32; 4],
}

/// Layouts shared by every sim scene.
pub struct SimKit {
    rw_layout: wgpu::BindGroupLayout,
    ro_layout: wgpu::BindGroupLayout,
    /// Compute: frame group + particles read-write.
    pub compute_pl: wgpu::PipelineLayout,
    /// Draw: frame group + particles read-only.
    pub draw_pl: wgpu::PipelineLayout,
}

fn buf_entry(binding: u32, vis: wgpu::ShaderStages, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: vis,
        ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

impl SimKit {
    pub fn new(device: &wgpu::Device, frame: &wgpu::BindGroupLayout) -> Self {
        use wgpu::ShaderStages as S;
        let ctl = |vis| buf_entry(1, vis, wgpu::BufferBindingType::Uniform);
        let rw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sim rw"),
            entries: &[buf_entry(0, S::COMPUTE, wgpu::BufferBindingType::Storage { read_only: false }), ctl(S::COMPUTE)],
        });
        let ro_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sim ro"),
            entries: &[ctl(S::VERTEX_FRAGMENT), buf_entry(2, S::VERTEX, wgpu::BufferBindingType::Storage { read_only: true })],
        });
        let pl = |label, l: &wgpu::BindGroupLayout| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(frame), Some(l)],
                immediate_size: 0,
            })
        };
        let compute_pl = pl("sim compute", &rw_layout);
        let draw_pl = pl("sim draw", &ro_layout);
        SimKit { rw_layout, ro_layout, compute_pl, draw_pl }
    }
}

/// Compile a sim scene's `cs_sim`.
pub fn compile_compute(device: &wgpu::Device, common: &str, path: &Path, kit: &SimKit) -> Result<wgpu::ComputePipeline> {
    let body = std::fs::read_to_string(path)?;
    let src = scene_src(common, &body);
    let module = wgpu::naga::front::wgsl::parse_str(&src).map_err(|e| anyhow!(e.emit_to_string(&src)))?;
    wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all())
        .validate(&module)
        .map_err(|e| anyhow!(e.emit_to_string(&src)))?;
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: path.to_str(),
        source: wgpu::ShaderSource::Wgsl(src.into()),
    });
    let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: path.to_str(),
        layout: Some(&kit.compute_pl),
        module: &shader,
        entry_point: Some("cs_sim"),
        compilation_options: Default::default(),
        cache: None,
    });
    if let Some(e) = pollster::block_on(scope.pop()) {
        return Err(anyhow!("{e}"));
    }
    Ok(pipe)
}

/// One scene's particles.
pub struct SimBuf {
    pub count: u32,
    ctl: wgpu::Buffer,
    rw: wgpu::BindGroup,
    ro: wgpu::BindGroup,
    frame: u32,
    last_hits: [f32; 4],
}

impl SimBuf {
    pub fn new(device: &wgpu::Device, kit: &SimKit, count: u32) -> Self {
        let count = count.clamp(1, MAX);
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sim particles"),
            size: count as u64 * STRIDE,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let ctl = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sim ctl"),
            size: std::mem::size_of::<SimCtl>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        fn entry(binding: u32, b: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry { binding, resource: b.as_entire_binding() }
        }
        let rw = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sim rw"),
            layout: &kit.rw_layout,
            entries: &[entry(0, &buf), entry(1, &ctl)],
        });
        let ro = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sim ro"),
            layout: &kit.ro_layout,
            entries: &[entry(1, &ctl), entry(2, &buf)],
        });
        SimBuf { count, ctl, rw, ro, frame: 0, last_hits: [0.0; 4] }
    }

    /// Queue one step (or a respawn of every particle when `reset`) into
    /// `enc`. The control block is written through the queue, so one step
    /// per submit.
    pub fn step(
        &mut self,
        queue: &wgpu::Queue,
        enc: &mut wgpu::CommandEncoder,
        pipe: &wgpu::ComputePipeline,
        frame_bg: &wgpu::BindGroup,
        u: &crate::render::Uniforms,
        reset: bool,
    ) {
        self.frame = self.frame.wrapping_add(1);
        let mut onset = [0.0f32; 4];
        for (k, o) in onset.iter_mut().enumerate() {
            if !reset {
                *o = (u.hits4[k] - self.last_hits[k]).max(0.0);
            }
        }
        self.last_hits = u.hits4;
        let c = SimCtl { count: self.count, reset: reset as u32, dt: u.dt.clamp(0.0, 0.05), frame: self.frame, onset };
        queue.write_buffer(&self.ctl, 0, bytemuck::bytes_of(&c));
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("sim"), timestamp_writes: None });
        pass.set_pipeline(pipe);
        pass.set_bind_group(0, frame_bg, &[]);
        pass.set_bind_group(1, &self.rw, &[]);
        pass.dispatch_workgroups(self.count.div_ceil(WG), 1, 1);
    }

    /// The read-only bind group for the draw (group 1).
    pub fn draw_group(&self) -> &wgpu::BindGroup {
        &self.ro
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_sim_parses_and_caps() {
        assert_eq!(header_sim("// @sim 120000 @bloom 0.5\n"), 120_000);
        assert_eq!(header_sim("// @particles 1000\n"), 0);
        assert_eq!(header_sim("// @sim 9000000\n"), MAX);
        assert!(scene_src("C", "// @sim 10\n").contains("fn cs_sim"));
        assert!(!scene_src("C", "// @particles 10\n").contains("fn cs_sim"));
    }

    /// Every sim scene's own header doesn't also claim `@particles` (the two
    /// draw paths would both run).
    #[test]
    fn sim_scenes_are_not_also_stateless() {
        for e in std::fs::read_dir("shaders/scenes").unwrap().flatten() {
            let body = std::fs::read_to_string(e.path()).unwrap_or_default();
            if header_sim(&body) > 0 {
                assert_eq!(crate::render::header_particles(&body), 0, "{}", e.path().display());
            }
        }
    }
}
