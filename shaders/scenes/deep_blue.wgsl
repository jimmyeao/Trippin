// @heavy — 2026 tier. @bloom 0.7 @tonemap agx
// Gliding over a sandy seabed in open water: caustics dance across rippled
// sand, god rays slant down through the blue, marine snow drifts past, and
// bioluminescent jellyfish pulse in the mid-water. Water absorbs red first,
// so distance fades everything to teal, then deep blue.
// Heightfield march (40 steps, 1 noise fetch each) for the seabed; 14-step
// shaft march with one fetch each for the god rays; jellyfish are
// camera-facing billboards, so they're nearly free.
// The glide follows the tempo clock; jellies contract on the beat (a pose of
// the beat phase) and glow with the bass; caustics brighten with the mids.

const SIGMA: vec3<f32> = vec3<f32>(0.2, 0.06, 0.04);   // absorption per unit
const WATER: vec3<f32> = vec3<f32>(0.004, 0.035, 0.07);   // in-scatter colour

fn sea_h(x: vec2<f32>) -> f32 {
    let n = tnoise(vec3<f32>(x * 0.025, 0.37));
    // Broad dunes + sand ripples that run across the current.
    let rip = sin(x.x * 3.1 + n.b * 9.0 + x.y * 0.6) * 0.03;
    return (n.b - 0.5) * 3.5 + (n.r - 0.22) * 1.6 + rip;
}

// Caustic brightness at a seabed point: two drifting noise sheets; light
// piles up where they cross (ridge of |a-b|).
fn caustic(x: vec2<f32>) -> f32 {
    let a = tnoise(vec3<f32>(x * 0.3, u.time * 0.04)).g;
    let b = tnoise(vec3<f32>(x * 0.3 + 0.37, -u.time * 0.035 + 0.5)).g;
    let c = 1.0 - abs(a - b) * 7.0;
    return pow(clamp(c, 0.0, 1.0), 8.0) * 1.4;
}

