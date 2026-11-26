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

// ---- Shape scenes -------------------------------------------------------
// A scene whose particles make a figure (a lattice, a knot, a sphere) keeps
// the figure as each particle's home and simulates only how far it is
// knocked off it: pos.xyz is the offset from home, vel.xyz its velocity.
// The scene draws home + offset, so the figure moves exactly as designed
// and the music shakes it apart and pulls it back together:
//   the bass stiffens the spring (a crisp figure), breakdowns slacken it;
//   the mids drag particles off along a curl-noise swirl (a looser haze);
//   the high-band quarter fizzes with the highs;
//   a kick is a shock out from `centre`, a snare a twist round the y axis.
// `scale` is the figure's size, so every figure takes the same share.

fn sim_shape_spawn() -> Particle {
    return Particle(vec4<f32>(0.0), vec4<f32>(0.0));
}

fn sim_shape_step(i: u32, p: Particle, home: vec3<f32>, centre: vec3<f32>, dt: f32, scale: f32) -> Particle {
    let calm = u.calm;
    var d = p.pos.xyz;
    var v = p.vel.xyz;
    let r = home - centre;
    let dir = r / max(length(r), 1e-3);
    let stiff = (16.0 + 20.0 * u.pres4.x + 14.0 * u.lvl4.x) * (1.0 - 0.55 * calm);
    var acc = -d * stiff - v * 4.0;
    let t = u.clock4.z * 0.02;
    acc += sim_curl(home * (0.45 / scale) + vec3<f32>(t, -0.4 * t, 0.7 * t)) * (0.25 + 1.8 * u.lvl4.y + 2.2 * calm) * scale;
    if sim_band(i) == 3u {
        let j = vec3<f32>(sim_rand_f(i, 0u), sim_rand_f(i, 1u), sim_rand_f(i, 2u)) - vec3<f32>(0.5);
        acc += j * u.lvl4.w * 30.0 * scale;
    }
    v += acc * dt;
    v += dir * simc.onset.x * 3.2 * scale * (1.0 - 0.85 * calm);
    v += vec3<f32>(-r.z, 0.0, r.x) * simc.onset.z * 1.0 * (1.0 - 0.6 * calm);
    d += v * dt;
    return Particle(vec4<f32>(d, p.pos.w + dt), vec4<f32>(v, p.vel.w));
}

// A dot drawn as a streak: `a` where it is in view space (camera at the
// origin, +z ahead), `b` where it was a moment ago. Its clip position, the
// corner (q, for the scene's fs_particles) and `dim`, the brightness share
// left after spreading over the streak's length (so speed reads as shape,
// not as glare). Off-screen (behind the camera) when `dim` is 0.
struct SimQuad {
    pos: vec4<f32>,
    q: vec2<f32>,
    dim: f32,
};

fn sim_quad(a: vec3<f32>, b: vec3<f32>, vi: u32, size_w: f32, fl: f32) -> SimQuad {
    var o: SimQuad;
    o.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    o.q = vec2<f32>(0.0);
    o.dim = 0.0;
    if a.z < 0.3 || b.z < 0.3 {
        return o;
    }
    let sa = a.xy * fl / a.z;
    let sb = b.xy * fl / b.z;
    let d = sa - sb;
    let dl = length(d);
    let tang = select(vec2<f32>(1.0, 0.0), d / max(dl, 1e-6), dl > 1e-5);
    let nrm = vec2<f32>(-tang.y, tang.x);
    let size = max(size_w * fl / a.z, 2.0 / u.res_y);
    let len = size + 0.5 * dl;
    let corners = array<vec2<f32>, 3>(vec2<f32>(-1.732, -1.0), vec2<f32>(1.732, -1.0), vec2<f32>(0.0, 2.0));
    let q = corners[vi % 3u];
    let p = sa - 0.5 * d + tang * q.x * len + nrm * q.y * size;
    o.pos = vec4<f32>(p.x / aspect(), p.y, 0.0, 1.0);
    o.q = q;
    o.dim = size / len;
    return o;
}
