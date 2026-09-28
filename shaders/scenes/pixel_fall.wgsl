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
    // speed. Keep it slow and steady — audio drives brightness, not rate
    // (audio-in-speed stutters and fast fall hurts the eyes).
    let speed = 0.02 + hash21(vec2<f32>(cx, u.seed)) * 0.05;
    let y = fract(uv.y * 1.5 + u.flow * speed + hash21(vec2<f32>(cx, 7.0)));

    // Glyph cells scroll at the same rate as the stream so the characters
    // ride the rain instead of strobing in place.
    let grow = floor(uv.y * rows + u.flow * speed * 1.5 * rows);
    let glyph = step(0.45, hash21(vec2<f32>(cx, grow)));

    // Head is bright, tail fades upward — softer contrast for the eyes.
    let trail = pow(1.0 - y, 3.0) * 0.6;
    let head = smoothstep(0.06, 0.0, abs(y - 0.02)) * 0.9;

    var col = palette(band + 0.15) * glyph * (trail + head) * (0.2 + v * 0.9);

    // Ghost of last frame drifting down at stream speed (flow ≈ bpm/60 beats/s).
    let fall_v = speed * 1.5 * (u.bpm / 60.0);
    col += prev(vec2<f32>(uv.x, uv.y - u.dt * fall_v)) * 0.45;

    // Column dividers.
    col *= 0.7 + 0.3 * smoothstep(0.0, 0.08, abs(fract(uv.x * cols) - 0.5));

    // Base glow.
    col += palette(0.5) * 0.02 * (0.5 + u.energy);

    return vec4<f32>(finite(col), 1.0);
}
