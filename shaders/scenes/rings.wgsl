// Rings: concentric neon circles whose radii ripple with the spectrum and race
// outward on the beat — a top-down view of energy leaving the stage.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * 0.6;
    let r = length(p);
    let a = angle(p) / TAU + 0.5;

    // Rings drift outward continuously; speed doubles while the kick lands.
    let scroll = u.flow * 0.12 + u.kick * 0.02;
    let n = 9.0;
    let cell = fract(r * n - scroll);
    let ring_i = floor(r * n - scroll);
    let line = smoothstep(0.12, 0.0, abs(cell - 0.5) - 0.36);

    // Each ring's brightness and warp come from the spectrum at this angle.
    let v = spec(a * 0.5 + ring_i / n * 0.5);
    let warp = sin(a * TAU * (2.0 + ring_i)) * v * 0.02;
    let wr = length(p + vec2<f32>(warp));

    var col = palette(a + ring_i / n + u.flow * 0.01) * line * (0.25 + v * 1.1);

    // Ripple the ring spacing with the band energy.
    let ripple = smoothstep(0.1, 0.0, abs(fract(wr * n - scroll) - 0.5) - 0.38);
    col += palette(0.5 + ring_i / n) * ripple * v * 0.5;

    // Centre ember.
    col += palette(0.0) * exp(-r * 8.0) * (0.3 + u.bass);

    // Radial fade into darkness.
    col *= smoothstep(1.3, 0.5, r);

    return vec4<f32>(finite(col), 1.0);
}
