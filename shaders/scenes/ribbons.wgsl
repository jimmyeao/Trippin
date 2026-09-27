// Ribbons: broad silk bands weaving across the screen. Each ribbon is a real
// strip — soft falloff edges, a lit spine down its length, a hue gradient
// along the flow — breathing with its own slice of the spectrum.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    var col = vec3<f32>(0.0);

    // A faint wash of the dominant hue so the bands never float on black.
    col += palette(u.hue) * 0.03 * (0.4 + u.energy);

    for (var i = 0; i < 4; i++) {
        let fi = f32(i);
        let band = fi / 4.0;
        // Each ribbon listens to a different spectrum region.
        let v = spec(uv.x * 0.4 + band * 0.6);

        // The strip's centre line: a travelling wave, amplitude driven by
        // its band plus a slow communal sway.
        let ph = u.flow * (0.5 + fi * 0.17) + fi * 1.9;
        let yc = 0.22 + fi * 0.19
            + sin(uv.x * (2.2 + fi * 0.7) + ph) * (0.03 + v * 0.10)
            + sin(uv.x * (5.5 + fi) - ph * 1.4 + 2.0) * 0.015;
        let d = uv.y - yc;

        // Width breathes with the band; a bright spine rides the middle.
        let w = 0.030 + v * 0.05;
        let edge = 1.0 - smoothstep(w * 0.55, w, abs(d));
        let spine = exp(-abs(d) * (90.0 - v * 40.0)) * 0.6;

        // Colour flows along the ribbon: a slow hue drift keyed to x.
        let tint = palette(band * 0.55 + uv.x * 0.12 + u.hue * 0.3);
        // Shade across the strip for a silk-sheen look.
        let sheen = 0.55 + 0.45 * cos(d / max(w, 0.01) * PI * 0.5);
        col += tint * edge * sheen * (0.35 + v * 1.1);
        col += palette(band + 0.45) * spine * (0.3 + v * 0.9);
    }

    // Beat shimmer: a soft brightness pinch on each kick, never a flash.
    col *= 0.9 + u.kick * 0.3;
    // Gentle trailing so the bands leave silk ghosts behind them.
    col += prev(uncentred(centred(uv) * 0.997)) * 0.12;
    return vec4<f32>(finite(col), 1.0);
}
