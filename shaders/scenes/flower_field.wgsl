// A field of geometric flowers opening and closing: polar petals on a
// grid, each cell's bloom phase offset and driven by the spectrum. Organic
// but graphic — good for melodic breaks.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let n = 7.0;
    let cell = floor(p * n);
    let local = (fract(p * n) - 0.5);
    let h = hash21(cell);

    let a = angle(local);
    let r = length(local) * n; // petal radius in cell units
    let petals = 3.0 + floor(h * 4.0);
    // Bloom: petals open over the bar, snapped wider by onsets.
    let open = 0.4 + 0.5 * sin(u.beat * PI * 0.25 + h * TAU) + u.onset * 0.3;
    let flower = cos(a * petals + h * TAU + u.flow * 0.3) * 0.5 + 0.5;
    let shape = smoothstep(open, open - 0.15, r - flower * 0.5);

    var col = vec3<f32>(0.008, 0.006, 0.02);
    let fc = palette(h * 2.0 + u.hue);
    let amp = spec(h) * 0.7 + u.energy * 0.5;
    col += fc * shape * (0.2 + amp * 1.6);
    // Stamen glow.
    col += fc * exp(-r * 4.0) * u.kick;
    col += prev(in.uv) * 0.15;
    return vec4<f32>(col, 1.0);
}
