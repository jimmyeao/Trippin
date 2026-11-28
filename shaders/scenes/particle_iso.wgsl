// @sim 110000 @bloom 0.5 @tonemap agx
// @title Particle Iso
// Particle Iso: an isometric city block of 49 wire cuboids drawn in 110,000
// points, whose heights are the music (shape): rings of the spectrum from
// the centre out, a wave rolling corner to corner, a ziggurat that pumps
// with the bass, and a step sequencer of the bands, a new formation every
// four bars rising out of the last over a beat. The block ticks a quarter
// turn every four beats, easing in and out (motion), winding back the other
// way each phrase (direction). Abstract blocks, not buildings. Every point
// is simulated: the columns spring to their heights, kicks throw the tops
// up, and breakdowns melt the block into a slow swell.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

const N: u32 = 7u;        // columns a side
const STEP: f32 = 0.34;   // column spacing
const HALF_W: f32 = 0.12; // a column's half width
const PER: u32 = 2244u;   // points per column (49 * 2244 = 109,956)

fn sgn(b: bool) -> f32 {
    return select(-1.0, 1.0, b);
}

// Formation f's height (0..1) for the column at grid (x, z), -3..3 each.
fn formation(f: u32, g: vec2<f32>) -> f32 {
    let d = length(g) / 4.25;
    switch f {
        case 0u: {
            // Rings of the spectrum: bass at the heart, highs at the rim.
            return spec(0.04 + 0.7 * d) * 1.15;
        }
        case 1u: {
            // A wave rolling corner to corner on the energy clock, as tall
            // as it is loud, each column topped up by its own band.
            let w = 0.5 + 0.5 * sin((g.x + g.y) * 0.75 - u.clock4.x * 0.9);
            return w * (0.3 + 0.7 * u.lvl4.x) + 0.25 * spec(0.1 + 0.05 * (g.x + 3.0));
        }
        case 2u: {
            // A ziggurat: steps by distance from the centre, pumped by the bass.
            let c = max(abs(g.x), abs(g.y));
            return (1.0 - c / 3.6) * (0.35 + 0.65 * u.lvl4.x) + 0.25 * kick_swell();
        }
        default: {
            // A step sequencer: rows are bands, columns the spectrum within.
            let band = (u32(g.y + 3.0) % 4u);
            let lv = u.lvl4[band];
            return spec((g.x + 3.5) / 7.0 * 0.6 + 0.05) * 0.8 + 0.35 * lv * f32((u32(g.x + 3.0) + u32(g.y + 3.0)) % 2u);
        }
    }
}

fn height(i: u32, g: vec2<f32>) -> f32 {
    let bar = u32(max(floor((u.beat - u.bar_phase * 4.0) / 4.0 + 0.5), 0.0));
    let block = bar / 4u;
    let into = f32(bar % 4u) * 4.0 + u.bar_phase * 4.0;
    // Columns rise into the new formation from the centre out.
    let delay = length(g) / 4.25 * 0.6;
    let x = clamp(into - delay, 0.0, 1.0);
    let e = x * x * (3.0 - 2.0 * x);
    let a = formation((block + 3u) % 4u, g);
    let b = formation(block % 4u, g);
    var h = mix(a, b, e);
    // Breakdowns: a slow swell instead of the beat.
    let swell = 0.25 + 0.2 * sin(length(g) * 0.9 - u.flow * 0.4) * (0.5 + u.pres4.y);
    h = mix(h, swell, u.calm);
    return 0.06 + 1.5 * clamp(h, 0.0, 1.1);
}

// Particle i's home: (point, height share for colour).
fn home(i: u32) -> vec4<f32> {
    let col = min(i / PER, N * N - 1u);
    let g = vec2<f32>(f32(col % N), f32(col / N)) - 3.0;
    let h = height(i, g);
    let r0 = sim_rand(i, 0u);
    let r1 = sim_rand(i, 1u);
    let c = g * STEP;
    var p: vec3<f32>;
    if r1 < 0.5 {
        // One of the four upright edges.
        let k = u32(r1 * 8.0) % 4u;
        p = vec3<f32>(sgn((k & 1u) != 0u) * HALF_W, r0 * h, sgn((k & 2u) != 0u) * HALF_W);
    } else {
        // An edge of the top or bottom square.
        let k = u32(r1 * 16.0) % 8u;
        let s = (r0 * 2.0 - 1.0) * HALF_W;
        let side = sgn((k & 1u) != 0u) * HALF_W;
        let y = select(0.0, h, (k & 4u) != 0u);
        p = select(vec3<f32>(s, y, side), vec3<f32>(side, y, s), (k & 2u) != 0u);
    }
    p += vec3<f32>(c.x, -0.75, c.y);
    return vec4<f32>(p, (p.y + 0.75) / 1.6);
}

fn sim_spawn(i: u32) -> Particle {
    return Particle(vec4<f32>(home(i).xyz, 0.0), vec4<f32>(0.0));
}

fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    let h4 = home(i);
    let h = h4.xyz;
    var x = p.pos.xyz;
    var v = p.vel.xyz;
    let calm = u.calm;
    // The spring is the smoothing: spectrum heights jitter, the columns don't.
    let k = (55.0 + 30.0 * u.pres4.x) * (1.0 - 0.4 * calm);
    v += ((h - x) * k - v * (1.7 * sqrt(k))) * dt;
    // Kicks throw everything up, the tops most.
    v.y += simc.onset.x * 1.6 * (0.2 + h4.w) * (1.0 - 0.85 * calm);
    x += v * dt;
    return Particle(vec4<f32>(x, h4.w), vec4<f32>(v, min(length(x - h) * 3.0, 1.0)));
}

// A quarter turn every four beats, eased; odd phrases wind back.
fn yaw() -> f32 {
    let b = u.beat * 0.25;
    let f = fract(b);
    let e = mix(f * f * f * (f * (f * 6.0 - 15.0) + 10.0), f, u.calm);
    let n = f32(u32(b) % 8u) + e;
    let back = (u32(b) / 8u) % 2u == 1u;
    return select(n, 8.0 - n, back) * (PI * 0.5) + 0.7854;
}

fn view(w_in: vec3<f32>, ya: f32) -> vec3<f32> {
    var w = w_in;
    let wy = rot(ya) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.6155) * w.yz;
    return vec3<f32>(w.x, wp.x - 0.05, wp.y + 9.0);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let pp = sim_p[ii];
    let ya = yaw();
    let w = pp.pos.xyz;
    let a = view(w, ya);
    let s = sim_quad(a, view(w - pp.vel.xyz * 0.03, ya), vi, 0.006, 6.8);
    o.pos = s.pos;
    o.q = s.q;
    let c = palette(0.1 + 0.55 * pp.pos.w + 0.15 * pp.vel.w);
    let b = (0.16 + 0.22 * pp.pos.w + 0.15 * pp.vel.w) * clamp(10.0 - a.z, 0.5, 1.5);
    o.col = c * c * b * s.dim;
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
    var col = prev(in.uv) * 0.35;
    col += vec3<f32>(0.004, 0.004, 0.01) * (1.0 - 0.45 * length(p));
    return vec4<f32>(finite(col), 1.0);
}
