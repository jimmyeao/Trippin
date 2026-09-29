// Christmas: a tree of lights (bulbs spiralling round a slowly turning cone,
// dimmer at the back), a white garland, a shimmering star on top, three layers
// of falling snow and a sparkling snowy ground. Seasonal (see config.rs).

fn sd_star5(p_in: vec2<f32>, r: f32, rf: f32) -> f32 {
    // Inigo Quilez's 5-pointed star.
    let k1 = vec2<f32>(0.809016994375, -0.587785252292);
    let k2 = vec2<f32>(-k1.x, k1.y);
    var p = p_in;
    p.x = abs(p.x);
    p -= 2.0 * max(dot(k1, p), 0.0) * k1;
    p -= 2.0 * max(dot(k2, p), 0.0) * k2;
    p.x = abs(p.x);
    p.y -= r;
    let ba = rf * vec2<f32>(-k1.y, k1.x) - vec2<f32>(0.0, 1.0);
    let h = clamp(dot(p, ba) / dot(ba, ba), 0.0, r);
    return length(p - ba * h) * sign(p.y * ba.x - p.x * ba.y);
}

fn bulb_colour(i: f32) -> vec3<f32> {
    let k = i32(hash21(vec2<f32>(i, 4.0)) * 5.0);
    if k == 0 { return vec3<f32>(1.0, 0.1, 0.08); }
    if k == 1 { return vec3<f32>(0.1, 0.9, 0.25); }
    if k == 2 { return vec3<f32>(1.0, 0.7, 0.15); }
    if k == 3 { return vec3<f32>(0.2, 0.4, 1.0); }
    return vec3<f32>(1.0, 0.3, 0.8);
}

