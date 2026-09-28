// Horizontal light bands rolling like a calm sea at night — a slow,
// elegant scene for warm-ups. Each band's crest height rides a spectrum
// bin; the sea swells with the bass but stays smooth.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.008, 0.01, 0.025);

    let lines = 12.0;
    for (var i = 0; i < 12; i++) {
        let fi = f32(i);
        let depth = fi / lines;
        // Bands stack from bottom of frame (+p.y = down) upward.
        let base = 0.9 - depth * 1.4;
        let amp = (0.03 + u.bass * 0.06) * (0.5 + spec(depth));
        let yw = base - depth * 0.1
               + sin(p.x * (4.0 + fi * 0.6) + u.flow * (0.4 + fi * 0.12) + fi) * amp
               + sin(p.x * 9.0 - u.flow * (0.6 + fi * 0.09)) * amp * 0.5;
        let dy = p.y - yw;
        let glow_line = exp(-dy * dy * 900.0);
        let soft = exp(-abs(dy) * 14.0) * 0.15;
        let lc = palette(depth * 0.7 + u.hue);
        col += lc * (glow_line * (0.4 + u.energy * 1.2) + soft);
    }
    col += prev(in.uv) * 0.2;
    return vec4<f32>(col, 1.0);
}
