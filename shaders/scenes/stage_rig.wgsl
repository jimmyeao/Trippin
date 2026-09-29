// @heavy — 2026 tier. @bloom 0.85 @tonemap agx
// Festival main stage from inside the crowd: a giant LED wall, truss towers
// and a roof rig firing beams up into the night haze, flame jets along the
// stage lip on the big moments, and the crowd in front — heads and raised
// hands in backlit silhouette, rim-lit by the stage.
// Beams use the same closed-form Gaussian-tube scattering as warehouse_haze.
// Everything moves as poses of the tempo clock; audio drives light only.

const N_BEAMS: i32 = 12;
const STAGE_Z: f32 = 28.0;
const ROOF_Y: f32 = 17.0;

fn beam_pos(i: i32) -> vec3<f32> {
    return vec3<f32>((f32(i) - 5.5) * 2.4, ROOF_Y - 0.4, STAGE_Z - 1.0);
}

fn beam_dir(i: i32) -> vec3<f32> {
    let fi = f32(i);
    let side = fi - 5.5;
    let ph = u.flow * 0.125;
    let form = 0.5 + 0.5 * sin(u.flow * 0.03125 * PI + 1.0);
    // Up-and-out fan over the crowd, sweeping; or a mirrored crossing.
    let fan = side * 0.09 + sin(ph * TAU * 0.5) * 0.35;
    let cross = sin(ph * TAU + fi * 0.2) * 0.45 * sign(side);
    let pan = mix(fan, cross, form);
    let tilt = -0.35 + 0.3 * sin(ph * TAU * 0.5 + fi * 0.5);   // negative = up
    return normalize(vec3<f32>(sin(pan), -sin(tilt), -cos(pan) * cos(tilt)));
}

fn beam_col(i: i32) -> vec3<f32> {
    // Alternate two palette colours across the rig, like a real show.
    return palette(select(0.15, 0.6, (i & 1) == 1) + f32(i) * 0.01);
}

fn beam_power(i: i32) -> f32 {
    let s = spec(f32(i % 6) / 6.0 * 0.8 + 0.05);
    return (0.3 + 1.2 * s) * (0.3 + 0.9 * u.intensity);
}

fn box_t(ro: vec3<f32>, rd: vec3<f32>, c: vec3<f32>, h: vec3<f32>) -> f32 {
    let inv = 1.0 / rd;
    let t0 = (c - h - ro) * inv;
    let t1 = (c + h - ro) * inv;
    let tn = max(max(min(t0.x, t1.x), min(t0.y, t1.y)), min(t0.z, t1.z));
    let tf = min(min(max(t0.x, t1.x), max(t0.y, t1.y)), max(t0.z, t1.z));
    return select(-1.0, tn, tn < tf && tn > 0.0);
}

// What the LED wall is playing: a slow palette tunnel with spectrum rings.
fn wall_video(q: vec2<f32>) -> vec3<f32> {
    let c = q - vec2<f32>(0.0, 8.5);
    let r = length(c) / 8.0;
    let a = angle(c);
    let z = 1.0 / max(r, 0.05) + u.flow * 0.5;
    let rings = smoothstep(0.35, 0.0, abs(fract(z * 0.5) - 0.5)) * (0.4 + 1.2 * spec(fract(z * 0.1)));
    let spokes = smoothstep(0.8, 1.0, sin(a * 8.0 + z * 0.4));
    var col = palette(z * 0.05 + a / TAU) * (rings + spokes * 0.3) * smoothstep(0.0, 0.3, r);
    col += palette(0.5) * exp(-r * 6.0) * 1.5;
    return col * (0.5 + 0.9 * u.intensity);
}

