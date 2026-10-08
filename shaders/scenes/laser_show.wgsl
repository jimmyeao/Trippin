// @bloom 0.9 @tonemap agx @no-dancer
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

    // The rig: 14 beams from behind the crowd running a *show* — a new
    // formation every 4 bars, morphing in over a beat (all poses of the
    // smooth clocks, so nothing jumps):
    //   0 fan sweep     — the classic fan panning side to side
    //   1 crossing scan — two halves scissoring across each other
    //   2 spinning star — beams fanned round, rotating (tips dip below the
    //                     horizon behind the crowd, like a real top-down rig)
    //   3 wave          — beams rippling up and down in a travelling wave
    //   4 tight beam    — the fan collapses to a narrow bundle that sweeps
    //                     wide, then bursts open on the kicks
    //   5 chase         — a wide static fan with beams firing in a rotating
    //                     sequence (soft fades, not hard pops)
    var beams = vec3<f32>(0.0);
    let o = vec2<f32>(0.0, 0.62);
    let n = 14;
    let bar = floor(u.beat / 16.0);
    let f_now = i32(hash21(vec2<f32>(bar, 3.1)) * 6.0);
    let f_prev = i32(hash21(vec2<f32>(bar - 1.0, 3.1)) * 6.0);
    let m = smoothstep(0.0, 1.0 / 16.0, fract(u.beat / 16.0));   // morph over a beat
    let t = u.clock4.x;
    let open = 0.6 + 0.4 * u.intensity + 0.4 * u.pres4.y;
    for (var i = 0; i < n; i++) {
        let fi = f32(i);
        let k = fi / f32(n - 1) - 0.5;                  // -0.5..0.5 across the rig
        var ang = vec2<f32>(0.0);                        // (prev, now) formation angle
        var lvl = vec2<f32>(1.0);
        for (var w = 0; w < 2; w++) {
            let f = select(f_prev, f_now, w == 1);
            var a = -PI * 0.5;
            var l = 1.0;
            if f == 0 {
                a += k * 1.35 * open + 0.35 * sin(t * 0.35 + fi * 0.3);
            } else if f == 1 {
                let s = select(-1.0, 1.0, i % 2 == 0);
                a += s * (0.15 + 0.55 * (0.5 + 0.5 * sin(t * 0.5))) + k * 0.25;
            } else if f == 2 {
                a += (fi / f32(n)) * TAU + t * 0.25;
            } else if f == 3 {
                a += k * 1.6 * open + 0.25 * sin(t * 0.8 - fi * 0.6);
            } else if f == 4 {
                a += k * (0.06 + 0.5 * u.hits4.x) + 0.7 * sin(t * 0.22);
                l = 0.35;                                   // 14 stacked beams
            } else {
                a += k * 1.7 * open;
                let seq = fract(t * 0.5 - fi / f32(n));
                l = 0.15 + 0.85 * exp(-seq * 6.0);
            }
            if w == 0 {
                ang.x = a;
                lvl.x = l;
            } else {
                ang.y = a;
                lvl.y = l;
            }
        }
        // Morph along the shortest way round.
        let da = atan2(sin(ang.y - ang.x), cos(ang.y - ang.x));
        let a = ang.x + da * m;
        let lv0 = mix(lvl.x, lvl.y, m);
        let hue = fi / f32(n) + u.hue * 0.15;
        let bc = palette(hue) * palette(hue);            // squared: saturated
        let bl = beam(p, o, a);
        let lv = lv0 * (0.3 + 0.9 * spec(abs(k) * 1.6 + 0.05)) * (0.6 + 0.4 * beat_pulse(2.5)) * 1.8;
        beams += bc * bl.x * lv;
        halo += bc * bl.y * lv;
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
