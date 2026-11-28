// @sim 105000 @bloom 0.6 @tonemap agx
// @title Christmas Snowflakes
// Christmas Snowflakes: five giant six-armed snowflakes of light, 100,000
// points in all, broad filled arms, that grow a new crystal every two
// beats: side branches sprout, slide and change length, plates open at the
// heart, each design spinning in and landing inside half a beat (shape). The mids
// grow the branches longer, each kick sends a wave out along the arms, and
// the flakes turn on the smooth energy clock, neighbours opposite ways, and
// bob on the phrase (motion), with fine snow falling past. Every point is
// simulated: kicks shake the crystals and snares twist them. Seasonal.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

const FIG: u32 = 100000u; // crystal points; the rest are fine snow

fn sgn(b: bool) -> f32 {
    return select(-1.0, 1.0, b);
}

// Flake f's centre and size (f 0 is the big one in the middle).
fn flake_at(f: u32) -> vec3<f32> {
    switch f {
        case 0u: { return vec3<f32>(0.0, 0.0, 0.8); }
        case 1u: { return vec3<f32>(-1.6, 0.55, 0.36); }
        case 2u: { return vec3<f32>(1.6, -0.5, 0.4); }
        case 3u: { return vec3<f32>(-1.75, -0.7, 0.3); }
        default: { return vec3<f32>(1.7, 0.68, 0.32); }
    }
}

fn flake_of(i: u32) -> u32 {
    // 40% in the middle one, 15% in each of the others.
    let s = f32(i) / f32(FIG);
    if s < 0.4 {
        return 0u;
    }
    return min(1u + u32((s - 0.4) / 0.15), 4u);
}

// A design's random in 0..1: design d, flake f, slot k.
fn dr(d: u32, f: u32, k: u32) -> f32 {
    return f32(pcg(d * 7919u + f * 104729u + k * 31u + 17u) & 0xFFFFu) / 65535.0;
}

// The point (in the flake's own unit space, xy) for randoms r, design d.
fn crystal(d: u32, f: u32, r: vec4<f32>) -> vec2<f32> {
    let arm = floor(r.y * 6.0);
    let side = sgn(fract(r.y * 6.0) < 0.5);
    let grow = 1.0 + 0.45 * u.lvl4.y;
    var q: vec2<f32>;
    // Arms and branches are broad, filled bands of light, not lines.
    let wid = (fract(r.w * 13.0) * 2.0 - 1.0);
    if r.z < 0.28 {
        // The arm's spine.
        q = vec2<f32>(r.x, wid * 0.055 * (1.0 - 0.6 * r.x) * grow);
    } else if r.z < 0.88 {
        // One of three side branches at 60 degrees, either side.
        let j = u32((r.z - 0.28) / 0.2) % 3u;
        let at = 0.2 + 0.22 * f32(j) + 0.16 * dr(d, f, j);
        let len = (0.08 + 0.34 * dr(d, f, j + 3u)) * (1.0 - 0.45 * at) * grow;
        q = vec2<f32>(at, 0.0) + vec2<f32>(0.5, 0.866 * side) * r.x * len + vec2<f32>(-0.866 * side, 0.5) * wid * 0.035 * (1.0 - 0.5 * r.x);
    } else {
        // At the heart: a hexagonal plate's rim (its size per design) or a
        // ring of six small hexagons near the tips.
        let rad = 0.08 + 0.26 * dr(d, f, 7u);
        let s = r.x * 6.0;
        let a0 = floor(s) / 6.0 * TAU;
        let c0 = vec2<f32>(cos(a0), sin(a0));
        let c1 = vec2<f32>(cos(a0 + TAU / 6.0), sin(a0 + TAU / 6.0));
        let hx = mix(c0, c1, fract(s));
        // A filled hexagonal plate, or small filled hexagons at the tips.
        let fill = sqrt(fract(r.w * 7.0));
        if dr(d, f, 8u) < 0.5 {
            return hx * rad * fill;
        }
        q = vec2<f32>(0.86, 0.0) + hx * 0.08 * grow * fill;
    }
    return rot(arm / 6.0 * TAU + PI * 0.5) * q;
}

fn rnd4(i: u32) -> vec4<f32> {
    return vec4<f32>(sim_rand(i, 0u), sim_rand(i, 1u), sim_rand(i, 2u), sim_rand(i, 3u));
}

