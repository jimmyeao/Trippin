// @sim 8000 @bloom 0.6 @tonemap agx
// @title Drone Show
// Drone Show: 8,000 drones over a mountain lake at night, flying from one
// formation to the next every two bars: a globe, a double helix, a spiral
// galaxy, a sheet whose heights are the spectrum, a beating heart, a
// trefoil knot, a starburst and three nested cubes. Each formation changes
// shape with the music (the kick pumps it, rings run out on the beat, the
// spectrum lifts the sheet) and turns on the energy clock (motion); each
// change is a flight, every drone on its own arc. A drone in eight flashes
// on the beat, and the lake mirrors the show.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

const N_DRONES: u32 = 8000u;
const CENTRE: vec3<f32> = vec3<f32>(0.0, 1.4, 0.0); // the show's middle in the sky
const Z0: f32 = 9.0;        // its distance from the camera
const FL: f32 = 1.8;        // focal length (screen y = world y * FL / z)
const HORIZON: f32 = 0.42;  // centred() y of the horizon (down positive)

fn sgn(b: bool) -> f32 {
    return select(-1.0, 1.0, b);
}

fn ring(a: f32, r: f32) -> vec2<f32> {
    return vec2<f32>(cos(a), sin(a)) * r;
}

fn ease(x: f32) -> f32 {
    let c = clamp(x, 0.0, 1.0);
    return c * c * (3.0 - 2.0 * c);
}

// A point on the unit sphere for two randoms.
fn sph(a: f32, b: f32) -> vec3<f32> {
    let z = a * 2.0 - 1.0;
    let q = ring(b * TAU, sqrt(max(0.0, 1.0 - z * z)));
    return vec3<f32>(q.x, z, q.y);
}

// A stable random for a 2D cell (any sign) and a salt.
fn h2(c: vec2<f32>, s: u32) -> f32 {
    let ci = vec2<i32>(floor(c)) + vec2<i32>(100000);
    return f32(pcg(u32(ci.x) * 7919u ^ pcg(u32(ci.y) * 104729u + s * 31337u)) & 0xFFFFu) / 65535.0;
}

// Colours ride in pos.w as 8-bit rgb (exact in an f32's 24-bit mantissa).
fn pack_rgb(c: vec3<f32>) -> f32 {
    let q = floor(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)) * 255.0 + 0.5);
    return q.x * 65536.0 + q.y * 256.0 + q.z;
}

fn unpack_rgb(w: f32) -> vec3<f32> {
    let v = u32(max(w, 0.0));
    return vec3<f32>(f32((v >> 16u) & 255u), f32((v >> 8u) & 255u), f32(v & 255u)) / 255.0;
}

// The bar count (0..1023; the beat count wraps at 4096), on the downbeat.
fn bar_now() -> u32 {
    return u32(max(floor((u.beat - u.bar_phase * 4.0) / 4.0 + 0.5), 0.0)) % 1024u;
}

// A drone's place in a formation, its colour and whether it's lit.
struct Form {
    p: vec3<f32>,
    col: vec3<f32>,
    lit: f32,
};

fn mk(p: vec3<f32>, col: vec3<f32>, lit: f32) -> Form {
    var f: Form;
    f.p = p;
    f.col = col;
    f.lit = lit;
    return f;
}

// A spare drone: dark, parked high behind the show.
fn parked(i: u32) -> Form {
    return mk(CENTRE + vec3<f32>((sim_rand(i, 7u) - 0.5) * 9.0, 1.5 + sim_rand(i, 8u) * 1.5, 4.0), vec3<f32>(0.0), 0.0);
}

// ---- Text (the title texture; two lines in TWO_LINE_SCENES) -------------

// Where line l sits in the title texture: its rows (v0..v1), its share of
// the width, and its box height in texels. Two lines pack both widths in
// look4.w (gfx::Statics::upload_title_text); one line serves both.
struct Line {
    v0: f32,
    v1: f32,
    share: f32,
    hpx: f32,
};

