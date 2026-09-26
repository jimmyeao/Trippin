// Helix: a double strand spiralling upward — each rung's length is a spectrum
// band. Bass rungs are thick and slow, high rungs quick and thin.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let p = vec2<f32>((uv.x - 0.5) * aspect(), uv.y);

    var col = vec3<f32>(0.0);

    // Helix scrolls upward with the beat clock.
    let scroll = u.flow * 0.35;
    let rungs = 14.0;
    for (var i = 0; i < 14; i++) {
        let fi = f32(i);
        let y0 = fract((fi + scroll) / rungs) * 1.2 - 0.1;
        let band = fi / rungs;
        let v = spec(band * 0.8 + 0.05);

        // Two strands at phase 0 and pi.
        let ph = fi * 0.55 + scroll * 2.0;
        let x1 = sin(ph) * 0.28;
        let x2 = sin(ph + PI) * 0.28;

        // Strand glow dots.
        let d1 = length(p - vec2<f32>(x1, y0));
        let d2 = length(p - vec2<f32>(x2, y0));
        let sz = 0.008 + v * 0.012;
        col += palette(band + u.hue) * (exp(-d1 * 60.0) + exp(-d2 * 60.0)) * (0.4 + v);
        col += palette(band + 0.5) * smoothstep(sz * 2.0, 0.0, min(d1, d2)) * (0.5 + v);

        // Rung between strands: length fades with depth of the twist.
        let rung_d = abs(p.y - y0) + max(abs(p.x) - abs(mix(x1, x2, clamp((p.y - y0) * 60.0 + 0.5, 0.0, 1.0))), 0.0);
        let depth = 0.5 + 0.5 * cos(ph);
        let rung = smoothstep(0.006, 0.0, rung_d) * step(abs(p.x), abs(x1 - x2) * 0.5 + 0.01);
        col += palette(band + 0.75) * rung * (0.2 + v * 0.8) * (0.4 + depth * 0.6);
    }

    // Soft column of light the helix lives in.
    col += palette(0.55) * exp(-abs(p.x) * 4.0) * 0.08 * (0.5 + u.energy);

    return vec4<f32>(finite(col), 1.0);
}
