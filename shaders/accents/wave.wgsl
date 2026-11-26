// Wave (accent layer): the music's own waveform drawn as three glowing
// ribbons across the frame. The ribbons' shape is the waveform (shape), they
// tilt on a phrase-long sine (direction) and their slow vertical drift rides
// the smooth energy clock (motion). Real music sits around +-0.1-0.3, so the
// trace is gained by the slow presence rather than drawn raw.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.0);
    let tilt = 0.12 * sin(TAU * u.flow / 32.0);
    let gain = 0.5 / (0.15 + 0.6 * max(u.pres4.x, u.pres4.y));
    let x = in.uv.x;
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let off = (fi - 1.0) * 0.06;
        let y0 = tilt * p.x + 0.04 * sin(u.clock4.x * 0.15 + fi * 2.0) + off;
        // Taper at the edges, so the ribbon fades rather than stops.
        let taper = smoothstep(0.0, 0.2, x) * smoothstep(1.0, 0.8, x);
        let w = clamp(wave(fract(x + fi * 0.08)) * gain, -1.0, 1.0);
        let y = y0 + w * 0.3 * taper * (1.0 - 0.25 * fi);
        let d = abs(p.y - y);
        let line = exp(-d * 140.0) + exp(-d * 25.0) * 0.15;
        let tone = palette(0.45 + fi * 0.15 + x * 0.2);
        col += tone * tone * line * taper * (0.8 - 0.2 * fi);
    }
    return vec4<f32>(finite(col), 1.0);
}
