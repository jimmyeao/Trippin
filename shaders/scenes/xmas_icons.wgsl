// @sim 127000 @bloom 0.55 @tonemap agx
// @title Christmas Icons
// Christmas Icons: 120,000 coloured lights in solid Christmas figures that
// change every two beats: a tree hung with bulbs, a star, a bauble, a candy
// cane, a bell, a snowflake, a wrapped present and a wreath, each spinning
// in and landing inside half a beat, pumped bigger by the kick (shape), while 7,000 snowflakes drift down behind. The
// figure sways on the smooth clocks, swinging back each phrase (direction),
// the bell rings on the beat, and kicks blast the lights out and gust the
// snow (motion). Every light is simulated. Seasonal (see config.rs).

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

const NSHAPES: u32 = 8u;   // divides the 1024 bars the beat count wraps at
const FIG: u32 = 120000u;  // lights in the figure; the rest are snow

fn sgn(b: bool) -> f32 {
    return select(-1.0, 1.0, b);
}

fn ring(a: f32, r: f32) -> vec2<f32> {
    return vec2<f32>(cos(a), sin(a)) * r;
}

// A point on a five-point star's outline (xy, outer radius 1), t 0..1.
fn star5(t: f32) -> vec2<f32> {
    let s = floor(t * 10.0);
    let f = fract(t * 10.0);
    let a0 = s / 10.0 * TAU + PI * 0.5;
    let a1 = a0 + TAU / 10.0;
    let odd = (u32(s) % 2u) == 1u;
    let r0 = select(1.0, 0.42, odd);
    let r1 = select(0.42, 1.0, odd);
    return mix(ring(a0, r0), ring(a1, r1), f);
}

fn star5_corner(k: u32) -> vec2<f32> {
    let a = f32(k) / 10.0 * TAU + PI * 0.5;
    return ring(a, select(1.0, 0.42, (k % 2u) == 1u));
}

// A point in or on a box: on its surface (uniform per face) pulled in by f.
fn box_pt(c: vec3<f32>, h: vec3<f32>, r3: vec3<f32>, f: f32) -> vec3<f32> {
    var d = r3 * 2.0 - 1.0;
    d /= max(max(abs(d.x), abs(d.y)), max(abs(d.z), 1e-4));
    return c + d * h * f;
}

// A point on the unit sphere for two randoms.
fn sph(a: f32, b: f32) -> vec3<f32> {
    let z = a * 2.0 - 1.0;
    let q = ring(b * TAU, sqrt(max(0.0, 1.0 - z * z)));
    return vec3<f32>(q.x, z, q.y);
}

