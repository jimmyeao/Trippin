// @sim 140000 @bloom 0.5 @tonemap agx
// @title Particle Morph
// Particle Morph: 140,000 points in solid geometric figures, seen
// isometrically from above, that change on every beat: a cube, a six-point
// cross, an octahedron, a star of David, a torus, a stella octangula, a
// spiked star and a hexagonal prism, each screwing in and landing inside
// half a beat, every beat its own size and the kick pumping it bigger
// (shape). Faces glow faintly and edges brightest. The figure ticks a
// quarter turn every two beats (motion) and swings the other way each
// phrase (direction); in breakdowns a figure a bar, turning steadily.
// Every point is simulated: kicks blast it out, snares wrench it round.

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

// A point on a box's surface (uniform per face: a random point in the cube
// pushed out along its ray), pulled in towards the centre by f.
// w: how near a box edge the point is (two coordinates at the faces).
fn box_pt(c: vec3<f32>, h: vec3<f32>, r3: vec3<f32>, f: f32) -> vec4<f32> {
    var d = r3 * 2.0 - 1.0;
    d /= max(max(abs(d.x), abs(d.y)), max(abs(d.z), 1e-4));
    let a = abs(d);
    let mid = a.x + a.y + a.z - max(a.x, max(a.y, a.z)) - min(a.x, min(a.y, a.z));
    return vec4<f32>(c + d * h * f, smoothstep(0.93, 0.985, mid) * skin_of(f));
}

// How much of a skin point this is (f near 1).
fn skin_of(f: f32) -> f32 {
    return step(0.99, f);
}

// A point a share t round a regular n-gon (unit radius, a corner at the top).
fn poly(n: f32, t: f32) -> vec2<f32> {
    let s = floor(t * n);
    let a0 = s / n * TAU + PI * 0.5;
    let a1 = a0 + TAU / n;
    return mix(vec2<f32>(cos(a0), sin(a0)), vec2<f32>(cos(a1), sin(a1)), fract(t * n));
}

// The same round a six-point star (corners alternate 1 and 0.577).
fn star6(t: f32) -> vec2<f32> {
    let s = floor(t * 12.0);
    let a0 = s / 12.0 * TAU + PI * 0.5;
    let a1 = a0 + TAU / 12.0;
    let odd = (u32(s) % 2u) == 1u;
    let r0 = select(1.0, 0.577, odd);
    let r1 = select(0.577, 1.0, odd);
    return mix(vec2<f32>(cos(a0), sin(a0)) * r0, vec2<f32>(cos(a1), sin(a1)) * r1, fract(t * 12.0));
}

