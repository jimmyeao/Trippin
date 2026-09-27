// Waveform: the actual audio drawn as a phosphor oscilloscope — the real
// time-domain trace (not the spectrum), beam core + bloom, a faint spectrum
// envelope behind it, graticule, and phosphor persistence from feedback.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Phosphor persistence: the previous frame fades fast.
    var col = prev(uv) * 0.82;

    // The beam: centre line driven by the live PCM trace.
    let w = wave(uv.x) * 0.38 * (0.7 + u.intensity * 0.6);
    let y = 0.5 + w;
    let d = uv.y - y;
    let core = exp(-d * d * 22000.0);
    let glow = exp(-d * d * 800.0) * 0.35;

    let beam = palette(u.hue + 0.33 + uv.x * 0.06);
    col += beam * (core * 2.2 + glow * 1.2) * (0.5 + u.energy * 0.9 + abs(w));

    // Faint spectrum envelope behind the beam for context.
    let env = 0.5 + (spec(uv.x) - 0.3) * 0.5;
    let de = (uv.y - env) * 24.0;
    col += palette(0.7) * exp(-de * de) * 0.10;

    // Mirrored ghost below centre — reads like the second channel.
    let d2 = (uv.y - (0.5 - w)) * 40.0;
    col += beam * exp(-d2 * d2) * 0.22;

    // CRT graticule: thin lines on the 10x8 divisions. In cell space the
    // distance to the nearest line is 0.5 - abs(fract - 0.5); the previous
    // version inverted that and filled the cell interiors instead.
    let gx = smoothstep(0.006, 0.001, 0.5 - abs(fract(uv.x * 10.0) - 0.5));
    let gy = smoothstep(0.010, 0.002, 0.5 - abs(fract(uv.y * 8.0) - 0.5));
    col += vec3<f32>(0.05, 0.11, 0.08) * max(gx, gy);
    // Centre axes slightly brighter, like a real scope.
    col += vec3<f32>(0.05, 0.10, 0.07) * exp(-abs(uv.y - 0.5) * 150.0);
    col += vec3<f32>(0.05, 0.10, 0.07) * exp(-abs(uv.x - 0.5) * 150.0) * 0.4;

    // The whole tube flashes faintly on the kick.
    col += beam * u.kick * 0.05;

    return vec4<f32>(finite(col), 1.0);
}
