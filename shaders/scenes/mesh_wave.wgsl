// An ocean of glowing nodes: a perspective dot-mesh plane whose surface
// heaves with travelling waves — wave height rides the spectrum, swells
// pass through on the kick. Cheap projection, no marching.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.006, 0.008, 0.02);

    // Camera looking slightly down at a node plane. Screen y → depth z.
    let horizon = -0.25;
    if (p.y > horizon) {
        // Reconstruct plane coords: near bottom = close.
        let rel = p.y - horizon;
        let z = 0.12 / max(rel, 0.002); // depth into the scene
        let wx = p.x * z * 4.0;
        let wz = z * 6.0 - u.flow * 1.2;

        // Wave field: swells travel across it, amplitude rides bass.
        let swell = sin(wx * 1.5 + u.flow * 2.0) * cos(wz * 0.8 + u.flow * 0.7)
                  * (0.3 + u.bass * 0.7 + u.kick * 0.3);

        // Nodes at grid intersections — draw glow at each lattice point.
        let g = vec2<f32>(wx, wz);
        let cell = fract(g) - 0.5;
        // Nodes bulge where the swell is high.
        let node_r = 0.06 + swell * 0.04;
        let node = smoothstep(node_r, 0.0, length(cell));
        let fade = clamp(1.0 - z * 0.09, 0.15, 1.0); // dim to horizon
        let band = spec(fract(wx * 0.06 + 0.5));
        col += palette(fract(wz * 0.05) + u.hue) * node * fade
             * (0.4 + band * 1.4 + swell * 0.8);

        // Faint connecting lines.
        let lx = smoothstep(0.02, 0.0, abs(cell.x));
        let ly = smoothstep(0.02, 0.0, abs(cell.y));
        col += palette(0.6 + u.hue) * max(lx, ly) * fade * 0.06;
    }
    // Horizon glow.
    col += palette(0.65 + u.hue) * exp(-abs(p.y - horizon) * 16.0) * 0.3;
    return vec4<f32>(finite(col), 1.0);
}
