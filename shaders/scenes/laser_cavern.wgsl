// @heavy — 2026 tier. @bloom 0.9 @tonemap agx
// An underground rave in a flooded cavern: a laser projector on a rock
// ledge fans razor-thin beams through the haze, tracing dots across the
// cave walls, and the still black pool doubles everything in reflection.
// Beams are analytic Gaussian tubes (thin), clipped where they leave the
// cavern's bounding sphere; the pool's mirror re-evaluates the beams along
// the reflected ray, so reflections cost no extra march.
// Formations change every bar (fan, sheet, cone, crossing scan) with a
// smooth morph; beam brightness per band; the pool ripples on each kick.

const N: i32 = 14;
const CAVE_R: f32 = 16.0;

fn emitter() -> vec3<f32> {
    return vec3<f32>(0.0, 2.6, 13.0);
}

// Beam direction for formation f (0..3) and beam i, as a pose of the bar.
fn formation(f: i32, i: i32) -> vec3<f32> {
    let k = (f32(i) / f32(N - 1)) * 2.0 - 1.0;         // −1..1 across the fan
    let ph = u.bar_phase * TAU;
    var d: vec3<f32>;
    if f == 0 {
        // Horizontal fan, sweeping up and down.
        d = vec3<f32>(k * 0.9, 0.05 + 0.2 * sin(ph), -1.0);
    } else if f == 1 {
        // Vertical sheet, panning left-right.
        d = vec3<f32>(0.5 * sin(ph), k * 0.6 + 0.15, -1.0);
    } else if f == 2 {
        // Rotating cone.
        let a = f32(i) / f32(N) * TAU + ph;
        d = vec3<f32>(cos(a) * 0.35, sin(a) * 0.25 + 0.15, -1.0);
    } else {
        // Crossing scan: two halves sweeping opposite ways.
        let s = select(-1.0, 1.0, (i & 1) == 0);
        d = vec3<f32>(s * sin(ph + k) * 0.8, 0.1 + k * 0.1, -1.0);
    }
    return normalize(d);
}

fn beam_dir(i: i32) -> vec3<f32> {
    let bar = floor(u.beat / 4.0);
    let f0 = i32(hash21(vec2<f32>(bar, 1.0)) * 4.0);
    let f1 = i32(hash21(vec2<f32>(bar - 1.0, 1.0)) * 4.0);
    let m = smoothstep(0.0, 0.25, u.bar_phase);         // morph in over a beat
    return normalize(mix(formation(f1, i), formation(f0, i), m));
}

fn beam_col(i: i32) -> vec3<f32> {
    let bar = floor(u.beat / 4.0);
    let scheme = hash21(vec2<f32>(bar, 5.0));
    if scheme < 0.4 {
        return vec3<f32>(0.1, 1.0, 0.2);                 // classic green
    }
    return palette(f32(i) / f32(N) * 0.6 + scheme);
}

// Where beam i leaves the cavern (distance along the beam).
fn beam_len(bd: vec3<f32>) -> f32 {
    let o = emitter();
    let b = dot(o, bd);
    let c = dot(o, o) - CAVE_R * CAVE_R;
    return -b + sqrt(max(b * b - c, 0.0));
}

// In-scattering from all beams along a ray segment [0, tmax].
fn lasers(ro: vec3<f32>, rd: vec3<f32>, tmax: f32) -> vec3<f32> {
    var c = vec3<f32>(0.0);
    let o = emitter();
    for (var i = 0; i < N; i++) {
        let bd = beam_dir(i);
        let w0 = ro - o;
        let b = dot(rd, bd);
        let dd = dot(rd, w0);
        let e = dot(bd, w0);
        let den = max(1.0 - b * b, 1e-4);
        let tc = (b * e - dd) / den;
        let len = beam_len(bd);
        let sc = clamp((e - b * dd) / den, 0.0, len);
        let dist = length(ro + rd * tc - (o + bd * sc));
        let rr = 0.012 + sc * 0.0015;
        let vis = step(0.0, tc) * step(tc, tmax);
        let lat = exp(-dist * dist / (rr * rr)) / (1.7725 * rr * max(sqrt(den), 0.1));
        let haze = 0.4 + 1.2 * tnoise((o + bd * sc) * 0.1 + vec3<f32>(u.time * 0.01, 0.0, 0.0)).r;
        let lvl = (0.35 + 1.3 * spec(f32(i) / f32(N) * 0.8)) * (0.4 + 0.8 * u.intensity);
        c += beam_col(i) * lat * haze * lvl * vis * 0.012;
    }
    return c;
}

