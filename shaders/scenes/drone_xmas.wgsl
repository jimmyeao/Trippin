// @sim 8000 @bloom 0.6 @tonemap agx
// @title Christmas Drone Show
// @no-dancer
// Christmas Drone Show: 8,000 drones over a snowy village fly a Christmas
// show, a new figure every two bars: a tree, a star, MERRY CHRISTMAS, a
// snowman, a snowflake, a bell, the DJ's name (or HAPPY HOLIDAYS), a
// present, and Santa on his sleigh pulled by three reindeer (Rudolph in
// front) galloping a stride a beat, trailing stardust. Each figure turns,
// swings, gallops or pumps with the music (shape and motion), each change is a flight with every drone on its own arc, a hop
// runs along the words on the beat, and snow falls. Seasonal (config.rs).
// Two-line title (gfx.rs).

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
const DRONE_BRIGHT: f32 = 0.6;

const RED: vec3<f32> = vec3<f32>(1.0, 0.1, 0.12);
const GREEN: vec3<f32> = vec3<f32>(0.15, 0.9, 0.3);
const GOLD: vec3<f32> = vec3<f32>(1.0, 0.7, 0.2);
const WHITE: vec3<f32> = vec3<f32>(0.9, 0.93, 1.0);

// Figures: 0 tree, 1 star, 2 MERRY CHRISTMAS, 3 snowman, 4 snowflake,
// 5 bell, 6 the DJ line, 7 present, 8 Santa's sleigh. Two bars each, in a
// sixteen-slot running order (it divides the 512 slots the bar count wraps
// at) that brings the sleigh round three times.
fn slot(bar: u32) -> u32 {
    switch (bar / 2u) % 16u {
        case 0u: { return 0u; }
        case 1u: { return 1u; }
        case 2u: { return 2u; }
        case 3u: { return 8u; }
        case 4u: { return 3u; }
        case 5u: { return 4u; }
        case 6u: { return 6u; }
        case 7u: { return 5u; }
        case 8u: { return 8u; }
        case 9u: { return 7u; }
        case 10u: { return 2u; }
        case 11u: { return 1u; }
        case 12u: { return 4u; }
        case 13u: { return 8u; }
        case 14u: { return 6u; }
        default: { return 3u; }
    }
}

// A point in a filled ellipse, and on a thick segment, for two randoms.
fn ell(c: vec2<f32>, rad: vec2<f32>, a: f32, b: f32) -> vec2<f32> {
    return c + ring(a * TAU, sqrt(b)) * rad;
}

fn seg(a: vec2<f32>, b: vec2<f32>, w: f32, t: f32, s: f32) -> vec2<f32> {
    let d = b - a;
    let n = normalize(vec2<f32>(-d.y, d.x) + vec2<f32>(1e-5, 0.0));
    return a + d * t + n * (s - 0.5) * w;
}

// Down a leg swung by angle a (positive: the foot forward).
fn leg_dir(a: f32) -> vec2<f32> {
    return vec2<f32>(sin(a), -cos(a));
}

// Where reindeer k is (its body's middle): in single file ahead of the
// sleigh, bobbing with its own stride.
fn deer_at(k: u32) -> vec2<f32> {
    let ph = fract(fract(u.beat) + f32(k) * 0.17);
    return vec2<f32>(-0.4 + 1.7 * f32(k), 0.1 + 0.09 * abs(sin(PI * ph)));
}

