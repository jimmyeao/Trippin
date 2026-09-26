// Pixel fall: columns of falling glyphs — rain speed and density follow the
// band under each column, bright head + fading tail via feedback.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let cols = 40.0;
    let cx = floor(uv.x * cols);
    let band = (cx + 0.5) / cols;
    let v = spec(band * 0.8);

    // Column stream: each column falls at a hashed speed, faster when its band
    // is loud. Glyph cells flicker.
    let speed = 0.15 + hash21(vec2<f32>(cx, u.seed)) * 0.25 + v * 0.4;
    let y = fract(uv.y * 1.5 + u.flow * speed + hash21(vec2<f32>(cx, 7.0)));
    let glyph = step(0.35, hash21(floor(vec2<f32>(cx, uv.y * 60.0 + floor(u.flow * speed * 40.0)))));

    // Head is bright, tail fades upward.
    let trail = pow(1.0 - y, 3.0);
    let head = smoothstep(0.06, 0.0, abs(y - 0.02));

    var col = palette(band + 0.15) * glyph * (trail * 0.7 + head * 1.4) * (0.3 + v * 1.1);

    // Ghost of last frame drifting down.
    col += prev(vec2<f32>(uv.x, uv.y - u.dt * speed * 0.4)) * 0.55;

    // Column dividers.
    col *= 0.7 + 0.3 * smoothstep(0.0, 0.08, abs(fract(uv.x * cols) - 0.5));

    // Base glow.
    col += palette(0.5) * 0.02 * (0.5 + u.energy);

    return vec4<f32>(finite(col), 1.0);
}
