// A slow drive past a night skyline — layered silhouettes with sparse
// warm windows (soft glowing panes, not speckle) that flash on beat and
// breathe with the band. Parallax scrolls steadily; audio lights the city.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let up = -p.y * 0.5 + 0.5; // 0 bottom, 1 top

    // Sky: deep gradient, moon low left.
    var col = mix(vec3<f32>(0.01, 0.008, 0.03), vec3<f32>(0.0, 0.002, 0.012), up);
    let moon = length(p - vec2<f32>(-0.7, -0.5));
    col += vec3<f32>(0.7, 0.75, 0.9) * (exp(-moon * 6.0) * 0.4
          + smoothstep(0.09, 0.07, moon) * 0.6);

    // Three parallax layers — far/mid/near scroll at different speeds.
    for (var layer = 0; layer < 3; layer++) {
        let fl = f32(layer);
        let depth = fl / 3.0;           // 0 far … ~0.67 near
        let scroll = u.flow * mix(0.01, 0.05, depth);
        let bcols = mix(28.0, 12.0, depth);
        let bx = floor((p.x / aspect() + 0.5 + scroll) * bcols);
        let fx = fract((p.x / aspect() + 0.5 + scroll) * bcols);
        let bh = hash21(vec2<f32>(bx, f32(layer) * 17.0));
        let h = 0.18 + bh * mix(0.2, 0.45, depth);
        let bw = 0.55 + bh * 0.3;       // building width fraction

        // Building silhouette — dark towers against the sky.
        let body = step(up, h) * step(abs(fx - 0.5), bw * 0.5);
        let shade = mix(0.16, 0.03, depth) * (0.7 + up * 0.5);
        col = mix(col, vec3<f32>(shade * 0.5, shade * 0.6, shade * 1.3),
                  body * 0.97);

        // Windows: only the two nearest layers, sparse and soft.
        if (layer >= 1) {
            let wr = floor(up * 24.0);
            let wc = floor(fx * 6.0);
            let on = step(0.82, hash21(vec2<f32>(bx * 6.0 + wc, wr + f32(layer) * 99.0)));
            let pane = smoothstep(0.0, 0.15, fract(fx * 6.0))
                     * smoothstep(1.0, 0.85, fract(fx * 6.0))
                     * smoothstep(0.0, 0.25, fract(up * 24.0))
                     * smoothstep(1.0, 0.75, fract(up * 24.0));
            let band = spec(fract(fx * 2.0 + fl * 0.3));
            let flick = 0.5 + 0.5 * sin(u.time * (1.0 + bh) + bh * 40.0);
            // Soft glowing panes + halo, hot when their band is loud.
            let glow = pane * on * body * (0.25 + band * 1.2 + flick * 0.2);
            col += vec3<f32>(1.0, 0.75, 0.45) * glow;
            col += vec3<f32>(0.9, 0.6, 0.3) * on * body * 0.03 * band;
        }
        // Roofline glow riding the beat on the nearest layer.
        if (layer == 2) {
            let rim = exp(-abs(up - h) * 70.0) * step(abs(fx - 0.5), bw * 0.5);
            col += palette(0.05 + u.hue) * rim * (0.4 + u.kick * 1.5);
        }
    }
    return vec4<f32>(finite(col), 1.0);
}
