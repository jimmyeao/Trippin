// Bounce: glowing orbs hop on the beat over a lit floor at the BOTTOM of
// the frame — each orb is a spectrum band, so the pattern of heights is
// the music's shape. up = 1 - uv.y (uv.y = 0 is screen top).

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let up = 1.0 - in.uv.y;
    let x = in.uv.x;

    var col = vec3<f32>(0.01, 0.008, 0.02) * (0.8 + up * 0.4);

    // Floor glow at the bottom.
    let fy = 0.14;
    col += palette(0.4 + u.hue) * exp(-abs(up - fy) * 50.0) * (0.3 + u.kick * 0.9);
    col += palette(0.4 + u.hue) * smoothstep(fy, 0.0, up) * 0.12;

    // Eight orbs, one per spectrum octave — staggered phase = cascade.
    for (var i = 0; i < 8; i++) {
        let fi = f32(i);
        let band = (fi + 0.5) / 8.0;
        let v = spec(band);
        let ox = 0.12 + fi * 0.105;
        // Hop on the beat; height rides the band.
        let hop = abs(sin(u.beat_phase * PI + fi * 0.35));
        let y = fy + 0.05 + v * (0.45 + u.energy * 0.2) + hop * 0.09 * (0.3 + v);
        let d = length(vec2<f32>((x - ox) * aspect(), up - y));
        let r = 0.035 + v * 0.03;
        let orb = smoothstep(r, r * 0.5, d);
        let halo = exp(-d * 9.0) * 0.4;
        col += palette(band + u.hue) * (orb * (0.5 + v * 1.4) + halo * v);
        // Impact flash at the floor under the orb right after the beat.
        let impact = exp(-u.beat_phase * 7.0) * exp(-abs(up - fy) * 40.0)
                   * exp(-abs(x - ox) * aspect() * 6.0);
        col += palette(band + 0.5 + u.hue) * impact * v;
    }
    return vec4<f32>(finite(col), 1.0);
}
