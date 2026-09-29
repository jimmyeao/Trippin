// @heavy — 2026 tier. @bloom 0.7 @tonemap agx
// Salt flats at night: a mirror-still film of water over a salt crust,
// ringed by layered mountain ranges, reflects the Milky Way and a faint
// aurora. An avenue of LED monoliths recedes into the distance, each face a
// spectrum display, their light caught in a low mist hanging over the flat.
// Mostly analytic (planes, six boxes, procedural sky, closed-form height
// fog), so it's cheap enough for an M2 at full res.
// The camera orbits the main monolith slowly on the tempo clock; the music
// drives the displays, the glow in the mist, a ripple ring on each kick and
// the aurora (mids).

const N_MONO: i32 = 6;
const MIST_H: f32 = 0.45;

// Monolith k: centre (x, z) and half-size.
fn mono_c(k: i32) -> vec3<f32> {
    if k == 0 {
        return vec3<f32>(0.0);
    }
    // An avenue receding behind the main monolith, alternating sides.
    let fk = f32(k);
    let zdir = select(1.0, -1.0, fract(u.seed * 0.37) > 0.5);
    let sgn = select(-1.0, 1.0, (k & 1) == 1);
    return vec3<f32>(sgn * (3.0 + fk * 2.2), 0.0, zdir * (fk * 11.0 + 5.0));
}

fn mono_half(k: i32) -> vec3<f32> {
    let s = 1.0 - f32(k) * 0.06;
    return vec3<f32>(0.85, 2.4, 0.18) * s;
}

// Ray vs all monoliths. Returns (t, face, index); t < 0 = miss.
fn mono_hit(ro: vec3<f32>, rd: vec3<f32>) -> vec3<f32> {
    var best = vec3<f32>(1e9, 0.0, -1.0);
    let inv = 1.0 / rd;
    for (var k = 0; k < N_MONO; k++) {
        let hh = mono_half(k);
        let c = mono_c(k) + vec3<f32>(0.0, hh.y, 0.0);
        let t0 = (c - hh - ro) * inv;
        let t1 = (c + hh - ro) * inv;
        let tmin = min(t0, t1);
        let tmax = max(t0, t1);
        let tn = max(max(tmin.x, tmin.y), tmin.z);
        let tf = min(min(tmax.x, tmax.y), tmax.z);
        if tn < tf && tn > 0.0 && tn < best.x {
            var face = 0.0;
            if tn == tmin.y {
                face = 1.0;
            } else if tn == tmin.z {
                face = 2.0;
            }
            best = vec3<f32>(tn, face, f32(k));
        }
    }
    if best.z < 0.0 {
        return vec3<f32>(-1.0, 0.0, -1.0);
    }
    return best;
}

// Emissive LED face: 16 spectrum columns of square pixels.
fn led_face(q: vec2<f32>, k: f32) -> vec3<f32> {
    // q: -1..1 across the face (x), 0..1 up (y).
    let cols = 16.0;
    let rows = 40.0;
    let cx = floor((q.x * 0.5 + 0.5) * cols);
    let cy = floor(q.y * rows);
    let cell = fract(vec2<f32>((q.x * 0.5 + 0.5) * cols, q.y * rows));
    let dot = smoothstep(0.5, 0.36, max(abs(cell.x - 0.5), abs(cell.y - 0.5)));
    // Outer monoliths mirror the analyser and take a hue offset.
    let xs = select((cx + 0.5) / cols, abs((cx + 0.5) / cols - 0.5) * 2.0, k > 0.5);
    let level = spec(xs * 0.85) * (0.6 + 0.6 * u.intensity);
    let on = step(cy / rows, level * 0.95);
    let peak = smoothstep(0.04, 0.0, abs(cy / rows - level * 0.95));
    let hue = palette(cx / cols * 0.7 + cy / rows * 0.2 + k * 0.13);
    let base = hue * (on * 1.6 + peak * 2.5) + hue * 0.012;
    return base * dot * (0.6 + 0.8 * u.intensity);
}