// Figure k's point (y up, about +-1.2) for the light with randoms r: solid
// figures, filled. f (0..1) is how deep the light sits (1 = the skin, where
// 60% of them are), the same for every figure, so skin morphs into skin.
fn shape(k: u32, r: vec4<f32>, f: f32) -> vec3<f32> {
    let t = r.x;
    switch k {
        case 0u: {
            // Tree: three stacked cones, a trunk and a star on top. The
            // bulbs (r.y < 0.16) sit on the skin.
            if r.y < 0.84 {
                let tier = floor(fract(r.y * 13.0) * 3.0);
                let base = -0.85 + tier * 0.55;
                let rad = 0.95 - tier * 0.22;
                let h = 1.0 - sqrt(t);
                let ff = select(f, 1.0, r.y < 0.16);
                let q = ring(r.z * TAU, rad * (1.0 - h) * ff);
                return vec3<f32>(q.x, base + h * 0.85, q.y);
            } else if r.y < 0.9 {
                return box_pt(vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(0.12, 0.15, 0.12), r.zwx, f);
            }
            let q = star5(t) * 0.2 * f;
            return vec3<f32>(q.x, 1.18 + q.y, (r.z * 2.0 - 1.0) * 0.05);
        }
        case 1u: {
            // Star: a solid five-point star, thickest at its heart.
            let q = star5(t) * 1.15 * f;
            return vec3<f32>(q.x, q.y, (r.y * 2.0 - 1.0) * 0.32 * (1.0 - 0.8 * f * length(star5(t))));
        }
        case 2u: {
            // Bauble: a solid ball, its cap and the hanging loop.
            if r.y < 0.86 {
                return vec3<f32>(0.0, -0.15, 0.0) + sph(t, r.z) * 0.88 * f;
            } else if r.y < 0.95 {
                let q = ring(r.z * TAU, 0.2 * f);
                return vec3<f32>(q.x, 0.72 + 0.14 * r.w, q.y);
            }
            let q = ring(t * TAU, 0.12);
            return vec3<f32>(q.x, 1.0 + q.y, 0.0);
        }
        case 3u: {
            // Candy cane: a solid hooked tube; r.y < 0.5 are the red stripes.
            let len_s = 1.7;
            let len_h = PI * 0.33;
            let s = t * (len_s + len_h);
            var c: vec3<f32>;
            var tang: vec3<f32>;
            if s < len_s {
                c = vec3<f32>(0.33, -1.15 + s, 0.0);
                tang = vec3<f32>(0.0, 1.0, 0.0);
            } else {
                let a = (s - len_s) / 0.33;
                c = vec3<f32>(0.33 * cos(a), 0.55 + 0.33 * sin(a), 0.0);
                tang = vec3<f32>(-sin(a), cos(a), 0.0);
            }
            let n1 = vec3<f32>(tang.y, -tang.x, 0.0);
            let n2 = vec3<f32>(0.0, 0.0, 1.0);
            let stripe = select(PI, 0.0, r.y < 0.5);
            let a = s * 9.0 + stripe + (r.z - 0.5) * PI;
            return c + (n1 * cos(a) + n2 * sin(a)) * 0.15 * f;
        }
        case 4u: {
            // Bell: a solid lathed bell and its clapper.
            if r.y < 0.9 {
                let y = t;
                let rad = 0.22 + 0.62 * y * y * y + 0.12 * smoothstep(0.85, 1.0, y);
                let q = ring(r.z * TAU, rad * f);
                return vec3<f32>(q.x, 0.95 - 1.8 * y, q.y);
            }
            return vec3<f32>(0.0, -0.95, 0.0) + sph(t, r.z) * 0.15 * f;
        }
        case 5u: {
            // Snowflake: six broad arms, each with two pairs of side branches.
            let arm = floor(r.y * 6.0);
            let ang = arm / 6.0 * TAU + PI * 0.5;
            var q: vec2<f32>;
            var w = 0.07;
            if r.z < 0.45 {
                q = vec2<f32>(t * 1.2, 0.0);
                w = 0.08 * (1.0 - 0.6 * t);
            } else {
                let b = u32(r.z * 10.0) % 4u;
                let at = select(0.42, 0.78, b >= 2u);
                let l = select(0.4, 0.28, b >= 2u);
                let side = sgn((b % 2u) == 0u);
                q = vec2<f32>(at, 0.0) + vec2<f32>(0.5, 0.866 * side) * t * l;
                w = 0.05 * (1.0 - 0.5 * t);
            }
            let p = rot(ang) * (q + vec2<f32>(0.0, (r.w * 2.0 - 1.0) * w));
            return vec3<f32>(p.x, p.y, (fract(r.w * 17.0) - 0.5) * 0.1);
        }
        case 6u: {
            // A present: a solid box, a ribbon round it both ways (r.y < 0.5)
            // and a bow of two solid loops.
            let hw = vec3<f32>(0.75, 0.58, 0.75);
            let c = vec3<f32>(0.0, -0.25, 0.0);
            if r.y < 0.25 {
                return box_pt(c, vec3<f32>(0.11, hw.y + 0.015, hw.z + 0.015), r.xzw, f);
            } else if r.y < 0.5 {
                return box_pt(c, vec3<f32>(hw.x + 0.015, hw.y + 0.015, 0.11), r.xzw, f);
            } else if r.y < 0.9 {
                return box_pt(c, hw, r.xzw, f);
            }
            let side = sgn(r.z < 0.5);
            let a = t * TAU;
            let th = r.w * TAU;
            let rr = 0.22 + 0.07 * f * cos(th);
            return vec3<f32>(side * (0.26 + rr * cos(a)), 0.58 + rr * sin(a) * 0.7, 0.07 * f * sin(th) + rr * sin(a) * 0.3 * side);
        }
        default: {
            // Wreath: a solid bumpy ring of greenery, berries and a bow.
            if r.y < 0.82 {
                let a = t * TAU;
                let th = r.z * TAU;
                let bump = (0.2 + 0.06 * sin(a * 24.0 + th * 2.0)) * f;
                let rr = 0.9 + bump * cos(th);
                return vec3<f32>(cos(a) * rr, sin(a) * rr, bump * sin(th));
            } else if r.y < 0.93 {
                let a = floor(r.z * 9.0) / 9.0 * TAU + 0.35;
                return vec3<f32>(cos(a) * 0.9, sin(a) * 0.9, 0.2) + sph(t, r.w) * 0.08 * f;
            }
            let side = sgn(r.z < 0.5);
            let q = ring(t * TAU, 0.18 * mix(0.6, 1.0, f));
            return vec3<f32>(side * (0.2 + q.x), -0.95 + q.y * 0.6, 0.25);
        }
    }
}

