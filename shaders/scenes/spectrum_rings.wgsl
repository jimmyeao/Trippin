// The spectrum wrapped into a glowing ring (mirrored, lows at the top). The
// feedback pushes old rings outwards and twists them with the mids.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let r = length(p);
    let a = angle(vec2<f32>(-p.y, p.x));
    let x = abs(a) / PI;
    let s = spec(x);

    let radius = 0.3 + 0.28 * s * (0.5 + u.intensity) + 0.05 * beat_pulse(5.0) * u.bass;
    let d = abs(r - radius);
    var c = palette(x * 0.7 + u.clock4.w * 0.015) * (0.004 / (d + 0.003)) * (0.3 + s);
    c += palette(0.1) * u.high * 0.02 / (r * r + 0.02) * u.onset;

    // Direction: the feedback swirl turns one way, then the other; shape:
    // kicks punch the echoes outward.
    let fb = rot(0.025 * sin(u.clock4.x * 0.03) * (0.4 + u.pres4.y)) * p * (0.985 - 0.025 * u.hits4.x);
    c += prev(uncentred(fb)) * vec3<f32>(0.93, 0.9, 0.95) * (0.85 + 0.08 * u.intensity);
    return vec4<f32>(c, 1.0);
}
