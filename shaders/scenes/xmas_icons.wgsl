// @sim 127000 @bloom 0.55 @tonemap agx
// @title Christmas Icons
// Christmas Icons: 120,000 coloured lights that snap from one Christmas
// figure to the next on every bar line: a tree wound with a garland, a
// star, a bauble, a candy cane, a bell, a snowflake, a wrapped present and
// a wreath, each bursting open and pulling into the next over the bar's
// first beat (shape), while 7,000 snowflakes drift down behind. The
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

// Figure k's point (y up, about +-1.2) for the light with randoms r.
fn shape(k: u32, r: vec4<f32>) -> vec3<f32> {
    let t = r.x;
    switch k {
        case 0u: {
            // Tree: a garland spiralling up a cone, three tier rims, a
            // trunk and a little star on top.
            if r.y < 0.55 {
                let h = t;
                let rad = 0.95 * (1.0 - h);
                let a = h * TAU * 6.0;
                return vec3<f32>(cos(a) * rad, -1.0 + 2.1 * h, sin(a) * rad);
            } else if r.y < 0.8 {
                let tier = floor(r.z * 3.0);
                let h = tier / 3.0;
                let rad = 0.95 * (1.0 - h) + 0.08;
                let q = ring(t * TAU, rad);
                return vec3<f32>(q.x, -1.0 + 2.1 * h + 0.05 * sin(t * TAU * 12.0), q.y);
            } else if r.y < 0.9 {
                let q = ring(t * TAU, 0.12);
                return vec3<f32>(q.x, -1.0 - 0.25 * r.z, q.y);
            }
            let q = star5(t) * 0.17;
            return vec3<f32>(q.x, 1.22 + q.y, 0.0);
        }
        case 1u: {
            // Star: the outline, and ridges from each corner to a raised
            // centre front and back.
            if r.y < 0.5 {
                let q = star5(t) * 1.15;
                return vec3<f32>(q.x, q.y, 0.0);
            }
            let c = star5_corner(u32(r.z * 10.0) % 10u) * 1.15;
            let z = sgn(r.w > 0.5) * 0.32;
            return mix(vec3<f32>(c, 0.0), vec3<f32>(0.0, 0.0, z), t);
        }
        case 2u: {
            // Bauble: a wire globe, its cap and the hanging loop.
            if r.y < 0.82 {
                let c = vec3<f32>(0.0, -0.15, 0.0);
                if r.z < 0.6 {
                    let ph = floor(r.w * 8.0) / 8.0 * PI;
                    let th = t * TAU;
                    return c + vec3<f32>(sin(th) * cos(ph), cos(th), sin(th) * sin(ph)) * 0.88;
                }
                let lat = (floor(r.w * 4.0) + 1.0) / 5.0 * PI;
                let q = ring(t * TAU, sin(lat) * 0.88);
                return c + vec3<f32>(q.x, cos(lat) * 0.88, q.y);
            } else if r.y < 0.94 {
                let q = ring(t * TAU, 0.2);
                return vec3<f32>(q.x, 0.72 + 0.14 * r.z, q.y);
            }
            let q = ring(t * TAU, 0.12);
            return vec3<f32>(q.x, 1.0 + q.y, 0.0);
        }
        case 3u: {
            // Candy cane: two stripes wound round a hooked tube.
            let stripe = select(0.0, PI, r.y < 0.5);
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
                c = vec3<f32>(0.0 + 0.33 * cos(a), 0.55 + 0.33 * sin(a), 0.0);
                tang = vec3<f32>(-sin(a), cos(a), 0.0);
            }
            let n1 = vec3<f32>(tang.y, -tang.x, 0.0);
            let n2 = vec3<f32>(0.0, 0.0, 1.0);
            let a = s * 22.0 + stripe + r.z * 0.6;
            return c + (n1 * cos(a) + n2 * sin(a)) * 0.13;
        }
        case 4u: {
            // Bell: a lathed profile, meridians and rims, and its clapper.
            if r.y < 0.9 {
                let y = select(t, floor(r.w * 5.0) / 4.0, r.z < 0.35);
                let rad = 0.22 + 0.62 * y * y * y + 0.12 * smoothstep(0.85, 1.0, y);
                let a = select(floor(r.w * 12.0) / 12.0 * TAU, t * TAU, r.z < 0.35);
                let q = ring(a, rad);
                return vec3<f32>(q.x, 0.95 - 1.8 * y, q.y);
            }
            let z = r.z * 2.0 - 1.0;
            let q = ring(t * TAU, sqrt(max(0.0, 1.0 - z * z)) * 0.14);
            return vec3<f32>(q.x, -0.95 + 0.14 * z, q.y);
        }
        case 5u: {
            // Snowflake: six arms, each with two pairs of side branches.
            let arm = floor(r.y * 6.0);
            let ang = arm / 6.0 * TAU + PI * 0.5;
            var q: vec2<f32>;
            if r.z < 0.4 {
                q = vec2<f32>(t * 1.2, 0.0);
            } else {
                let b = u32(r.z * 10.0) % 4u;
                let at = select(0.42, 0.78, b >= 2u);
                let l = select(0.38, 0.26, b >= 2u);
                let side = sgn((b % 2u) == 0u);
                q = vec2<f32>(at, 0.0) + vec2<f32>(0.5, 0.866 * side) * t * l;
            }
            let p = rot(ang) * q;
            return vec3<f32>(p.x, p.y, (r.w - 0.5) * 0.04);
        }
        case 6u: {
            // A present: the box's edges, a ribbon round it both ways, a bow.
            let hw = vec3<f32>(0.75, 0.58, 0.75);
            if r.y < 0.5 {
                let e = u32(r.z * 12.0) % 12u;
                let axis = e % 3u;
                let b = e / 3u;
                let s1 = sgn((b & 1u) != 0u);
                let s2 = sgn((b & 2u) != 0u);
                var p = vec3<f32>(t * 2.0 - 1.0, s1, s2);
                if axis == 1u {
                    p = vec3<f32>(s1, t * 2.0 - 1.0, s2);
                } else if axis == 2u {
                    p = vec3<f32>(s1, s2, t * 2.0 - 1.0);
                }
                return p * hw - vec3<f32>(0.0, 0.25, 0.0);
            } else if r.y < 0.82 {
                // Round the box: a square loop in the x=0 or z=0 plane.
                let s = t * 4.0;
                let f = fract(s) * 2.0 - 1.0;
                let side = u32(s) % 4u;
                var q = vec2<f32>(f, 1.0);
                if side == 1u {
                    q = vec2<f32>(1.0, -f);
                } else if side == 2u {
                    q = vec2<f32>(-f, -1.0);
                } else if side == 3u {
                    q = vec2<f32>(-1.0, f);
                }
                let off = (r.w - 0.5) * 0.1;
                let p = select(vec3<f32>(off, q.y, q.x), vec3<f32>(q.x, q.y, off), r.z < 0.5);
                return p * hw - vec3<f32>(0.0, 0.25, 0.0);
            }
            // The bow: two tilted loops on top.
            let side = sgn(r.z < 0.5);
            let q = ring(t * TAU, 0.24);
            return vec3<f32>(side * (0.26 + q.x), 0.38 + 0.2 + q.y * 0.7, q.y * 0.3 * side);
        }
        default: {
            // Wreath: a bumpy ring of greenery, berries and a bow below.
            if r.y < 0.8 {
                let a = t * TAU;
                let th = r.z * TAU;
                let bump = 0.2 + 0.06 * sin(a * 24.0 + th * 2.0);
                let rr = 0.9 + bump * cos(th);
                return vec3<f32>(cos(a) * rr, sin(a) * rr, bump * sin(th));
            } else if r.y < 0.92 {
                let a = floor(r.z * 9.0) / 9.0 * TAU + 0.35;
                let z = r.w * 2.0 - 1.0;
                let q = ring(t * TAU, sqrt(max(0.0, 1.0 - z * z)) * 0.07);
                return vec3<f32>(cos(a) * 0.9 + q.x, sin(a) * 0.9 + q.y, 0.2 + 0.07 * z);
            }
            let side = sgn(r.z < 0.5);
            let q = ring(t * TAU, 0.18);
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
            if r.y < 0.55 {
                // Bulbs: red, gold, blue, pink.
                let li = u32(r.w * 4.0) % 4u;
                let rb = select(red, gold, li == 1u);
                let bp = select(vec3<f32>(0.2, 0.45, 1.0), vec3<f32>(1.0, 0.2, 0.75), li == 3u);
                return select(rb, bp, li >= 2u);
            } else if r.y < 0.8 {
                return green;
            } else if r.y < 0.9 {
                return vec3<f32>(0.6, 0.3, 0.1);
            }
            return gold;
        }
        case 1u: { return gold; }
        case 2u: { return select(red, gold, r.y >= 0.82); }
        case 3u: { return select(white, red, r.y < 0.5); }
        case 4u: { return select(gold, vec3<f32>(1.0, 0.85, 0.5), r.y >= 0.9); }
        case 5u: { return mix(white, vec3<f32>(0.4, 0.75, 1.0), r.x); }
        case 6u: { return select(gold, red, r.y < 0.5); }
        default: {
            if r.y < 0.8 {
                return green;
            }
            return red;
        }
    }
}

