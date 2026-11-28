// @sim 140000 @bloom 0.5 @tonemap agx
// @title Particle Morph
// Particle Morph: 140,000 points that snap from one geometric figure to the
// next on every bar line, seen isometrically: a nested cube, a six-point
// cross, an octahedron, a star of David, a torus, a stella octangula, a
// gyroscope's rings and a wire globe. Each morph bursts the figure open and
// pulls it into the next over the bar's first beat, rippling through the
// points (shape). The figure ticks a quarter turn every two beats, easing in
// and out like a clock hand (motion), and swings the other way each phrase
// (direction). In breakdowns it holds each figure two bars and turns steadily.
// Every point is simulated: kicks blast the figure out and it springs back,
// snares wrench it round, the bass pulls it taut.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

const NSHAPES: u32 = 8u; // divides the 1024 bars the beat count wraps at

fn sgn(b: bool) -> f32 {
    return select(-1.0, 1.0, b);
}

// A vertex of the regular tetrahedron (k 0..3), corners of a cube.
fn tetv(k: u32) -> vec3<f32> {
    let x = sgn(k == 0u || k == 1u);
    let y = sgn(k == 0u || k == 2u);
    return vec3<f32>(x, y, x * y);
}

// Figure k's point for the particle with randoms r (stable per particle).
fn shape(k: u32, r: vec4<f32>) -> vec3<f32> {
    let t = r.x;
    switch k {
        case 0u: {
            // Nested cubes: a big wire cube with a small one inside.
            let e = u32(r.y * 12.0) % 12u;
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
            return p * select(0.85, 0.38, r.z < 0.28);
        }
        case 1u: {
            // The six-point cross: three square bars through the centre,
            // drawn as wire: each arm's four long edges and its end square.
            let axis = u32(r.y * 3.0) % 3u;
            var along = (t * 2.0 - 1.0) * 1.3;
            let ci = u32(r.z * 4.0) % 4u;
            var c = vec2<f32>(sgn((ci & 1u) != 0u), sgn((ci & 2u) != 0u));
            if r.w < 0.3 {
                // An end square: walk its perimeter at either end.
                along = sgn(t > 0.5) * 1.3;
                let side = fract(r.z * 4.0) * 2.0 - 1.0;
                let si = u32(r.w / 0.3 * 4.0) % 4u;
                c = vec2<f32>(side, 1.0);
                if si == 1u {
                    c = vec2<f32>(1.0, side);
                } else if si == 2u {
                    c = vec2<f32>(side, -1.0);
                } else if si == 3u {
                    c = vec2<f32>(-1.0, side);
                }
            } else {
                // Long edges stop where the other arms' faces meet them.
                let a = abs(along);
                along = sgn(along > 0.0) * mix(0.2, 1.3, a / 1.3);
            }
            c *= 0.2;
            if axis == 0u {
                return vec3<f32>(along, c.x, c.y);
            } else if axis == 1u {
                return vec3<f32>(c.x, along, c.y);
            }
            return vec3<f32>(c.x, c.y, along);
        }
        case 2u: {
            // Octahedron: twelve edges between the six axis points.
            let e = u32(r.y * 12.0) % 12u;
            let pr = e / 4u;
            let sa = sgn((e & 1u) != 0u);
            let sb = sgn((e & 2u) != 0u);
            var a = vec3<f32>(sa, 0.0, 0.0);
            var b = vec3<f32>(0.0, sb, 0.0);
            if pr == 1u {
                b = vec3<f32>(0.0, 0.0, sb);
            } else if pr == 2u {
                a = vec3<f32>(0.0, sa, 0.0);
                b = vec3<f32>(0.0, 0.0, sb);
            }
            return mix(a, b, t) * 1.25;
        }
        case 3u: {
            // Star of David: two triangles, a thin prism, a hexagon at heart.
            let e = u32(r.y * 6.0) % 6u;
            let tri = f32(e / 3u);
            let j = f32(e % 3u);
            let a0 = PI * 0.5 + tri * PI / 3.0 + j * TAU / 3.0;
            let a1 = a0 + TAU / 3.0;
            var q = mix(vec2<f32>(cos(a0), sin(a0)), vec2<f32>(cos(a1), sin(a1)), t) * 1.3;
            var z = sgn(r.z > 0.5) * 0.14;
            if r.w < 0.18 {
                let h = floor(r.z * 6.0);
                let b0 = h * TAU / 6.0;
                q = mix(vec2<f32>(cos(b0), sin(b0)), vec2<f32>(cos(b0 + TAU / 6.0), sin(b0 + TAU / 6.0)), t) * 0.5;
                z = 0.0;
            }
            return vec3<f32>(q.x, q.y, z);
        }
        case 4u: {
            // Torus, wireframe: sixteen meridians and four parallels.
            var phi = t * TAU;
            var th = floor(r.y * 4.0) / 4.0 * TAU;
            if r.z < 0.7 {
                phi = floor(r.y * 16.0) / 16.0 * TAU;
                th = t * TAU;
            }
            let rr = 0.85 + 0.38 * cos(th);
            return vec3<f32>(rr * cos(phi), 0.38 * sin(th), rr * sin(phi));
        }
        case 5u: {
            // Stella octangula: two interlocked tetrahedra.
            let e = u32(r.y * 6.0) % 6u;
            var i0 = 0u;
            var i1 = 1u;
            if e == 1u { i1 = 2u; }
            if e == 2u { i1 = 3u; }
            if e == 3u { i0 = 1u; i1 = 2u; }
            if e == 4u { i0 = 1u; i1 = 3u; }
            if e == 5u { i0 = 2u; i1 = 3u; }
            let p = mix(tetv(i0), tetv(i1), t) * 0.82;
            return select(p, -p, r.z < 0.5);
        }
        case 6u: {
            // Gyroscope: three rings at right angles round a small core.
            let a = t * TAU;
            let ring = u32(r.y * 3.0) % 3u;
            let c = vec2<f32>(cos(a), sin(a));
            if r.z < 0.15 {
                let z = r.y * 2.0 - 1.0;
                let s = sqrt(max(0.0, 1.0 - z * z));
                return vec3<f32>(s * c.x, z, s * c.y) * 0.22;
            }
            let rad = 1.2 - 0.15 * f32(ring);
            if ring == 0u {
                return vec3<f32>(c.x, c.y, 0.0) * rad;
            } else if ring == 1u {
                return vec3<f32>(0.0, c.x, c.y) * rad;
            }
            return vec3<f32>(c.x, 0.0, c.y) * rad;
        }
        default: {
            // Wire globe: ten meridians and five parallels.
            if r.z < 0.6 {
                let ph = floor(r.y * 10.0) / 10.0 * PI;
                let th = t * TAU;
                return vec3<f32>(sin(th) * cos(ph), cos(th), sin(th) * sin(ph)) * 1.1;
            }
            let lat = (floor(r.y * 5.0) + 1.0) / 6.0 * PI;
            let a = t * TAU;
            return vec3<f32>(sin(lat) * cos(a), cos(lat), sin(lat) * sin(a)) * 1.1;
        }
    }
}

