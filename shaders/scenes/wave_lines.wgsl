// A stack of glowing waveform ribbons — like a dozen oscilloscopes at
// once, each band offset and phase-shifted. Calm but alive; good for
// groove sections.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.006, 0.008, 0.02);

    let lines = 9.0;
    for (var i = 0; i < 9; i++) {
        let fi = f32(i);
        let y = (fi / (lines - 1.0) - 0.5) * 1.6;
        // Each line samples the wave at a different zoom/offset.
        let w = wave(fract((p.x / aspect() + 1.0) * 0.5 + fi * 0.11)) * (0.10 + u.mid * 0.16);
        let wob = sin(p.x * (3.0 + fi) + u.flow * (1.0 + fi * 0.2)) * 0.02 * (0.5 + u.bass);
        let dy = p.y - (y + w + wob);
        let trace = exp(-abs(dy) * 90.0);
        let halo = exp(-abs(dy) * 10.0) * 0.1;
        let lc = palette(fi / lines + u.hue);
        col += lc * (trace * (0.5 + u.energy) + halo);
    }
    col += prev(in.uv) * 0.2;
    return vec4<f32>(col, 1.0);
}
