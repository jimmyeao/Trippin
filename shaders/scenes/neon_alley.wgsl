// @heavy — 2026 tier. @bloom 0.8 @tonemap agx
// A photoreal take on city_rain: a narrow back alley at night after rain.
// Raymarched brick walls with recessed windows, AC units and drainpipes,
// blade-mounted neon signs that light the walls and glow in the damp air,
// and a wet asphalt floor whose puddles reflect it all through a second
// short march. Rain streaks and puddle ripples on top.
// The camera walks the alley on the tempo clock; each sign's brightness is
// a spectrum band, and one sign stutters on the kick.

const HALF_W: f32 = 1.6;
const SIGN_P: f32 = 4.2;     // sign spacing along the alley

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// Sign in cell k: side (±1), height, colour index.
fn sign_info(k: f32) -> vec3<f32> {
    let h = hash21(vec2<f32>(k, 4.2));
    return vec3<f32>(select(-1.0, 1.0, h > 0.5), 2.3 + hash21(vec2<f32>(k, 1.7)) * 1.4, hash21(vec2<f32>(k, 8.8)));
}

fn sign_level(k: f32) -> f32 {
    let inf = sign_info(k);
    let base = (0.7 + 1.0 * spec(inf.z * 0.8 + 0.05)) * (0.6 + 0.7 * u.intensity);
    // Each sign has a rhythm role:
    //  0 steady (breathes with its band), 1 punches on the kick,
    //  2 chase — lit on its own beat of the bar, so light runs down the
    //    alley, 3 hi-hat strobe, 4 faulty flicker.
    let role = i32(hash21(vec2<f32>(k, 3.3)) * 5.0);
    let flash = beat_pulse(6.0);
    var l = base;
    if role == 1 {
        l = base * (0.35 + 1.6 * u.kick);
    } else if role == 2 {
        let my_beat = f32(((i32(k) % 4) + 4) % 4);
        let on = select(0.0, 1.0, abs(floor(u.beat) % 4.0 - my_beat) < 0.5);
        l = base * (0.3 + 2.0 * on * flash);
    } else if role == 3 {
        l = base * (0.4 + 1.4 * u.high * step(0.5, fract(u.beat * 4.0)));
    } else if role == 4 {
        l = base * (1.0 - 0.8 * step(0.4, u.kick) * step(0.5, fract(u.time * 23.0)));
    }
    // Drops: the whole alley flares.
    return l * (1.0 + 1.2 * u.flash);
}

// Neon tube shape of sign k in its local frame (blade sign perpendicular to
// the wall): a rounded frame with a glyph bar and a ring.
fn neon_d(p: vec3<f32>) -> vec2<f32> {
    let k = round(p.z / SIGN_P);
    let inf = sign_info(k);
    let c = vec3<f32>(inf.x * (HALF_W - 0.45), inf.y, k * SIGN_P);
    let q = p - c;
    // Blade sign: tubes lie in the z-y plane (along the alley), thin in x.
    let fr = abs(max(abs(q.z) - 0.34, abs(q.y) - 0.64));
    let frame = length(vec2<f32>(fr, q.x)) - 0.02;
    let ring = length(vec2<f32>(length(q.zy - vec2<f32>(0.0, 0.26)) - 0.17, q.x)) - 0.02;
    let bar = length(vec3<f32>(q.x, max(abs(q.y + 0.26) - 0.24, 0.0), q.z)) - 0.02;
    return vec2<f32>(min(frame, min(ring, bar)), k);
}