fn line_info(l: u32) -> Line {
    let w = u.look4.w;
    let hi = floor(w);
    var o: Line;
    if hi < 0.5 {
        o.v0 = 0.0;
        o.v1 = 1.0;
        o.share = max(w, 0.0005);
        o.hpx = 160.0;
        return o;
    }
    o.share = max(select(w - hi, hi / 1000.0, l == 1u), 0.0005);
    o.v0 = select(0.0, 0.5, l == 1u);
    o.v1 = o.v0 + 0.5;
    o.hpx = 80.0;
    return o;
}

// The drone's point in line l's box (0..1 each way), or -1: the first of a
// few fixed random tries that lands in a letter, so it is the same every
// frame until the text changes.
fn text_hit(i: u32, l: u32) -> vec2<f32> {
    let li = line_info(l);
    for (var k = 0u; k < 14u; k++) {
        let x = sim_rand(i, 20u + 2u * k + 40u * l);
        let y = sim_rand(i, 21u + 2u * k + 40u * l);
        let m = textureSampleLevel(title_tex, samp, vec2<f32>(x * li.share, mix(li.v0, li.v1, y)), 0.0).r;
        if m > 0.5 {
            return vec2<f32>(x, y);
        }
    }
    return vec2<f32>(-1.0);
}

// Line l spelt in drones across the sky, as wide as maxw or as tall as
// maxh allows. Each beat a hop runs along the letters (towards the camera
// and up), the kick pumps the line, and in breakdowns it ripples. col.x
// carries the point's share along the line for the caller's colours.
fn text_form(i: u32, l: u32, maxw: f32, maxh: f32) -> Form {
    let hit = text_hit(i, l);
    if hit.x < 0.0 {
        return parked(i);
    }
    let li = line_info(l);
    let wpx = li.share * 2048.0;
    let s = min(maxw / wpx, maxh / li.hpx);
    var p = vec3<f32>((hit.x - 0.5) * wpx * s, (0.5 - hit.y) * li.hpx * s, 0.0);
    p *= 1.0 + 0.08 * kick_swell();
    let hop = beat_ring(hit.x, 0.14);
    p.y += 0.3 * hop;
    p.z -= 1.0 * hop;
    p.z += 0.35 * sin(hit.x * 7.0 - u.flow * 1.2) * (0.25 + 0.75 * u.calm);
    return mk(CENTRE + p, vec3<f32>(hit.x, hit.y, 0.0), 1.0);
}

// ---- Abstract formations -------------------------------------------------

