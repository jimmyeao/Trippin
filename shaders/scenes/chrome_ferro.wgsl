// @heavy — 2026 tier. @bloom 0.55 @tonemap agx
// Liquid chrome in a photo studio: a mercury-like blob pooled on a black
// gloss floor, pulled into ferrofluid spikes by the music — rings of spikes
// from crown to rim, each ring driven by its own slice of the spectrum.
// Lit only by what it reflects: a big overhead softbox and two coloured
// strip lights (palette) that swell with the track.
// Camera orbits slowly on the tempo clock. Spike height is pose (spectrum
// → displacement), never accumulated, so nothing stutters.

const C: vec3<f32> = vec3<f32>(0.0, 0.42, 0.0);
const RAD: f32 = 0.82;
const DTH: f32 = 0.23;      // ring spacing (radians of polar angle)

fn sph(th: f32, ph: f32) -> vec3<f32> {
    return vec3<f32>(sin(th) * cos(ph), cos(th), sin(th) * sin(ph));
}

// Height of ring k (0 = crown spike): bass at the crown, highs at the rim.
fn ring_h(k: f32) -> f32 {
    let band = clamp(k / 7.0, 0.0, 1.0) * 0.8;
    let s = spec(band);
    return (0.08 + 0.42 * s * s * (0.6 + 0.6 * u.intensity)) * smoothstep(7.5, 4.0, k);
}

fn spikes(n: vec3<f32>) -> f32 {
    let th = acos(clamp(n.y, -1.0, 1.0));
    let ph = angle(n.xz);
    let k0 = round(th / DTH);
    var bump = 0.0;
    for (var dk = -1; dk <= 1; dk++) {
        let k = k0 + f32(dk);
        if k < 0.0 || k > 7.0 {
            continue;
        }
        let tk = k * DTH;
        let nk = max(1.0, round(TAU * sin(tk) / DTH));
        let stp = TAU / nk;
        let off = select(0.0, stp * 0.5, (i32(k) & 1) == 1);
        let pk = round((ph - off) / stp) * stp + off;
        let dir = sph(tk, pk);
        let ang = acos(clamp(dot(n, dir), -1.0, 1.0));
        let w = DTH * 0.52;
        let t = max(0.0, 1.0 - ang / w);
        bump = max(bump, ring_h(k) * t * t * (1.0 + 0.5 * t));
    }
    return bump;
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

fn map(p: vec3<f32>) -> f32 {
    // A squat dome rather than a ball — fluid slumps under its own weight.
    let q = (p - C) * vec3<f32>(1.0, 1.3, 1.0);
    let r = length(q);
    let n = q / max(r, 1e-4);
    let breathe = RAD * (1.0 + 0.04 * u.bass);
    // Spikes only where they'd stand up (upper ~3/4 of the blob).
    let blob = (r - breathe - spikes(n) * smoothstep(-0.55, -0.1, n.y)) * 0.55;
    // The puddle it's pooled in: a flat ellipsoid on the floor.
    let e = p / vec3<f32>(1.3, 0.09, 1.3);
    let pud = (length(e) - 1.0) * 0.09;
    return smin(blob, pud, 0.5);
}

fn normal(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.0015, 0.0);
    return normalize(vec3<f32>(
        map(p + e.xyy) - map(p - e.xyy),
        map(p + e.yxy) - map(p - e.yxy),
        map(p + e.yyx) - map(p - e.yyx)
    ));
}

fn march(ro: vec3<f32>, rd: vec3<f32>, tmax: f32, steps: i32) -> f32 {
    // Bounding sphere around blob + puddle.
    let oc = ro - vec3<f32>(0.0, 0.3, 0.0);
    let b = dot(oc, rd);
    let c = dot(oc, oc) - 1.8 * 1.8;
    let h = b * b - c;
    if h < 0.0 {
        return -1.0;
    }
    var t = max(-b - sqrt(h), 0.0);
    let t_out = min(-b + sqrt(h), tmax);
    for (var i = 0; i < steps; i++) {
        let d = map(ro + rd * t);
        if d < 0.0006 * t {
            return t;
        }
        t += d;
        if t > t_out {
            break;
        }
    }
    return -1.0;
}