// Santa on his sleigh, side on and heading right, pulled by three reindeer
// whose legs gallop a stride a beat (front and back pairs half a stride
// apart), Rudolph's red nose in front, gold reins sagging from Santa's
// hand, and stardust streaming behind. Design units, roughly x -6.5..4.4.
fn sleigh(i: u32, r: vec4<f32>) -> Form {
    let r5 = sim_rand(i, 17u);
    var q = vec2<f32>(0.0);
    var col = RED;
    var lit = 1.0;
    if r.y < 0.42 {
        let k = min(u32(r.y / 0.14), 2u);
        let sub = fract(r.y / 0.14);
        let o = deer_at(k);
        let ph = fract(fract(u.beat) + f32(k) * 0.17);
        col = vec3<f32>(0.85, 0.52, 0.26);
        if sub < 0.4 {
            q = ell(o, vec2<f32>(0.5, 0.21), r.x, r.z);
        } else if sub < 0.5 {
            q = seg(o + vec2<f32>(0.36, 0.05), o + vec2<f32>(0.62, 0.45), 0.16, r.x, r.z);
        } else if sub < 0.58 {
            q = ell(o + vec2<f32>(0.74, 0.5), vec2<f32>(0.17, 0.085), r.x, r.z);
        } else if sub < 0.68 {
            // Antlers: two beams sweeping back and up, each with two tines.
            let base = o + vec2<f32>(select(0.66, 0.59, r.z < 0.5), 0.57);
            let tip = base + vec2<f32>(-0.22, 0.38);
            let part = u32(r.w * 3.0) % 3u;
            if part == 0u {
                q = seg(base, tip, 0.045, r.x, r5);
            } else {
                let at = mix(base, tip, select(0.4, 0.75, part == 2u));
                q = seg(at, at + vec2<f32>(0.12, 0.13), 0.04, r.x, r5);
            }
            col = vec3<f32>(1.0, 0.9, 0.72);
        } else if sub < 0.96 {
            // Legs: thigh and shin, swinging about the hip; each knee
            // folds as its leg comes forward off the ground.
            let leg = u32(r.z * 4.0) % 4u;
            let front = leg < 2u;
            let lp = fract(ph + select(0.5, 0.0, front) + f32(leg % 2u) * 0.12);
            let ang = 0.7 * sin(TAU * lp);
            let fold = 0.5 + 0.5 * cos(TAU * lp);
            let hip = o + vec2<f32>(select(-0.36, 0.34, front), -0.12);
            let knee = hip + leg_dir(ang) * 0.3;
            let foot = knee + leg_dir(ang + select(0.6, -0.9, front) * fold) * 0.3;
            q = select(seg(knee, foot, 0.06, r.x, r5), seg(hip, knee, 0.09, r.x, r5), r.w < 0.5);
        } else {
            if k == 2u && r.w < 0.6 {
                q = ell(o + vec2<f32>(0.93, 0.5), vec2<f32>(0.065), r.x, r.z);
                col = vec3<f32>(1.0, 0.05, 0.05);
            } else {
                q = ell(o + vec2<f32>(-0.53, 0.12), vec2<f32>(0.08, 0.05), r.x, r.z);
            }
        }
    } else if r.y < 0.6 {
        // The sleigh: a red tub, higher at the back with a gold rim, a
        // curled front, and gold runners on struts.
        let sub = fract((r.y - 0.42) / 0.18);
        if sub < 0.55 {
            let x = mix(-3.7, -1.75, r.x);
            let top = 0.5 + 0.3 * smoothstep(-2.6, -3.7, x);
            q = vec2<f32>(x, mix(-0.35, top, r.z));
            col = select(RED, GOLD, r.z > 0.93);
        } else if sub < 0.7 {
            q = vec2<f32>(-1.75, 0.12) + ring(r.x * 1.6 * PI - 0.5 * PI, 0.4 * mix(0.7, 1.0, r.z));
        } else if sub < 0.9 {
            if r.z < 0.72 {
                q = seg(vec2<f32>(-3.9, -0.75), vec2<f32>(-1.5, -0.75), 0.06, r.x, r5);
            } else {
                q = vec2<f32>(-1.5, -0.45) + ring(-0.5 * PI + r.x * 1.2 * PI, 0.3);
            }
            col = GOLD;
        } else {
            let sx = select(-3.3, -2.05, r.z < 0.5);
            q = seg(vec2<f32>(sx, -0.75), vec2<f32>(sx, -0.35), 0.06, r.x, r5);
            col = GOLD;
        }
    } else if r.y < 0.76 {
        // Santa: red coat trimmed white, face, beard, hat and pompom, one
        // arm out on the reins.
        let sub = fract((r.y - 0.6) / 0.16);
        if sub < 0.45 {
            q = ell(vec2<f32>(-2.55, 0.85), vec2<f32>(0.4, 0.45), r.x, r.z);
            col = select(RED, WHITE, q.y < 0.52);
        } else if sub < 0.55 {
            q = ell(vec2<f32>(-2.35, 1.42), vec2<f32>(0.17), r.x, r.z);
            col = vec3<f32>(1.0, 0.72, 0.58);
        } else if sub < 0.7 {
            q = ell(vec2<f32>(-2.27, 1.22), vec2<f32>(0.18, 0.22), r.x, r.z);
            col = WHITE;
        } else if sub < 0.85 {
            if r.w < 0.3 {
                q = seg(vec2<f32>(-2.6, 1.56), vec2<f32>(-2.12, 1.56), 0.09, r.x, r5);
                col = WHITE;
            } else {
                q = seg(vec2<f32>(-2.38, 1.58), vec2<f32>(-2.75, 1.92), 0.34 * (1.0 - r.x) + 0.04, r.x, r5);
            }
        } else if sub < 0.9 {
            q = ell(vec2<f32>(-2.78, 1.92), vec2<f32>(0.08), r.x, r.z);
            col = WHITE;
        } else {
            q = seg(vec2<f32>(-2.35, 1.0), vec2<f32>(-1.95, 0.8), 0.13, r.x, r5);
        }
    } else if r.y < 0.81 {
        // The sack of presents behind him.
        q = ell(vec2<f32>(-3.25, 0.85), vec2<f32>(0.36, 0.42), r.x, r.z);
        col = vec3<f32>(0.8, 0.5, 0.22);
    } else if r.y < 0.88 {
        // Reins from his hand to each reindeer's collar, sagging.
        let k = u32(r.z * 3.0) % 3u;
        let b = deer_at(k) + vec2<f32>(0.42, 0.15);
        q = mix(vec2<f32>(-1.95, 0.8), b, r.x) - vec2<f32>(0.0, 0.18 * sin(PI * r.x));
        col = GOLD;
    } else {
        // Stardust streaming out behind, fading as it goes; dark where a
        // grain wraps round to the sleigh again.
        let t = fract(r.x + u.time * 0.35);
        let x = -3.9 - t * 2.6;
        q = vec2<f32>(x, -0.6 + 0.25 * sin(x * 2.5 + u.time * 2.0) * t + (r.z - 0.5) * 0.35 * t);
        col = mix(GOLD, WHITE, r.w);
        lit = smoothstep(0.0, 0.08, t) * (1.0 - t);
    }
    // The team rides the phrase up and down, nose up a little.
    var p2 = (q + vec2<f32>(0.9, -0.5)) * 0.8;
    p2.y += 0.12 * sin(TAU * u.flow / 16.0);
    p2 = rot(0.06 + 0.04 * sin(TAU * u.flow / 32.0)) * p2;
    return mk(vec3<f32>(p2, (r5 - 0.5) * 0.12), col, lit);
}