fn jelly_pos(i: i32) -> vec3<f32> {
    let fi = f32(i);
    let z = floor(u.clock4.x * 0.35 / 14.0 + fi * 0.37) * 14.0 + fi * 4.7 + 10.0;
    let h = hash21(vec2<f32>(floor(z / 14.0), fi));
    return vec3<f32>((h - 0.5) * 9.0 + sin(u.time * 0.2 + fi) * 0.4, 3.8 + fi * 0.9 + sin(u.time * 0.3 + fi * 2.0) * 0.3, z);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Energy: glide speed follows the mix.
    let z = u.clock4.x * 0.35;
    let ro = vec3<f32>(sin(u.flow * 0.015) * 2.0, 4.2 + sin(u.flow * 0.02) * 0.5, z);
    // Direction: the gaze swings across the seabed and back.
    let ta = ro + vec3<f32>(sin(u.clock4.x * 0.025) * 0.9, -0.28, 1.0);
    let rd = cam_ray(p, ro, ta, sin(u.flow * 0.009) * 0.04, 1.5);
    let sun = normalize(vec3<f32>(0.25, 1.0, 0.35));

    // --- Seabed heightfield march ---------------------------------------------
    var t = 0.1;
    var hit = false;
    let tmax = 60.0;
    for (var i = 0; i < 40; i++) {
        let q = ro + rd * t;
        let dh = q.y - sea_h(q.xz);
        if dh < 0.01 * t {
            hit = true;
            break;
        }
        t += max(dh * 0.6, 0.05 + t * 0.01);
        if t > tmax {
            break;
        }
    }
    var col: vec3<f32>;
    let drive = 0.6 + 0.6 * u.intensity;
    if hit {
        let hp = ro + rd * t;
        let e = 0.08;
        let n = normalize(vec3<f32>(
            sea_h(hp.xz - vec2<f32>(e, 0.0)) - sea_h(hp.xz + vec2<f32>(e, 0.0)),
            2.0 * e,
            sea_h(hp.xz - vec2<f32>(0.0, e)) - sea_h(hp.xz + vec2<f32>(0.0, e))
        ));
        let sand = vec3<f32>(0.75, 0.68, 0.52) * (0.8 + 0.4 * tnoise(vec3<f32>(hp.xz * 0.6, 0.1)).b);
        let depth_light = exp(-SIGMA * (9.0 - hp.y));        // sunlight travels down
        let diff = max(dot(n, sun), 0.0);
        let caus = caustic(hp.xz) * (1.2 + 1.6 * u.mid);
        col = sand * depth_light * (diff * (0.5 + caus) + 0.08) * 2.2 * drive;
    } else {
        t = tmax;
        col = vec3<f32>(0.0);
    }
    // Absorption along the view ray + in-scatter toward the water colour,
    // brighter looking up toward the surface.
    let tr = exp(-SIGMA * t);
    let up = clamp(rd.y * 0.8 + 0.5, 0.0, 1.0);
    let scat = WATER * (0.4 + 1.6 * up * up) * drive;
    col = col * tr + scat * (1.0 - tr);

    // Snell's window: the bright rippling surface when looking up.
    if rd.y > 0.0 {
        let ts = (12.0 - ro.y) / rd.y;
        let sp = ro + rd * ts;
        let rip = caustic(sp.xz * 0.5);
        let win = smoothstep(0.55, 0.9, rd.y);
        col += vec3<f32>(0.4, 0.75, 0.9) * (0.15 + rip * 0.6) * win * exp(-SIGMA * ts) * 3.0 * drive;
    }

    // --- God rays: march the first stretch, sampling the surface light
    // pattern projected down along the sun direction.
    var shafts = 0.0;
    let sl = min(t, 22.0);
    let jit = bluen(in.pos.xy);
    for (var i = 0; i < 14; i++) {
        let s = (f32(i) + jit) / 14.0 * sl;
        let q = ro + rd * s;
        let proj = q.xz - sun.xz / sun.y * (12.0 - q.y);
        let l = tnoise(vec3<f32>(proj * 0.05, u.time * 0.02)).g;
        shafts += smoothstep(0.5, 0.75, l) * exp(-SIGMA.b * s) * exp(-(12.0 - q.y) * 0.05);
    }
    col += vec3<f32>(0.3, 0.6, 0.7) * shafts / 14.0 * sl * 0.09 * (0.5 + 0.8 * u.intensity);

    // --- Jellyfish (billboards) ---------------------------------------------------
    for (var i = 0; i < 3; i++) {
        let jp = jelly_pos(i);
        let v = jp - ro;
        let jt = dot(v, rd);
        if jt < 0.5 || jt > t {
            continue;
        }
        // Local billboard coords: x across, y up (world up projected).
        let off = ro + rd * jt - jp;
        let rgt = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), rd));
        let upv = cross(rd, rgt);
        var q = vec2<f32>(dot(off, rgt), dot(off, upv));
        let pulse = 1.0 - 0.18 * beat_pulse(3.0);   // contracts on the beat
        q.x /= pulse;
        let R = 0.55;
        // Bell: an upper half-ellipse with a scalloped rim.
        let bell_d = length(q / vec2<f32>(R, R * 0.75)) - 1.0;
        let in_bell = smoothstep(0.05, -0.05, bell_d) * smoothstep(-0.25, 0.05, q.y);
        let rim = exp(-abs(bell_d) * 14.0) * smoothstep(-0.3, 0.2, q.y);
        let gonad = exp(-length(q - vec2<f32>(0.0, 0.12)) * 9.0);
        // Tentacles: wavy lines trailing down.
        var tent = 0.0;
        for (var k = 0; k < 5; k++) {
            let fx = (f32(k) - 2.0) * 0.18 * pulse;
            // Shape: tentacles stream wider as the mids build.
            let wav = sin(q.y * 6.0 + u.clock4.z + f32(k) * 1.7) * (0.03 + 0.09 * u.pres4.y) * (-q.y);
            let dx = abs(q.x - fx - wav);
            tent += exp(-dx * 90.0) * smoothstep(0.0, -0.1, q.y) * smoothstep(-2.2, -0.3, q.y);
        }
        let jc = palette(0.55 + f32(i) * 0.15);
        let glow = (0.4 + 1.8 * u.bass) * drive;
        let fade = exp(-SIGMA.g * jt);
        col += jc * (in_bell * 0.12 + rim * 0.9 + gonad * 0.6 + tent * 0.35) * glow * fade;
        col += jc * exp(-length(q) * 2.2) * 0.04 * glow * fade;           // halo
    }

    // --- Marine snow: three parallax layers of slow drifting specks ---------------
    for (var l = 0; l < 3; l++) {
        let fl = f32(l) + 1.0;
        let g = (p + vec2<f32>(u.flow * 0.01 * fl, -u.time * 0.012 / fl)) * (18.0 * fl);
        let cell = floor(g);
        let h = hash22(cell + fl * 31.0);
        let d = length(fract(g) - 0.5 - (h - 0.5) * 0.7);
        col += vec3<f32>(0.5, 0.7, 0.75) * step(0.8, h.x) * smoothstep(0.08, 0.0, d) * 0.06 / fl * drive;
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
