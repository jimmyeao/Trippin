// Palm beach at sunset: the sun sinking toward a glittering sea, a foam line
// breathing in and out, wet sand mirroring it all, and two leaning palms
// framing the view with a few gulls crossing. The halo breathes on the beat;
// the sea sparkles on the highs.

fn sd_seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// Palm silhouette; base at origin, up is -y. `lean` bends the trunk.
fn palm(p_in: vec2<f32>, lean: f32, sc: f32) -> f32 {
    let q = p_in / sc;
    var d = 1e9;
    // Trunk: five tapering segments curving toward the lean.
    var pt = vec2<f32>(0.0, 0.0);
    for (var i = 1; i <= 5; i++) {
        let f = f32(i) / 5.0;
        let nt = vec2<f32>(lean * f * f * 0.55 + 0.03 * sin(f * 5.0), -f * 0.55);
        d = min(d, sd_seg(q, pt, nt) - mix(0.028, 0.013, f));
        pt = nt;
    }
    let crown = pt;
    // Fronds: a fan of arcs that droop at the tips.
    for (var i = 0; i < 8; i++) {
        let ang = (f32(i) - 3.5) * 0.42;
        var dir = vec2<f32>(sin(ang), -cos(ang) * 0.65);
        var fp = crown;
        for (var s = 0; s < 3; s++) {
            let fs = f32(s);
            dir = normalize(dir + vec2<f32>(sign(dir.x) * 0.15, 0.55 + fs * 0.3));
            let np = fp + dir * (0.17 - fs * 0.03);
            d = min(d, sd_seg(q, fp, np) - 0.007 * (1.0 - fs * 0.3));
            fp = np;
        }
    }
    // Coconuts under the crown.
    d = min(d, length(q - crown - vec2<f32>(0.02, 0.025)) - 0.016);
    d = min(d, length(q - crown - vec2<f32>(-0.018, 0.02)) - 0.013);
    return d * sc;
}

