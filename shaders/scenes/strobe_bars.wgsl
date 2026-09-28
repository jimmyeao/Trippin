// Strobe bars: vertical light columns that fire on the beat like a wall of
// strobes. Bars sequence left-to-right or centre-out, picked per scene
// seed; each onset relights the sweep.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let cols = 16.0;
    let cx = floor((p.x / aspect() + 0.5) * cols);
    let fx = fract((p.x / aspect() + 0.5) * cols);

    // Sweep order: hashed per scene — sequential, centre-out, or random.
    let mode = floor(hash21(vec2<f32>(floor(u.seed), 0.0)) * 3.0);
    var pos = cx / cols;
    if mode == 1.0 {
        pos = abs(cx / cols - 0.5) * 2.0; // centre-out
    } else if mode == 2.0 {
        pos = hash21(vec2<f32>(cx, 0.0)); // scattered
    }

    // Each bar fires as the beat-fraction sweep passes it.
    let sweep = fract(u.beat * 0.5);
    let d = fract(sweep - pos);
    let fire = exp(-d * (10.0 - u.energy * 4.0));

    let bar = smoothstep(0.0, 0.15, fx) * smoothstep(1.0, 0.85, fx);
    let band = spec(cx / cols);
    var col = vec3<f32>(0.008, 0.006, 0.018);
    col += palette(pos + u.hue) * bar * fire * (0.4 + u.bass * 1.6 + band);
    // Dim idle bars keep the wall present between sweeps.
    col += palette(pos + u.hue) * bar * 0.015;
    col += prev(in.uv) * 0.10;
    return vec4<f32>(col, 1.0);
}