fn rnd4(i: u32) -> vec4<f32> {
    return vec4<f32>(sim_rand(i, 0u), sim_rand(i, 1u), sim_rand(i, 2u), sim_rand(i, 3u));
}

// Where the morph stands: (from, to, progress 0..1 for this particle).
// Bars come from the beat count less the bar phase, so morphs land on the
// one; a figure holds two bars in breakdowns.
struct Morph {
    a: u32,
    b: u32,
    e: f32,
};

fn morph(i: u32) -> Morph {
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
    // Points ripple across, a third of a beat from first to last.
    let delay = sim_rand(i, 5u) * 0.35 * len;
    let x = clamp((into - delay) / len, 0.0, 1.0);
    var m: Morph;
    m.a = (idx + NSHAPES - 1u) % NSHAPES;
    m.b = idx % NSHAPES;
    m.e = x * x * (3.0 - 2.0 * x);
    return m;
}

// Particle i's place in the figure (morph included), and its colour key.
fn home(i: u32) -> vec4<f32> {
    let r = rnd4(i);
    let m = morph(i);
    var p = mix(shape(m.a, r), shape(m.b, r), m.e);
    // Mid-morph the figure bursts open and comes back together.
    let burst = sin(PI * m.e);
    let dir = normalize(p + (r.yzw - 0.5) * 0.3 + vec3<f32>(1e-4));
    p += dir * burst * (0.45 + 0.25 * u.lvl4.x);
    // The kick swells it, the bass presence holds it a little larger.
    p *= 1.0 + 0.1 * kick_swell() + 0.06 * u.pres4.x;
    return vec4<f32>(p, mix(f32(m.a), f32(m.b), m.e));
}