fn star5(t: f32) -> vec2<f32> {
    let s = floor(t * 10.0);
    let a0 = s / 10.0 * TAU + PI * 0.5;
    let a1 = a0 + TAU / 10.0;
    let odd = (u32(s) % 2u) == 1u;
    return mix(ring(a0, select(1.0, 0.42, odd)), ring(a1, select(0.42, 1.0, odd)), fract(t * 10.0));
}

fn box_pt(c: vec3<f32>, h: vec3<f32>, r3: vec3<f32>) -> vec3<f32> {
    var d = r3 * 2.0 - 1.0;
    d /= max(max(abs(d.x), abs(d.y)), max(abs(d.z), 1e-4));
    return c + d * h;
}

// Turn a figure about its upright axis.
fn yaw(p: vec3<f32>, a: f32) -> vec3<f32> {
    let w = rot(a) * p.xz;
    return vec3<f32>(w.x, p.y, w.y);
}

fn slot_form(s: u32, i: u32) -> Form {
    let r = vec4<f32>(sim_rand(i, 12u), sim_rand(i, 13u), sim_rand(i, 14u), sim_rand(i, 15u));
    let ks = kick_swell();
    let cl = u.clock4.x;
    var p = vec3<f32>(0.0);
    var col = WHITE;
    switch s {
        case 8u: {
            let f = sleigh(i, r);
            return mk(CENTRE + f.p * 1.35, f.col, f.lit);
        }
        case 0u: {
            // A tree: a cone of drones hung with coloured bulbs, a gold star
            // on top, turning; the kick shakes the bulbs bright.
            if r.y < 0.9 {
                let h = 1.0 - sqrt(r.x);
                let q = ring(r.z * TAU, 1.0 * (1.0 - h));
                p = vec3<f32>(q.x, -1.3 + 2.6 * h, q.y);
                col = GREEN;
                if r.w < 0.18 {
                    let li = u32(r.w * 40.0) % 4u;
                    col = select(select(RED, GOLD, li == 1u), select(vec3<f32>(0.3, 0.5, 1.0), vec3<f32>(1.0, 0.3, 0.8), li == 3u), li >= 2u);
                }
            } else {
                let q = star5(r.x) * 0.3 * sqrt(r.z);
                p = vec3<f32>(q.x, 1.55 + q.y, 0.0);
                col = GOLD;
            }
            p = yaw(p, cl * 0.12) * (1.0 + 0.1 * ks);
        }
        case 1u: {
            // A star, filled, swinging on the phrase, pumped by the kick.
            let q = star5(r.x) * 2.0 * select(sqrt(r.y), 1.0, r.z < 0.4);
            p = yaw(vec3<f32>(q.x, q.y, (r.w - 0.5) * 0.3), 0.6 * sin(TAU * u.flow / 16.0)) * (1.0 + 0.25 * ks);
            col = mix(GOLD, WHITE, select(0.0, 0.6, r.z < 0.4));
        }
        case 3u: {
            // A snowman: three balls, a red hat and scarf, a carrot nose,
            // gold buttons; he bobs on the beat.
            if r.y < 0.74 {
                var c = vec3<f32>(0.0, -0.95, 0.0);
                var rad = 0.7;
                if r.z > 0.45 {
                    c = vec3<f32>(0.0, 0.05, 0.0);
                    rad = 0.52;
                }
                if r.z > 0.8 {
                    c = vec3<f32>(0.0, 0.85, 0.0);
                    rad = 0.36;
                }
                p = c + sph(r.x, r.w) * rad;
            } else if r.y < 0.85 {
                let top = r.z < 0.7;
                let q = ring(r.x * TAU, select(0.42, 0.26 * sqrt(r.w), top) * select(sqrt(r.w), 1.0, top));
                p = vec3<f32>(q.x, select(1.18, 1.22 + 0.4 * r.w, top), q.y);
                col = RED;
            } else if r.y < 0.93 {
                let a = r.x * TAU;
                let th = r.z * TAU;
                let rr = 0.42 + 0.07 * cos(th);
                p = vec3<f32>(cos(a) * rr, 0.52 + 0.07 * sin(th), sin(a) * rr);
                col = GREEN;
            } else if r.y < 0.965 {
                p = mix(vec3<f32>(0.0, 0.85, 0.34), vec3<f32>(0.0, 0.82, 0.68), r.x) + (vec3<f32>(r.z, r.w, 0.5) - 0.5) * 0.06 * (1.0 - r.x);
                col = vec3<f32>(1.0, 0.45, 0.05);
            } else {
                let k = u32(r.x * 5.0) % 5u;
                p = select(vec3<f32>(0.0, 0.3 - 0.25 * (f32(k) - 2.0), 0.53), vec3<f32>(sgn(k == 0u) * 0.13, 0.95, 0.31), k < 2u) + (vec3<f32>(r.z, r.w, 0.5) - 0.5) * 0.06;
                col = select(GOLD, vec3<f32>(0.3, 0.6, 1.0), k < 2u);
            }
            p.y += 0.12 * kick_swell();
            p = yaw(p, 0.4 * sin(TAU * u.flow / 32.0));
        }
        case 4u: {
            // A snowflake: six broad arms with branches, turning, a ring of
            // push running out along the arms on each beat.
            let arm = floor(r.y * 6.0);
            var q: vec2<f32>;
            if r.z < 0.45 {
                q = vec2<f32>(r.x * 2.1, (r.w - 0.5) * 0.14 * (1.0 - 0.6 * r.x));
            } else {
                let b = u32(r.z * 10.0) % 4u;
                let at = select(0.75, 1.35, b >= 2u);
                let l = select(0.7, 0.5, b >= 2u);
                q = vec2<f32>(at, 0.0) + vec2<f32>(0.5, 0.866 * sgn((b % 2u) == 0u)) * r.x * l;
            }
            q *= 1.0 + 0.3 * beat_ring(length(q) / 2.1, 0.3);
            let w = rot(arm / 6.0 * TAU + cl * 0.06) * q;
            p = vec3<f32>(w.x, w.y, (r.w - 0.5) * 0.1);
            col = mix(WHITE, vec3<f32>(0.45, 0.75, 1.0), r.x);
        }
        case 5u: {
            // A bell, swinging on the beat, its clapper inside.
            if r.y < 0.9 {
                let y = r.x;
                let rad = (0.35 + 1.05 * y * y * y + 0.18 * smoothstep(0.85, 1.0, y));
                let q = ring(r.z * TAU, rad);
                p = vec3<f32>(q.x, 1.4 - 2.8 * y, q.y);
                col = GOLD;
            } else {
                p = vec3<f32>(0.0, -1.45, 0.0) + sph(r.x, r.z) * 0.22;
                col = vec3<f32>(1.0, 0.9, 0.6);
            }
            let sw = 0.35 * sin(PI * u.flow) * (1.0 - 0.6 * u.calm);
            let q = rot(sw) * (p.xy - vec2<f32>(0.0, 1.6));
            p = vec3<f32>(q.x, q.y + 1.6, p.z);
        }
        case 7u: {
            // A present: a red box tied with gold ribbon and a bow, turning.
            let hw = vec3<f32>(1.1, 0.85, 1.1);
            let c = vec3<f32>(0.0, -0.35, 0.0);
            if r.y < 0.22 {
                p = box_pt(c, vec3<f32>(0.14, hw.y + 0.02, hw.z + 0.02), r.xzw);
                col = GOLD;
            } else if r.y < 0.44 {
                p = box_pt(c, vec3<f32>(hw.x + 0.02, hw.y + 0.02, 0.14), r.xzw);
                col = GOLD;
            } else if r.y < 0.9 {
                p = box_pt(c, hw, r.xzw);
                col = RED;
            } else {
                let side = sgn(r.z < 0.5);
                let q = ring(r.x * TAU, 0.34);
                p = vec3<f32>(side * (0.36 + q.x), 0.85 + q.y * 0.7, q.y * 0.3 * side);
                col = GOLD;
            }
            p = yaw(p, cl * 0.1 + 0.5) * (1.0 + 0.15 * ks);
        }
        default: {
            // The words: MERRY CHRISTMAS, then the DJ line, in letters that
            // alternate red, green and gold.
            var f = text_form(i, select(0u, 1u, s == 6u), 11.0, 3.2);
            if f.lit > 0.0 {
                let band = u32(floor(f.col.x * 14.0 + 0.5 * sin(u.flow))) % 3u;
                f.col = select(select(RED, GREEN, band == 1u), GOLD, band == 2u);
            }
            return f;
        }
    }
    return mk(CENTRE + p * 1.35, col, 1.0);
}

