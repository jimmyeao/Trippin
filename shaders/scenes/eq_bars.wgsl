// Spectrum analyser: 32 bars mirrored about the centre line, hot tips, gaps
// between columns. Direct and readable — the "what's playing right now" scene.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let bars = 32.0;
    let cell = fract(in.uv.x * bars);
    let i = floor(in.uv.x * bars);
    let fx = (i + 0.5) / bars;
    let v = spec(fx);

    // Distance from the centre line; each bar grows both ways. Edge AA is
    // pixel-proportional (fwidth) so bars stay crisp at any resolution.
    // Shape + direction: the bar field bends into an arc that flips from
    // smile to frown over a phrase, deeper with bass presence.
    let bend = (fx - 0.5) * (fx - 0.5) * 4.0 - 0.33;
    let d = abs(in.uv.y - 0.5 - bend * 0.12 * sin(u.clock4.x * 0.03) * (0.4 + u.pres4.x)) * 2.0;
    let h = 0.04 + v * (0.8 + 0.15 * u.intensity);
    let aay = fwidth(d) * 1.5;
    let bar = smoothstep(-aay, aay, h - d);

    // 10% gutter between columns, crisp sides, brighter tip.
    let edge = min(cell, 1.0 - cell);
    let aax = fwidth(cell) * 1.5;
    let side = smoothstep(-aax, aax, edge - 0.10);
    let tip = smoothstep(0.06, 0.0, h - d);
    let base = palette(fx * 0.85 + 0.15) * (0.55 + 0.65 * v);

    var col = base * bar * side;
    col += vec3<f32>(1.0) * bar * side * tip * 0.55;

    // Faint ghost of the full bar so quiet bands still hold a shape.
    col += base * 0.05 * side * smoothstep(1.0, 0.0, d);

    // Centre line flashes on the kick.
    col += palette(0.5) * exp(-d * 18.0) * (0.15 + u.kick * 0.4);

    return vec4<f32>(finite(col), 1.0);
}