fn sim_spawn(i: u32) -> Particle {
    return Particle(vec4<f32>(home(i).xyz, 0.0), vec4<f32>(0.0));
}

// Free points on a stiff spring to their home: the morph's path is the
// home's, and anything that jumps (the bar moved) becomes a short flight.
fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    let h4 = home(i);
    let h = h4.xyz;
    var x = p.pos.xyz;
    var v = p.vel.xyz;
    let calm = u.calm;
    let k = (120.0 + 60.0 * u.pres4.x) * (1.0 - 0.5 * calm);
    var acc = (h - x) * k - v * (1.6 * sqrt(k));
    let t = u.clock4.z * 0.02;
    acc += sim_curl(h * 0.5 + vec3<f32>(t, -0.4 * t, 0.7 * t)) * (0.06 + 0.5 * u.lvl4.y + 1.5 * calm);
    v += acc * dt;
    let dir = h / max(length(h), 1e-3);
    v += dir * simc.onset.x * 2.4 * (1.0 - 0.85 * calm);
    v += vec3<f32>(-h.z, 0.0, h.x) * simc.onset.z * 1.2 * (1.0 - 0.6 * calm);
    x += v * dt;
    // w: the colour key and how far the point is knocked loose, for the draw.
    let hw = h4.w;
    return Particle(vec4<f32>(x, hw), vec4<f32>(v, min(length(x - h) * 2.5, 1.0)));
}

// The turn: a quarter turn every two beats, eased in and out (a clock
// hand), its direction flipping each eight-bar phrase; slow and steady in
// breakdowns. Isometric pitch.
fn yaw() -> f32 {
    // Sixteen ticks a phrase; odd phrases wind back the way they came, so
    // the angle is continuous at every phrase line (and the 4096 wrap).
    let b = u.beat * 0.5;
    let f = fract(b);
    // In breakdowns the ticks melt into a steady turn.
    let e = mix(f * f * f * (f * (f * 6.0 - 15.0) + 10.0), f, u.calm);
    let n = f32(u32(b) % 16u) + e;
    let back = (u32(b) / 16u) % 2u == 1u;
    let ticked = select(n, 16.0 - n, back) * (PI * 0.5);
    return ticked + 0.7854;
}

fn view(w_in: vec3<f32>, ya: f32) -> vec3<f32> {
    var w = w_in;
    let wy = rot(ya) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.6155 + 0.08 * sin(TAU * u.flow / 32.0)) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 8.0);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let pp = sim_p[ii];
    let ya = yaw();
    let w = pp.pos.xyz;
    let a = view(w, ya);
    let s = sim_quad(a, view(w - pp.vel.xyz * 0.035, ya), vi, 0.0055, 5.2);
    o.pos = s.pos;
    o.q = s.q;
    let loose = pp.vel.w;
    let c = palette(0.11 * pp.pos.w + 0.25 * sim_rand(ii, 2u) + 0.15 * loose);
    let b = (0.2 + 0.18 * loose + 0.1 * u.lvl4.y) * clamp(9.2 - a.z, 0.5, 1.5);
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
    var col = prev(in.uv) * 0.45;
    col += vec3<f32>(0.005, 0.004, 0.011) * (1.0 - 0.45 * length(p));
    return vec4<f32>(finite(col), 1.0);
}
