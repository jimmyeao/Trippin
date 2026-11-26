// Added after common.wgsl to scenes with `// @sim <count>` (src/sim.rs).
// A sim scene keeps every particle's state between frames: `cs_sim` runs
// once a frame before the scene draws, calling the scene's
//   fn sim_spawn(i: u32) -> Particle             (on a cut-in / reset)
//   fn sim_step(i: u32, p: Particle, dt: f32) -> Particle
// and its `vs_particles` reads `sim_p[instance_index]`.

struct Particle {
    pos: vec4<f32>, // xyz position, w: age (s) or anything the scene likes
    vel: vec4<f32>, // xyz velocity, w: scene's own
};

struct SimCtl {
    count: u32,
    reset: u32,
    dt: f32,
    frame: u32,
    // Per band (bass, mid, mid-high, high): how far u.hits4 jumped this
    // frame, else 0. A hit's onset, one frame long: add it to a velocity
    // as an impulse and the push lands on the beat, where an acceleration
    // from the decaying hits4 peaks a quarter second late.
    onset: vec4<f32>,
};

@group(1) @binding(0) var<storage, read_write> sim_rw: array<Particle>;
@group(1) @binding(1) var<uniform> simc: SimCtl;
@group(1) @binding(2) var<storage, read> sim_p: array<Particle>;

@compute @workgroup_size(64)
fn cs_sim(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= simc.count {
        return;
    }
    if simc.reset != 0u {
        sim_rw[i] = sim_spawn(i);
        return;
    }
    var p = sim_step(i, sim_rw[i], simc.dt);
    // One NaN would live forever: respawn it instead.
    if !all(abs(p.pos) < vec4<f32>(1e6)) || !all(abs(p.vel) < vec4<f32>(1e6)) {
        p = sim_spawn(i);
    }
    sim_rw[i] = p;
}

// A per-particle random in 0..1, stable for the particle (k picks a stream).
fn sim_rand(i: u32, k: u32) -> f32 {
    return f32(pcg(i * 4u + k * 0x9E3779B9u) & 0xFFFFFFu) / 16777216.0;
}

// A fresh random each frame (for respawns and jitter).
fn sim_rand_f(i: u32, k: u32) -> f32 {
    return f32(pcg(i ^ pcg(simc.frame * 7u + k)) & 0xFFFFFFu) / 16777216.0;
}

// Divergence-free 3D flow at p: the curl of the noise volume's three
// channels (one noise period = 1 unit of p). About unit speed on average;
// swirls never bunch particles up or thin them out, so a scene's density
// comes only from its own forces.
fn sim_curl(p: vec3<f32>) -> vec3<f32> {
    let e = 1.5 / 64.0;
    let dx = tnoise(p + vec3<f32>(e, 0.0, 0.0)).xyz - tnoise(p - vec3<f32>(e, 0.0, 0.0)).xyz;
    let dy = tnoise(p + vec3<f32>(0.0, e, 0.0)).xyz - tnoise(p - vec3<f32>(0.0, e, 0.0)).xyz;
    let dz = tnoise(p + vec3<f32>(0.0, 0.0, e)).xyz - tnoise(p - vec3<f32>(0.0, 0.0, e)).xyz;
    return vec3<f32>(dy.z - dz.y, dz.x - dx.z, dx.y - dy.x) * (0.3 / (2.0 * e));
}

// The same in 2D: the curl of one noise channel, sliced at depth z.
fn sim_curl2(p: vec2<f32>, z: f32) -> vec2<f32> {
    let e = 1.5 / 64.0;
    let dx = tnoise(vec3<f32>(p.x + e, p.y, z)).x - tnoise(vec3<f32>(p.x - e, p.y, z)).x;
    let dy = tnoise(vec3<f32>(p.x, p.y + e, z)).x - tnoise(vec3<f32>(p.x, p.y - e, z)).x;
    return vec2<f32>(dy, -dx) * (0.3 / (2.0 * e));
}

// The common audio drive for sims: overall loudness (eased, auto-gained).
fn sim_loud() -> f32 {
    return dot(u.lvl4, vec4<f32>(0.45, 0.3, 0.15, 0.1));
}

// Particle i's own band (0 bass, 1 mid, 2 mid-high, 3 high): sims let each
// quarter of the cloud answer its own part of the spectrum.
fn sim_band(i: u32) -> u32 {
    return pcg(i * 3u + 17u) % 4u;
}

// A point uniformly in the unit ball from three randoms.
fn sim_ball(a: f32, b: f32, c: f32) -> vec3<f32> {
    let z = a * 2.0 - 1.0;
    let ph = b * TAU;
    let s = sqrt(max(0.0, 1.0 - z * z));
    return vec3<f32>(s * cos(ph), z, s * sin(ph)) * pow(c, 1.0 / 3.0);
}
