// Synth-city flyover: neon building outlines stream past in fake
// perspective — cheap 3D via screen-space scaling, no marching. Skyline
// blocks rise with the bass, windows flicker on the highs.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let up = -p.y; // world up

    // Sky gradient + low sun glow.
    var col = mix(palette(u.hue) * 0.4, vec3<f32>(0.01, 0.005, 0.03),
                  smoothstep(0.0, 0.9, up + 0.2));
    col += palette(0.05 + u.hue) * exp(-abs(up + 0.05) * 8.0) * (0.3 + u.energy * 0.4);

    // Below the horizon: buildings receding toward centre, sliced by depth.
    if up < -0.1 {
        let depth = -(up + 0.1); // 0 at horizon, growing downward on screen
        let persp = 0.15 / max(depth, 0.02);
        // City columns scrolling toward the viewer via flow.
        let lane = p.x * persp;
        let zz = depth * 4.0 + u.flow * 0.8;
        let cell_x = floor(lane * 4.0);
        let cell_z = floor(zz);
        let h = hash21(vec2<f32>(cell_x, cell_z));
        let lx = fract(lane * 4.0);
        let lz = fract(zz);

        // Building block: lit edges + window grid.
        let bw = 0.6 + h * 0.3;
        let bx = step(abs(lx - 0.5), bw * 0.5);
        let outline = max(
            smoothstep(0.05, 0.02, abs(lx - 0.5 - bw * 0.5)),
            smoothstep(0.05, 0.02, abs(lx - 0.5 + bw * 0.5)));
        let win = step(0.92, hash21(floor(vec2<f32>(lane * 40.0, zz * 20.0)) + cell_x))
                * step(fract(lane * 40.0), 0.6) * step(fract(zz * 20.0), 0.6);
        let fade = exp(-depth * 6.0);
        let bc = palette(h * 0.6 + u.hue);
        col += bc * bx * outline * fade * (0.4 + u.bass * 1.2);
        col += vec3<f32>(0.9, 0.85, 0.6) * bx * win * fade * (0.2 + u.high * 0.8);
    }
    // Horizon flash on drops.
    col += palette(0.5 + u.hue) * exp(-abs(up + 0.1) * 30.0) * u.onset * 0.8;
    return vec4<f32>(col, 1.0);
}