// Figure k's point for the particle with randoms r: solid figures, filled.
// f (0..1, mostly near 1) is how far out from the figure's heart the point
// sits, the same for every figure, so the inside morphs into the inside and
// the skin into the skin (the draw lights the skin brighter).
// w is how much of an edge the point is on (lit brightest).
fn shape(k: u32, r: vec4<f32>, f: f32) -> vec4<f32> {
    let r3 = r.yzw;
    switch k {
        case 0u: {
            // A solid cube.
            return box_pt(vec3<f32>(0.0), vec3<f32>(0.82), r3, f);
        }
        case 1u: {
            // The six-point cross: six square arms and the block at its heart.
            let part = u32(r.x * 7.0) % 7u;
            if part == 6u {
                return box_pt(vec3<f32>(0.0), vec3<f32>(0.3), r3, f);
            }
            let ax = part / 2u;
            let sg = sgn(part % 2u == 0u);
            var e = vec3<f32>(0.0);
            e[ax] = 1.0;
            let h = mix(vec3<f32>(0.3), vec3<f32>(0.52), e);
            return box_pt(e * sg * 0.8, h, r3, f);
        }
        case 2u: {
            // A solid octahedron (the L1 ball).
            var d = r3 * 2.0 - 1.0;
            d /= max(abs(d.x) + abs(d.y) + abs(d.z), 1e-4);
            let a = abs(d);
            let e = 1.0 - smoothstep(0.0, 0.035, min(a.x, min(a.y, a.z)));
            return vec4<f32>(d * 1.35 * f, e * skin_of(f));
        }
        case 3u: {
            // A star of David, cut thick from a slab.
            let q = star6(r.x) * 1.35 * f;
            let z = r.y * 2.0 - 1.0;
            return vec4<f32>(q.x, q.y, z * 0.22, skin_of(f) * (0.4 + 0.6 * smoothstep(0.75, 1.0, abs(z))));
        }
        case 4u: {
            // A solid torus.
            let phi = r.x * TAU;
            let th = r.y * TAU;
            let a = 0.4 * f;
            let rr = 0.85 + a * cos(th);
            // Rings of light round the tube.
            let ring = 1.0 - smoothstep(0.0, 0.06, abs(fract(r.x * 12.0) - 0.5) * 2.0 - 0.9);
            return vec4<f32>(rr * cos(phi), a * sin(th), rr * sin(phi), skin_of(f) * max(0.25, ring));
        }
        case 5u: {
            // Stella octangula: two interlocked solid tetrahedra.
            let j = u32(r.z * 4.0) % 4u;
            let i0 = (j + 1u) % 4u;
            let i1 = (j + 2u) % 4u;
            let i2 = (j + 3u) % 4u;
            let s = sqrt(r.x);
            let bc = vec3<f32>(1.0 - s, s * (1.0 - r.w), s * r.w);
            let p = (tetv(i0) * bc.x + tetv(i1) * bc.y + tetv(i2) * bc.z) * 0.85 * f;
            let e = 1.0 - smoothstep(0.0, 0.03, min(bc.x, min(bc.y, bc.z)));
            return vec4<f32>(select(p, -p, r.y < 0.5), e * skin_of(f));
        }
        case 6u: {
            // A spiked star: six solid cones out along the axes.
            let part = u32(r.y * 6.0) % 6u;
            let ax = part / 2u;
            let sg = sgn(part % 2u == 0u);
            let h = 1.0 - sqrt(r.x);
            let rad = 0.42 * (1.0 - h);
            let a = r.z * TAU;
            var q = vec3<f32>(0.15 + 1.25 * h, cos(a) * rad, sin(a) * rad);
            if ax == 1u {
                q = vec3<f32>(q.y, q.x, q.z);
            } else if ax == 2u {
                q = vec3<f32>(q.y, q.z, q.x);
            }
            var p = q;
            p[ax] *= sg;
            // Bright tips and a seam down each spike.
            let e = max(smoothstep(0.75, 1.0, h), 1.0 - smoothstep(0.0, 0.08, abs(fract(r.z * 4.0) - 0.5) * 2.0 - 0.88));
            return vec4<f32>(p * f, e * skin_of(f));
        }
        default: {
            // A solid hexagonal prism.
            if r.y < 0.6 {
                let q = poly(6.0, r.x);
                let y = r.z * 2.0 - 1.0;
                let side = abs(fract(r.x * 6.0) - 0.5) * 2.0;
                let e = max(smoothstep(0.9, 1.0, side), smoothstep(0.93, 1.0, abs(y)));
                return vec4<f32>(vec3<f32>(q.x, y * 0.72, q.y) * f, e * skin_of(f));
            }
            let q = poly(6.0, r.x) * sqrt(r.z);
            return vec4<f32>(vec3<f32>(q.x, sgn(r.w < 0.5) * 0.72, q.y) * f, smoothstep(0.93, 1.0, sqrt(r.z)) * skin_of(f));
        }
    }
}

fn rnd4(i: u32) -> vec4<f32> {
    return vec4<f32>(sim_rand(i, 0u), sim_rand(i, 1u), sim_rand(i, 2u), sim_rand(i, 3u));
}

// How deep in the figure the particle sits (1 = the skin).
// 60% of the points sit on the skin itself, the rest fill the volume.
fn fill(i: u32) -> f32 {
    let r = sim_rand(i, 4u);
    if r < 0.6 {
        return 1.0;
    }
    return 0.96 * pow((r - 0.6) / 0.4, 0.3333);
}

// The beat's own size: big on the one, small on the two, and so on.
fn beat_size(n: u32) -> f32 {
    switch n % 4u {
        case 0u: { return 1.18; }
        case 1u: { return 0.8; }
        case 2u: { return 1.04; }
        default: { return 0.74; }
    }
}

// Where the morph stands: (from, to figure, progress 0..1 for this
// particle, size). A new figure on every beat, landing fast (ease out over
// under half a beat); in breakdowns one a bar, eased over a beat and a half.
struct Morph {
    a: u32,
    b: u32,
    e: f32,
    s: f32,
};

