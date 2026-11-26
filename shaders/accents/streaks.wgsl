// Streaks (accent layer): soft anamorphic light streaks, like lens flares off
// a passing light, gliding back and forth across the frame. Each glides on
// its own 4-8 bar sine of the smooth beat clock (direction swings with the
// phrase), and their length follows the slow mid presence (shape). Six
// streaks, always six.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let asp = aspect();
    var col = vec3<f32>(0.0);
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let h = hash22(vec2<f32>(fi, 3.7));
        let period = select(16.0, 32.0, h.x > 0.5);
        let y = (h.y - 0.5) * 1.5 + 0.08 * sin(TAU * u.flow / 64.0 + fi);
        let x = asp * 0.95 * sin(TAU * u.flow / period + fi * 1.7);
        let len = (0.35 + 0.5 * u.pres4.y) * (0.6 + 0.8 * h.x);
        let dx = (p.x - x) / len;
        let dy = abs(p.y - y);
        let along = exp(-dx * dx * 2.5);
        let core = exp(-dy * 260.0);
        let halo = exp(-dy * 32.0) * 0.18;
        let hot = exp(-(dx * dx * 40.0 + dy * dy * 4000.0)) * 0.8;
        let c = palette(0.1 + fi * 0.13);
        col += (c * c * (core * 1.4 + halo) * along + vec3<f32>(hot)) * 1.1;
    }
    return vec4<f32>(finite(col), 1.0);
}
