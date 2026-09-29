// Ink: onsets splat glowing blooms at hashed positions; the feedback buffer
// diffuses them into curling smoke. Bass tints the wash, highs sparkle the rim.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let p = centred(uv);

    // Diffuse: sample a jittered neighbourhood and slightly shrink — smoke curl.
    // Shape: the paper grain distorts more with mid presence; energy: it
    // crawls faster.
    let j1 = (noise(uv * 9.0 + u.clock4.z * 0.08) - 0.5) * (0.6 + 0.8 * u.pres4.y);
    let j2 = (noise(uv * 9.0 - u.clock4.z * 0.08 + 40.0) - 0.5) * (0.6 + 0.8 * u.pres4.y);
    let duv = uv + vec2<f32>(j1, j2) * 0.004;
    var col = prev(uncentred(centred(duv) * 0.997)) * 0.955;

    // Splat on beats and onsets — up to three blooms staggered per beat.
    for (var i = 0; i < 3; i++) {
        let bi = floor(u.beat) - f32(i);          // recent beats
        let age = fract(u.beat) + f32(i);         // 0..1 into this beat
        if age > 3.0 { continue; }
        let pos_h = hash22(vec2<f32>(bi, u.seed + f32(i)));
        let pos = (pos_h - 0.5) * vec2<f32>(aspect(), 1.0) * 1.1;
        let rad = age * (0.05 + u.pres4.x * 0.12 + u.hits4.x * 0.04) + 0.01;
        let d = length(p - pos);
        let bloom = exp(-d * d / (rad * rad)) * exp(-age * 1.6);
        // Per-frame deposit: the feedback integrates these over ~20 frames
        // (1/(1-0.955)), so a full-strength add each frame washed to white.
        col += palette(hash21(vec2<f32>(bi, f32(i) * 9.0)) + u.hue) * bloom * (0.5 + u.onset) * 0.2;
    }

    // Ambient wash follows the bass; kick brightens the centre.
    col += palette(0.55) * exp(-length(p) * 1.8) * (0.04 + u.bass * 0.12) * 0.05;
    col += palette(0.0) * u.hits4.x * 0.003;

    return vec4<f32>(finite(col), 1.0);
}
