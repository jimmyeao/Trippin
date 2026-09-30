// @bloom 0.9 @tonemap agx
// Festival laser rig: volumetric beam fans sweeping from a point behind the
// crowd, side rigs crossing overhead, haze catching it all. Sweeps are on the
// tempo clock, brightness rides the beat, and the fans open wider as the
// track drives. The crowd silhouette holds phone lights twinkling on the
// highs. Big-room imagery, Tomorrowland-style.

// Thin laser core + scatter halo (common.wgsl laser_line); the smoke
// density decides how much of the core is visible.
fn beam(p: vec2<f32>, o: vec2<f32>, ang: f32) -> vec2<f32> {
    return laser_line(p, o, ang);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);            // y grows downward
    let asp = aspect();

    // Night air with a haze glow around the rig.
    var col = mix(vec3<f32>(0.015, 0.01, 0.04), vec3<f32>(0.0, 0.0, 0.012), clamp(p.y + 0.6, 0.0, 1.0));

    // Smoke: two drifting fog banks — a slow mid-height haze and a thicker
    // ground layer rolling over the crowd. Beams modulate by it below, so
    // they read as shafts cutting through the smoke rather than flat lines.
    // Two scales of cloud so the whole sky is hazed, with wisps and holes.
    let haze = max(smoke2d(p + vec2<f32>(u.seed, 0.0), u.clock4.z * 2.0), 0.6 * smoke2d(p * 2.2 + vec2<f32>(7.0, u.seed), u.clock4.z * 2.4));
    let ground_fog = smoke2d(p * vec2<f32>(1.6, 3.0) + vec2<f32>(3.0 + u.seed, 1.0), u.clock4.z * 2.6)
                   * smoothstep(0.15, 0.75, p.y);
    let fog = clamp(haze * 0.9 + ground_fog * 0.8, 0.0, 1.2);
    var halo = vec3<f32>(0.0);

    // Main fan: a dozen beams from behind the crowd, panning on the tempo
    // clock. The fan opens as intensity climbs. Beams accumulate separately
    // so they can light the smoke they pass through.
    var beams = vec3<f32>(0.0);
    let o = vec2<f32>(0.0, 0.62);
    // Shape: the fan opens wider with mid presence.
    let spread = 1.35 * (0.6 + 0.4 * u.intensity + 0.4 * u.pres4.y);
    let chase = floor(u.beat * 2.0);                     // eighth-note chase
    for (var i = 0; i < 12; i++) {
        let fi = f32(i);
        // Eighth-note chase: only some beams fire at once, pattern rotates.
        let on = step(fract((fi - chase) * 0.25 + 0.5), 0.55);
        let base = -PI * 0.5 + (fi / 11.0 - 0.5) * spread;
        // Energy: sweeps quicken with the mix; wider with the bass.
        let sweep = (0.2 + 0.2 * u.pres4.x) * sin(u.clock4.x * 0.35 + fi * 0.8) + 0.12 * sin(u.clock4.z * 1.1 + fi * 2.1);
        let ang = base + sweep;
        let hue = fi / 12.0 + u.hue * 0.15;
        let bc = palette(hue) * palette(hue);            // squared: saturated
        let bl = beam(p, o, ang);
        let lv = on * (0.55 + 0.45 * beat_pulse(2.5)) * 1.6;
        beams += bc * bl.x * lv;
        halo += bc * bl.y * lv;
    }

    // Side rigs near the top corners firing down across the fan.
    for (var i = 0; i < 2; i++) {
        let side = f32(i) * 2.0 - 1.0;
        let so = vec2<f32>(side * asp * 0.92, -0.85);
        for (var j = 0; j < 4; j++) {
            let fj = f32(j);
            let ang = PI * 0.5 - side * (0.5 + fj * 0.35)
                    + 0.25 * sin(u.flow * 0.5 + fj * 1.3 + side * 2.0);
            let bc = palette(0.6 + fj * 0.11 + u.hue * 0.15);
            let bl = beam(p, so, ang);
            let lv = (0.5 + 0.5 * beat_pulse(2.0)) * 0.9;
            beams += bc * bc * bl.x * lv;
            halo += bc * bc * bl.y * lv;
        }
    }

    // Shafts: the beam's own light, brighter and sharper where the smoke is
    // thick, plus a diffuse scatter so the fog bank itself glows where the
    // beams cross it.
    // A laser is only visible where smoke scatters it: patchy along its
    // length; the smoke around each beam glows in the beam's colour.
    col += beams * (0.12 + fog * 2.4);
    col += halo * fog * 0.45;

    // Unlit smoke stays faintly visible — cool ambient drifting past.
    col += vec3<f32>(0.012, 0.015, 0.028) * fog * (0.5 + u.energy * 0.4);
    // Smoke rolling low across the crowd picks up spill light.
    col += palette(u.hue * 0.15 + 0.5) * ground_fog * (0.02 + u.bass * 0.05);

    // Haze bloom around the rig point, breathing on the beat.
    let d2 = dot(p - o, p - o);
    col += palette(u.hue * 0.15) * 0.22 * (0.5 + 0.5 * beat_pulse(2.0) + 0.4 * u.energy)
         / (1.0 + d2 * 18.0);

    // Crowd: a head-bumpy silhouette across the bottom that sways with the
    // groove, plus phone torches that twinkle with the highs.
    let sway = (fbm(vec2<f32>(p.x * 8.0, floor(u.time * 2.0))) - 0.5) * 0.05;
    let heads = 0.78 + 0.06 * abs(noise(vec2<f32>(p.x * 14.0, 2.0)) - 0.5) * 2.0 + sway;
    let crowd = step(heads, p.y);
    // Raised arms: sparse vertical dashes above the head line.
    let arm = step(abs(fract(p.x * asp / 0.13) - 0.5), 0.02)
            * step(heads - 0.1 - 0.04 * abs(sin(u.flow * 0.5 + p.x * 20.0)), p.y)
            * step(0.75, hash21(vec2<f32>(floor(p.x * asp / 0.13), 4.0)));
    col = mix(col, vec3<f32>(0.005, 0.003, 0.01), max(crowd, arm));
    // Phone lights in the crowd, riding the high band.
    let cell = floor(p * vec2<f32>(60.0, 40.0));
    let phone = step(0.985, hash21(cell + floor(u.time * 3.0)))
              * crowd * step(heads + 0.02, p.y);
    col += vec3<f32>(0.9, 0.9, 1.0) * phone * (0.2 + 0.8 * spec(0.85));

    col = pow(col, vec3<f32>(1.2));
    return vec4<f32>(col, 1.0);
}