fn morph(i: u32) -> Morph {
    let bt = u32(max(floor(u.beat), 0.0));
    let slow = u.calm > 0.5;
    var idx = bt;
    var into = fract(u.beat);
    var len = 0.42;
    if slow {
        idx = bt / 4u;
        into += f32(bt % 4u);
        len = 1.5;
    }
    // Points ripple across, a tenth of the morph from first to last.
    let x = clamp((into - sim_rand(i, 5u) * 0.12 * len) / len, 0.0, 1.0);
    let ex = 1.0 - x;
    var m: Morph;
    m.a = (idx + NSHAPES - 1u) % NSHAPES;
    m.b = idx % NSHAPES;
    m.e = 1.0 - ex * ex * ex;
    m.s = select(mix(beat_size(idx + 3u), beat_size(idx), m.e), 1.0, slow);
    return m;
}

// Particle i's place in the figure (morph included), and its colour key.
fn home(i: u32) -> vec4<f32> {
    let r = rnd4(i);
    let f = fill(i);
    let m = morph(i);
    let sh = mix(shape(m.a, r, f), shape(m.b, r, f), m.e);
    var p = sh.xyz;
    // The new figure screws in as it lands (an eighth turn, alternating
    // ways), and the mids wring it about its axis.
    let ang = (1.0 - m.e) * 0.8 * sgn(m.b % 2u == 0u) + p.y * 0.9 * u.lvl4.y;
    let q = rot(ang) * p.xz;
    p = vec3<f32>(q.x, p.y, q.y);
    // Each beat sets its size; the kick pumps it bigger and stretches it.
    let ks = kick_swell();
    p *= 0.86 * m.s * (1.0 + 0.38 * ks + 0.08 * u.pres4.x);
    p.y *= 1.0 + 0.18 * ks;
    // w: the colour key (figure + progress, 0..8) plus 16 x the edge
    // weight in 16 steps, unpacked by the draw.
    return vec4<f32>(p, mix(f32(m.a), f32(m.b), m.e) + 16.0 * floor(sh.w * 15.0 + 0.5));
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
    let k = (170.0 + 60.0 * u.pres4.x) * (1.0 - 0.5 * calm);
    var acc = (h - x) * k - v * (1.6 * sqrt(k));
    let t = u.clock4.z * 0.02;
    // No swirl while the beat plays: it frays the straight edges.
    acc += sim_curl(h * 0.5 + vec3<f32>(t, -0.4 * t, 0.7 * t)) * (0.6 * calm);
    v += acc * dt;
    let dir = h / max(length(h), 1e-3);
    v += dir * simc.onset.x * 1.4 * (1.0 - 0.85 * calm);
    v += vec3<f32>(-h.z, 0.0, h.x) * simc.onset.z * 1.2 * (1.0 - 0.6 * calm);
    x += v * dt;
    // w: the colour key and how far the point is knocked loose, for the draw.
    // pos.w: the colour key plus 16 x the edge weight (0..15); vel.w: loose.
    return Particle(vec4<f32>(x, h4.w), vec4<f32>(v, min(length(x - h) * 2.5, 1.0)));
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
    let wp = rot(-0.6155 + 0.08 * sin(TAU * u.flow / 32.0)) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 8.0);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let pp = sim_p[ii];
    let ya = yaw();
    let w = pp.pos.xyz;
    let a = view(w, ya);
    let s = sim_quad(a, view(w - pp.vel.xyz * 0.02, ya), vi, 0.0036, 4.6);
    o.pos = s.pos;
    o.q = s.q;
    let loose = pp.vel.w;
    let edge = floor(pp.pos.w / 16.0) / 15.0;
    let key = pp.pos.w - floor(pp.pos.w / 16.0) * 16.0;
    let c = palette(0.11 * key + 0.25 * sim_rand(ii, 2u) + 0.15 * loose);
    // Faces glow faintly and the edges brightest, so a solid figure keeps
    // its shape in additive light.
    let skin = step(sim_rand(ii, 4u), 0.6);
    let b = (0.025 + 0.05 * skin + 0.3 * edge + 0.15 * loose + 0.05 * u.lvl4.y) * clamp(9.2 - a.z, 0.5, 1.5);
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
