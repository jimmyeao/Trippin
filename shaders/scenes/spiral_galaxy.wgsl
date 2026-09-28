// A slow spiral galaxy of particles: logarithmic arms dotted with star
// clumps, rotating gently. Bass spins it up, onsets spark new stars.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let r = length(p);
    let a = angle(p);

    let arms = 3.0;
    let wind = u.flow * 0.12;
    // Logarithmic spiral arms.
    let arm = fract((a + r * 6.0 + wind * TAU) / TAU * arms);
    let arm_dist = min(arm, 1.0 - arm);
    let arm_glow = exp(-arm_dist * arm_dist * 18.0 * (1.0 + r)) * exp(-r * 1.1);

    // Star clumps: hashed cells in polar space.
    let cell = vec2<f32>(floor((a + wind * TAU * 0.9) * 24.0 / PI), floor(r * 30.0));
    let star = hash21(cell);
    let twinkle = step(0.93, star) * (0.5 + 0.5 * sin(u.time * (2.0 + star * 6.0) + star * 40.0));
    let near_arm = exp(-arm_dist * 8.0);

    var col = vec3<f32>(0.004, 0.004, 0.012);
    col += palette(r * 0.4 + u.hue) * arm_glow * (0.5 + u.bass * 1.6);
    col += vec3<f32>(0.9, 0.95, 1.0) * twinkle * near_arm * exp(-r * 0.8) * (0.5 + u.onset * 2.0);
    // Galactic core.
    col += palette(0.6 + u.hue) * exp(-r * 7.0) * (0.6 + u.kick * 1.4);
    col += prev(in.uv) * 0.2;
    return vec4<f32>(col, 1.0);
}