// Figure k's colour for the light (fixed Christmas colours, not the palette).
fn colour(k: u32, r: vec4<f32>) -> vec3<f32> {
    let gold = vec3<f32>(1.0, 0.62, 0.15);
    let red = vec3<f32>(1.0, 0.08, 0.1);
    let green = vec3<f32>(0.1, 0.85, 0.25);
    let white = vec3<f32>(0.85, 0.9, 1.0);
    switch k {
        case 0u: {
            if r.y < 0.16 {
                // Bulbs: red, gold, blue, pink.
                let li = u32(r.w * 4.0) % 4u;
                let rb = select(red, gold, li == 1u);
                let bp = select(vec3<f32>(0.2, 0.45, 1.0), vec3<f32>(1.0, 0.2, 0.75), li == 3u);
                return select(rb, bp, li >= 2u) * 1.6;
            } else if r.y < 0.84 {
                return green;
            } else if r.y < 0.9 {
                return vec3<f32>(0.6, 0.3, 0.1);
            }
            return gold;
        }
        case 1u: { return gold; }
        case 2u: { return select(red, gold, r.y >= 0.86); }
        case 3u: { return select(white, red, r.y < 0.5); }
        case 4u: { return select(gold, vec3<f32>(1.0, 0.85, 0.5), r.y >= 0.9); }
        case 5u: { return mix(white, vec3<f32>(0.4, 0.75, 1.0), r.x); }
        case 6u: { return select(red, gold, r.y < 0.5 || r.y >= 0.9); }
        default: {
            if r.y < 0.82 {
                return green;
            }
            return red;
        }
    }
}

fn rnd4(i: u32) -> vec4<f32> {
    return vec4<f32>(sim_rand(i, 0u), sim_rand(i, 1u), sim_rand(i, 2u), sim_rand(i, 3u));
}

// How deep in the figure the light sits: 60% on the skin, the rest inside.
fn fill(i: u32) -> f32 {
    let r = sim_rand(i, 4u);
    if r < 0.6 {
        return 1.0;
    }
    return 0.96 * pow((r - 0.6) / 0.4, 0.3333);
}

// (to-figure index + progress): a new figure every two beats, landing fast;
// one a bar in breakdowns, lights rippling across.
fn morph(i: u32) -> vec2<f32> {
    let bt = u32(max(floor(u.beat), 0.0));
    var idx = bt / 2u;
    var into = fract(u.beat) + f32(bt % 2u);
    var len = 0.6;
    if u.calm > 0.5 {
        idx = bt / 4u;
        into = fract(u.beat) + f32(bt % 4u);
        len = 1.5;
    }
    let x = clamp((into - sim_rand(i, 5u) * 0.12 * len) / len, 0.0, 1.0);
    let ex = 1.0 - x;
    return vec2<f32>(f32(idx % NSHAPES), 1.0 - ex * ex * ex);
}

fn place(k: u32, r: vec4<f32>, f: f32) -> vec3<f32> {
    var p = shape(k, r, f);
    if k == 4u {
        // The bell rings: a swing about its crown, two beats a cycle.
        let sw = 0.32 * sin(PI * u.flow) * (1.0 - 0.6 * u.calm);
        let q = rot(sw) * (p.xy - vec2<f32>(0.0, 1.0));
        p = vec3<f32>(q.x, q.y + 1.0, p.z);
    }
    return p;
}

fn home(i: u32, m: vec2<f32>) -> vec3<f32> {
    let r = rnd4(i);
    let f = fill(i);
    let b = u32(m.x);
    let a = (b + NSHAPES - 1u) % NSHAPES;
    var p = mix(place(a, r, f), place(b, r, f), m.y);
    // The new figure spins in (a quarter turn, alternating ways), and the
    // kick pumps it bigger, more on the one.
    let q = rot((1.0 - m.y) * 1.4 * sgn(b % 2u == 0u)) * p.xz;
    p = vec3<f32>(q.x, p.y, q.y);
    let one = select(0.85, 1.0, (u32(max(floor(u.beat), 0.0)) % 2u) == 0u);
    return p * one * (1.0 + 0.32 * kick_swell() + 0.05 * u.pres4.x);
}

// Snow: a slab behind and around the figure, falling and drifting.
fn snow_spawn(i: u32, top: bool) -> vec3<f32> {
    let y = select(sim_rand_f(i, 1u) * 3.6 - 1.8, 1.9, top);
    return vec3<f32>(sim_rand_f(i, 0u) * 6.0 - 3.0, y, sim_rand_f(i, 2u) * 3.0 - 1.2);
}

