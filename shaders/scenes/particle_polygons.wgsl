// @sim 96000 @bloom 0.55 @tonemap agx
// @title Particle Polygons
// Particle Polygons: six concentric solid bands of 96,000 points that change
// shape together on every beat: triangle, square, hexagon, six-point star,
// octagon, four-point star, pentagram and a twelve-tooth gear, each landing
// inside half a beat, every beat its own size (shape). Neighbouring
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

// The beat's own size: big on the one, small on the two, and so on.
fn beat_size(n: u32) -> f32 {
    switch n % 4u {
        case 0u: { return 1.15; }
        case 1u: { return 0.8; }
        case 2u: { return 1.02; }
        default: { return 0.76; }
    }
}

fn home(i: u32) -> vec4<f32> {
    let ring = min(i / PER, RINGS - 1u);
    let t = f32(i % PER) / f32(PER);
    // A new shape on every beat, landing fast; one a bar in breakdowns.
    let bt = u32(max(floor(u.beat), 0.0));
    let slow = u.calm > 0.5;
    var idx = bt;
    var into = fract(u.beat);
    var len = 0.45;
    if slow {
        idx = bt / 4u;
        into += f32(bt % 4u);
        len = 1.5;
    }
    // Rings fold in from the centre out.
    let x = clamp((into - f32(ring) * 0.035 * len / 0.45) / len, 0.0, 1.0);
    let ex = 1.0 - x;
    let e = 1.0 - ex * ex * ex;
    let pa = outline(shape_def(idx + 7u), t);
    let pb = outline(shape_def(idx), t);
    // Each ring is a solid band, its outer edge densest (and lit brightest).
    let rr = 0.17 + 0.13 * f32(ring);
    let band = sim_rand(i, 1u);
    let depth = select(0.115 * pow(band, 1.6), 0.0, band < 0.35);
    var p = mix(pa, pb, e) * (rr - depth);
    // The beat sets its size, and the kick's ring passes out through the
    // bands, swelling each as it goes.
    let sz = select(mix(beat_size(idx + 3u), beat_size(idx), e), 1.0, slow);
    let push = beat_ring(f32(ring) / 5.0, 0.3);
    p *= sz * (1.0 + 0.4 * push + 0.05 * u.pres4.x);
    // Neighbours turn opposite ways, and each new shape screws in a twelfth
    // of a turn; the phrase swing turns them all back.
    let dir = select(-1.0, 1.0, ring % 2u == 0u);
    let a = dir * (u.clock4.x * 0.05 * (1.0 + 0.3 * f32(ring)) + (1.0 - e) * 0.5) + 0.6 * sin(TAU * u.flow / 64.0);
    p = rot(a) * p;
    return vec4<f32>(p, f32(ring) / 5.0, mix(f32((idx + 7u) % 8u), f32(idx % 8u), e));
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
    // The band's outer edge lights brightest; its body is a softer fill.
    let rim = step(sim_rand(ii, 1u), 0.35);
    let c = palette(0.08 * pp.pos.z + 0.5 * ring + 0.15 * loose);
    let bri = (0.05 + 0.13 * rim) * (1.0 + 0.6 * ring) + 0.15 * loose + 0.04 * u.lvl4.z;
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
