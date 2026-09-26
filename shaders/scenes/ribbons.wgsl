// Ribbons: stacked sine strands displaced by the spectrum — each strand reads
// a different slice of the audio and undulates with it.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    var col = vec3<f32>(0.0);

    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let yb = 0.15 + fi * 0.175;
        // Each ribbon reads a different band region.
        let band = fi / 5.0;
        let v = spec(uv.x * 0.5 + band * 0.5);
        let wob = sin(uv.x * (4.0 + fi * 1.3) * TAU * 0.5 + u.flow * (0.6 + fi * 0.2) + fi * 2.0);
        let d = uv.y - yb - wob * (0.015 + v * 0.09);
        let ribbon = exp(-abs(d) * (60.0 - v * 20.0));
        col += palette(band + 0.1 + u.hue * 0.4) * ribbon * (0.25 + v * 1.3);
        // Ribbon body glow.
        col += palette(band + 0.35) * exp(-abs(d) * 14.0) * v * 0.18;
    }

    // Beat shimmer: brightness pinch on each kick.
    col *= 1.0 + u.kick * 0.25;

    // Tiny sparkle dust where ribbons cross.
    col += vec3<f32>(0.8, 0.9, 1.0) * step(0.997, hash21(floor(uv * 250.0) + floor(u.time * 3.0))) * u.high;

    return vec4<f32>(finite(col), 1.0);
}
