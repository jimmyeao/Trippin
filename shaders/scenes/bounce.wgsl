// Bounce: glowing orbs rise with the beat and slam a lit floor — each orb is a
// spectrum band, so the pattern of heights is the music's shape.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let p = vec2<f32>((uv.x - 0.5) * aspect(), uv.y);

    var col = vec3<f32>(0.0);

    // Floor.
    let fy = 0.12;
    col += palette(0.4) * exp(-abs(uv.y - fy) * 60.0) * (0.3 + u.kick * 0.7);
    col += palette(0.4) * smoothstep(fy, fy - 0.4, uv.y) * 0.10;

    // Eight orbs, one per spectrum octave — phase offset makes them cascade.
    for (var i = 0; i < 8; i++) {
        let fi = f32(i);
        let band = (fi + 0.5) / 8.0;
        let v = spec(band);
        let x = -0.75 + fi * 0.22;
        // Height rides the band; the bounce bobs with the beat.
        let hop = abs(sin(u.beat_phase * PI + fi * 0.35));
        let y = fy + 0.06 + v * 0.55 + hop * 0.10 * (0.3 + v);
        let orb = exp(-pow(length(vec2<f32>(uv.x - (0.5 + x / aspect()), uv.y - y)) * 22.0, 2.0));
        col += palette(band + u.hue * 0.3) * orb * (0.6 + v * 1.2);

        // Impact flash on the floor under the orb at the beat's bottom.
        let impact = exp(-u.beat_phase * 8.0) * smoothstep(0.4, 0.0, uv.y - fy) *
                     exp(-abs(uv.x - (0.5 + x / aspect())) * 18.0);
        col += palette(band + 0.5) * impact * v * 0.5;
    }

    // Dust motes in the light.
    col += vec3<f32>(0.3, 0.4, 0.5) * step(0.996, hash21(floor(uv * 300.0) + floor(u.time * 2.0))) * u.mid * 0.5;

    return vec4<f32>(finite(col), 1.0);
}
