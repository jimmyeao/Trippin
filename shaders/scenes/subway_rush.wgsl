// @heavy — 2026 tier. @bloom 0.8 @tonemap agx
// Cab view from a subway train at speed: tunnel lights streak past, rails
// gleam ahead, cable runs blur along the walls, and every 16 beats the
// tunnel bursts open into a bright tiled station that flashes by on the
// downbeat of the phrase.
// Analytic box geometry per zone (tunnel / station) with a zone-hopping
// trace (≤3 hops), lights drawn as motion-streaked capsules. Cheap.
// Travel is locked to the tempo clock (12 m per beat); each tunnel light is
// tied to a spectrum band, stations light up with the track's intensity.

const SPEED: f32 = 12.0;       // metres per beat
const LIGHT_P: f32 = 4.0;      // light spacing
const ST_P: f32 = 192.0;       // station period (16 beats)
const ST_LEN: f32 = 70.0;

struct Zone {
    hw: f32,       // half-width
    ceil: f32,     // ceiling height
    station: f32,  // 1 inside a station
};

fn zone_at(z: f32) -> Zone {
    var zn: Zone;
    let m = z - floor(z / ST_P) * ST_P;
    if m < ST_LEN {
        zn.hw = 7.0;
        zn.ceil = 5.5;
        zn.station = 1.0;
    } else {
        // Shape: the tunnel walls breathe with bass presence.
        zn.hw = 2.1 + 0.5 * u.pres4.x;
        zn.ceil = 4.2;
        zn.station = 0.0;
    }
    return zn;
}

// Next zone boundary ahead of z (along +z) — or behind for rd.z < 0.
fn zone_edge(z: f32, dir: f32) -> f32 {
    let base = floor(z / ST_P) * ST_P;
    let m = z - base;
    if dir > 0.0 {
        return base + select(ST_P, ST_LEN, m < ST_LEN);
    }
    return base + select(ST_LEN, 0.0, m < ST_LEN);
}

struct Hit {
    t: f32,
    n: vec3<f32>,
    station: f32,
};

fn trace(ro: vec3<f32>, rd: vec3<f32>) -> Hit {
    var h: Hit;
    h.t = 1e4;
    var o = ro;
    var t_acc = 0.0;
    for (var hop = 0; hop < 3; hop++) {
        let zn = zone_at(o.z + rd.z * 0.001);
        var t = 1e4;
        var n = vec3<f32>(0.0);
        let tw = (sign(rd.x) * zn.hw - o.x) / rd.x;
        if tw > 0.0 && tw < t {
            t = tw;
            n = vec3<f32>(-sign(rd.x), 0.0, 0.0);
        }
        let tf = select(1e4, -o.y / rd.y, rd.y < 0.0);
        if tf < t {
            t = tf;
            n = vec3<f32>(0.0, 1.0, 0.0);
        }
        let tc = select(1e4, (zn.ceil - o.y) / rd.y, rd.y > 0.0);
        if tc < t {
            t = tc;
            n = vec3<f32>(0.0, -1.0, 0.0);
        }
        // Station platform on the right (a box top at y = 1.1, x > 2.4).
        if zn.station > 0.5 && rd.y < 0.0 {
            let tp = (1.1 - o.y) / rd.y;
            let pp = o + rd * tp;
            if tp > 0.0 && tp < t && pp.x > 2.4 {
                t = tp;
                n = vec3<f32>(0.0, 1.0, 0.0);
            }
            let te = (2.4 - o.x) / rd.x;
            let pe = o + rd * te;
            if te > 0.0 && te < t && pe.y < 1.1 {
                t = te;
                n = vec3<f32>(-1.0, 0.0, 0.0);
            }
        }
        // Did we leave this zone first?
        let edge = zone_edge(o.z, rd.z);
        let t_edge = (edge - o.z) / rd.z;
        if t_edge > 0.0 && t_edge < t && hop < 2 {
            o = o + rd * (t_edge + 0.01);
            t_acc += t_edge + 0.01;
            continue;
        }
        h.t = t_acc + t;
        h.n = n;
        h.station = zn.station;
        return h;
    }
    return h;
}

