// @particles 120000 @bloom 0.5 @tonemap agx
// @title Particle Orbits
// Particle Orbits: 120,000 motes on six tilted orbital shells around a
// glowing core, a slow camera circling the system. Each shell is a band of
// the music: its radius swells with that band (shape), it spins on the smooth
// energy clock, alternate shells counter-rotating (motion), and the shells'
// tilts swing over eight bars (direction). After the GPU particle systems of
// Imaginando VS; stateless (render.rs `@particles`).

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

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let ring = ii % RINGS;
    let fr = f32(ring);
    let cell = vec2<f32>(f32(ii % 1024u), f32(ii / 1024u));
    let h = hash22(cell + vec2<f32>(3.1, 7.3));
    let h2 = hash22(cell + vec2<f32>(11.7, 1.9));
    let lvl = ring_level(ring);
    // Shell: radius swells with its band; motes spread in a thin torus.
    // Wide, dusty belts (thin ones read as solid neon hoops).
    let rad = (0.55 + fr * 0.27) * (1.0 + 0.22 * lvl) + (h2.x - 0.5) * (0.22 + 0.04 * fr);
    let dir = select(1.0, -1.0, (ring & 1u) == 1u);
    let a = h.x * TAU + u.clock4.x * dir * 0.5 / (1.0 + fr * 0.45);
    var w = vec3<f32>(cos(a) * rad, (h.y - 0.5) * 0.14 * (1.0 + fr * 0.3), sin(a) * rad);
    // Tilt: each shell its own plane, swinging over eight bars.
    let tilt = (fr - 2.5) * 0.22 + 0.25 * sin(TAU * u.flow / 32.0 + fr * 1.3);
    let tw = rot(tilt) * w.yz;
    w = vec3<f32>(w.x, tw.x, tw.y);
    let twz = rot(fr * 0.55) * w.xy;
    w = vec3<f32>(twz.x, twz.y, w.z);
    // Camera: circles the system once every 32 bars, gently bobbing.
    let yaw = TAU * u.flow / 128.0;
    let wy = rot(yaw) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let pitch = 0.38 + 0.12 * sin(TAU * u.flow / 64.0);
    let wp = rot(pitch) * w.yz;
    w = vec3<f32>(w.x, wp.x, wp.y);
    let z = w.z + 4.6;
    if z < 0.3 {
        o.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return o;
    }
    let fl = 1.7;
    let sp = vec2<f32>(w.x, -w.y) * fl / z;
    // One triangle around the dot (the unit disc fits inside it).
    let corners = array<vec2<f32>, 3>(vec2<f32>(-1.732, -1.0), vec2<f32>(1.732, -1.0), vec2<f32>(0.0, 2.0));
    let q = corners[vi % 3u];
    let size = max(0.0045 * fl / z, 2.0 / u.res_y);
    let p = sp + q * size;
    o.pos = vec4<f32>(p.x / aspect(), -p.y, 0.0, 1.0);
    o.q = q;
    let c = palette(fr / 6.0 + h2.y * 0.08);
    // Nearer motes brighter; 120k additive dots, so each is faint.
    o.col = c * c * (0.11 + 0.09 * lvl) * clamp(5.2 - z, 0.4, 1.6);
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
