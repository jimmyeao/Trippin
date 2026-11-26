// Spectrum (accent layer): the music's spectrum as soft columns of light
// rising from the bottom edge, bass in the middle and the highs out at the
// sides, with a faint reflection hanging from the top edge. The column
// heights are the spectrum (shape), a slow shimmer runs along them on the
// energy clock (motion), and the mirror's centre drifts over eight bars
// (direction). A fixed 48 columns.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = 48.0;
    let centre = 0.5 + 0.08 * sin(TAU * u.flow / 32.0);
    let x = in.uv.x;
    let col_i = floor(x * n);
    let cx = (col_i + 0.5) / n;
    let fx = fract(x * n);
    // Mirrored from the centre: bass in the middle.
    let k = clamp(abs(cx - centre) / max(centre, 1.0 - centre), 0.0, 1.0);
    let s = spec(k * 0.9);
    let height = 0.03 + 0.32 * sqrt(clamp(s, 0.0, 1.0));
    // A column with soft sides and a gap between neighbours.
    let side = smoothstep(0.0, 0.18, fx) * smoothstep(1.0, 0.82, fx);
    let from_bottom = 1.0 - in.uv.y;
    let shimmer = 0.85 + 0.15 * sin(u.clock4.x * 0.6 + col_i * 0.7);
    var c = 0.0;
    // Bright cap, softly fading body.
    let body = smoothstep(height, height - 0.02, from_bottom) * (0.25 + 0.75 * from_bottom / height);
    let cap = exp(-abs(from_bottom - height) * 120.0);
    c += (body * 0.5 + cap * 0.9) * side * shimmer;
    // The faint reflection from the top edge.
    let from_top = in.uv.y;
    let rh = height * 0.5;
    c += smoothstep(rh, 0.0, from_top) * 0.15 * side;
    let tone = palette(0.15 + 0.7 * k);
    return vec4<f32>(finite(tone * tone * c * 0.8), 1.0);
}