// Formation k (0..7) for drone i: a globe, a double helix, a spiral galaxy,
// a sheet whose heights are the spectrum, a beating heart, a trefoil knot,
// a starburst and three nested cubes. Each moves on the energy clock and
// changes shape with the music (kick pumps, beat rings, the spectrum).
fn abs_form(k: u32, i: u32) -> Form {
    let q = (f32(i) + 0.5) / f32(N_DRONES);
    let r0 = sim_rand(i, 12u);
    let r1 = sim_rand(i, 13u);
    let r2 = sim_rand(i, 14u);
    let ks = kick_swell();
    let cl = u.clock4.x;
    var p = vec3<f32>(0.0);
    var t = 0.0;
    var col = vec3<f32>(-1.0);
    switch k {
        case 0u: {
            // A globe (Fibonacci points), tilted, turning, pumped by the kick.
            let y = 1.0 - 2.0 * q;
            let ph = f32(i) * 2.39996;
            let c = ring(ph, sqrt(max(0.0, 1.0 - y * y)));
            p = vec3<f32>(c.x, y, c.y) * 2.1 * (1.0 + 0.22 * ks);
            let w = rot(cl * 0.1) * p.xz;
            p = vec3<f32>(w.x, p.y, w.y);
            let wt = rot(-0.35) * p.yz;
            p = vec3<f32>(p.x, wt.x, wt.y);
            t = 0.5 + 0.5 * y;
        }
        case 1u: {
            // A double helix with rungs, twisting on the clock; the mids wring it.
            let strand = i % 3u;
            var yy = q;
            if strand == 2u {
                yy = (floor(q * 30.0) + 0.5) / 30.0;
            }
            let y = (yy - 0.5) * 4.6;
            let ang = yy * TAU * 2.4 + cl * 0.18 + y * 0.6 * u.lvl4.y;
            let rad = 1.15 * (1.0 + 0.25 * ks);
            if strand < 2u {
                let c = ring(ang + f32(strand) * PI, rad);
                p = vec3<f32>(c.x, y, c.y);
                t = select(0.15, 0.75, strand == 1u);
            } else {
                let c = ring(ang, rad) * (r0 * 2.0 - 1.0);
                p = vec3<f32>(c.x, y, c.y);
                t = 0.45;
            }
        }
        case 2u: {
            // A spiral galaxy, tilted towards us; each beat a ring runs out.
            let arm = f32(i % 3u);
            let tt = sqrt(r0);
            let ang = arm * TAU / 3.0 + tt * 4.2 + cl * 0.12 + (r1 - 0.5) * 0.35;
            let rad = tt * 2.7 * (1.0 + 0.3 * beat_ring(tt, 0.25)) + 0.05;
            p = vec3<f32>(cos(ang) * rad, (r2 - 0.5) * 0.2 * (1.0 - tt), sin(ang) * rad);
            let wt = rot(-1.0) * p.yz;
            p = vec3<f32>(p.x, wt.x, wt.y);
            t = tt;
        }
        case 3u: {
            // A sheet of 100 x 80 drones whose heights are the spectrum,
            // with a swell rolling through it.
            let gx = f32(i % 100u) / 99.0;
            let gz = f32((i / 100u) % 80u) / 79.0;
            let x = (gx - 0.5) * 7.5;
            let z = (gz - 0.5) * 3.2;
            let h = 1.6 * spec(0.03 + 0.55 * gx) * (1.0 - 0.4 * abs(gz - 0.5) * 2.0) + 0.35 * sin(x * 1.1 - cl * 0.6 + z * 1.3);
            p = vec3<f32>(x, h - 0.6, z);
            let wt = rot(-0.5) * p.yz;
            p = vec3<f32>(p.x, wt.x, wt.y);
            t = clamp(h * 0.45 + 0.3, 0.0, 1.0);
        }
        case 4u: {
            // A heart that beats on the kick, swinging on the phrase.
            let a = r0 * TAU;
            let s3 = sin(a);
            let hx = 16.0 * s3 * s3 * s3;
            let hy = 13.0 * cos(a) - 5.0 * cos(2.0 * a) - 2.0 * cos(3.0 * a) - cos(4.0 * a);
            let fill = select(sqrt(r1), 1.0, r2 < 0.45);
            p = vec3<f32>(hx, hy + 2.5, 0.0) * 0.13 * fill;
            p.z = (fract(r1 * 7.0) - 0.5) * 0.7 * (1.0 - 0.6 * fill);
            p *= 1.0 + 0.35 * ks;
            let w = rot(0.5 * sin(TAU * u.flow / 32.0)) * p.xz;
            p = vec3<f32>(w.x, p.y, w.y);
            col = mix(vec3<f32>(1.0, 0.08, 0.2), vec3<f32>(1.0, 0.45, 0.7), fill * fill);
        }
        case 5u: {
            // A trefoil knot, a thick tube of drones, tumbling.
            let a = q * TAU;
            let c = vec3<f32>(sin(a) + 2.0 * sin(2.0 * a), cos(a) - 2.0 * cos(2.0 * a), -sin(3.0 * a));
            p = (c * 0.62 + (vec3<f32>(r0, r1, r2) - 0.5) * 0.28) * (1.0 + 0.15 * ks);
            let w = rot(cl * 0.1) * p.xz;
            p = vec3<f32>(w.x, p.y, w.y);
            let wt = rot(0.3) * p.yz;
            p = vec3<f32>(p.x, wt.x, wt.y);
            t = q;
        }
        case 6u: {
            // A starburst of 40 rays that shoot out on every beat.
            let ray = i % 40u;
            let yy = 1.0 - 2.0 * (f32(ray) + 0.5) / 40.0;
            let c = ring(f32(ray) * 2.39996, sqrt(max(0.0, 1.0 - yy * yy)));
            let dir = vec3<f32>(c.x, yy, c.y);
            let len = 2.4 * (0.55 + 0.35 * ks) * (1.0 + 0.4 * beat_ring(r0, 0.3));
            p = dir * (0.15 + r0 * len);
            let w = rot(cl * 0.08) * p.xz;
            p = vec3<f32>(w.x, p.y, w.y);
            t = r0;
        }
        default: {
            // Three nested wire cubes, neighbours turning opposite ways.
            let cube = i % 3u;
            let sz = (0.75 + 0.55 * f32(cube)) * (1.0 + 0.2 * ks);
            let e = u32(r0 * 12.0) % 12u;
            let axis = e % 3u;
            let b = e / 3u;
            let s1 = sgn((b & 1u) != 0u);
            let s2 = sgn((b & 2u) != 0u);
            let al = r1 * 2.0 - 1.0;
            p = vec3<f32>(al, s1, s2);
            if axis == 1u {
                p = vec3<f32>(s1, al, s2);
            } else if axis == 2u {
                p = vec3<f32>(s1, s2, al);
            }
            p *= sz;
            let w = rot(cl * 0.12 * sgn(cube == 1u) + 0.6) * p.xz;
            p = vec3<f32>(w.x, p.y, w.y);
            let wt = rot(-0.5) * p.yz;
            p = vec3<f32>(p.x, wt.x, wt.y);
            t = f32(cube) / 2.0;
        }
    }
    if col.x < 0.0 {
        let c = palette(fract(t * 0.8 + cl * 0.01));
        col = c;
    }
    return mk(CENTRE + p, col, 1.0);
}

