// @heavy — 2026 tier. @bloom 0.7 @tonemap agx
// Salt flats at night: a mirror-still film of water over a hexagonal salt
// crust reflects the Milky Way, and a lone LED monolith stands in the middle
// of it all, its face a spectrum display. Mostly analytic (planes, one box,
// procedural sky), so it's cheap enough for an M2 at full res.
// The camera orbits the monolith slowly on the tempo clock; the music drives
// the display, the glow on the water, and a ripple ring on each kick.

const MONO_HALF: vec3<f32> = vec3<f32>(0.85, 2.4, 0.18);
const MONO_BASE: f32 = 0.0;

// Ray vs the monolith's box (centred at (0, half.y, 0)). Returns (t, face)
// where face 0/1/2 is the axis of the hit normal; t < 0 = miss.
fn mono_hit(ro: vec3<f32>, rd: vec3<f32>) -> vec2<f32> {
    let c = vec3<f32>(0.0, MONO_HALF.y + MONO_BASE, 0.0);
    let inv = 1.0 / rd;
    let t0 = (c - MONO_HALF - ro) * inv;
    let t1 = (c + MONO_HALF - ro) * inv;
    let tmin = min(t0, t1);
    let tmax = max(t0, t1);
    let tn = max(max(tmin.x, tmin.y), tmin.z);
    let tf = min(min(tmax.x, tmax.y), tmax.z);
    if tn > tf || tf < 0.0 {
        return vec2<f32>(-1.0, 0.0);
    }
    var face = 0.0;
    if tn == tmin.y {
        face = 1.0;
    } else if tn == tmin.z {
        face = 2.0;
    }
    return vec2<f32>(tn, face);
}

// Emissive LED face: 16 spectrum columns of square pixels.
fn led_face(q: vec2<f32>) -> vec3<f32> {
    // q: -1..1 across the face (x), 0..1 up (y).
    let cols = 16.0;
    let rows = 40.0;
    let cx = floor((q.x * 0.5 + 0.5) * cols);
    let cy = floor(q.y * rows);
    let cell = fract(vec2<f32>((q.x * 0.5 + 0.5) * cols, q.y * rows));
    let dot = smoothstep(0.5, 0.36, max(abs(cell.x - 0.5), abs(cell.y - 0.5)));
    let level = spec((cx + 0.5) / cols * 0.85) * (0.6 + 0.6 * u.intensity);
    let on = step(cy / rows, level * 0.95);
    let peak = smoothstep(0.04, 0.0, abs(cy / rows - level * 0.95));
    let hue = palette(cx / cols * 0.7 + cy / rows * 0.2);
    let base = hue * (on * 1.6 + peak * 2.5) + hue * 0.012;
    return base * dot * (0.6 + 0.8 * u.intensity);
}

fn mono_shade(ro: vec3<f32>, rd: vec3<f32>, h: vec2<f32>) -> vec3<f32> {
    let p = ro + rd * h.x;
    let face = i32(h.y + 0.5);
    if face == 2 {
        let q = vec2<f32>(p.x / MONO_HALF.x, (p.y - MONO_BASE) / (MONO_HALF.y * 2.0));
        // Bezel.
        let bez = step(0.94, abs(q.x)) + step(q.y, 0.03) + step(0.97, q.y);
        return mix(led_face(q), vec3<f32>(0.01), clamp(bez, 0.0, 1.0));
    }
    // Brushed dark metal sides/top catch a little of the display glow.
    let g = palette(0.3) * 0.004 * (0.5 + u.intensity);
    return vec3<f32>(0.002) + g;
}

// Direction → night sky with stars and the Milky Way.
fn sky(rd: vec3<f32>) -> vec3<f32> {
    let h = max(rd.y, 0.0);
    // Deep blue zenith, violet airglow and a faint warm light-dome low down.
    var c = mix(vec3<f32>(0.025, 0.02, 0.05), vec3<f32>(0.004, 0.006, 0.018), pow(h, 0.45));
    c += vec3<f32>(0.09, 0.05, 0.08) * exp(-h * 14.0);
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
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Orbit the monolith; slow, with a gentle height drift.
    // Swing ±~45° around the front of the display (either face, by seed).
    let side = select(0.0, PI, fract(u.seed * 0.37) > 0.5);
    let ang = sin(u.flow * 0.011 + u.seed) * 0.8 + side;
    let rad = 10.0 + sin(u.flow * 0.021) * 2.0;
    let ro = vec3<f32>(sin(ang) * rad, 0.42 + 0.12 * sin(u.flow * 0.017), -cos(ang) * rad);
    let ta = vec3<f32>(0.0, 1.9, 0.0);
    let rd = cam_ray(p, ro, ta, 0.0, 1.8);

    var col = vec3<f32>(0.0);
    let mh = mono_hit(ro, rd);
    let t_ground = select(-1.0, -ro.y / rd.y, rd.y < 0.0);
    // Display glow colour: average of what the face is showing.
    let glow_c = palette(0.35) * (0.4 + 0.9 * u.intensity) * (0.5 + 0.8 * spec(0.1));

    if mh.x > 0.0 && (t_ground < 0.0 || mh.x < t_ground) {
        col = mono_shade(ro, rd, mh);
    } else if t_ground > 0.0 {
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
        // Reflection: monolith, else sky.
        let rro = hp + n * 0.001;
        let rmh = mono_hit(rro, rr);
        var refl = sky(rr);
        if rmh.x > 0.0 {
            refl = mono_shade(rro, rr, rmh);
        }
        let fr = fresnel(0.02, dot(-rd, n));
        // Salt crust: pale hex-ish cells showing through where the film is thin.
        let cz = hp.xz * 1.6;
        let vc = tnoise(vec3<f32>(cz * 0.08, 0.5)).g;
        let edge = smoothstep(0.82, 0.95, vc);
        let thin = smoothstep(0.25, 0.75, tnoise(vec3<f32>(hp.xz * 0.02, 0.2)).r);
        let crust = vec3<f32>(0.03, 0.03, 0.035) * (0.4 + edge) * thin;
        // The display lights the ground around its base.
        let spill = glow_c * 0.35 / (1.0 + r * r * 0.35);
        let mirror = mix(0.55, 0.95, 1.0 - thin);
        col = crust + spill * (1.0 - fr) * 0.6 + refl * mix(fr, 1.0, mirror);
        // Soft haze toward the horizon.
        col = mix(col, sky(vec3<f32>(rd.x, 0.001, rd.z)), smoothstep(30.0, 140.0, t_ground) * 0.6);
    } else {
        col = sky(rd);
    }
    // Thin haze glow around the display: distance from the ray to the
    // monolith's vertical axis, only over its height.
    let w = vec2<f32>(rd.z, -rd.x) / max(length(rd.xz), 1e-4);
    let perp = abs(dot(-ro.xz, w));
    let tc = dot(-ro.xz, rd.xz) / max(dot(rd.xz, rd.xz), 1e-4);
    let yc = ro.y + rd.y * tc;
    let over = smoothstep(-0.3, 0.2, yc) * smoothstep(MONO_HALF.y * 2.0 + 0.6, MONO_HALF.y * 1.4, yc);
    col += glow_c * 0.05 / (0.05 + perp * perp) * 0.05 * over * select(0.0, 1.0, tc > 0.0);
    // Dither against banding in the dark gradients.
    col += (bluen(in.pos.xy) - 0.5) * 0.004;
    return vec4<f32>(col, 1.0);
}
