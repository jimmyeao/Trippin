// Two ring gratings sliding against each other — classic moiré
// interference. The centres breathe apart with the bass and the whole
// interference figure rotates slowly.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var p = centred(in.uv);
    // Direction: the rings turn one way, then back.
    p = rot(1.2 * sin(u.clock4.x * 0.02)) * p;

    // Shape: the two centres drift apart as the bass builds.
    let sep = 0.25 + u.pres4.x * 0.5 + 0.05 * sin(u.clock4.z * 0.2);
    let d1 = length(p - vec2<f32>(sep, 0.0));
    let d2 = length(p + vec2<f32>(sep, 0.0));

    let freq = 60.0 + u.mid * 50.0;
    let g1 = sin(d1 * freq - u.clock4.x * 3.0);
    let g2 = sin(d2 * freq + u.clock4.z * 2.0);
    let m = g1 * g2 * 0.5 + 0.5;

    // Interference bands are sharp where the gratings differ most.
    let band = smoothstep(0.2, 0.9, m);
    let glow = exp(-abs(d1 - d2) * 6.0) * 0.3;

    var col = vec3<f32>(0.01, 0.01, 0.025);
    col += palette(d1 * 0.25 + u.hue) * band * (0.3 + u.energy * 1.3);
    col += palette(0.5 + u.hue) * glow * beat_pulse(4.0);
    col += prev(in.uv) * 0.15;
    return vec4<f32>(col, 1.0);
}
