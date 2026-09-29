// A whirlpool in space: polar-domain swirl pulling toward the centre —
// hypnotic, heavy, drop-friendly. Rotation locks to the beat clock; the
// throat glows with the kick.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var p = centred(in.uv);
    let r = length(p);
    let a = angle(p);

    // Spiral domain: tighter wind nearer the centre.
    // Shape: the whirlpool winds tighter with bass presence.
    let wind = (4.0 + 4.0 * u.pres4.x) / (r + 0.15) + u.clock4.x * 0.6;
    let sa = a + wind;
    let bands = sin(sa * 3.0 + log(r + 0.03) * 10.0);
    let streak = smoothstep(-0.2, 0.8, bands);

    // Depth illusion: centre is a darker, faster-swirling throat.
    let depth = exp(-r * 1.6);
    // palette() has period 1 — indexing it by sa/TAU*K (K integer) keeps the
    // colour continuous across the ±π wrap at screen left (sa*0.05 didn't).
    var col = palette(sa / TAU * 4.0 + r * 0.3 + u.hue) * streak * (0.3 + u.energy);
    col *= exp(-r * 0.9);
    col += palette(0.5 + u.hue) * exp(-r * 6.0) * (0.5 + u.kick * 2.0);
    col -= vec3<f32>(0.05) * depth; // darken the eye

    // Swirl trails via feedback rotate slightly each frame.
    // Direction: the feedback swirl reverses over a phrase.
    let tp = rot(0.02 * sin(u.clock4.x * 0.03)) * p * 1.005;
    col = max(col, prev(uncentred(tp)) * 0.92);
    return vec4<f32>(finite(col), 1.0);
}
