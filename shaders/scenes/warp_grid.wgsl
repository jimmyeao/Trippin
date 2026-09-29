// A tron-style ground plane rushing under the camera — bright neon grid,
// glowing horizon line, spectrum-lit line colour. The grid "kicks"
// toward the viewer on each beat.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);

    // Sky: gradient with a bright horizon glow.
    var col = mix(vec3<f32>(0.04, 0.01, 0.08), vec3<f32>(0.0, 0.0, 0.015),
                  clamp(-p.y * 1.5 + 0.5, 0.0, 1.0));
    col += palette(0.9 + u.hue) * exp(-abs(p.y) * 14.0) * (0.6 + u.kick * 1.2);

    if (p.y > 0.005) {
        // Perspective depth: 1/y, clamped so the horizon isn't infinite.
        let z = clamp(0.18 / p.y, 0.0, 12.0);
        // Energy: rush speed follows the mix; direction: the grid slides
        // sideways, then back (a banking run).
        let g = vec2<f32>(p.x * z * 5.0 + 4.0 * sin(u.clock4.x * 0.02), z * 3.0 - u.clock4.x * 3.0);
        // Line thickness grows with depth so screen width stays visible.
        let th = 0.03 + z * 0.045;
        let lx = smoothstep(0.5 - th, 0.5, abs(fract(g.x) - 0.5));
        let ly = smoothstep(0.5 - th, 0.5, abs(fract(g.y) - 0.5));
        let band = spec(fract(p.x / aspect() * 0.5 + 0.5));
        let line_col = mix(palette(0.5 + u.hue), palette(0.85 + u.hue), band);
        // Bright near horizon (converge), taper at the very bottom.
        let gain = (0.6 + band * 1.4 + u.hits4.x * 0.6) * (0.5 + z * 0.12);
        col += line_col * (lx + ly) * gain;
        // Dark ground fill between lines.
        col = mix(col, vec3<f32>(0.008, 0.004, 0.03),
                  (1.0 - max(lx, ly)) * clamp(p.y * 2.0, 0.0, 0.9));
        // Haze pooling at the horizon.
        col += palette(0.7 + u.hue) * exp(-p.y * 9.0) * 0.25;
    }
    return vec4<f32>(finite(col), 1.0);
}
