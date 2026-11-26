// Halo (accent layer): a tilted ring of sparks orbiting the centre of the
// frame, seen in perspective, with the near side brighter and bigger. The
// ring spins on the smooth energy clock (motion), its tilt rocks over eight
// bars (direction), and it widens as the slow bass presence swells (shape).
// Forty sparks, always forty.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let rad = 0.62 + 0.14 * u.pres4.x;
    let tilt = 0.28 + 0.12 * sin(TAU * u.flow / 32.0);
    let roll = 0.18 * sin(TAU * u.flow / 64.0 + 1.0);
    let q = rot(roll) * p;
    // Early out far from the ring's ellipse.
    let e = length(q / vec2<f32>(rad * 1.2, rad * tilt));
    if e < 0.6 || e > 1.4 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    var col = vec3<f32>(0.0);
    let spin = u.clock4.x * 0.08;
    for (var i = 0; i < 40; i++) {
        let fi = f32(i);
        let a = fi / 40.0 * TAU + spin;
        let pos = vec2<f32>(cos(a) * rad * 1.2, sin(a) * rad * tilt);
        // sin(a) > 0 is the near side (lower on screen).
        let near = 0.5 + 0.5 * sin(a);
        let d = length(q - pos);
        let s = 0.007 + 0.008 * near;
        let c = smoothstep(s, s * 0.2, d) + exp(-d / (s * 2.5)) * 0.25;
        let tone = palette(0.35 + 0.4 * fract(fi / 40.0 * 3.0));
        col += mix(tone * tone, vec3<f32>(1.0), 0.3) * c * (0.35 + 0.65 * near);
    }
    // A faint band of light along the orbit.
    col += palette(0.4) * palette(0.4) * exp(-abs(e - 1.0) * 40.0) * 0.08;
    return vec4<f32>(finite(col), 1.0);
}
