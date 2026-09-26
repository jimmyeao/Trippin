// Sparks: pyro fountains launch on every beat — particles climb, arc and die,
// their colour hashed per volley. High-end crackle adds glitter.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    var col = prev(uv) * 0.88;   // short trails

    let beat_i = floor(u.beat);
    let t = u.beat_phase;        // 0..1 through the beat = volley age

    // Two fountains, symmetric about the centre.
    for (var s = 0; s < 2; s++) {
        let fx = select(0.30, 0.70, s == 1);
        let base = vec2<f32>(fx, 0.06);
        let volley_hue = hash21(vec2<f32>(beat_i, f32(s) * 5.0 + u.seed));
        for (var i = 0; i < 18; i++) {
            let h = hash22(vec2<f32>(f32(i) + f32(s) * 100.0, beat_i));
            let v0 = 0.5 + h.x * 0.6;
            let vx = (h.y - 0.5) * 0.5;
            // Projectile: up then fall, scaled to land within the beat.
            let life = clamp(t / max(h.y * 0.7 + 0.3, 0.05), 0.0, 1.0);
            let pos = base + vec2<f32>(vx * life * 0.4, v0 * life * 0.5 - 0.4 * life * life);
            let d = length((uv - pos) * vec2<f32>(aspect(), 1.0));
            let spark = exp(-d * 220.0) * (1.0 - life) * (1.0 - t * 0.4);
            col += palette(volley_hue + h.x * 0.15) * spark * (0.7 + u.kick * 0.8);
        }
        // Fountain mouth glow.
        col += palette(volley_hue) * exp(-length((uv - base) * vec2<f32>(aspect(), 1.0)) * 25.0) * beat_pulse(5.0);
    }

    // Crackle glitter across the top at high frequencies.
    col += vec3<f32>(1.0, 0.9, 0.7) * step(0.9965, hash21(floor(uv * 400.0) + floor(u.time * 20.0))) * spec(0.9);

    return vec4<f32>(finite(col), 1.0);
}
