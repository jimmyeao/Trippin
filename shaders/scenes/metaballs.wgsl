// Metaballs: lava blobs orbiting a centre of mass; each ball's radius is a
// spectrum band, so the mass swells where the music is loud.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * 0.55;
    var f = 0.0;
    var cw = vec3<f32>(0.0);

    for (var i = 0; i < 8; i++) {
        let fi = f32(i);
        let band = fi / 8.0;
        let v = spec(band * 0.6 + 0.05);
        let ang = u.flow * (0.3 + fi * 0.07) * select(1.0, -1.0, i % 2 == 1) + fi * 1.7;
        let orbit = 0.16 + 0.14 * sin(u.flow * 0.23 + fi * 2.1);
        let pos = vec2<f32>(cos(ang), sin(ang)) * orbit;
        let r = 0.05 + v * 0.14 + u.kick * 0.008;
        let d = length(p - pos);
        let w = r * r / max(d * d, 1e-5);
        f += w;
        cw += palette(band + u.hue) * w;
    }

    let iso = smoothstep(0.9, 1.1, f);
    let rim = smoothstep(0.9, 1.1, f) - smoothstep(1.6, 3.2, f);
    var col = cw / max(f, 1e-4) * iso * 0.55;
    col += palette(0.5 + u.hue) * rim * (0.8 + u.kick * 0.6);
    col += palette(0.8) * exp(-f * 0.6) * 0.15 * u.energy; // ambient halo

    // Distant background glimmer.
    col += palette(0.2) * 0.015;

    return vec4<f32>(finite(col), 1.0);
}
