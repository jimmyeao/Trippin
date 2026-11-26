// @sim 120000 @bloom 0.5 @tonemap agx
// @title Particle Orbits
// Particle Orbits: 120,000 motes on six tilted orbital shells around a
// glowing core, a slow camera circling the system. Each shell is a band of
// the music: its radius swells with that band (shape), it spins on the smooth
// energy clock, alternate shells counter-rotating (motion), and the shells'
// tilts swing over eight bars (direction). Every mote is simulated (sim.rs,
// the shape-scene pattern): kicks blast the shells outward and they spring
// back, snares wrench them round, the mids scatter the belts and the bass
// pulls them tight; in breakdowns they loosen into drifting dust.
// After the GPU particle systems of Imaginando VS.

const RINGS: u32 = 6u;

fn ring_level(r: u32) -> f32 {
    // Inner shells bass, outer shells highs; the slow presence plus a little
    // of the faster level, so shells breathe rather than twitch.
    let i = min(r / 2u, 3u);
    return u.pres4[i] * 0.7 + u.lvl4[i] * 0.3;
}

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

// Mote i on its shell, tilted into the shell's plane (w: the shell).
fn home(i: u32) -> vec4<f32> {
    let ring = i % RINGS;
    let fr = f32(ring);
    let cell = vec2<f32>(f32(i % 1024u), f32(i / 1024u));
    let h = hash22(cell + vec2<f32>(3.1, 7.3));
    let h2 = hash22(cell + vec2<f32>(11.7, 1.9));
    let lvl = ring_level(ring);
    // Shell: radius swells with its band; motes spread in wide, dusty belts
    // (thin ones read as solid neon hoops).
    let rad = (0.55 + fr * 0.27) * (1.0 + 0.22 * lvl) + (h2.x - 0.5) * (0.22 + 0.04 * fr);
    let dir = select(1.0, -1.0, (ring & 1u) == 1u);
    let a = h.x * TAU + u.clock4.x * dir * 0.5 / (1.0 + fr * 0.45);
    // The highs thicken the belts.
    var w = vec3<f32>(cos(a) * rad, (h.y - 0.5) * 0.14 * (1.0 + fr * 0.3) * (1.0 + 1.2 * u.lvl4.w), sin(a) * rad);
    // Tilt: each shell its own plane, swinging over eight bars.
    let tilt = (fr - 2.5) * 0.22 + 0.25 * sin(TAU * u.flow / 32.0 + fr * 1.3);
    let tw = rot(tilt) * w.yz;
    w = vec3<f32>(w.x, tw.x, tw.y);
    let twz = rot(fr * 0.55) * w.xy;
    return vec4<f32>(twz.x, twz.y, w.z, fr);
}

fn sim_spawn(i: u32) -> Particle {
    return sim_shape_spawn();
}

fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    return sim_shape_step(i, p, home(i).xyz, vec3<f32>(0.0), dt, 2.0);
}

// Camera: circles the system once every 32 bars, gently bobbing.
fn view(w_in: vec3<f32>) -> vec3<f32> {
    var w = w_in;
    let yaw = TAU * u.flow / 128.0;
    let wy = rot(yaw) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let pitch = 0.38 + 0.12 * sin(TAU * u.flow / 64.0);
    let wp = rot(pitch) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 4.6);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let h = home(ii);
    let pp = sim_p[ii];
    let w = h.xyz + pp.pos.xyz;
    let a = view(w);
    let sq = sim_quad(a, view(w - pp.vel.xyz * 0.04), vi, 0.0045, 1.7);
    o.pos = sq.pos;
    o.q = sq.q;
    let ring = u32(h.w);
    let lvl = ring_level(ring);
    let loose = min(length(pp.pos.xyz) * 3.0, 1.0);
    let c = palette(h.w / 6.0 + fract(f32(ii) * 0.618) * 0.08 + 0.1 * loose);
    // Nearer motes brighter; 120k additive dots, so each is faint.
    o.col = c * c * (0.11 + 0.09 * lvl + 0.05 * loose) * clamp(5.2 - a.z, 0.4, 1.6) * sq.dim;
    return o;
}

@fragment
fn fs_particles(in: POut) -> @location(0) vec4<f32> {
    let r2 = dot(in.q, in.q);
    if r2 > 1.0 {
        discard;
    }
    let g = 1.0 - r2;
    return vec4<f32>(in.col * g * g, 0.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Short trails: last frame, faded.
    var col = prev(in.uv) * 0.55;
    // The core: a soft glow at the centre, swelling with the bass presence.
    let core = exp(-length(p) * (14.0 - 4.0 * u.pres4.x));
    col += palette(0.02) * palette(0.02) * core * (0.12 + 0.2 * u.pres4.x);
    // Deep-space floor so it isn't flat black.
    col += vec3<f32>(0.004, 0.005, 0.012) * (1.0 - 0.4 * length(p));
    return vec4<f32>(finite(col), 1.0);
}
