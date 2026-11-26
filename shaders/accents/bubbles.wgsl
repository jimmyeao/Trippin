// Bubbles (accent layer): soap bubbles rising through the frame in two
// depths, each a thin iridescent rim with a window highlight. They rise on
// the smooth energy clock and wobble side to side (motion), their path
// leans over eight bars (direction), and they grow and squash with the slow
// bass presence (shape). A fixed share of cells: the count never moves.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.0);
    for (var layer = 0; layer < 2; layer++) {
        let fl = f32(layer);
        let scale = 1.8 + fl * 1.4;
        let lean = 0.15 * sin(TAU * u.flow / 32.0 + fl);
        let rise = u.clock4.x * (0.07 - 0.02 * fl);
        let q = (p + vec2<f32>(lean, rise)) * scale;
        let base = floor(q);
        for (var j = -1; j <= 1; j++) {
            for (var i = -1; i <= 1; i++) {
                let cell = base + vec2<f32>(f32(i), f32(j));
                let h = hash22(cell + vec2<f32>(fl * 9.0, 31.0));
                if h.x > 0.13 {
                    continue;
                }
                let wob = 0.15 * sin(u.clock4.x * 0.4 + h.y * TAU + cell.y);
                let c = cell + 0.5 + vec2<f32>(wob + (h.y - 0.5) * 0.3, (h.x / 0.13 - 0.5) * 0.5);
                let rel = q - c;
                let r = (0.2 + 0.12 * h.y) * (1.0 + 0.15 * u.pres4.x);
                // A slight squash that breathes with the bass presence.
                let sq = 1.0 + 0.08 * u.pres4.x * sin(u.time * 2.0 + h.x * 20.0);
                let d = length(rel * vec2<f32>(sq, 1.0 / sq));
                if d > r * 1.5 {
                    continue;
                }
                let rim = exp(-abs(d - r) * 90.0) + exp(-abs(d - r) * 25.0) * 0.2;
                let inner = smoothstep(r, r * 0.5, d) * 0.05;
                // A window highlight up and to the left.
                let hl = exp(-length(rel - vec2<f32>(-0.42, -0.42) * r) * 60.0);
                // Thin-film colour runs round the rim.
                let film = palette(0.2 + 0.25 * rel.y / r + h.y * 0.4 + u.flow * 0.01);
                col += (film * film * (rim * 0.6 + inner) + vec3<f32>(hl * 0.6)) * (1.0 - 0.35 * fl);
            }
        }
    }
    return vec4<f32>(finite(col), 1.0);
}
