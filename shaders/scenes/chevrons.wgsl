// Chevrons: nested angular bands racing toward the centre, each band's
// brightness a spectrum slice — a tunnel made of arrowheads.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * 0.6;
    let chev = abs(p.x) + abs(p.y) * 0.7;    // diamond-ish distance

    let n = 10.0;
    let scroll = u.flow * 0.35 + u.kick * 0.05;
    let cell = fract(chev * n - scroll);
    let band_i = floor(chev * n - scroll);
    let seg = abs(cell - 0.5);

    // Brightness = spectrum at this ring's slot + the kick riding the front.
    let v = spec(band_i / n * 0.6 + 0.05);
    let line = smoothstep(0.14, 0.04, seg);

    var col = palette(band_i / n + u.flow * 0.02 + u.hue) * line * (0.2 + v * 1.2);

    // The leading edge of each chevron flashes on the beat.
    let front = smoothstep(0.06, 0.0, seg) * beat_pulse(6.0);
    col += vec3<f32>(1.0, 0.95, 0.8) * front * 0.4;

    // Centre glow.
    col += palette(0.5) * exp(-chev * 6.0) * (0.2 + u.bass * 0.7);

    col *= smoothstep(1.5, 0.6, chev);

    return vec4<f32>(finite(col), 1.0);
}