// A gull: two short arcs that flap.
fn gull(p: vec2<f32>, flap: f32) -> f32 {
    let w = rot(flap * 0.5) * vec2<f32>(0.035, -0.018);
    let wl = rot(-flap * 0.5) * vec2<f32>(-0.035, -0.018);
    return min(sd_seg(p, vec2<f32>(0.0), w), sd_seg(p, vec2<f32>(0.0), wl)) - 0.004;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);            // y grows downward
    let t = u.time;
    let asp = aspect();
    let horizon = 0.05;                // sea meets sky
    let sand_line = 0.45;              // sea meets wet sand
    let sun_x = 0.12 * asp;
    let sun_y = -0.16 + 0.03 * sin(t * 0.03);   // sinking very slowly
    let breathe = 1.0 + 0.3 * beat_pulse(3.0) * (0.4 + 0.6 * u.kick);

    // Sky: embers at the horizon into violet, with early stars high up.
    var col = mix(vec3<f32>(0.85, 0.32, 0.12), vec3<f32>(0.07, 0.06, 0.25),
                  smoothstep(horizon, -0.9, p.y));
    col = mix(col, vec3<f32>(0.45, 0.12, 0.3),
              smoothstep(0.0, 0.35, -p.y) * smoothstep(0.9, 0.2, -p.y) * 0.5);
    let star = step(0.996, hash21(floor(in.uv * vec2<f32>(u.res_x, u.res_y) / 3.0)));
    col += star * vec3<f32>(0.8, 0.85, 1.0) * smoothstep(-0.3, -0.9, p.y) * 0.4;

    // Sun and halo.
    let sd = length(p - vec2<f32>(sun_x, sun_y));
    col += vec3<f32>(1.0, 0.55, 0.2) * 0.3 * breathe / (1.0 + pow(max(sd - 0.14, 0.0) * 7.0, 2.0));
    col = mix(col, vec3<f32>(1.0, 0.75, 0.4) * 1.6, smoothstep(0.145, 0.135, sd));

    // Cloud streaks near the horizon catching the light.
    let streak = fbm(vec2<f32>(p.x * 0.8 + t * 0.01, p.y * 9.0));
    col += vec3<f32>(0.9, 0.35, 0.25) * smoothstep(0.55, 0.8, streak)
         * smoothstep(0.25, 0.0, abs(p.y + 0.18)) * (0.3 + 0.3 * u.intensity);

    if p.y > horizon && p.y < sand_line {
        // Sea: dark teal band, sun glitter column, drifting wave lines.
        let depth = smoothstep(horizon, sand_line, p.y);
        var sea = mix(vec3<f32>(0.5, 0.18, 0.1), vec3<f32>(0.02, 0.05, 0.1), depth);
        // Glitter column under the sun.
        let gx = abs(p.x - sun_x + (noise(vec2<f32>(p.y * 90.0, t * 1.5)) - 0.5) * 0.15);
        let glit = exp(-gx * 18.0) * (0.5 + 0.5 * noise(vec2<f32>(p.x * 60.0, p.y * 200.0 - t * 2.0)));
        sea += vec3<f32>(1.0, 0.6, 0.3) * glit * (0.6 + 0.8 * spec(0.85));
        // Wave lines marching in.
        let wline = pow(0.5 + 0.5 * sin(p.y * 140.0 + fbm(vec2<f32>(p.x * 2.0, 0.0)) * 8.0 - t * 0.7), 8.0);
        sea += vec3<f32>(0.5, 0.3, 0.25) * wline * depth * 0.3;
        // Reflected sky near the horizon.
        sea = mix(col * 0.55, sea, smoothstep(0.0, 0.1, depth));
        col = sea;
    }

    if p.y >= sand_line {
        // Wet sand: dark warm gradient with the sun's smeared reflection and
        // the foam line hissing in and out on a slow cycle.
        let depth = (p.y - sand_line) / (1.0 - sand_line);
        var sand = mix(vec3<f32>(0.16, 0.07, 0.05), vec3<f32>(0.05, 0.03, 0.04), depth);
        let sr = abs(p.x - sun_x + (noise(vec2<f32>(p.y * 40.0, t)) - 0.5) * 0.3);
        sand += vec3<f32>(1.0, 0.5, 0.25) * exp(-sr * 10.0) * (1.0 - depth) * 0.5;
        // Foam edge: advances and retreats, leaving lace behind.
        let edge = sand_line + 0.04 * sin(t * 0.24) + 0.03 * fbm(vec2<f32>(p.x * 3.0, t * 0.15));
        let foam = exp(-abs(p.y - edge) * 30.0) * (0.6 + 0.4 * noise(vec2<f32>(p.x * 25.0, t * 0.8)));
        sand += vec3<f32>(0.9, 0.75, 0.65) * foam * (0.5 + 0.5 * u.energy);
        col = sand;
    }

    // Two leaning palms, mirrored in the wet sand below the foam line.
    let pl = palm(p - vec2<f32>(-asp * 0.62, 0.78), 0.55, 0.9);
    let pr = palm(vec2<f32>(-(p.x - asp * 0.68), p.y - 0.82), 0.5, 0.75);
    let palmd = min(pl, pr);
    col = mix(col, vec3<f32>(0.01, 0.005, 0.015), smoothstep(0.004, -0.004, palmd));
    if p.y > sand_line {
        let rp = vec2<f32>(p.x, 2.0 * sand_line - p.y + 0.05);
        let rr = min(palm(rp - vec2<f32>(-asp * 0.62, 0.78), 0.55, 0.9),
                     palm(vec2<f32>(-(rp.x - asp * 0.68), rp.y - 0.82), 0.5, 0.75));
        let blur = (noise(p * 60.0 + t) - 0.5) * 0.01;
        col = mix(col, vec3<f32>(0.04, 0.015, 0.03),
                  smoothstep(0.01, -0.005, rr + blur) * 0.5 * smoothstep(0.0, 0.3, p.y - sand_line));
    }

    // Gulls crossing high up, a couple at a time.
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let bx = ((t * (0.05 + 0.02 * fi) + hash21(vec2<f32>(fi, 3.0))) % 2.6) - 1.3;
        let by = -0.5 - 0.2 * hash21(vec2<f32>(fi, 8.0)) + 0.04 * sin(t * 0.8 + fi * 2.0);
        let flap = sin(t * (6.0 + fi) + fi * 1.7);
        let g = gull((p - vec2<f32>(bx * asp, by)) / (0.7 + 0.5 * fi), flap);
        col = mix(col, vec3<f32>(0.02, 0.01, 0.02), smoothstep(0.003, 0.0, g));
    }

    col += prev(in.uv) * 0.05;
    return vec4<f32>(col, 1.0);
}
