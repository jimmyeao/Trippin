// Lissajous figure: a glowing loop traced by two oscillators whose frequencies
// follow the low and high bands; phase drifts with the beat. Feedback trails
// smear the curve into ribbons.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Direction: the figure tilts one way then the other.
    let p = rot(0.6 * sin(u.clock4.x * 0.025)) * centred(in.uv) * 0.62;

    // Trails: short, so the figure stays crisp while it morphs.
    var col = prev(in.uv * 0.996 + vec2<f32>(0.002, 0.0)) * 0.78;

    // Oscillator ratio from slow band *presence* (not the raw spectrum, which
    // flipped the figure every few frames). Integer ratios close the curve.
    let a = 2.0 + floor(u.pres4.x * 4.0 + 0.5);
    let b = 3.0 + floor(u.pres4.z * 4.0 + 0.5);
    // Shape: the phase (and so the figure) morphs with the energy clock.
    let ph = u.clock4.z * 0.4;

    // Distance to the curve as a polyline (segments between samples), so
    // fast figures stay continuous instead of breaking into dots.
    var d = 10.0;
    var along = 0.0;
    var prev_q = vec2<f32>(sin(ph), 0.0) * 0.42;
    for (var i = 1; i <= 160; i++) {
        let t = f32(i) / 160.0 * TAU;
        let q = vec2<f32>(sin(a * t + ph), sin(b * t)) * 0.42;
        let pa = p - prev_q;
        let ba = q - prev_q;
        let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-8), 0.0, 1.0);
        let dd = length(pa - ba * h);
        if dd < d {
            d = dd;
            along = (f32(i) - 1.0 + h) / 160.0;
        }
        prev_q = q;
    }
    let line = exp(-d * 180.0);
    let glow = exp(-d * 70.0) * 0.025;
    // Colour runs along the curve; kicks brighten the line.
    col += palette(along + u.hue) * (line * (0.8 + 0.8 * u.hits4.x) + glow);

    // A tracer dot at the head of the curve.
    let head = vec2<f32>(sin(a * ph * 0.1 + ph), sin(b * ph * 0.1)) * 0.42;
    col += vec3<f32>(1.0, 0.95, 0.8) * exp(-length(p - head) * 40.0) * (0.3 + u.energy * 0.5);
    return vec4<f32>(finite(col), 1.0);
}