// Falling snow, three layers deep (the near one bigger and faster).
fn snow(p: vec2<f32>) -> f32 {
    var a = 0.0;
    for (var l = 0u; l < 3u; l++) {
        let fl = f32(l);
        let sc = 9.0 + 7.0 * (2.0 - fl);
        let sp = 0.08 + 0.06 * fl;
        let q = vec2<f32>(p.x + 0.08 * sin(u.time * 0.4 + fl * 2.0 + p.y * 2.0), p.y - u.time * sp) * sc;
        let c = floor(q);
        let f = fract(q) - 0.5;
        let h = h2(c, 30u + l);
        let off = vec2<f32>(h2(c, 40u + l), h2(c, 50u + l)) - 0.5;
        a += smoothstep(0.08 + 0.04 * fl, 0.0, length(f - off * 0.6)) * step(h, 0.3) * (0.35 + 0.3 * fl);
    }
    return a;
}

// The village: houses with snowy roofs and warm windows, and pines, along
// rolling snowy ground. Architecture: none of it moves or changes shape.
fn ground(x: f32) -> f32 {
    return HORIZON + 0.1 - 0.05 * sin(x * 1.1 + 0.5) - 0.03 * sin(x * 2.7);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = mix(vec3<f32>(0.004, 0.006, 0.02), vec3<f32>(0.02, 0.028, 0.06), smoothstep(-1.0, HORIZON, p.y));
    col += vec3<f32>(0.75, 0.8, 1.0) * stars(p) * 0.45 * smoothstep(0.2, -0.5, p.y);
    // The moon.
    let md = length(p - vec2<f32>(1.15, -0.68));
    col += vec3<f32>(0.9, 0.92, 1.0) * (smoothstep(0.075, 0.07, md) * 0.5 + 0.03 * exp(-md * 6.0));
    let gy = ground(p.x);
    let cell = floor(p.x * 2.2);
    let cx = (cell + 0.5) / 2.2;
    let hh = h2(vec2<f32>(cell, 1.0), 61u);
    let base = ground(cx) + 0.01;
    let lx = p.x - cx;
    if hh < 0.65 {
        // A house: walls, a pitched roof capped with snow, two windows.
        let hw = 0.12 + 0.04 * hh;
        let wall = 0.1 + 0.06 * hh;
        let roof = 0.085;
        let ry = base - wall - roof * (1.0 - abs(lx) / (hw + 0.02));
        if abs(lx) < hw && p.y > base - wall && p.y < base {
            col = vec3<f32>(0.006, 0.006, 0.01);
            let wx = abs(abs(lx) - hw * 0.5);
            let wy = p.y - (base - wall * 0.6);
            if wx < 0.024 && abs(wy) < 0.022 && h2(vec2<f32>(cell, 2.0), 62u) < 0.85 {
                col = vec3<f32>(1.0, 0.65, 0.25) * 0.35;
            }
        } else if abs(lx) < hw + 0.02 && p.y > ry && p.y < base - wall {
            col = select(vec3<f32>(0.008, 0.008, 0.012), vec3<f32>(0.1, 0.11, 0.14), p.y < ry + 0.014);
        }
    } else {
        // A pine: three stacked triangles, snow on their edges.
        let th = 0.3 + 0.12 * hh;
        let y0 = base - p.y;
        if y0 > 0.0 && y0 < th {
            let tier = fract(y0 / th * 3.0);
            let half = (1.0 - tier) * 0.1 * (1.0 - 0.25 * floor(y0 / th * 3.0));
            if abs(lx) < half {
                col = select(vec3<f32>(0.004, 0.012, 0.008), vec3<f32>(0.08, 0.09, 0.11), abs(lx) > half - 0.008);
            }
        }
    }
    if p.y > gy {
        // Snowy ground, faintly blue, glittering.
        col = mix(vec3<f32>(0.05, 0.06, 0.085), vec3<f32>(0.025, 0.03, 0.05), smoothstep(gy, 1.0, p.y));
        col += vec3<f32>(0.6, 0.7, 1.0) * step(0.997, h2(floor(p * 300.0), 70u)) * 0.3;
    }
    col += vec3<f32>(0.75, 0.8, 0.95) * snow(p) * 0.12;
    return vec4<f32>(finite(col), 1.0);
}