fn light_col(k: f32) -> vec3<f32> {
    let band = fract(k * 0.137);
    return mix(vec3<f32>(1.0, 0.85, 0.6), palette(band), 0.6) * (0.4 + 1.4 * spec(band * 0.85));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.clock4.x * SPEED;
    // Cab sway and a little track judder (poses of the beat clock).
    let ro = vec3<f32>(-0.6 + sin(u.clock4.x * 0.2) * 0.05, 1.7 + sin(u.clock4.x * PI * 2.0) * 0.008, z);
    let ta = ro + vec3<f32>(sin(u.clock4.x * 0.13) * 0.06, -0.06, 1.0);
    let rd = cam_ray(p, ro, ta, sin(u.clock4.x * 0.17) * 0.012, 1.25);
    let drive = 0.5 + 0.8 * u.intensity;
    let blur = SPEED * u.bpm / 60.0 * (1.0 / 60.0) * 0.8;   // metres per frame of shutter

    let h = trace(ro, rd);
    let hp = ro + rd * h.t;
    let zn_st = h.station;
    var col = vec3<f32>(0.0);

    // --- Surfaces ----------------------------------------------------------
    var albedo = vec3<f32>(0.06, 0.058, 0.055) * (0.6 + 0.7 * tnoise(vec3<f32>(hp.z * 0.05, hp.y * 0.3, hp.x * 0.3)).b);
    // Tunnel ring joints every 2 m, cable runs on the walls.
    let joint = smoothstep(0.05, 0.0, abs(fract(hp.z * 0.5) - 0.5) - 0.46);
    albedo *= 1.0 - joint * 0.5;
    if abs(h.n.x) > 0.5 && zn_st < 0.5 {
        let cables = smoothstep(0.03, 0.0, abs(fract(hp.y * 5.0) - 0.5) - 0.42) * step(2.2, hp.y) * step(hp.y, 3.0);
        albedo = mix(albedo, vec3<f32>(0.015), cables);
    }
    if zn_st > 0.5 {
        // Station: white tiles, a platform edge stripe.
        let tg = abs(fract(vec2<f32>(hp.z, hp.y) * 3.0) - 0.5);
        let grout = smoothstep(0.44, 0.48, max(tg.x, tg.y));
        albedo = mix(vec3<f32>(0.55, 0.56, 0.58), vec3<f32>(0.2), grout);
        if h.n.y > 0.5 && hp.y > 1.0 {
            albedo = mix(vec3<f32>(0.12), vec3<f32>(0.8, 0.65, 0.1), smoothstep(2.9, 2.8, hp.x));
        }
    }
    // Irradiance: nearest tunnel lights (alternating walls), or the station.
    var irr = vec3<f32>(0.012, 0.012, 0.016);
    if zn_st > 0.5 {
        irr += vec3<f32>(0.9, 0.95, 1.0) * (0.35 + 0.5 * u.intensity) * (0.6 + 0.4 * max(-h.n.y, 0.0) + 0.3 * max(h.n.y, 0.0));
    } else {
        let k0 = round(hp.z / LIGHT_P);
        for (var j = -1; j <= 1; j++) {
            let k = k0 + f32(j);
            let side = select(-1.0, 1.0, fract(k * 0.5) > 0.25);
            let lp = vec3<f32>(side * 2.2, 3.2, k * LIGHT_P);
            let v = lp - hp;
            let d2 = dot(v, v);
            irr += light_col(k) * (0.3 + max(dot(h.n, v / sqrt(d2)), 0.0)) * 5.0 / (d2 + 1.0);
        }
    }
    col = albedo * irr * drive;
    // Rails: two bright steel lines catching the light ahead.
    if h.n.y > 0.5 && hp.y < 0.05 {
        for (var r = 0; r < 2; r++) {
            let rx = -0.6 + select(-0.72, 0.72, r == 1);
            let rail = smoothstep(0.04, 0.0, abs(hp.x - rx));
            col = mix(col, vec3<f32>(0.6, 0.62, 0.65) * (irr * 4.0 + 0.02), rail);
        }
        let sleeper = step(0.7, fract(hp.z * 1.6)) * step(abs(hp.x + 0.6), 1.1);
        col *= 1.0 - sleeper * 0.4;
    }

    // --- Lights: motion-streaked capsules on the walls ------------------------
    let kc = round(ro.z / LIGHT_P);
    for (var j = 0; j < 22; j++) {
        let k = kc + f32(j);
        let side = select(-1.0, 1.0, fract(k * 0.5) > 0.25);
        let lz = k * LIGHT_P;
        if zone_at(lz).station > 0.5 {
            continue;
        }
        let a = vec3<f32>(side * 2.15, 3.2, lz - blur * 0.5);
        let ba = vec3<f32>(0.0, 0.0, blur + 0.35);
        let oa = ro - a;
        let dd = dot(rd, ba);
        let den = max(dot(ba, ba) - dd * dd, 1e-5);
        let s = clamp((dot(oa, ba) - dot(oa, rd) * dd) / den, 0.0, 1.0);
        let pt = a + ba * s;
        let tt = dot(pt - ro, rd);
        if tt > 0.0 && tt < h.t + 0.5 {
            let dist = length(ro + rd * tt - pt);
            col += light_col(k) * (exp(-dist * dist * 900.0) * 3.0 + exp(-dist * 8.0) * 0.08) * drive;
        }
    }
    // Continuous LED guide strips low on both walls, pulsing with the bass —
    // a coloured line racing into the distance.
    if abs(h.n.x) > 0.5 && zn_st < 0.5 {
        let strip = smoothstep(0.05, 0.015, abs(hp.y - 0.9));
        let band = fract(hp.z * 0.004 - u.clock4.x * 0.05);
        col += palette(band + select(0.0, 0.5, hp.x > 0.0)) * strip * (0.4 + 1.6 * u.bass) * drive;
    }
    // Station ceiling strip lights.
    if zn_st > 0.5 && h.n.y < -0.5 {
        let strip = smoothstep(0.12, 0.05, abs(fract(hp.x * 0.25) - 0.5)) * step(0.3, fract(hp.z * 0.12));
        col += vec3<f32>(1.0, 0.97, 0.9) * strip * 2.5 * (0.5 + 0.6 * u.intensity);
    }
    // Tunnel haze: darkness swallows the distance (lighter toward a station).
    let fog = 1.0 - exp(-h.t * 0.025);
    col = mix(col, vec3<f32>(0.018, 0.016, 0.022) * drive + zone_at(z + 60.0).station * vec3<f32>(0.06, 0.06, 0.05), fog);
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
