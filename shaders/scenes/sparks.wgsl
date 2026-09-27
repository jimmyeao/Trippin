// Sparks: pyro fountains on every beat. Embers erupt from burner mouths,
// climb, arc under gravity and cool from white to ember-orange; a short
// feedback trail streaks each particle into a proper spark line.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    // Trails: pull the last frame up a touch so sparks smear along their path.
    var col = prev(uv + vec2<f32>(0.0, 0.0018)) * 0.70;

    let beat_i = floor(u.beat);
    let t = u.beat_phase; // 0..1 through the beat = volley age

    // Three burners across the footlights.
    for (var s = 0; s < 3; s++) {
        let fs = f32(s);
        let fx = 0.2 + fs * 0.3;
        let base = vec2<f32>(fx, 0.93);
        let vh = hash21(vec2<f32>(beat_i, fs * 5.0 + u.seed));
        // Hot metal colours, occasionally a palette accent.
        let ember = mix(vec3<f32>(1.0, 0.62, 0.18), palette(vh), step(0.7, fract(vh * 3.7)));
        for (var i = 0; i < 30; i++) {
            let h = hash22(vec2<f32>(f32(i) + fs * 100.0, beat_i));
            let h2 = hash22(vec2<f32>(f32(i) * 1.7 + fs * 31.0, beat_i + 7.0));
            // Projectile: up fast, gravity pulls it back down.
            let v0 = 0.55 + h.x * 0.7;
            let vx = (h.y - 0.5) * 0.9;
            let life = clamp(t / max(h2.x * 0.8 + 0.25, 0.06), 0.0, 1.0);
            let pos = base + vec2<f32>(
                vx * life * 0.42 + sin(life * 20.0 + h2.y * 9.0) * 0.005,
                -v0 * life * 0.55 + 0.40 * life * life);
            let d = length((uv - pos) * vec2<f32>(aspect(), 1.0));
            // White-hot core shrinking to a dull ember as it dies.
            let core = exp(-d * (170.0 + life * 300.0));
            let fade = (1.0 - life * life) * (1.0 - t * 0.55);
            col += mix(vec3<f32>(1.3, 1.2, 1.0), ember, life * 0.8) * core * fade * (1.6 + u.kick);
        }
        // Burner mouth: a tight molten pool that flares on the launch beat.
        let md = length((uv - base) * vec2<f32>(aspect(), 1.0));
        col += mix(vec3<f32>(1.0, 0.55, 0.15), ember, 0.4)
             * exp(-md * 70.0) * (0.15 + beat_pulse(7.0) * 1.1);
    }

    // Warm haze hugging the floor so the stage never sits on dead black.
    col += vec3<f32>(0.20, 0.10, 0.04) * exp(-abs(uv.y - 1.02) * 10.0) * (0.3 + u.energy);
    return vec4<f32>(finite(col), 1.0);
}