fn snow_layer(uv: vec2<f32>, scale: f32, speed: f32, size: f32, t: f32) -> f32 {
    var q = uv * vec2<f32>(aspect(), 1.0) * scale;
    q.y -= t * speed * scale;
    q.x += sin(q.y * 0.35 + t * 0.7) * 0.4;
    let cell = floor(q);
    let f = fract(q) - 0.5;
    let off = (hash22(cell) - 0.5) * 0.7;
    let d = length(f - off);
    return smoothstep(size, size * 0.3, d) * step(0.6, hash21(cell + 7.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);            // y grows downward
    let t = u.clock4.z * 0.5;

    // Night sky and stars.
    var col = mix(vec3<f32>(0.03, 0.05, 0.13), vec3<f32>(0.005, 0.01, 0.04), clamp(-p.y * 0.7 + 0.35, 0.0, 1.0));
    col += step(0.997, hash21(floor(in.uv * vec2<f32>(u.res_x, u.res_y) / 3.0))) * 0.5 * step(p.y, 0.4);

    // Snowy ground with sparkles.
    let ground_y = 0.6 + 0.03 * sin(p.x * 2.0) + 0.02 * fbm(vec2<f32>(p.x * 4.0, 3.0));
    let ground = smoothstep(ground_y - 0.004, ground_y + 0.004, p.y);
    let sparkle = step(0.992, hash21(floor(in.uv * vec2<f32>(u.res_x, u.res_y) / 2.0) + floor(t * 4.0)));
    col = mix(col, vec3<f32>(0.22, 0.26, 0.36) + sparkle * 0.8, ground);

    // The tree: a dark cone with tiered edges behind the lights.
    let top = -0.72;
    let bottom = 0.55;
    let rb = 0.5;
    let s_h = clamp((p.y - top) / (bottom - top), 0.0, 1.0);
    // Five tiers that flare out at their base, with a ragged needle edge.
    let tiers = 0.72 + 0.28 * fract(s_h * 5.0);
    let ragged = 0.012 * sin(p.y * 90.0 + p.x * 20.0);
    let cone = abs(p.x) - rb * s_h * tiers - ragged;
    let in_tree = step(cone, 0.0) * step(top, p.y) * step(p.y, bottom);
    col = mix(col, vec3<f32>(0.004, 0.03, 0.012) * (0.7 + 0.8 * fbm(p * 18.0)), in_tree * 0.9);
    col = mix(col, vec3<f32>(0.08, 0.04, 0.02), step(abs(p.x), 0.04) * step(bottom, p.y) * step(p.y, bottom + 0.08));

    // Coloured bulbs on a spiral round the cone; the whole spiral slowly turns.
    let spin = t * 0.25;
    let glow_amt = 0.55 + 0.45 * u.intensity;
    for (var i = 0; i < 110; i++) {
        let fi = f32(i);
        let s = (fi + 0.5) / 110.0;
        let a = s * 7.0 * TAU + spin;
        let r = rb * s * 0.92;
        let bp = vec2<f32>(r * cos(a), top + s * (bottom - top) * 0.97);
        let depth = sin(a);                       // -1 back .. 1 front
        let d2 = dot(p - bp, p - bp);
        let twinkle = 0.55 + 0.45 * sin(t * (2.0 + hash21(vec2<f32>(fi, 1.0)) * 3.0) + fi * 1.7);
        // Each bulb sits on a spectrum band by height — the spiral lights in
        // the shape of the music, bass at the base, highs at the tip.
        let band = spec(s * 0.8 + 0.05);
        let bright = twinkle * mix(0.25, 1.0, depth * 0.5 + 0.5) * glow_amt * (0.3 + band * 1.4);
        col += bulb_colour(fi) * bright * 0.00022 / (d2 + 0.00006);
    }
    // White garland lights on a counter-spiral, chasing on the beat.
    for (var i = 0; i < 60; i++) {
        let fi = f32(i);
        let s = (fi + 0.5) / 60.0;
        let a = -s * 4.0 * TAU + spin + 1.0;
        let bp = vec2<f32>(rb * s * 0.95 * cos(a), top + s * (bottom - top) * 0.97);
        let d2 = dot(p - bp, p - bp);
        let chase = 0.5 + 0.5 * sin(a - u.beat * TAU * 0.5);
        let bright = (0.3 + 0.7 * chase * (sin(a) * 0.5 + 0.5)) * glow_amt * (0.5 + u.mid);
        col += vec3<f32>(1.0, 0.9, 0.7) * bright * 0.00006 / (d2 + 0.00003);
    }

    // A warm wave climbs the tree once per beat.
    let wave = exp(-abs(s_h - (1.0 - u.beat_phase)) * 8.0) * beat_pulse(6.0);
    col += vec3<f32>(1.0, 0.75, 0.35) * wave * in_tree * 0.22;

    // Star on top: gold, slowly turning, shimmering with the music.
    let sp = rot(sin(t * 0.5) * 0.2) * (p - vec2<f32>(0.0, top - 0.04));
    let star = sd_star5(vec2<f32>(sp.x, -sp.y), 0.075, 0.45);
    let shimmer = 0.8 + 0.2 * sin(t * 3.0) + 0.4 * u.intensity + u.hits4.x * 0.7;
    col = mix(col, vec3<f32>(1.0, 0.8, 0.3) * shimmer, smoothstep(0.003, -0.003, star));
    col += vec3<f32>(1.0, 0.7, 0.2) * shimmer * 0.02 / (abs(star) + 0.02) * 0.35;

    // Falling snow, three depths — heavier and faster when the music is loud.
    let storm = 1.0 + u.energy * 0.8;
    let snow = snow_layer(in.uv, 7.0, 0.05 * storm, 0.06, t) * 0.4
        + snow_layer(in.uv + 0.3, 13.0, 0.08 * storm, 0.06, t) * 0.55
        + snow_layer(in.uv + 0.7, 22.0, 0.12 * storm, 0.07, t) * 0.8;
    col += vec3<f32>(0.85, 0.9, 1.0) * snow * (0.4 + u.high * 0.4);

    col += prev(in.uv) * 0.06;
    return vec4<f32>(col, 1.0);
}