// ---- The show: a formation per slot, flown between ------------------------

// From formation a to b over a couple of beats after the bar line, each
// drone setting off a little after the last and swinging out on its own
// arc, so the change reads as a flight, not a morph.
fn fly(i: u32, a: Form, b: Form, into: f32) -> Form {
    let d = sim_rand(i, 6u) * 0.8;
    let e = ease((into - d) / 1.8);
    let rd = vec3<f32>(sim_rand(i, 9u), sim_rand(i, 10u), sim_rand(i, 11u)) - 0.5;
    var o: Form;
    o.p = mix(a.p, b.p, e) + sin(PI * e) * (rd * 2.4 + vec3<f32>(0.0, 0.4, -1.4));
    o.col = mix(a.col, b.col, e);
    o.lit = mix(a.lit, b.lit, e);
    return o;
}

fn home(i: u32) -> Form {
    let bar = bar_now();
    let pbar = (bar + 1023u) % 1024u;
    let cur = slot(bar);
    let prv = slot(pbar);
    let b = slot_form(cur, i);
    if cur == prv {
        return b;
    }
    return fly(i, slot_form(prv, i), b, u.bar_phase * 4.0);
}

fn sim_spawn(i: u32) -> Particle {
    let f = home(i);
    return Particle(vec4<f32>(f.p, pack_rgb(f.col)), vec4<f32>(0.0, 0.0, 0.0, f.lit));
}

// Drones are free on a spring to their place in the show: the spring is
// their inertia, and the kick knocks the whole formation out a little.
fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    let f = home(i);
    var x = p.pos.xyz;
    var v = p.vel.xyz;
    let calm = u.calm;
    let k = 60.0 * (1.0 - 0.35 * calm);
    v += ((f.p - x) * k - v * (1.7 * sqrt(k))) * dt;
    let r = f.p - CENTRE;
    v += r / max(length(r), 0.4) * simc.onset.x * 0.9 * (1.0 - 0.85 * calm);
    x += v * dt;
    return Particle(vec4<f32>(x, pack_rgb(f.col)), vec4<f32>(v, f.lit));
}

// On each beat about one drone in eight flashes white and fades.
fn sparkle(i: u32) -> f32 {
    let b = u32(max(floor(u.beat), 0.0));
    let h = f32(pcg(i * 9781u + b * 6271u) & 1023u) / 1023.0;
    return select(0.0, exp(-5.0 * fract(u.beat)), h < 0.12) * (1.0 - 0.7 * u.calm);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    o.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    o.q = vec2<f32>(0.0);
    o.col = vec3<f32>(0.0);
    let pp = sim_p[ii];
    let lit = pp.vel.w;
    if lit < 0.01 {
        return o;
    }
    let a = vec3<f32>(pp.pos.xy, pp.pos.z + Z0);
    let s = sim_quad(a, a - pp.vel.xyz * 0.015, vi, DRONE_SIZE, FL);
    o.pos = s.pos;
    o.q = s.q;
    let c = unpack_rgb(pp.pos.w);
    o.col = (c * DRONE_BRIGHT + vec3<f32>(0.5 * sparkle(ii))) * lit * s.dim;
    return o;
}