// Scene SDF (positive in air) → (distance, material)
// materials: 0 floor, 1 wall, 2 metal fixtures, 3 sign backing panel.
fn map(p: vec3<f32>) -> vec2<f32> {
    // Walls with recessed windows.
    var d = HALF_W - abs(p.x);
    let wz = p.z - floor(p.z / 3.2) * 3.2 - 1.6;
    let wy = p.y - clamp(round((p.y - 1.5) / 2.3), 0.0, 3.0) * 2.3 - 1.5;
    let win = sd_box(vec3<f32>(abs(p.x) - HALF_W, wy, wz), vec3<f32>(0.18, 0.62, 0.48));
    d = max(d, -win);
    var res = vec2<f32>(d, 1.0);
    // Floor.
    if p.y < res.x {
        res = vec2<f32>(p.y, 0.0);
    }
    // AC units every few metres on alternating walls.
    let az = p.z - floor(p.z / 4.7) * 4.7 - 2.35;
    let aside = select(-1.0, 1.0, fract(floor(p.z / 4.7) * 0.5) > 0.25);
    let ac = sd_box(vec3<f32>(p.x - aside * (HALF_W - 0.3), p.y - 2.9, az), vec3<f32>(0.3, 0.28, 0.42)) - 0.02;
    // Drainpipes.
    let pz = p.z - floor(p.z / 7.3) * 7.3 - 3.65;
    let pipe = length(vec2<f32>(abs(p.x) - (HALF_W - 0.12), pz)) - 0.07;
    let fix = min(ac, pipe);
    if fix < res.x {
        res = vec2<f32>(fix, 2.0);
    }
    // Sign backing panel (dark), inside the tube frame.
    let k = round(p.z / SIGN_P);
    let inf = sign_info(k);
    let panel = sd_box(p - vec3<f32>(inf.x * (HALF_W - 0.45), inf.y, k * SIGN_P), vec3<f32>(0.012, 0.6, 0.32));
    let arm = sd_box(p - vec3<f32>(inf.x * (HALF_W - 0.22), inf.y + 0.55, k * SIGN_P), vec3<f32>(0.22, 0.02, 0.02));
    if min(panel, arm) < res.x {
        res = vec2<f32>(min(panel, arm), 3.0);
    }
    return res;
}

fn normal(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.002, 0.0);
    return normalize(vec3<f32>(
        map(p + e.xyy).x - map(p - e.xyy).x,
        map(p + e.yxy).x - map(p - e.yxy).x,
        map(p + e.yyx).x - map(p - e.yyx).x
    ));
}

struct Hit {
    t: f32,
    mat: f32,
    glow: vec3<f32>,
};

fn march(ro: vec3<f32>, rd: vec3<f32>, steps: i32, tmax: f32) -> Hit {
    var h: Hit;
    h.t = -1.0;
    h.glow = vec3<f32>(0.0);
    var t = 0.02;
    for (var i = 0; i < steps; i++) {
        let p = ro + rd * t;
        let m = map(p);
        let nd = neon_d(p);
        // Neon: accumulate glow as the ray passes near the tubes (acts as
        // both the tube's emission and its halo in the damp air).
        let inf = sign_info(nd.y);
        let nc = palette(inf.z * 0.9);
        h.glow += nc * sign_level(nd.y) * (0.0025 / (0.0004 + nd.x * nd.x)) * min(min(m.x, nd.x), 0.3) * 0.12;
        if nd.x < 0.004 {
            h.glow += nc * sign_level(nd.y) * 3.0;
            h.t = t;
            h.mat = 9.0;
            return h;
        }
        let dd = min(m.x, nd.x);
        if m.x < 0.0015 * t {
            h.t = t;
            h.mat = m.y;
            return h;
        }
        t += dd * 0.9;
        if t > tmax {
            break;
        }
    }
    return h;
}

// Light from the nearest signs (no shadows) plus a cool sky fill from above.
fn light(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var l = vec3<f32>(0.06, 0.065, 0.1) * (0.4 + 0.6 * max(n.y, 0.0)) + vec3<f32>(0.025, 0.022, 0.035);
    let k0 = round(p.z / SIGN_P);
    for (var j = -1; j <= 1; j++) {
        let k = k0 + f32(j);
        let inf = sign_info(k);
        let c = vec3<f32>(inf.x * (HALF_W - 0.45), inf.y, k * SIGN_P);
        let v = c - p;
        let d2 = dot(v, v);
        let ndl = max(dot(n, v / sqrt(d2)), 0.0);
        l += palette(inf.z * 0.9) * sign_level(k) * (0.25 + 0.75 * ndl) * 9.0 / (1.0 + d2 * 1.4);
    }
    return l;
}