fn rnd4(i: u32) -> vec4<f32> {
    return vec4<f32>(sim_rand(i, 0u), sim_rand(i, 1u), sim_rand(i, 2u), sim_rand(i, 3u));
}

// (to-figure index + progress), as in particle_morph: a figure per bar, two
// in breakdowns, lights rippling across a third of a beat.
fn morph(i: u32) -> vec2<f32> {
    let bar = u32(max(floor((u.beat - u.bar_phase * 4.0) / 4.0 + 0.5), 0.0));
    var idx = bar;
    var into = u.bar_phase * 4.0;
    var len = 1.0;
    if u.calm > 0.5 {
        idx = bar / 2u;
        into += f32(bar % 2u) * 4.0;
        len = 2.0;
    }
    let x = clamp((into - sim_rand(i, 5u) * 0.35 * len) / len, 0.0, 1.0);
    return vec2<f32>(f32(idx % NSHAPES), x * x * (3.0 - 2.0 * x));
}

fn place(k: u32, r: vec4<f32>) -> vec3<f32> {
    var p = shape(k, r);
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
    let b = u32(m.x);
    let a = (b + NSHAPES - 1u) % NSHAPES;
    var p = mix(place(a, r), place(b, r), m.y);
    let burst = sin(PI * m.y);
    let dir = normalize(p + (r.yzw - 0.5) * 0.3 + vec3<f32>(1e-4));
    p += dir * burst * (0.45 + 0.2 * u.lvl4.x);
    return p * (1.0 + 0.08 * kick_swell() + 0.05 * u.pres4.x);
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
    let k = (120.0 + 60.0 * u.pres4.x) * (1.0 - 0.5 * calm);
    var acc = (h - x) * k - v * (1.6 * sqrt(k));
    let t = u.clock4.z * 0.02;
    acc += sim_curl(h * 0.5 + vec3<f32>(t, -0.4 * t, 0.7 * t)) * (0.06 + 0.5 * u.lvl4.y + 1.5 * calm);
    v += acc * dt;
    let dir = h / max(length(h), 1e-3);
    v += dir * simc.onset.x * 2.2 * (1.0 - 0.85 * calm);
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
        o.col = vec3<f32>(0.75, 0.8, 0.95) * 0.22 * s.dim * clamp(6.0 - a.z, 0.25, 1.0);
        return o;
    }
    let a = view(w);
    let s = sim_quad(a, view(w - pp.vel.xyz * 0.035), vi, 0.0055, 2.4);
    o.pos = s.pos;
    o.q = s.q;
    let r = rnd4(ii);
    let b = u32(pp.pos.w) % NSHAPES;
    let e = fract(pp.pos.w);
    let c = mix(colour((b + NSHAPES - 1u) % NSHAPES, r), colour(b, r), e);
    let tw = 0.75 + 0.25 * sin(u.time * (2.0 + 3.0 * r.z) + r.w * TAU);
    let bri = (0.2 + 0.15 * pp.vel.w) * tw * clamp(5.4 - a.z, 0.5, 1.5);
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
