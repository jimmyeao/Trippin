// Bokeh (accent layer): big, soft out-of-focus discs of light drifting
// across the frame in two depths, each with the brighter rim a real lens
// gives. They drift on the smooth energy clock (motion), their path bends
// over eight bars (direction), and they swell with the slow bass presence
// (shape). Few and large: an overlay of light, not confetti.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.0);
    for (var layer = 0; layer < 2; layer++) {
        let fl = f32(layer);
        let scale = 1.6 + fl * 1.1;
        let bend = 0.3 * sin(TAU * u.flow / 32.0 + fl * 2.0);
        let t = u.clock4.x * (0.05 + 0.03 * fl);
        var q = (p + vec2<f32>(t, -0.3 * t + bend * 0.3)) * scale;
        let base = floor(q);
        for (var j = -1; j <= 1; j++) {
            for (var i = -1; i <= 1; i++) {
                let cell = base + vec2<f32>(f32(i), f32(j));
                let h = hash22(cell + vec2<f32>(fl * 17.0, 2.0));
                if h.x > 0.22 {
                    continue;
                }
                let c = cell + 0.5 + (hash22(cell + vec2<f32>(5.0, fl * 3.0)) - 0.5) * 0.7;
                let d = length(q - c);
                let r = (0.22 + 0.16 * h.y) * (1.0 + 0.18 * u.pres4.x);
                // Flat disc with a brighter rim and a soft edge.
                let disc = smoothstep(r, r * 0.9, d);
                let rim = exp(-abs(d - r * 0.9) * 40.0) * 0.6;
                let tone = palette(0.15 + h.y * 0.5 + fl * 0.2);
                col += tone * tone * (disc * 0.16 + rim * 0.2) * (1.0 - 0.3 * fl);
            }
        }
    }
    return vec4<f32>(finite(col), 1.0);
}