fn surface(p: vec3<f32>, rd: vec3<f32>, mat: f32) -> vec3<f32> {
    let n = normal(p);
    var albedo = vec3<f32>(0.05);
    if mat < 0.5 {
        albedo = vec3<f32>(0.03, 0.03, 0.032) * (0.6 + 0.8 * tnoise(vec3<f32>(p.xz * 0.9, 0.1)).b);
    } else if mat < 1.5 {
        // Brick: running bond courses with dark mortar.
        let by = p.y * 13.0;
        let row = floor(by);
        let bz = p.z * 4.4 + select(0.0, 0.5, fract(row * 0.5) > 0.25);
        let mortar = smoothstep(0.08, 0.14, fract(by)) * smoothstep(0.05, 0.1, fract(bz)) * smoothstep(0.05, 0.1, 1.0 - fract(bz));
        let tone = 0.7 + 0.6 * hash21(vec2<f32>(floor(bz), row));
        albedo = mix(vec3<f32>(0.02), vec3<f32>(0.09, 0.045, 0.03) * tone, mortar);
        albedo *= 0.6 + 0.8 * tnoise(p * 0.4).b;
    } else if mat < 2.5 {
        albedo = vec3<f32>(0.08, 0.085, 0.09);
    } else {
        albedo = vec3<f32>(0.01);
    }
    return albedo * light(p, n);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.flow * 0.45;
    let ro = vec3<f32>(sin(u.flow * 0.05) * 0.25, 1.62 + 0.03 * sin(u.flow * PI), z);
    let ta = vec3<f32>(sin(u.flow * 0.03) * 0.3, 1.9, z + 6.0);
    let rd = cam_ray(p, ro, ta, 0.0, 1.35);

    let h = march(ro, rd, 96, 45.0);
    var col = vec3<f32>(0.004, 0.005, 0.01) + vec3<f32>(0.02, 0.015, 0.03) * max(rd.y, 0.0);
    var dist = 45.0;
    if h.t > 0.0 && h.mat < 8.0 {
        let hp = ro + rd * h.t;
        dist = h.t;
        col = surface(hp, rd, h.mat);
        if h.mat < 0.5 {
            // Wet asphalt: puddles mirror the alley via a short second march;
            // elsewhere a rough, dim sheen. Rain rings ripple the puddles.
            let pud = smoothstep(0.48, 0.56, tnoise(vec3<f32>(hp.xz * 0.12, 0.7)).g);
            let cell = floor(hp.xz * 3.0);
            let rp = fract(u.time * 0.9 + hash21(cell));
            let rc = length(fract(hp.xz * 3.0) - 0.5 - (hash22(cell) - 0.5) * 0.4);
            let ring = sin((rc - rp * 0.45) * 60.0) * exp(-rp * 4.0) * smoothstep(0.45, 0.2, rc);
            let n = normalize(vec3<f32>(ring * 0.04, 1.0, ring * 0.04 * 0.7));
            let rr = reflect(rd, n);
            let rh = march(hp + n * 0.01, rr, 48, 25.0);
            var refl = vec3<f32>(0.006, 0.007, 0.012) + rh.glow;
            if rh.t > 0.0 && rh.mat < 8.0 {
                refl += surface(hp + n * 0.01 + rr * rh.t, rr, rh.mat) * exp(-rh.t * 0.05);
            }
            let fr = fresnel(0.02, dot(-rd, n));
            col = mix(col, refl, mix(fr * 0.5 + 0.15, fr * 0.6 + 0.7, pud));
        }
    }
    // Damp air: depth haze tinted by the neon, plus the tubes' glow.
    let haze = 1.0 - exp(-dist * 0.06);
    // The haze picks up the nearest sign's colour, so the air pulses too.
    let kn = round((ro.z + 4.0) / SIGN_P);
    let haze_c = vec3<f32>(0.04, 0.03, 0.06) + palette(sign_info(kn).z * 0.9) * sign_level(kn) * 0.05;
    col = mix(col, haze_c * (0.6 + 0.8 * u.intensity), haze * 0.8);
    col += h.glow;

    // Rain streaks in two parallax layers.
    for (var i = 0; i < 2; i++) {
        let fi = f32(i) + 1.0;
        let cells = 110.0 * fi;
        let cx = floor((p.x + p.y * 0.05) * cells);
        let ry = fract(p.y * 1.6 * fi - u.time * 1.4 / fi + hash21(vec2<f32>(cx, fi * 4.0)));
        let dash = step(0.93, hash21(vec2<f32>(cx, fi))) * smoothstep(0.06, 0.0, abs(ry - 0.5)) * 0.05 / fi;
        col += vec3<f32>(0.5, 0.55, 0.7) * dash * (0.3 + 0.5 * u.intensity);
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
