// Concentric rings fired outward on every beat — a visual echo of the
// kick drum. Rings accumulate through feedback so the floor feels like it
// has memory.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var p = centred(in.uv);
    // Direction: the whole field tilts one way then the other; kicks
    // squash it (shape).
    p = rot(sin(u.clock4.x * 0.03) * 0.4) * p;
    p = p * vec2<f32>(1.0 + 0.12 * u.hits4.x, 1.0 - 0.12 * u.hits4.x);
    let r = length(p);

    // Ring train: phase advances a ring outward each beat.
    // Shape: ring spacing tightens as the bass builds; energy: they
    // travel faster.
    let phase = r * (1.1 + 0.8 * u.pres4.x) - u.clock4.x * 0.5;
    let ring = exp(-abs(fract(phase) - 0.12) * 14.0);
    let ring2 = exp(-abs(fract(phase * 0.5) - 0.1) * 10.0) * 0.5;

    let fade = exp(-r * 1.3);
    var col = vec3<f32>(0.008, 0.006, 0.018);
    col += palette(r * 0.5 + u.beat * 0.03 + u.hue) * ring * fade * (0.5 + u.energy * 1.8);
    col += palette(0.5 + u.hue) * ring2 * fade * u.mid;

    // Hot centre pulse.
    col += palette(u.hue) * exp(-r * 5.0) * (0.3 + u.kick * 2.0);

    // Feedback: gentle zoom keeps past rings drifting outward.
    let trail = prev(uncentred(centred(in.uv) * 0.99));
    col = max(col * 0.98, trail * 0.94);
    return vec4<f32>(col, 1.0);
}
