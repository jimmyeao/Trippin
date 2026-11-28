// @sim 96000 @bloom 0.55 @tonemap agx
// @title Particle Polygons
// Particle Polygons: six concentric outlines of 96,000 points that change
// shape together on every bar line: triangle, square, hexagon, six-point
// star, octagon, four-point star, pentagram and a twelve-tooth gear, each
// folding into the next over the bar's first beat (shape). Neighbouring
// rings turn opposite ways on the smooth energy clock (motion) and the
// whole mandala swings back each phrase (direction). Each kick sends a ring
// of push out from the centre through the outlines, and every point is
// simulated: kicks blast the outlines apart and they spring back, snares
// twist them, and breakdowns loosen them into a drifting haze.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

const RINGS: u32 = 6u;
const PER: u32 = 16000u;

// Shape k: (points, inner radius share; 1 = a plain polygon).
fn shape_def(k: u32) -> vec2<f32> {
    switch k % 8u {
        case 0u: { return vec2<f32>(3.0, 1.0); }
        case 1u: { return vec2<f32>(4.0, 1.0); }
        case 2u: { return vec2<f32>(6.0, 1.0); }
        case 3u: { return vec2<f32>(6.0, 0.58); }
        case 4u: { return vec2<f32>(8.0, 1.0); }
        case 5u: { return vec2<f32>(4.0, 0.38); }
        case 6u: { return vec2<f32>(5.0, 0.4); }
        default: { return vec2<f32>(12.0, 0.8); }
    }
}

// The point a share t round shape d's outline (unit outer radius), so
// points keep their order round the ring through a morph.
fn outline(d: vec2<f32>, t: f32) -> vec2<f32> {
    let n = d.x;
    if d.y > 0.99 {
        let s = floor(t * n);
        let f = fract(t * n);
        let a0 = s / n * TAU + PI * 0.5;
        let a1 = a0 + TAU / n;
        return mix(vec2<f32>(cos(a0), sin(a0)), vec2<f32>(cos(a1), sin(a1)), f);
    }
    // A star: 2n corners, alternating outer and inner.
    let m = 2.0 * n;
    let s = floor(t * m);
    let f = fract(t * m);
    let a0 = s / m * TAU + PI * 0.5;
    let a1 = a0 + TAU / m;
    let odd = (u32(s) % 2u) == 1u;
    let r0 = select(1.0, d.y, odd);
    let r1 = select(d.y, 1.0, odd);
    return mix(vec2<f32>(cos(a0), sin(a0)) * r0, vec2<f32>(cos(a1), sin(a1)) * r1, f);
}

fn home(i: u32) -> vec4<f32> {
    let ring = min(i / PER, RINGS - 1u);
    let t = f32(i % PER) / f32(PER);
    let bar = u32(max(floor((u.beat - u.bar_phase * 4.0) / 4.0 + 0.5), 0.0));
    let slow = u.calm > 0.5;
    var idx = bar;
    var into = u.bar_phase * 4.0;
    var len = 1.0;
    if slow {
        idx = bar / 2u;
        into += f32(bar % 2u) * 4.0;
        len = 2.0;
    }
    // Rings fold in from the centre out.
    let x = clamp((into - f32(ring) * 0.08 * len) / len, 0.0, 1.0);
    let e = x * x * (3.0 - 2.0 * x);
    let pa = outline(shape_def(idx + 7u), t);
    let pb = outline(shape_def(idx), t);
    var p = mix(pa, pb, e);
    let rr = 0.16 + 0.13 * f32(ring);
    // The kick ring passes out through the outlines.
    let push = beat_ring(f32(ring) / 5.0, 0.35);
    p *= rr * (1.0 + 0.18 * push + 0.05 * u.pres4.x) * (1.0 + 0.25 * sin(PI * e));
    // Neighbours turn opposite ways; the phrase swing turns them all back.
    let dir = select(-1.0, 1.0, ring % 2u == 0u);
    let a = dir * u.clock4.x * 0.05 * (1.0 + 0.3 * f32(ring)) + 0.6 * sin(TAU * u.flow / 64.0);
    p = rot(a) * p;
    return vec4<f32>(p, f32(ring) / 5.0, mix(f32(idx + 7u), f32(idx), e));
}

fn sim_spawn(i: u32) -> Particle {
    let h = home(i);
    return Particle(vec4<f32>(h.xy, 0.0, h.z), vec4<f32>(0.0, 0.0, 0.0, 0.0));
}

fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    let h = home(i);
    var x = p.pos.xy;
    var v = p.vel.xy;
    let calm = u.calm;
    let k = (110.0 + 60.0 * u.pres4.x) * (1.0 - 0.55 * calm);
    var acc = (h.xy - x) * k - v * (1.6 * sqrt(k));
    let t = u.clock4.z * 0.02;
    acc += sim_curl2(h.xy * 0.7 + vec2<f32>(t, -0.5 * t), 0.3 + 0.1 * t) * (0.05 + 0.4 * u.lvl4.y + 1.2 * calm);
    v += acc * dt;
    let r = length(h.xy);
    let dir = h.xy / max(r, 1e-3);
    v += dir * simc.onset.x * 1.4 * (1.0 - 0.85 * calm);
    v += vec2<f32>(-h.y, h.x) * simc.onset.z * 1.5 * (1.0 - 0.6 * calm);
    x += v * dt;
    return Particle(vec4<f32>(x, h.w, h.z), vec4<f32>(v, 0.0, min(length(x - h.xy) * 3.0, 1.0)));
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let pp = sim_p[ii];
    let a = vec3<f32>(pp.pos.xy, 1.0);
    let b = vec3<f32>(pp.pos.xy - pp.vel.xy * 0.03, 1.0);
    let s = sim_quad(a, b, vi, 0.0022, 1.0);
    o.pos = s.pos;
    o.q = s.q;
    let ring = pp.pos.w;
    let loose = pp.vel.w;
    let c = palette(0.08 * pp.pos.z + 0.5 * ring + 0.15 * loose);
    let bri = (0.15 + 0.12 * ring + 0.15 * loose + 0.08 * u.lvl4.z);
    o.col = c * c * bri * s.dim;
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
    var col = prev(in.uv) * 0.4;
    col += vec3<f32>(0.005, 0.004, 0.011) * (1.0 - 0.45 * length(p));
    return vec4<f32>(finite(col), 1.0);
}
