// Pixel fall: columns of falling glyphs — rain speed and density follow the
// band under each column, bright head + fading tail via feedback.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let cols = 40.0;
    let rows = 26.0;
    let cx = floor(uv.x * cols);
    let band = (cx + 0.5) / cols;
    let v = spec(band * 0.8);

    // Stream phase (u.flow is in beats): each column falls at a hashed
    // speed. uv.y grows DOWN-screen, so the scroll must subtract flow —
    // adding made the pixels climb. Audio drives brightness, not rate.
    let speed = 0.02 + hash21(vec2<f32>(cx, u.seed)) * 0.05;
    // Energy: fall rate follows the smooth energy clock.
    let y = fract(uv.y * 1.5 - u.clock4.x * speed + hash21(vec2<f32>(cx, 7.0)));

    // Glyph cells scroll at the same rate as the stream so the characters
    // ride the rain instead of strobing in place.
    let grow = floor(uv.y * rows - u.clock4.x * speed * 1.5 * rows);
    let glyph = step(0.45, hash21(vec2<f32>(cx, grow)));

    // Head is bright, tail fades upward — softer contrast for the eyes.
    let trail = pow(1.0 - y, 3.0) * 0.6;
    let head = smoothstep(0.06, 0.0, abs(y - 0.02)) * 0.9;

    // Colour pulse: hue sweeps slowly, brightness swells on each beat and
    // with the column's band.
    let hue_shift = sin(u.beat_phase * TAU) * 0.04;
    let pulse = 0.25 + v * 0.9 + beat_pulse(5.0) * 0.7;
    var col = palette(band + 0.15 + hue_shift) * glyph * (trail + head) * pulse;

    // Ghost of last frame drifting down at stream speed (flow ≈ bpm/60 beats/s).
    let fall_v = speed * 1.5 * (u.bpm / 60.0);
    col += prev(vec2<f32>(uv.x, uv.y + u.dt * fall_v)) * 0.45;

    // Column dividers.
    col *= 0.7 + 0.3 * smoothstep(0.0, 0.08, abs(fract(uv.x * cols) - 0.5));

    // Base glow.
    col += palette(0.5) * 0.02 * (0.5 + u.energy);

    return vec4<f32>(finite(col), 1.0);
}
