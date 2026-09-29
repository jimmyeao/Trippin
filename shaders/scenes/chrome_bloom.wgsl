// @heavy — raymarched. @bloom 0.6
// A chrome flower that unfolds and folds: three rings of hinged, cupped
// metal petals open from a tight bud into full bloom and close again over
// an 8-bar cycle, inner rings trailing the outer ones. The track's
// intensity pushes it wider open, petals flutter on the kick, and the core
// glows with the bass. All motion is a pose of the smooth tempo clock (the
// old version jerked back every bar because it used bar_phase directly).

const RINGS: i32 = 3;

// How open the flower is, 0 = bud, 1 = full bloom — smooth, never jumps.
fn bloom_amt() -> f32 {
    // Energy: the unfold/fold cycle runs on the energy clock — it lingers
    // in breakdowns and blooms quicker on drops.
    let cyc = 0.5 - 0.5 * cos(u.clock4.x * TAU / 32.0);
    return clamp(cyc * (0.8 + 0.25 * u.intensity), 0.0, 1.0);
}

fn ring_n(k: i32) -> f32 {
    return select(select(11.0, 8.0, k == 1), 5.0, k == 0);
}

// Hinge angle (from vertical) of ring k's petals.
fn open_angle(k: i32) -> f32 {
    let fk = f32(k);                               // 0 = inner
    let b = smoothstep(0.0, 1.0, clamp(bloom_amt() * 1.3 - (2.0 - fk) * 0.15, 0.0, 1.0));
    let closed = 0.02 + fk * 0.07;
    let open = 0.9 + fk * 0.35;
    return mix(closed, open, b) + 0.06 * u.hits4.x * (0.5 + fk * 0.3);
}

struct Hit {
    d: f32,
    part: f32,     // ring index, 3 = core, 4 = stem
};

fn petal(p: vec3<f32>, k: i32) -> f32 {
    let fk = f32(k);
    let n = ring_n(k);
    let sector = TAU / n;
    // Direction: the petal rings turn one way, then back.
    let spin = 1.2 * sin(u.clock4.x * 0.02) + fk * 0.35;
    // Polar domain repetition around the stem axis (y).
    let a = angle(p.xz) + spin;
    let ai = round(a / sector);
    let la = a - ai * sector;
    let r = length(p.xz);
    let rad = r * cos(la);                         // radial coord
    let tan_ = r * sin(la);                        // tangential (petal width)
    let th = open_angle(k);
    let dir = vec2<f32>(sin(th), cos(th));         // petal axis in (radial, y)
    let nrm = vec2<f32>(cos(th), -sin(th));
    let base = vec2<f32>(0.18 + fk * 0.06, 0.05);
    let q2 = vec2<f32>(rad, p.y) - base;
    let along = dot(q2, dir);
    var thick = dot(q2, nrm);
    let len = 0.7 + fk * 0.28;
    let wid = 0.26 + fk * 0.06;
    // Cup the petal (curve its sides up) and taper it to a point.
    let s = clamp(along / len, 0.0, 1.0);
    thick += 0.9 * tan_ * tan_ / (wid + 0.1) - 0.08 * sin(s * PI);
    let w = wid * sin(clamp(s, 0.0, 1.0) * PI * 0.9 + 0.15);
    let e = vec3<f32>((along - len * 0.5) / (len * 0.5), tan_ / max(w, 0.02), thick / 0.025);
    let d = (length(e) - 1.0) * min(0.025, w);
    return d;
}

fn map(p: vec3<f32>) -> Hit {
    var h: Hit;
    // Core (pistil).
    h.d = length(p - vec3<f32>(0.0, 0.12, 0.0)) - (0.13 + 0.07 * u.pres4.x);
    h.part = 3.0;
    // Stem.
    let stem = max(length(p.xz) - 0.05, p.y);
    if stem < h.d {
        h.d = stem;
        h.part = 4.0;
    }
    for (var k = 0; k < RINGS; k++) {
        let d = petal(p, k);
        if d < h.d {
            h.d = d;
            h.part = f32(k);
        }
    }
    return h;
}

fn normal_at(p: vec3<f32>) -> vec3<f32> {
    let e = 0.002;
    let k = vec2<f32>(1.0, -1.0);
    return normalize(k.xyy * map(p + k.xyy * e).d + k.yyx * map(p + k.yyx * e).d + k.yxy * map(p + k.yxy * e).d + k.xxx * map(p + k.xxx * e).d);
}

// Studio environment for the chrome: soft palette gradient, a big warm key
// softbox above and a cool rim strip behind.
fn env(d: vec3<f32>) -> vec3<f32> {
    let band = d.y * 0.5 + 0.5;
    var c = mix(palette(0.0 + u.hue), palette(0.5 + u.hue), band);
    c = c * c * 0.6 + 0.02;
    c += vec3<f32>(1.0, 0.9, 0.75) * smoothstep(0.75, 0.95, d.y) * 2.0;
    let az = angle(d.xz);
    c += palette(0.75 + u.hue) * smoothstep(0.25, 0.05, abs(fract(az / TAU + u.flow * 0.01) - 0.5) - 0.2) * smoothstep(0.5, 0.0, abs(d.y - 0.1)) * (0.8 + 1.2 * u.mid);
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let drift = u.clock4.x * 0.025 + u.seed;
    let elev = 0.75 + 0.2 * sin(u.flow * 0.02);
    let ro = vec3<f32>(sin(drift) * cos(elev), sin(elev), cos(drift) * cos(elev)) * 3.2;
    let rd = cam_ray(p, ro, vec3<f32>(0.0, 0.25, 0.0), 0.0, 1.6);

    var t = 0.5;
    var hit = false;
    var h: Hit;
    for (var i = 0; i < 90; i++) {
        h = map(ro + rd * t);
        if h.d < 0.0008 * t {
            hit = true;
            break;
        }
        t += h.d * 0.8;
        if t > 8.0 {
            break;
        }
    }

    var col = vec3<f32>(0.0);
    if hit {
        let pos = ro + rd * t;
        let n = normal_at(pos);
        let rdir = reflect(rd, n);
        // Cheap AO: petals crowding in the bud stay darker.
        let ao = clamp(map(pos + n * 0.06).d / 0.06, 0.2, 1.0);
        if h.part == 3.0 {
            col = mix(vec3<f32>(1.0, 0.8, 0.4), palette(0.1 + u.hue), 0.4) * (0.6 + 2.2 * u.bass);
        } else if h.part == 4.0 {
            col = env(rdir) * 0.3 * ao;
        } else {
            let tint = mix(vec3<f32>(1.0), palette(h.part * 0.25 + u.hue), 0.35);
            let fres = fresnel(0.6, dot(n, -rd));
            col = env(rdir) * tint * fres * ao * (0.7 + 0.5 * u.intensity);
            col += vec3<f32>(1.0, 0.95, 0.85) * pow(max(dot(rdir, normalize(vec3<f32>(0.4, 0.9, 0.3))), 0.0), 80.0) * 2.0 * ao;
            // Core light reflected in the inner petals.
            col += mix(vec3<f32>(1.0, 0.8, 0.4), palette(0.1 + u.hue), 0.4) * u.bass * 0.6 * exp(-length(pos - vec3<f32>(0.0, 0.12, 0.0)) * 4.0);
        }
    }
    // Aura behind the bloom, swelling as it opens.
    col += palette(0.55 + u.hue) * exp(-length(p) * 2.2) * (0.04 + 0.08 * bloom_amt()) * (0.5 + u.energy);
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