// Cavern: a lumpy chamber, air-positive.
fn map(p: vec3<f32>) -> f32 {
    let n = tnoise(p * 0.05);
    let n2 = tnoise(p * 0.18 + 0.3);
    var d = CAVE_R - length(p * vec3<f32>(1.0, 1.35, 1.0)) + (n.b - 0.5) * 7.0 + (n2.r - 0.22) * 2.0;
    // Stalactites hanging from the roof.
    let sc = floor(p.xz / 2.2);
    let h = hash22(sc);
    let sp = (sc + 0.5 + (h - 0.5) * 0.6) * 2.2;
    let stal = length(p.xz - sp) - (0.25 + 0.3 * h.x) * clamp((p.y - 4.0) / 6.0, 0.0, 1.0);
    if h.y > 0.55 {
        d = min(d, max(stal, 4.5 + h.x * 2.0 - p.y));
    }
    return d;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let ro = vec3<f32>(sin(u.flow * 0.012) * 3.0, 1.8 + 0.2 * sin(u.flow * 0.017), -9.0 + sin(u.flow * 0.008) * 1.5);
    let ta = vec3<f32>(sin(u.flow * 0.009) * 1.5, 2.6, 8.0);
    let rd = cam_ray(p, ro, ta, 0.0, 1.35);
    let drive = 0.5 + 0.8 * u.intensity;

    // Pool surface at y = 0.
    let t_pool = select(1e5, -ro.y / rd.y, rd.y < 0.0);
    var t = 0.05;
    var hit = false;
    for (var i = 0; i < 64; i++) {
        let d = map(ro + rd * t);
        if d < 0.003 * t {
            hit = true;
            break;
        }
        t += d * 0.8;
        if t > 40.0 || t > t_pool {
            break;
        }
    }
    var col = vec3<f32>(0.0);
    // Bounce light from the lasers on the rock (average beam colour).
    let amb = mix(beam_col(0), beam_col(N - 1), 0.5) * 0.12 * drive + vec3<f32>(0.012, 0.012, 0.016);
    if t_pool < t || (!hit && t_pool < 1e4) {
        // Black pool: mirror of the cave + lasers, rippled on the kick.
        let hp = ro + rd * t_pool;
        let r = length(hp.xz - emitter().xz * vec2<f32>(1.0, 0.6));
        let ring = sin(r * 3.0 - u.beat_phase * 12.0) * beat_pulse(3.0) * u.kick * 0.03;
        let wn = tnoise(vec3<f32>(hp.xz * 0.4, u.time * 0.05)).b - 0.5;
        let n = normalize(vec3<f32>(wn * 0.02 + ring * hp.x / max(r, 0.1), 1.0, wn * 0.02 + ring * hp.z / max(r, 0.1)));
        let rr = reflect(rd, n);
        let fr = fresnel(0.03, dot(-rd, n));
        // Reflected rock: short march from the pool.
        var tr = 0.05;
        var rhit = false;
        for (var i = 0; i < 32; i++) {
            let d = map(hp + rr * tr);
            if d < 0.01 * tr {
                rhit = true;
                break;
            }
            tr += d * 0.9;
            if tr > 30.0 {
                break;
            }
        }
        var refl = lasers(hp, rr, select(40.0, tr, rhit));
        if rhit {
            // Reflected rock, lit by the projector spill (no normal — cheap).
            let rp = hp + rr * tr;
            let lv = emitter() - rp;
            let rock = 0.08 * (0.5 + 0.9 * tnoise(rp * 0.4).b);
            refl += mix(beam_col(0), beam_col(N - 1), 0.5) * rock * 20.0 / (dot(lv, lv) + 12.0) * drive + amb * 0.1;
        }
        col = refl * (0.15 + fr * 0.85) + lasers(ro, rd, t_pool);
    } else {
        if hit {
            let hp = ro + rd * t;
            let e = 0.02;
            let k = vec2<f32>(1.0, -1.0);
            let n = normalize(k.xyy * map(hp + k.xyy * e) + k.yyx * map(hp + k.yyx * e) + k.yxy * map(hp + k.yxy * e) + k.xxx * map(hp + k.xxx * e));
            let rock = vec3<f32>(0.08, 0.075, 0.07) * (0.5 + 0.9 * tnoise(hp * 0.4).b);
            // Lit by the projector's spill (a point light with falloff) and
            // the lasers' bounce; wet rock catches a highlight.
            let lv = emitter() - hp;
            let ld = length(lv);
            let spill = mix(beam_col(0), beam_col(N - 1), 0.5) * max(dot(n, lv / ld), 0.0) * 40.0 / (ld * ld + 12.0) * drive;
            let hv = normalize(lv / ld - rd);
            let wet = pow(max(dot(n, hv), 0.0), 40.0) * 0.4;
            col = rock * (amb * 1.5 + spill) + spill * wet;
            // Laser dots where beams strike the walls.
            let o = emitter();
            for (var i = 0; i < N; i++) {
                let bd = beam_dir(i);
                let end = o + bd * beam_len(bd);
                let dd = length(hp - end);
                col += beam_col(i) * (exp(-dd * dd * 25.0) * 3.0 + exp(-dd * 1.5) * 0.08) * drive;
            }
        }
        col += lasers(ro, rd, select(40.0, t, hit));
    }
    // The projector's aperture.
    let ov = emitter() - ro;
    let ot = dot(ov, rd);
    col += vec3<f32>(1.0) * smoothstep(0.08, 0.0, length(ov - rd * ot)) * step(0.0, ot) * drive;
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
