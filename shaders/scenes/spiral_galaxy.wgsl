// A slow spiral galaxy of particles: logarithmic arms dotted with star
// clumps, rotating gently. Bass spins it up, onsets spark new stars.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let r = length(p);
    let a = angle(p);

    let arms = 3.0;
    // Energy: the galaxy turns faster as the track drives.
    let wind = u.clock4.x * 0.12;
    // Logarithmic spiral arms.
    // Shape: the arms wind tighter as the bass builds.
    let arm = fract((a + r * (4.0 + 5.0 * u.pres4.x) + wind * TAU) / TAU * arms);
    let arm_dist = min(arm, 1.0 - arm);
    let arm_glow = exp(-arm_dist * arm_dist * 18.0 * (1.0 + r)) * exp(-r * 1.1);

    // Star clumps: round dots inside hashed polar cells (squares read as
    // rectangles — use distance to the cell's centre point instead).
    let cell = vec2<f32>(floor((a + wind * TAU * 0.9) * 24.0 / PI), floor(r * 30.0));
    let star = hash21(cell);
    // Cell-local position of the star (jittered off-centre).
    let cell_f = vec2<f32>(fract((a + wind * TAU * 0.9) * 24.0 / PI), fract(r * 30.0));
    let sp = cell_f - vec2<f32>(hash21(cell + 5.0), hash21(cell + 9.0)) * 0.7 - 0.15;
    let dot_r = length(sp * vec2<f32>(PI / 24.0, 1.0 / 30.0)); // ~metric distance
    let twinkle = step(0.88, star)
                * smoothstep(0.012 + star * 0.01, 0.0, dot_r)
                * (0.5 + 0.5 * sin(u.time * (2.0 + star * 6.0) + star * 40.0));
    let near_arm = exp(-arm_dist * 8.0);

    var col = vec3<f32>(0.004, 0.004, 0.012);
    col += palette(r * 0.4 + u.hue) * arm_glow * (0.5 + u.bass * 1.6);
    col += vec3<f32>(0.9, 0.95, 1.0) * twinkle * near_arm * exp(-r * 0.8) * (0.8 + u.onset * 3.0);
    // Galactic core.
    col += palette(0.6 + u.hue) * exp(-r * 7.0) * (0.6 + u.kick * 1.4);
    col += prev(in.uv) * 0.2;
    return vec4<f32>(col, 1.0);
}
