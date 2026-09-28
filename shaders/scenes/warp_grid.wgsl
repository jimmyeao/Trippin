// A tron-style ground grid rushing toward the camera — horizon at the
// vertical centre, lines brighten and the grid "bounces" on the kick.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);

    // Sky half: dark gradient with a glow band at the horizon.
    var col = mix(vec3<f32>(0.02, 0.0, 0.05), vec3<f32>(0.0, 0.0, 0.01),
                  clamp(-p.y + 0.5, 0.0, 1.0));
    col += palette(0.9 + u.hue) * exp(-abs(p.y) * 12.0) * (0.3 + u.kick * 0.8);

    // Ground: only below the horizon (p.y > 0 is down-screen).
    if (p.y > 0.0) {
        // Perspective: depth grows as we approach the horizon.
        let z = 0.3 / max(p.y, 0.005);       // fake perspective depth
        let g = vec2<f32>(p.x * z * 3.0, z * 2.0 - u.flow * 1.5);
        let fx_ = abs(fract(g.x) - 0.5);
        let fy_ = abs(fract(g.y) - 0.5);
        let line = smoothstep(0.48, 0.5, max(fx_, fy_));
        let fade = exp(-p.y * 2.5);          // dim into the distance
        let band = spec(fract(p.x / aspect() * 0.5 + 0.5));
        col += mix(palette(0.5 + u.hue), palette(0.0 + u.hue), band)
             * line * fade * (0.5 + band * 1.2 + u.kick * 0.8);
        // Ground fill below the grid.
        col = mix(col, vec3<f32>(0.01, 0.002, 0.03), p.y * 0.8);
    }
    return vec4<f32>(finite(col), 1.0);
}
