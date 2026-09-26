// Festival laser rig: volumetric beam fans sweeping from a point behind the
// crowd, side rigs crossing overhead, haze catching it all. Sweeps are on the
// tempo clock, brightness rides the beat, and the fans open wider as the
// track drives. The crowd silhouette holds phone lights twinkling on the
// highs. Big-room imagery, Tomorrowland-style.

// Light at point p from a beam leaving o at angle ang (screen coords, y down).
fn beam(p: vec2<f32>, o: vec2<f32>, ang: f32) -> f32 {
    let d = p - o;
    let dir = vec2<f32>(cos(ang), sin(ang));
    let along = dot(d, dir);
    let perp = abs(d.x * dir.y - d.y * dir.x);
    let w = 0.0035 + along * 0.011;               // beams widen into a fan
    return exp(-perp * perp / (w * w)) * smoothstep(0.0, 0.03, along) * exp(-along * 0.45);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);            // y grows downward
    let asp = aspect();

    // Night air with a haze glow around the rig.
    var col = mix(vec3<f32>(0.015, 0.01, 0.04), vec3<f32>(0.0, 0.0, 0.012), clamp(p.y + 0.6, 0.0, 1.0));

    // Main fan: a dozen beams from behind the crowd, panning on the tempo
    // clock. The fan opens as intensity climbs.
    let o = vec2<f32>(0.0, 0.62);
    let spread = 1.35 * (0.75 + 0.45 * u.intensity);
    let chase = floor(u.beat * 2.0);                     // eighth-note chase
    for (var i = 0; i < 12; i++) {
        let fi = f32(i);
        // Eighth-note chase: only some beams fire at once, pattern rotates.
        let on = step(fract((fi - chase) * 0.25 + 0.5), 0.55);
        let base = -PI * 0.5 + (fi / 11.0 - 0.5) * spread;
        let sweep = 0.3 * sin(u.flow * 0.35 + fi * 0.8) + 0.12 * sin(u.flow * 1.1 + fi * 2.1);
        let ang = base + sweep;
        let hue = fi / 12.0 + u.hue * 0.15;
        let bc = palette(hue) * palette(hue);            // squared: saturated
        col += bc * beam(p, o, ang) * on * (0.55 + 0.45 * beat_pulse(2.5)) * 1.6;
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
            col += bc * bc * beam(p, so, ang) * (0.5 + 0.5 * beat_pulse(2.0)) * 0.9;
        }
    }

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
