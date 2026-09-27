// Levels: three fat horizontal meters — bass, mid, high — stacked like a
// console, each tipped by a peak marker, framed like a giant mixer's bridge.

fn meter(uv: vec2<f32>, y0: f32, h: f32, level: f32, tint: f32) -> vec3<f32> {
    if uv.y < y0 || uv.y > y0 + h {
        return vec3<f32>(0.0);
    }
    // Segment ticks.
    let ticks = 40.0;
    let t = fract(uv.x * ticks);
    let tick = step(0.08, t) * step(t, 0.92);
    let lit = step(uv.x, level * 0.96 + 0.02);
    // Green -> amber -> red as the bar fills.
    var c = mix(vec3<f32>(0.1, 0.8, 0.3), vec3<f32>(1.0, 0.65, 0.15), step(0.55, uv.x));
    c = mix(c, vec3<f32>(1.0, 0.2, 0.2), step(0.82, uv.x));
    c = mix(c, palette(tint), 0.3);
    var col = c * lit * tick * (0.35 + level * 0.9);
    col += c * 0.05 * tick; // unlit segments barely glow
    // Peak marker.
    col += vec3<f32>(1.0) * smoothstep(0.015, 0.0, abs(uv.x - level)) * 0.7;
    return col;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Brushed dark console background.
    var col = vec3<f32>(0.025, 0.02, 0.03);
    col += palette(0.6) * 0.05 * exp(-abs(uv.y - 0.5) * 2.0) * (0.4 + u.energy);

    col += meter(uv, 0.62, 0.16, u.bass, 0.05);
    col += meter(uv, 0.42, 0.16, u.mid, 0.35);
    col += meter(uv, 0.22, 0.16, u.high, 0.65);

    // Spectrum strip along the bottom edge for detail.
    let strip = step(0.90, uv.y);
    col += palette(uv.x * 0.8) * strip * spec(uv.x) * 0.5;

    return vec4<f32>(finite(col), 1.0);
}