// A drone: a hot core in a soft halo.
@fragment
fn fs_particles(in: POut) -> @location(0) vec4<f32> {
    let r2 = dot(in.q, in.q);
    if r2 > 1.0 {
        discard;
    }
    let g = 1.0 - r2;
    return vec4<f32>(in.col * (g * g * g * 1.6 + g * 0.12), 0.0);
}

// ---- Backdrop helpers -----------------------------------------------------

// Stars: a sparse field, twinkling.
fn stars(p: vec2<f32>) -> f32 {
    let g = p * 70.0;
    let c = floor(g);
    let f = fract(g) - 0.5;
    let h = h2(c, 1u);
    if h < 0.975 {
        return 0.0;
    }
    let off = vec2<f32>(h2(c, 2u), h2(c, 3u)) - 0.5;
    let d = length(f - off * 0.6);
    let tw = 0.6 + 0.4 * sin(u.time * (1.0 + 3.0 * h2(c, 4u)) + h * 40.0);
    return smoothstep(0.12, 0.0, d) * tw * (0.3 + 0.7 * h2(c, 5u));
}

// The water below the horizon: last frame's sky mirrored, rippled (the
// bass stirs it), darker with distance from the shore.
fn water(p: vec2<f32>) -> vec3<f32> {
    let dy = p.y - HORIZON;
    let wav = sin(p.y * 170.0 + u.time * 1.6 + sin(p.x * 3.0) * 2.0) * (0.002 + 0.025 * dy) * (1.0 + 0.8 * u.lvl4.x);
    let m = vec2<f32>(p.x + wav, HORIZON - dy * 1.15);
    let refl = prev(uncentred(m)) * (0.55 - 0.35 * smoothstep(0.0, 0.6, dy));
    return vec3<f32>(0.003, 0.005, 0.012) + refl;
}

const DRONE_SIZE: f32 = 0.03;
const DRONE_BRIGHT: f32 = 0.55;

// A formation every two bars, in order.
fn slot(bar: u32) -> u32 {
    return (bar / 2u) % 8u;
}

fn slot_form(s: u32, i: u32) -> Form {
    return abs_form(s, i);
}

// Mountain ridges, far and near.
fn ridge(x: f32, k: f32) -> f32 {
    return HORIZON - (0.13 - 0.05 * k) - (0.07 + 0.02 * k) * sin(x * (1.3 + k) + 1.0 + 3.0 * k) - 0.04 * sin(x * (3.7 - k) + 2.0 * k) - 0.015 * sin(x * 9.0 + k * 5.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    if p.y > HORIZON {
        return vec4<f32>(finite(water(p)), 1.0);
    }
    var col = mix(vec3<f32>(0.004, 0.006, 0.016), vec3<f32>(0.016, 0.02, 0.045), smoothstep(-1.0, HORIZON, p.y));
    col += vec3<f32>(0.75, 0.8, 1.0) * stars(p) * 0.5 * smoothstep(HORIZON, -0.2, p.y);
    // Far ridge in haze, near ridge dark.
    let far = ridge(p.x, 0.0);
    let near = ridge(p.x * 0.8 + 3.0, 1.0) + 0.06;
    if p.y > far {
        col = vec3<f32>(0.012, 0.016, 0.03);
    }
    if p.y > near {
        col = vec3<f32>(0.004, 0.005, 0.009);
    }
    // Shore lights: a scatter of warm points at the waterline.
    let sx = p.x * 45.0;
    let c = floor(sx);
    let lit = step(h2(vec2<f32>(c, 7.0), 8u), 0.16);
    let off = h2(vec2<f32>(c, 9.0), 10u) - 0.5;
    let d = length(vec2<f32>(fract(sx) - 0.5 - off * 0.6, (p.y - (HORIZON - 0.004)) * 45.0));
    col += vec3<f32>(1.0, 0.7, 0.35) * lit * smoothstep(0.16, 0.0, d) * 1.5;
    return vec4<f32>(finite(col), 1.0);
}
