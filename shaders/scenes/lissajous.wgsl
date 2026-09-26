// Lissajous figure: a glowing loop traced by two oscillators whose frequencies
// follow the low and high bands; phase drifts with the beat. Feedback trails
// smear the curve into ribbons.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * 0.62;

    // Trails.
    var col = prev(in.uv * 0.996 + vec2<f32>(0.002, 0.0)) * 0.90;

    // Oscillator frequencies ride the spectrum; phase beats with the track.
    let a = 2.0 + floor(spec(0.08) * 6.0 + 0.5);
    let b = 2.0 + floor(spec(0.85) * 6.0 + 0.5);
    let ph = u.beat_phase * TAU;

    // Evaluate the curve parametrically and accumulate distance to each sample.
    var d = 10.0;
    for (var i = 0; i < 72; i++) {
        let t = f32(i) / 72.0 * TAU;
        let q = vec2<f32>(sin(a * t + ph), sin(b * t)) * 0.42;
        d = min(d, length(p - q));
    }
    let line = exp(-d * 60.0);
    let glow = exp(-d * 12.0) * 0.3;

    col += palette(0.25 + u.energy * 0.3) * (line + glow) * (0.6 + u.kick * 0.6);

    // A tracer dot at the head of the curve.
    let head = vec2<f32>(sin(ph), sin(b / max(a, 1.0) * ph)) * 0.42;
    col += vec3<f32>(1.0, 0.95, 0.8) * exp(-length(p - head) * 25.0) * (0.5 + u.energy);

    // Soft wash behind so trails have something to fade into.
    col += palette(0.6) * 0.012;
    col *= 0.96;

    return vec4<f32>(finite(col), 1.0);
}