fn sky(rd: vec3<f32>) -> vec3<f32> {
    let h = max(rd.y, 0.0);
    return mix(vec3<f32>(0.012, 0.01, 0.022), vec3<f32>(0.002, 0.002, 0.006), sqrt(h));
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

fn seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// One person in local units (1 = shoulder width-ish; y DOWN, 0 = shoulder
// line). Returns (distance, phone-screen emission).
fn person(q: vec2<f32>, h1: f32, h2: f32, h3: f32) -> vec2<f32> {
    let hw = 0.9 + 0.25 * h3;                        // build
    // Head: slightly tall ellipse; some have a bun, cap or long hair.
    let hq = q - vec2<f32>((h1 - 0.5) * 0.08, -0.78);
    var head = length(hq * vec2<f32>(1.0, 0.86)) - 0.26;
    let style = fract(h3 * 7.3);
    if style < 0.25 {
        head = smin(head, length(hq - vec2<f32>(0.05, -0.27)) - 0.11, 0.06);       // bun
    } else if style < 0.45 {
        head = smin(head, max(abs(hq.x) - 0.3, abs(hq.y + 0.12) - 0.05), 0.04);   // cap brim
    } else if style < 0.7 {
        head = smin(head, max(abs(hq.x) - 0.24, abs(hq.y - 0.28) - 0.3) - 0.04, 0.1); // long hair
    }
    let neck = seg(q, vec2<f32>(0.0, -0.55), vec2<f32>(0.0, -0.3)) - 0.1;
    // Sloped shoulders and a torso that widens slightly downward.
    let sh = seg(q, vec2<f32>(-0.36 * hw, -0.12), vec2<f32>(0.36 * hw, -0.12)) - 0.17;
    let torso = max(abs(q.x) - 0.42 * hw - max(q.y, 0.0) * 0.08, -(q.y + 0.1)) - 0.04;
    var d = smin(smin(head, neck, 0.07), smin(sh, torso, 0.1), 0.1);
    var phone = 0.0;
    // Arms: most people have at least one hand up on the drop; the rest keep
    // them low. Upper arm + forearm with an elbow, swaying on the beat.
    let ups = h2 * (0.4 + 0.8 * u.intensity);
    for (var a = 0; a < 2; a++) {
        let side = select(-1.0, 1.0, a == 1);
        let raised = step(0.45 + f32(a) * 0.25, ups);
        if raised < 0.5 {
            continue;
        }
        let sw = sin(u.beat * PI + h1 * 6.0 + f32(a) * 1.3) * 0.22 * (0.3 + u.intensity);
        let s0 = vec2<f32>(side * 0.4 * hw, -0.15);
        let el = s0 + vec2<f32>(side * (0.3 + 0.1 * h3) + sw * 0.3, -0.62);
        let hand = el + vec2<f32>(side * (-0.1 + 0.15 * h1) + sw, -0.62 - h3 * 0.2);
        let arm = min(seg(q, s0, el) - 0.1, seg(q, el, hand) - 0.075);
        d = smin(d, arm, 0.05);
        d = min(d, length(q - hand - vec2<f32>(0.0, -0.06)) - 0.1);
        // Phone held up: a small glowing screen.
        if a == 1 && fract(h1 * 13.1) > 0.72 {
            let pq = q - hand - vec2<f32>(0.0, -0.2);
            let scr = max(abs(pq.x) - 0.08, abs(pq.y) - 0.13);
            d = min(d, scr);
            phone = smoothstep(0.01, -0.02, scr + 0.015);
        }
    }
    return vec2<f32>(d, phone);
}

// The crowd, composited over `col` in screen space: five parallax rows,
// back (small, hazy, stage-tinted) to front (big, black, slightly soft),
// everyone bouncing on the beat with their own timing, top edges rim-lit by
// the stage, some phones up.
fn crowd(p: vec2<f32>, sway: f32, col_in: vec3<f32>, rim_c: vec3<f32>) -> vec3<f32> {
    var col = col_in;
    for (var row = 0; row < 5; row++) {
        let fr = f32(row) / 4.0;
        let scale = mix(0.06, 0.34, fr * fr);          // nearer rows are bigger
        let base = mix(0.36, 1.02, fr);                // shoulder line (y-down)
        let x = (p.x + sway * (0.2 + fr * 0.8) + f32(row) * 0.37) / (scale * 1.05);
        var d = 1e3;
        var ph = 0.0;
        var d_up = 1e3;
        let e = 2.5 / (u.res_y * scale);               // ~2.5 px in local units
        for (var k = -1; k <= 1; k++) {
            let cell = floor(x) + f32(k);
            let h1 = hash21(vec2<f32>(cell, f32(row) * 13.0));
            let h2 = hash21(vec2<f32>(cell, f32(row) * 29.0 + 3.0));
            let h3 = hash21(vec2<f32>(cell, f32(row) * 7.0 + 11.0));
            let bounce = -abs(sin((u.beat + h1 * 0.5) * PI)) * 0.12 * (0.2 + u.intensity);
            let lx = x - cell - 0.5 - (h1 - 0.5) * 0.45;
            let ly = (p.y - base) / scale + (h3 - 0.5) * 0.3 - bounce;
            let r = person(vec2<f32>(lx, ly), h1, h2, h3);
            d = min(d, r.x);
            ph = max(ph, r.y);
            d_up = min(d_up, person(vec2<f32>(lx, ly - e * 3.0), h1, h2, h3).x);
        }
        // Front row slightly out of focus.
        let soft = e * mix(0.6, 2.5, fr * fr);
        let cov = smoothstep(soft, -soft, d);
        // Rim: inside now, outside a few pixels up → a top-facing edge.
        // Both conditions must hold outright — in the anti-aliased band both
        // terms sit near 0.5 and would outline every edge.
        let rim = smoothstep(0.0, -soft, d) * smoothstep(0.0, soft, d_up);
        let haze = mix(0.35, 0.0, fr);                  // far rows sit in the haze
        let body = mix(vec3<f32>(0.002, 0.002, 0.003), col_in * 0.35 + rim_c * 0.05, haze);
        col = mix(col, body, cov);
        col += rim_c * rim * mix(0.5, 0.25, fr);
        col += vec3<f32>(0.7, 0.8, 1.0) * ph * cov * 0.8;
    }
    return col;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let sway = sin(u.flow * 0.0625 * PI) * 0.08;
    let ro = vec3<f32>(sway * 4.0, 2.2 + 0.05 * sin(u.beat * PI) * u.intensity, 0.0);
    let rd = cam_ray(p, ro, vec3<f32>(sway * 2.0, 7.5, STAGE_Z), 0.0, 1.6);

    // --- Opaque stage elements -------------------------------------------------
    var col = sky(rd);
    var t_hit = 1e4;
    // LED wall plane at the back of the stage.
    let tw = (STAGE_Z + 2.0 - ro.z) / rd.z;
    let wp = ro + rd * tw;
    if abs(wp.x) < 13.0 && wp.y > 2.0 && wp.y < 15.0 {
        t_hit = tw;
        let q = wp.xy;
        let cell = fract(q * 6.0);
        let px = smoothstep(0.5, 0.4, max(abs(cell.x - 0.5), abs(cell.y - 0.5)));
        col = wall_video(q) * (0.78 + 0.22 * px);
    }
    // Side screens.
    for (var s = -1; s <= 1; s += 2) {
        let cx = f32(s) * 21.0;
        if abs(wp.x - cx) < 4.0 && wp.y > 5.0 && wp.y < 11.0 && tw < t_hit + 1.0 {
            t_hit = tw;
            col = wall_video(vec2<f32>((wp.x - cx) * 2.5, (wp.y - 8.0) * 2.0 + 8.5)) * 0.8;
        }
    }
    // Stage deck.
    let td = box_t(ro, rd, vec3<f32>(0.0, 1.0, STAGE_Z), vec3<f32>(18.0, 1.0, 4.0));
    if td > 0.0 && td < t_hit {
        t_hit = td;
        col = vec3<f32>(0.01, 0.01, 0.012) + palette(0.4) * 0.03 * u.intensity;
    }
    // Roof truss + two towers (dark, rim-lit by the wall).
    let truss = array<vec4<f32>, 3>(
        vec4<f32>(0.0, ROOF_Y, 15.5, 0.5),
        vec4<f32>(-15.5, ROOF_Y * 0.5, 0.5, ROOF_Y * 0.5),
        vec4<f32>(15.5, ROOF_Y * 0.5, 0.5, ROOF_Y * 0.5),
    );
    for (var k = 0; k < 3; k++) {
        let b = truss[k];
        let tt = box_t(ro, rd, vec3<f32>(b.x, b.y, STAGE_Z - 1.0), vec3<f32>(b.z, b.w, 0.5));
        if tt > 0.0 && tt < t_hit {
            t_hit = tt;
            // Lattice look: dark with a grid of catch-lights.
            let hp = ro + rd * tt;
            let g = abs(fract(hp.xy * 1.2) - 0.5);
            let lat = smoothstep(0.42, 0.5, max(g.x, g.y));
            col = vec3<f32>(0.006) + lat * palette(0.5) * 0.06 * (0.5 + u.intensity);
        }
    }

    // --- Flame jets on the stage lip (big moments) ----------------------------
    let drop = smoothstep(0.65, 0.9, u.intensity);
    if drop > 0.0 {
        for (var j = 0; j < 6; j++) {
            let fx = (f32(j) - 2.5) * 6.0;
            let base = vec3<f32>(fx, 2.0, STAGE_Z - 3.5);
            // Project the jet's axis: distance from ray to the vertical line.
            let w = base - ro;
            let tc = dot(w.xz, rd.xz) / dot(rd.xz, rd.xz);
            let yc = ro.y + rd.y * tc - base.y;
            let dx = length((ro + rd * tc).xz - base.xz);
            let env = exp(-u.beat_phase * 3.0) * drop;        // burst per beat
            let hgt = 7.0 * env;
            if yc > 0.0 && yc < hgt + 1.5 && tc > 0.0 && tc < t_hit {
                let n = tnoise(vec3<f32>(dx * 0.3 + f32(j), yc * 0.12 - u.time * 1.4, u.time * 0.2)).r;
                let width = (0.6 + yc * 0.12) * (0.7 + n);
                let core = smoothstep(width, 0.0, dx) * smoothstep(hgt + 1.5, hgt * 0.4, yc);
                let temp = core * (1.3 - yc / (hgt + 1.5));
                col += mix(vec3<f32>(1.0, 0.25, 0.02), vec3<f32>(1.0, 0.8, 0.4), clamp(temp, 0.0, 1.0)) * core * 3.0 * env;
            }
        }
    }

    // --- Beams in the haze -------------------------------------------------------
    let drift = vec3<f32>(u.time * 0.015, 0.0, u.time * -0.01);
    for (var i = 0; i < N_BEAMS; i++) {
        let bp = beam_pos(i);
        let bd = beam_dir(i);
        let w0 = ro - bp;
        let b = dot(rd, bd);
        let dd = dot(rd, w0);
        let e = dot(bd, w0);
        let den = max(1.0 - b * b, 1e-4);
        let tc = (b * e - dd) / den;
        let sc = max((e - b * dd) / den, 0.0);
        let dist = length(ro + rd * tc - (bp + bd * sc));
        let rr = 0.05 + sc * 0.02;
        let vis = smoothstep(0.0, 1.0, tc) * smoothstep(0.0, 1.0, t_hit - tc);
        let haze = 0.3 + 1.3 * tnoise((bp + bd * sc) * 0.05 + drift).r;
        let lat = exp(-dist * dist / (rr * rr)) / (1.7725 * rr * max(sqrt(den), 0.12));
        let along = exp(-sc * 0.02) * smoothstep(0.0, 0.5, sc);
        col += beam_col(i) * beam_power(i) * lat * haze * along * vis * 0.028;
        // Lens hot-spot.
        let lv = bp - ro;
        let lt = dot(lv, rd);
        let ld = length(lv - rd * lt);
        col += beam_col(i) * beam_power(i) * smoothstep(0.14, 0.0, ld) * 1.2 * step(0.0, lt);
    }
    // Haze glow from the LED wall into the air above the crowd.
    let glow_p = p - vec2<f32>(0.0, -0.05);
    col += wall_video(vec2<f32>(0.0, 8.5)) * 0.02 * exp(-dot(glow_p, glow_p) * 1.5);

    // --- Crowd -------------------------------------------------------------------
    let rim_c = mix(palette(0.5), vec3<f32>(1.0, 0.6, 0.3), drop * 0.6) * (0.3 + 0.7 * u.intensity);
    col = crowd(p, sway, col, rim_c);

    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