fn sim_spawn(i: u32) -> Particle {
    if i >= FIG {
        return Particle(vec4<f32>(snow_spawn(i, false), 0.0), vec4<f32>(0.0, -0.25, 0.0, 1.0));
    }
    let m = morph(i);
    return Particle(vec4<f32>(home(i, m), m.x + m.y * 0.99), vec4<f32>(0.0));
}

fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    var x = p.pos.xyz;
    var v = p.vel.xyz;
    let calm = u.calm;
    if i >= FIG {
        // Falls at its own pace, drifts on the swirl, and kicks gust it.
        let fall = 0.15 + 0.2 * sim_rand(i, 6u);
        let drift = sim_curl(x * 0.25 + vec3<f32>(0.0, u.clock4.x * 0.01, 0.0)) * 0.5;
        let goal = vec3<f32>(drift.x + 0.15 * sin(TAU * u.flow / 64.0), -fall, drift.z * 0.5);
        v += (goal - v) * 2.0 * dt;
        v.x += simc.onset.x * 0.5 * (1.0 - 0.85 * calm) * (0.5 + sim_rand(i, 7u));
        x += v * dt;
        if x.y < -1.9 || abs(x.x) > 3.4 {
            x = snow_spawn(i, x.y < -1.9);
            v = vec3<f32>(0.0, -fall, 0.0);
        }
        return Particle(vec4<f32>(x, 0.0), vec4<f32>(v, 1.0));
    }
    let m = morph(i);
    let h = home(i, m);
    let k = (160.0 + 60.0 * u.pres4.x) * (1.0 - 0.5 * calm);
    var acc = (h - x) * k - v * (1.6 * sqrt(k));
    let t = u.clock4.z * 0.02;
    acc += sim_curl(h * 0.5 + vec3<f32>(t, -0.4 * t, 0.7 * t)) * (0.03 + 0.15 * u.lvl4.y + 1.2 * calm);
    v += acc * dt;
    let dir = h / max(length(h), 1e-3);
    v += dir * simc.onset.x * 1.4 * (1.0 - 0.85 * calm);
    v += vec3<f32>(-h.z, 0.0, h.x) * simc.onset.z * 1.0 * (1.0 - 0.6 * calm);
    x += v * dt;
    return Particle(vec4<f32>(x, m.x + m.y * 0.99), vec4<f32>(v, min(length(x - h) * 2.5, 1.0)));
}

fn view(w_in: vec3<f32>) -> vec3<f32> {
    var w = w_in;
    // Sways on the energy clock, swinging back each phrase.
    let ya = 0.55 * sin(TAU * u.flow / 32.0) + 0.12 * sin(u.clock4.x * 0.15);
    let wy = rot(ya) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.12) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 4.2);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let pp = sim_p[ii];
    var w = pp.pos.xyz;
    if ii >= FIG {
        // Snow doesn't sway with the figure.
        let a = vec3<f32>(w.x, w.y, w.z + 4.2);
        let sz = 0.008 + 0.018 * sim_rand(ii, 8u) * sim_rand(ii, 9u);
        let s = sim_quad(a, a - pp.vel.xyz * vec3<f32>(0.04, 0.04, 0.0), vi, sz, 2.4);
        o.pos = s.pos;
        o.q = s.q;
        o.col = vec3<f32>(0.75, 0.8, 0.95) * 0.14 * s.dim * clamp(6.0 - a.z, 0.25, 1.0);
        return o;
    }
    let a = view(w);
    let s = sim_quad(a, view(w - pp.vel.xyz * 0.02), vi, 0.004, 2.4);
    o.pos = s.pos;
    o.q = s.q;
    let r = rnd4(ii);
    let b = u32(pp.pos.w) % NSHAPES;
    let e = fract(pp.pos.w);
    let c = mix(colour((b + NSHAPES - 1u) % NSHAPES, r), colour(b, r), e);
    let tw = 0.75 + 0.25 * sin(u.time * (2.0 + 3.0 * r.z) + r.w * TAU);
    // The skin lights brighter than the inside, so the solid keeps its form.
    let skin = step(sim_rand(ii, 4u), 0.6);
    let bri = ((0.05 + 0.1 * skin) + 0.15 * pp.vel.w) * tw * clamp(5.4 - a.z, 0.5, 1.5);
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
    // A deep winter-night blue, lighter towards the bottom.
    col += mix(vec3<f32>(0.004, 0.006, 0.018), vec3<f32>(0.012, 0.016, 0.035), smoothstep(-0.6, 0.9, p.y));
    return vec4<f32>(finite(col), 1.0);
}