fn home(i: u32) -> vec3<f32> {
    let f = flake_of(i);
    let r = rnd4(i);
    // A new crystal every two beats, landing fast; one a bar in breakdowns.
    let bt = u32(max(floor(u.beat), 0.0));
    var idx = bt / 2u;
    var into = fract(u.beat) + f32(bt % 2u);
    var len = 0.6;
    if u.calm > 0.5 {
        idx = bt / 4u;
        into = fract(u.beat) + f32(bt % 4u);
        len = 1.5;
    }
    // Flakes change one after another, a twentieth of a beat apart.
    let x = clamp((into - f32(f) * 0.05 - r.w * 0.06) / len, 0.0, 1.0);
    let ex = 1.0 - x;
    let e = 1.0 - ex * ex * ex;
    // Designs are indexed mod 1024, so the 4096-beat wrap is seamless.
    let da = (idx + 1023u) % 1024u;
    let db = idx % 1024u;
    var q = mix(crystal(da, f, r), crystal(db, f, r), e);
    // A kick's wave out along the arms; a burst mid-change.
    // A kick's wave out along the arms, the kick pumping the whole flake,
    // and each new crystal spinning in a twelfth of a turn.
    q *= 1.0 + 0.3 * beat_ring(length(q), 0.3) + 0.25 * kick_swell();
    let c = flake_at(f);
    let dir = sgn(f % 2u == 0u);
    q = rot(dir * (u.clock4.x * 0.035 + (1.0 - e) * 0.52) + f32(f)) * q;
    let bob = 0.07 * sin(TAU * u.flow / 32.0 + f32(f) * 1.3);
    return vec3<f32>(c.xy + q * c.z + vec2<f32>(0.0, bob), 0.12 * f32(f % 2u));
}

fn snow_spawn(i: u32, top: bool) -> vec3<f32> {
    let y = select(sim_rand_f(i, 1u) * 3.2 - 1.6, 1.7, top);
    return vec3<f32>(sim_rand_f(i, 0u) * 6.0 - 3.0, y, sim_rand_f(i, 2u) * 2.0 - 0.6);
}

fn sim_spawn(i: u32) -> Particle {
    if i >= FIG {
        return Particle(vec4<f32>(snow_spawn(i, false), 0.0), vec4<f32>(0.0, -0.2, 0.0, 0.0));
    }
    return Particle(vec4<f32>(home(i), 0.0), vec4<f32>(0.0));
}

fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    var x = p.pos.xyz;
    var v = p.vel.xyz;
    let calm = u.calm;
    if i >= FIG {
        let fall = 0.12 + 0.18 * sim_rand(i, 6u);
        let drift = sim_curl(x * 0.25 + vec3<f32>(0.0, u.clock4.x * 0.01, 0.0)) * 0.4;
        let goal = vec3<f32>(drift.x, -fall, 0.0);
        v += (goal - v) * 2.0 * dt;
        v.x += simc.onset.x * 0.4 * (1.0 - 0.85 * calm) * (0.5 + sim_rand(i, 7u));
        x += v * dt;
        if x.y < -1.7 || abs(x.x) > 3.3 {
            x = snow_spawn(i, x.y < -1.7);
            v = vec3<f32>(0.0, -fall, 0.0);
        }
        return Particle(vec4<f32>(x, 0.0), vec4<f32>(v, 0.0));
    }
    let h = home(i);
    let c = flake_at(flake_of(i));
    let k = (130.0 + 60.0 * u.pres4.x) * (1.0 - 0.5 * calm);
    var acc = (h - x) * k - v * (1.6 * sqrt(k));
    let t = u.clock4.z * 0.02;
    acc += sim_curl(h * 0.6 + vec3<f32>(t, -0.4 * t, 0.7 * t)) * (0.04 + 0.3 * u.lvl4.y + 1.2 * calm) * c.z;
    v += acc * dt;
    let rel = h.xy - c.xy;
    let dir = rel / max(length(rel), 1e-3);
    v += vec3<f32>(dir, 0.0) * simc.onset.x * 1.4 * c.z * (1.0 - 0.85 * calm);
    v += vec3<f32>(-rel.y, rel.x, 0.0) * simc.onset.z * 1.6 * (1.0 - 0.6 * calm);
    x += v * dt;
    return Particle(vec4<f32>(x, length(rel) / c.z), vec4<f32>(v, min(length(x - h) * 4.0 / c.z, 1.0)));
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let pp = sim_p[ii];
    let a = vec3<f32>(pp.pos.xy, pp.pos.z + 3.6);
    let b = a - vec3<f32>(pp.vel.xy * 0.035, 0.0);
    if ii >= FIG {
        let sz = 0.007 + 0.016 * sim_rand(ii, 8u) * sim_rand(ii, 9u);
        let s = sim_quad(a, b, vi, sz, 2.4);
        o.pos = s.pos;
        o.q = s.q;
        o.col = vec3<f32>(0.75, 0.82, 1.0) * 0.12 * s.dim * clamp(5.0 - a.z, 0.3, 1.0);
        return o;
    }
    let s = sim_quad(a, b, vi, 0.0038, 2.4);
    o.pos = s.pos;
    o.q = s.q;
    // Icy white at the heart, the palette's cold tint towards the tips.
    let rr = clamp(pp.pos.w, 0.0, 1.2);
    let tint = palette(0.2 + 0.4 * rr);
    let c = mix(vec3<f32>(0.75, 0.88, 1.0), tint * tint, 0.25 + 0.35 * rr);
    let bri = (0.13 + 0.18 * pp.vel.w + 0.06 * u.lvl4.z) * (1.15 - 0.4 * rr);
    o.col = c * bri * s.dim;
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
    col += mix(vec3<f32>(0.002, 0.004, 0.012), vec3<f32>(0.006, 0.011, 0.026), smoothstep(-0.7, 0.9, p.y));
    return vec4<f32>(finite(col), 1.0);
}
