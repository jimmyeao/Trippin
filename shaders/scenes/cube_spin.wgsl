// Wireframe cubes tumbling in space, projected by hand — cheap 3D, no
// raymarching. Cube size pumps with the kick; rotation speeds follow the
// phrase. Old-school demo look, but tinted by the track palette.

fn edge(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    // Distance from p to segment ab.
    let pa = p - a;
    let ba = b - a;
    let t = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * t);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.008, 0.008, 0.02);

    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let scale = 0.35 + fi * 0.28 - u.kick * 0.1;
        let rx = u.flow * (0.4 + fi * 0.15);
        let ry = u.flow * (0.3 + fi * 0.2);

        // Rotate 3D cube verts and project.
        var pts: array<vec2<f32>, 8>;
        for (var v = 0; v < 8; v++) {
            let b = vec3<f32>(
                f32(v & 1) - 0.5, f32((v >> 1) & 1) - 0.5, f32((v >> 2) & 1) - 0.5) * 2.0 * scale;
            // Yaw then pitch (note: world up stays +y of the vertex).
            let vxy = rot(ry) * vec2<f32>(b.x, b.z);
            let vyz = rot(rx) * vec2<f32>(b.y, vxy.y);
            let vv = vec3<f32>(vxy.x, vyz.x, vyz.y);
            // Perspective project — camera at z = -2 looking +z.
            let pr = vv.xy / max(vv.z + 2.2, 0.3);
            // Screen: +p.y is down, so flip y back.
            pts[v] = vec2<f32>(pr.x, -pr.y);
        }

        // 12 edges of the cube.
        var d = 1e5;
        for (var e = 0; e < 12; e++) {
            let ei = array<u32, 24>(0u, 1u, 1u, 3u, 3u, 2u, 2u, 0u,
                                    4u, 5u, 5u, 7u, 7u, 6u, 6u, 4u,
                                    0u, 4u, 1u, 5u, 3u, 7u, 2u, 6u);
            d = min(d, edge(p, pts[ei[e * 2]], pts[ei[e * 2 + 1]]));
        }
        let glow = exp(-d * 160.0) + exp(-d * 20.0) * 0.25;
        col += palette(fi * 0.33 + u.hue) * glow * (0.4 + u.energy * 1.4);
    }
    return vec4<f32>(col, 1.0);
}
