// Radar arcs: concentric arcs sweeping around like a sonar display, each
// ring's sweep position driven by the beat clock, brightness by spectrum
// band. Crisp, technical look for techy sets.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let r = length(p);
    let a = angle(p) / TAU + 0.5; // 0..1 around

    var col = vec3<f32>(0.006, 0.01, 0.02);
    let rings = 6.0;
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        // Shape: each ring swells with its band's presence and ripples on
        // the kick.
        let rr = 0.15 + fi * 0.22 + 0.05 * u.pres4[i % 4] + 0.025 * u.hits4.x * sin(a * TAU * 6.0 + fi * 1.7);
        let band = exp(-abs(r - rr) * 30.0);
        // Sweep head travels around the ring at its own rate.
        // Direction: neighbouring rings sweep opposite ways; speed rides
        // the energy clock.
        let dir = select(1.0, -1.0, (i & 1) == 1);
        let head = fract(dir * (a - u.clock4.x * (0.25 + fi * 0.125)));
        let sweep = smoothstep(0.12, 0.0, head);
        let trail = smoothstep(0.45, 0.0, head) * 0.3;
        let amp = spec(fi / rings);
        col += palette(fi / rings + u.hue) * band * (sweep + trail) * (0.3 + amp * 2.0);
        // Faint full ring.
        col += palette(fi / rings + u.hue) * band * 0.06;
    }
    col += palette(0.5 + u.hue) * exp(-r * 8.0) * (0.3 + u.kick);
    return vec4<f32>(col, 1.0);
}