// The studio: black cyclorama, a huge overhead softbox, two tall coloured
// strip lights either side, a faint warm kicker behind.
fn env(rd: vec3<f32>) -> vec3<f32> {
    var c = vec3<f32>(0.02, 0.02, 0.026) * (0.5 + 0.5 * rd.y);
    // Seamless cyclorama: the sweep where wall meets floor catches light,
    // giving the chrome a horizon line to read its curvature by.
    c += vec3<f32>(0.22, 0.22, 0.25) * exp(-abs(rd.y + 0.02) * 7.0);
    c += vec3<f32>(0.06, 0.06, 0.07) * smoothstep(-0.2, 0.6, rd.y);
    // Overhead softbox: a rounded rectangle high above.
    let top = rd.xz / max(rd.y, 0.05);
    let sb = max(abs(top.x) - 0.45, abs(top.y) - 0.3);
    c += vec3<f32>(1.0, 0.97, 0.92) * (2.6 + 1.5 * u.kick * u.intensity) * smoothstep(0.06, -0.02, sb) * step(0.0, rd.y);
    // Strip lights: vertical bars at fixed world azimuths.
    let az = angle(rd.xz);
    let drive = 0.6 + 0.9 * u.intensity;
    for (var i = 0; i < 4; i++) {
        let a0 = f32(i) * 1.5708 + 0.75;
        let da = abs(fract((az - a0) / TAU + 0.5) - 0.5) * TAU;
        let bar = smoothstep(0.12, 0.06, da) * smoothstep(0.8, 0.6, abs(rd.y - 0.2));
        let col = palette(0.15 + f32(i) * 0.22);
        let lvl = drive * (0.8 + 1.4 * spec(0.1 + f32(i) * 0.2));
        c += col * bar * 5.0 * lvl;
    }
    // Kicker behind the camera side.
    let ka = abs(fract((az - 2.2) / TAU + 0.5) - 0.5) * TAU;
    c += vec3<f32>(1.0, 0.6, 0.35) * 0.6 * smoothstep(0.5, 0.1, ka) * smoothstep(0.4, 0.0, abs(rd.y - 0.05));
    return c;
}

fn chrome(p: vec3<f32>, rd: vec3<f32>) -> vec3<f32> {
    let n = normal(p);
    let r = reflect(rd, n);
    // Reflected ray: the floor, else the studio.
    var refl = env(r);
    if r.y < 0.0 {
        let tf = -p.y / r.y;
        let fp = p + r * tf;
        let fall = exp(-length(fp.xz) * 0.25);
        refl = vec3<f32>(0.004) + env(vec3<f32>(r.x, -r.y, r.z)) * 0.04 * fall;
    }
    // Cheap AO from the field.
    var ao = 0.0;
    for (var i = 1; i <= 3; i++) {
        let h = 0.05 * f32(i);
        ao += (h - map(p + n * h)) / h;
    }
    ao = clamp(1.0 - ao * 0.3, 0.2, 1.0);
    let f = fresnel(0.78, dot(-rd, n));
    let tint = vec3<f32>(0.95, 0.96, 1.0);
    return refl * f * tint * ao;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let ang = u.flow * 0.02 + u.seed;
    let ro = vec3<f32>(sin(ang) * 4.3, 1.55 + 0.25 * sin(u.flow * 0.013), cos(ang) * 4.3);
    let rd = cam_ray(p, ro, vec3<f32>(0.0, 0.45, 0.0), 0.0, 2.1);

    var col = vec3<f32>(0.0);
    let t = march(ro, rd, 20.0, 110);
    if t > 0.0 {
        col = chrome(ro + rd * t, rd);
    } else if rd.y < 0.0 {
        // Black gloss floor: blurred-looking reflection of the blob + studio.
        let tf = -ro.y / rd.y;
        let fp = ro + rd * tf;
        let rr = vec3<f32>(rd.x, -rd.y, rd.z);
        var refl = env(rr);
        let rt = march(fp + rr * 0.002, rr, 8.0, 60);
        if rt > 0.0 {
            refl = chrome(fp + rr * (rt + 0.002), rr);
        }
        let fr = fresnel(0.04, -rd.y);
        // Soft contact shadow under the puddle.
        let cs = smoothstep(1.2, 2.4, length(fp.xz));
        let fade = exp(-length(fp.xz) * 0.12);
        col = refl * fr * (0.55 + 0.45 * cs) * fade + vec3<f32>(0.002) * cs;
        // Floor sweeps up into the cyc wall — no horizon seam.
        col = mix(col, env(vec3<f32>(rd.x, 0.0, rd.z)) * 0.25, 1.0 - fade);
    } else {
        col = env(rd) * 0.25;
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