fn mono_shade(ro: vec3<f32>, rd: vec3<f32>, h: vec3<f32>) -> vec3<f32> {
    let k = i32(h.z + 0.5);
    let hh = mono_half(k);
    let p = ro + rd * h.x - mono_c(k);
    let face = i32(h.y + 0.5);
    if face == 2 {
        let q = vec2<f32>(p.x / hh.x, p.y / (hh.y * 2.0));
        // Bezel.
        let bez = step(0.94, abs(q.x)) + step(q.y, 0.03) + step(0.97, q.y);
        return mix(led_face(q, h.z), vec3<f32>(0.01), clamp(bez, 0.0, 1.0));
    }
    // Brushed dark metal sides/top catch a little of the display glow.
    let g = palette(0.3) * 0.004 * (0.5 + u.intensity);
    return vec3<f32>(0.002) + g;
}

// Two ranges of mountains around the horizon; returns (ridge elevation of
// the near range, ridge elevation of the far range) for azimuth direction.
fn ridges(d: vec3<f32>) -> vec2<f32> {
    let a = angle(d.xz);
    let c = vec2<f32>(cos(a), sin(a));
    let far = 0.03 + 0.05 * (tnoise(vec3<f32>(c * 0.7, 0.3)).b - 0.37) * 3.0 + 0.012 * (tnoise(vec3<f32>(c * 2.5, 0.6)).a - 0.5);
    let near = 0.012 + 0.035 * max(tnoise(vec3<f32>(c * 1.1 + 0.4, 0.8)).b - 0.47, 0.0) * 5.0 + 0.006 * (tnoise(vec3<f32>(c * 4.0, 0.1)).g - 0.5);
    return vec2<f32>(near, far);
}

