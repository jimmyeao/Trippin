// Aurora curtains drifting overhead — vertical shimmer bands that wave
// like fabric and brighten with the highs. Up-screen is -p.y.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let up = -p.y * 0.5 + 0.5;

    var col = vec3<f32>(0.0, 0.005, 0.015);

    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let phase = p.x * (1.5 + fi * 0.7) + u.flow * (0.3 + fi * 0.15) + fi * 2.1;
        let centre = 0.55 + sin(phase) * 0.15 + fi * 0.08;
        // Curtain: a soft sheet hanging from above, edge wobbles.
        let edge_wob = sin(p.x * 6.0 + u.flow * 1.2 + fi * 5.0) * 0.05;
        let sheet = smoothstep(centre + edge_wob + 0.3, centre + edge_wob - 0.35, up);
        // Vertical strands inside the curtain shimmer on the highs.
        let strand = 0.5 + 0.5 * sin(p.x * 40.0 + fi * 9.0 + u.time * 0.7);
        let glow = sheet * (0.3 + strand * 0.7) * (0.4 + u.high * 1.4 + band_amp(fi));
        // Falloff: fade toward the top of the sheet, bounded below.
        let fall = exp(-max(up - centre, 0.0) * 3.0) * exp(-max(centre - up - 0.25, 0.0) * 4.0);
        col += palette(0.35 + fi * 0.12 + u.hue) * glow * fall * 0.35
             * (0.25 + up * 0.75);
    }
    return vec4<f32>(finite(col), 1.0);
}

fn band_amp(i: f32) -> f32 { return spec(i * 0.3 + 0.1) * 0.5; }
