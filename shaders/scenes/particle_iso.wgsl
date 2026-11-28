// @sim 110000 @bloom 0.5 @tonemap agx
// @title Particle Iso
// Particle Iso: an isometric block of 49 solid cuboids of 110,000 points,
// seen from above, whose heights are the music (shape): rings of the
// spectrum from the centre out, a wave rolling corner to corner, a ziggurat
// that pumps with the bass, and a step sequencer of the bands, a new
// formation every bar shooting up out of the last inside a beat, and every
// kick punching the centre up. The block ticks a quarter
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
const HALF_W: f32 = 0.105; // a column's half width
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
    let into = u.bar_phase * 4.0;
    // A new formation every bar: the columns shoot into it from the centre
    // out, landing inside a beat.
    let delay = length(g) / 4.25 * 0.35;
    let x = clamp((into - delay) / 0.55, 0.0, 1.0);
    let ex = 1.0 - x;
    let e = 1.0 - ex * ex * ex;
    let a = formation((bar + 3u) % 4u, g);
    let b = formation(bar % 4u, g);
    var h = mix(a, b, e);
    // Breakdowns: a slow swell instead of the beat.
    let swell = 0.25 + 0.2 * sin(length(g) * 0.9 - u.flow * 0.4) * (0.5 + u.pres4.y);
    h = mix(h, swell, u.calm);
    // The kick punches every column up, the centre most.
    h += 0.3 * kick_swell() * (1.0 - length(g) / 5.0);
    return 0.06 + 1.5 * clamp(h, 0.0, 1.2);
}

// Particle i's home: (point, height share for colour), and how much of an
// edge it is on. Columns are solid: 60% of the points on the skin (by
// face area), the rest filling the inside.
struct Home {
    p: vec3<f32>,
    share: f32,
    edge: f32,
};

fn home(i: u32) -> Home {
    let col = min(i / PER, N * N - 1u);
    let g = vec2<f32>(f32(col % N), f32(col / N)) - 3.0;
    let h = height(i, g);
    let r0 = sim_rand(i, 0u) * 2.0 - 1.0;
    let r1 = sim_rand(i, 1u);
    let r2 = sim_rand(i, 2u);
    let rf = sim_rand(i, 4u);
    var p: vec3<f32>;
    var edge = 0.0;
    if rf < 0.6 {
        let side_area = 4.0 * h;
        let cap_area = 2.0 * 2.0 * HALF_W;
        if r1 * (side_area + cap_area) < side_area {
            // One of the four sides.
            let k = u32(r2 * 4.0) % 4u;
            let y = fract(r1 * 37.0) * h;
            let sd = sgn((k & 1u) != 0u) * HALF_W;
            p = select(vec3<f32>(r0 * HALF_W, y, sd), vec3<f32>(sd, y, r0 * HALF_W), (k & 2u) != 0u);
            edge = max(smoothstep(0.9, 0.98, abs(r0)), smoothstep(h - 0.025, h, y));
        } else {
            // The top or the bottom.
            let q = vec2<f32>(r0, fract(r1 * 37.0) * 2.0 - 1.0) * HALF_W;
            let y = select(0.0, h, r2 < 0.75);
            p = vec3<f32>(q.x, y, q.y);
            edge = smoothstep(0.9, 0.98, max(abs(q.x), abs(q.y)) / HALF_W) * select(0.4, 1.0, y > 0.0);
        }
    } else {
        p = vec3<f32>(r0 * HALF_W, r1 * h, (r2 * 2.0 - 1.0) * HALF_W) * vec3<f32>(0.92, 1.0, 0.92);
    }
    let c = g * STEP;
    p += vec3<f32>(c.x, -0.75, c.y);
    var o: Home;
    o.p = p;
    o.share = (p.y + 0.75) / 1.6;
    o.edge = edge;
    return o;
}

fn sim_spawn(i: u32) -> Particle {
    return Particle(vec4<f32>(home(i).p, 0.0), vec4<f32>(0.0));
}

fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    let hm = home(i);
    let h = hm.p;
    var x = p.pos.xyz;
    var v = p.vel.xyz;
    let calm = u.calm;
    // The spring is the smoothing: spectrum heights jitter, the columns don't.
    let k = (90.0 + 40.0 * u.pres4.x) * (1.0 - 0.4 * calm);
    v += ((h - x) * k - v * (1.7 * sqrt(k))) * dt;
    // Kicks throw everything up, the tops most.
    v.y += simc.onset.x * 1.2 * (0.2 + hm.share) * (1.0 - 0.85 * calm);
    x += v * dt;
    // pos.w: height share + 4 x the edge weight in 16 steps; vel.w: loose.
    let w = min(hm.share, 1.2) * 0.8 + 4.0 * floor(hm.edge * 15.0 + 0.5);
    return Particle(vec4<f32>(x, w), vec4<f32>(v, min(length(x - h) * 3.0, 1.0)));
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
    let wp = rot(-0.6155) * w.yz;
    return vec3<f32>(w.x, wp.x + 0.42, wp.y + 9.0);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let pp = sim_p[ii];
    let ya = yaw();
    let w = pp.pos.xyz;
    let a = view(w, ya);
    let s = sim_quad(a, view(w - pp.vel.xyz * 0.008, ya), vi, 0.0034, 5.6);
    o.pos = s.pos;
    o.q = s.q;
    let edge = floor(pp.pos.w / 4.0) / 15.0;
    let share = (pp.pos.w - floor(pp.pos.w / 4.0) * 4.0) / 0.8;
    let skin = step(sim_rand(ii, 4u), 0.6);
    // Colour by ring out from the centre and by height.
    let col = min(ii / PER, N * N - 1u);
    let ring = length(vec2<f32>(f32(col % N), f32(col / N)) - 3.0) / 4.25;
    let c = palette(0.05 + 0.45 * ring + 0.35 * share + 0.15 * pp.vel.w);
    // Faces glow faintly, edges brightest, so the solid blocks keep their lines.
    let b = (0.025 + 0.035 * skin + 0.42 * edge + 0.05 * share + 0.15 * pp.vel.w) * clamp(10.0 - a.z, 0.5, 1.5);
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
