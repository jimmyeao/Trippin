// Petals (accent layer): petals tumbling down through the frame in three
// depths. They fall and spin on the smooth energy clock (motion), the wind
// pushing them swings over eight bars (direction), and each petal's tumble
// flattens and opens it, wider as the slow bass presence swells (shape).

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.0);
    let wind = 0.35 * sin(TAU * u.flow / 32.0);
    for (var layer = 0; layer < 3; layer++) {
        let fl = f32(layer);
        let scale = 2.8 + fl * 1.6;
        let fall = u.clock4.x * (0.09 - 0.02 * fl);
        let q = (p + vec2<f32>(-wind * 0.3, -fall)) * scale;
        let base = floor(q);
        for (var j = -1; j <= 1; j++) {
            for (var i = -1; i <= 1; i++) {
                let cell = base + vec2<f32>(f32(i), f32(j));
                let h = hash22(cell + vec2<f32>(fl * 5.0, 61.0));
                if h.x > 0.13 {
                    continue;
                }
                let sway = 0.25 * sin(u.clock4.x * 0.35 + h.y * TAU + cell.y * 0.7);
                let c = cell + 0.5 + vec2<f32>(sway, (h.y - 0.5) * 0.5);
                let spin = u.clock4.x * 0.3 * (h.y - 0.5) * 2.0 + h.x * 20.0;
                let tumble = cos(u.clock4.x * (0.2 + 0.25 * h.y) + h.x * 30.0);
                let r = rot(spin) * (q - c);
                let len = 0.2 + 0.08 * h.y;
                let wid = len * (0.15 + 0.4 * abs(tumble)) * (1.0 + 0.2 * u.pres4.x);
                // A petal: an ellipse pinched to a point at one end.
                let pinch = 1.0 - 0.45 * smoothstep(-len, len, r.y);
                let e = length(vec2<f32>(r.x / (wid * pinch), r.y / len));
                if e > 2.0 {
                    continue;
                }
                let body = smoothstep(1.0, 0.7, e);
                let edge = exp(-abs(e - 0.85) * 8.0) * 0.25 * smoothstep(2.0, 1.0, e);
                // The side facing the light is brighter.
                let lit = 0.55 + 0.45 * tumble;
                let tone = palette(0.82 + 0.1 * h.y);
                col += tone * tone * (body * 0.45 * lit + edge) * (1.0 - 0.28 * fl);
            }
        }
    }
    return vec4<f32>(finite(col), 1.0);
}