// Direction → night sky with stars, Milky Way, aurora and mountains.
fn sky(rd: vec3<f32>) -> vec3<f32> {
    let h = max(rd.y, 0.0);
    // Deep blue zenith, violet airglow and a faint warm light-dome low down.
    var c = mix(vec3<f32>(0.025, 0.02, 0.05), vec3<f32>(0.004, 0.006, 0.018), pow(h, 0.45));
    let horizon_c = vec3<f32>(0.09, 0.05, 0.08);
    c += horizon_c * exp(-h * 14.0);
    // Sky slowly turns.
    let spin = u.time * 0.004;
    let d = vec3<f32>(rd.x * cos(spin) - rd.z * sin(spin), rd.y, rd.x * sin(spin) + rd.z * cos(spin));
    // Milky Way: a tilted great circle with noisy dust lanes.
    // Mirrored in z so it arcs up behind the monolith from either side the
    // camera orbits on.
    let band_n = normalize(vec3<f32>(-0.69, 0.69, -0.17));
    let bd = dot(vec3<f32>(d.x, d.y, abs(d.z)), band_n);
    let core = exp(-bd * bd * 30.0);
    let dust = tnoise(d * 1.6 + 0.3);
    let fine = tnoise(d * 5.0 + 0.7);
    let lanes = smoothstep(0.35, 0.75, dust.g) * smoothstep(0.55, 0.3, abs(bd) * 3.0 + dust.a * 0.4);
    let glow = core * (0.2 + 0.8 * dust.r * fine.b * 1.6) * (1.0 - 0.85 * lanes);
    let hub = exp(-length(vec3<f32>(d.x, d.y, abs(d.z)) - normalize(vec3<f32>(-0.2, 0.15, 0.95))) * 3.0);
    c += glow * mix(vec3<f32>(0.14, 0.14, 0.24), vec3<f32>(0.5, 0.36, 0.24), hub) * 0.8;
    // Stars: one candidate per cell of a fine direction grid, most cells
    // empty; brightness follows a steep power law like the real sky, and the
    // band is denser.
    let g = d * 220.0;
    let cell = floor(g);
    let rnd = hash22(cell.xy + cell.z * 17.3);
    let rn2 = hash21(cell.yz + cell.x * 7.1);
    let keep = step(0.9 - core * 0.12, rnd.x);
    let sp = cell + vec3<f32>(rnd.y, rn2, fract(rn2 * 13.1)) * 0.6 + 0.2;
    let sd = length(g - sp);
    let bright = pow(rn2, 8.0) * 3.0 + 0.05;
    let tw = 0.75 + 0.25 * sin(u.time * (2.0 + rnd.y * 5.0) + rn2 * 40.0);
    let star_col = mix(vec3<f32>(0.7, 0.8, 1.0), vec3<f32>(1.0, 0.82, 0.62), rnd.y);
    c += star_col * keep * bright * tw * smoothstep(0.22, 0.0, sd) * smoothstep(0.0, 0.08, rd.y);

    // Aurora: slow curtains low in the sky — a bright lower hem fading up,
    // with fine vertical rays. Brightness follows the mids.
    let a = angle(rd.xz);
    let ca = vec2<f32>(cos(a), sin(a));
    let fold = tnoise(vec3<f32>(ca * 0.9, u.time * 0.012)).b;
    let hem = 0.09 + (fold - 0.5) * 0.35;
    let above = rd.y - hem;
    let curtain = smoothstep(-0.01, 0.01, above) * exp(-max(above, 0.0) * 7.0);
    let rays = 0.4 + 0.9 * tnoise(vec3<f32>(ca * 9.0, u.time * 0.03)).a;
    let where_ = smoothstep(0.46, 0.58, tnoise(vec3<f32>(ca * 0.6 + 0.5, u.time * 0.006)).b);
    let aur_col = mix(vec3<f32>(0.1, 0.9, 0.45), vec3<f32>(0.6, 0.2, 0.8), smoothstep(0.0, 0.2, above));
    c += aur_col * curtain * rays * where_ * 0.12 * (0.35 + 1.4 * u.mid);

    // Mountains: far range hazy blue, near range almost black, each with a
    // faint skyglow rim along the ridge.
    let rg = ridges(rd);
    if rd.y < rg.y {
        let fr_c = mix(horizon_c * 0.6, vec3<f32>(0.012, 0.014, 0.03), 0.4);
        c = fr_c + horizon_c * 0.4 * smoothstep(0.004, 0.0, rg.y - rd.y);
    }
    if rd.y < rg.x {
        c = vec3<f32>(0.004, 0.004, 0.009) + horizon_c * 0.25 * smoothstep(0.003, 0.0, rg.x - rd.y);
    }
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Orbit the main monolith; slow, with a gentle height drift.
    // Swing ±~45° around the front of the display (either face, by seed).
    let side = select(0.0, PI, fract(u.seed * 0.37) > 0.5);
    let ang = sin(u.flow * 0.011 + u.seed) * 0.7 + side;
    let rad = 10.0 + sin(u.flow * 0.021) * 2.0;
    let ro = vec3<f32>(sin(ang) * rad, 0.38 + 0.1 * sin(u.flow * 0.017), -cos(ang) * rad);
    let ta = vec3<f32>(0.0, 1.7, 0.0);
    let rd = cam_ray(p, ro, ta, 0.0, 1.6);

    var col = vec3<f32>(0.0);
    let mh = mono_hit(ro, rd);
    let t_ground = select(-1.0, -ro.y / rd.y, rd.y < 0.0);
    // Display glow colour: average of what the faces are showing.
    let glow_c = palette(0.35) * (0.4 + 0.9 * u.intensity) * (0.5 + 0.8 * spec(0.1));
    var t_end = 300.0;

    if mh.x > 0.0 && (t_ground < 0.0 || mh.x < t_ground) {
        col = mono_shade(ro, rd, mh);
        t_end = mh.x;
    } else if t_ground > 0.0 {
        t_end = t_ground;
        let hp = ro + rd * t_ground;
        let r = length(hp.xz);
        // Water film normal: fine wind ripples + the kick ring from the base.
        let wn = tnoise(vec3<f32>(hp.xz * 0.35, u.time * 0.03)).b - 0.5;
        let wn2 = tnoise(vec3<f32>(hp.xz * 1.3 + 7.0, u.time * 0.05)).b - 0.5;
        let ring_r = u.beat_phase * 7.0;
        let ring = sin((r - ring_r) * 9.0) * exp(-abs(r - ring_r) * 2.0) * (1.0 - u.beat_phase) * u.kick;
        var n = normalize(vec3<f32>(wn * 0.03 + wn2 * 0.012, 1.0, wn2 * 0.03 + wn * 0.012));
        n = normalize(n + vec3<f32>(hp.x / max(r, 0.01), 0.0, hp.z / max(r, 0.01)) * ring * 0.035);
        let rr = reflect(rd, n);
        // Reflection: monoliths, else sky.
        let rro = hp + n * 0.001;
        let rmh = mono_hit(rro, rr);
        var refl = sky(rr);
        if rmh.x > 0.0 {
            refl = mono_shade(rro, rr, rmh);
        }
        let fr = fresnel(0.02, dot(-rd, n));
        // Salt crust: dry polygon plates with raised rims where the water
        // film has evaporated, mirror water elsewhere.
        let cz = hp.xz * 1.6;
        let vc = tnoise(vec3<f32>(cz * 0.08, 0.5)).g;
        let edge = smoothstep(0.78, 0.9, vc);
        let dry = smoothstep(0.3, 0.5, tnoise(vec3<f32>(hp.xz * 0.025, 0.2)).r);
        let lit = glow_c * 0.5 / (1.0 + r * r * 0.3) + vec3<f32>(0.02, 0.02, 0.035);
        let crust = vec3<f32>(0.5, 0.5, 0.52) * (0.35 + 0.9 * edge) * lit * dry;
        let mirror = mix(0.95, 0.25, dry);
        col = crust + refl * mix(fr, 1.0, mirror);
    } else {
        col = sky(rd);
    }

    // Low mist: closed-form height fog, exp(−y/H) integrated along the ray,
    // textured by one noise fetch and lit by the displays.
    let dy = rd.y;
    let e0 = exp(-ro.y / MIST_H);
    let tt = min(t_end, 120.0);
    var od = e0 * tt;
    if abs(dy) > 1e-4 {
        od = MIST_H / dy * (e0 - exp(-(ro.y + dy * tt) / MIST_H));
    }
    let mid_pt = ro + rd * min(tt * 0.5, 20.0);
    let tex = 0.4 + 1.2 * tnoise(mid_pt * 0.06 + vec3<f32>(u.time * 0.01, 0.0, u.time * 0.006)).r;
    let amt = 1.0 - exp(-od * 0.05 * tex);
    let near_main = 1.0 / (1.0 + dot(mid_pt.xz, mid_pt.xz) * 0.04);
    let mist_c = vec3<f32>(0.045, 0.045, 0.075) + glow_c * (0.04 + 0.35 * near_main);
    col = mix(col, mist_c, amt);

    // Thin haze glow around the main display: distance from the ray to its
    // vertical axis, only over its height.
    let w = vec2<f32>(rd.z, -rd.x) / max(length(rd.xz), 1e-4);
    let perp = abs(dot(-ro.xz, w));
    let tc = dot(-ro.xz, rd.xz) / max(dot(rd.xz, rd.xz), 1e-4);
    let yc = ro.y + rd.y * tc;
    let over = smoothstep(-0.3, 0.2, yc) * smoothstep(4.8 + 0.6, 4.8 * 0.7, yc);
    col += glow_c * 0.05 / (0.05 + perp * perp) * 0.05 * over * select(0.0, 1.0, tc > 0.0);
    // Dither against banding in the dark gradients.
    col += (bluen(in.pos.xy) - 0.5) * 0.004;
    return vec4<f32>(col, 1.0);
}
