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
    col += beam * (core * 1.8 + glow) * (0.5 + u.energy * 0.9 + abs(w));

    // Faint spectrum envelope behind the beam for context.
    let env = 0.5 + (spec(uv.x) - 0.3) * 0.5;
    let de = (uv.y - env) * 24.0;
    col += palette(0.7) * exp(-de * de) * 0.10;

    // Mirrored ghost below centre — reads like the second channel.
    let d2 = (uv.y - (0.5 - w)) * 40.0;
    col += beam * exp(-d2 * d2) * 0.22;

    // CRT graticule + centre line.
    let gx = smoothstep(0.0035, 0.0, abs(fract(uv.x * 10.0 + 0.5) - 0.5) - 0.46);
    let gy = smoothstep(0.0035, 0.0, abs(fract(uv.y * 8.0 + 0.5) - 0.5) - 0.46);
    col += vec3<f32>(0.03, 0.06, 0.05) * max(gx, gy);
    col += vec3<f32>(0.05, 0.10, 0.07) * exp(-abs(uv.y - 0.5) * 150.0);

    // The whole tube flashes faintly on the kick.
    col += beam * u.kick * 0.05;

    return vec4<f32>(finite(col), 1.0);
}
