// Falling columns of light — the cyber-rain look. Each column is a stream
// of stacked cells scrolling down-screen; heads glow white-hot, tails fade
// by spectrum. Bass thickens the rain.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let cols = 48.0;
    // Shape: the streams bend into waves with mid presence.
    let px = p.x + 0.05 * sin(p.y * 2.0 + u.clock4.z * 0.2) * u.pres4.y;
    let cx = floor(px * cols);
    let hx = hash21(vec2<f32>(cx, 11.0));
    let speed = 0.07 + hx * 0.22; // slower, steady — onsets light cells, not speed

    // Column stream scrolling down (+p.y = down-screen).
    let rows = 26.0;
    // Energy: fall speed follows the smooth energy clock.
    let y = p.y - u.clock4.x * speed;
    let cy = floor(y * rows);
    let glyph = hash21(vec2<f32>(cx, cy + floor(u.clock4.x * speed * 2.0)));
    let lit = step(glyph, 0.6 + u.bass * 0.3);
    // Cell shading: notch the cell so it reads as stacked blocks.
    let local = fract(vec2<f32>(px * cols, y * rows));
    let block = smoothstep(0.0, 0.08, local.x) * smoothstep(1.0, 0.92, local.x)
              * smoothstep(0.0, 0.12, local.y) * smoothstep(1.0, 0.88, local.y);

    // Head of each stream is the brightest cell.
    let phase = fract(y * 0.5);
    let head = exp(-phase * 6.0);
    let band = spec(fract(p.x * 0.5 / aspect() + 0.5));

    var col = vec3<f32>(0.004, 0.01, 0.006);
    let base = mix(vec3<f32>(0.1, 0.9, 0.3), palette(0.35 + u.hue), 0.4);
    col += base * lit * block * (0.15 + head * 1.4) * (0.4 + band * 1.6);
    col += vec3<f32>(0.9, 1.0, 0.9) * lit * block * head * head * 0.9;
    return vec4<f32>(col, 1.0);
}
