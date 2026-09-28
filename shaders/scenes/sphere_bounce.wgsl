// Bouncing orbs on a floor plane — each sphere hops on its own subdivision
// of the beat, squash-and-stretch on landing. Clean pseudo-3D (analytic,
// no marching). The "floor" shadow grounds them so they read as real.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // World coords: up = -p.y.
    var col = vec3<f32>(0.01, 0.008, 0.025);

    // Floor horizon low in the frame; slight perspective grid.
    let horizon = -0.35; // world-y of floor line (screen +y is down → floor is +p.y side)
    if -p.y < horizon {
        let depth = (horizon - (-p.y));
        let gp = p * vec2<f32>(1.0, 1.0) / max(depth, 0.02);
        let g = max(smoothstep(0.05, 0.02, abs(fract(gp.x * 0.5 + u.flow * 0.2) - 0.5)),
                    smoothstep(0.05, 0.02, abs(fract(gp.y * 0.5) - 0.5)));
        col += palette(0.6 + u.hue) * g * exp(-depth * 3.0) * 0.4;
    }

    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let h = hash21(vec2<f32>(fi, 4.0));
        let sx = (h - 0.5) * aspect() * 1.5;
        let hop_h = 0.15 + h * 0.35;
        let phase = fract(u.beat * (0.25 + h * 0.5) + h * 3.0);
        // Parabolic hop, peak mid-beat-fraction.
        let y = horizon + hop_h * 4.0 * phase * (1.0 - phase);
        let squash = 1.0 + (1.0 - abs(phase - 0.5) * 2.0) * 0.0
                   + exp(-phase * 12.0) * 0.4; // squash right after landing
        let r = 0.09 + h * 0.05;
        let d = length(vec2<f32>((p.x - sx), (p.y - -y) / squash));
        let ball = smoothstep(r, r - 0.02, d);
        // Lit sphere: brighter top (-screen y).
        let shadel = clamp((-p.y - y) / r * 0.5 + 0.6, 0.2, 1.2);
        let bc = palette(h + u.hue);
        col = mix(col, bc * shadel * (0.4 + u.energy * 1.5), ball);
        // Shadow on the floor under the ball.
        let sh_d = length(vec2<f32>(p.x - sx, (p.y - -horizon) * 4.0));
        col *= 1.0 - exp(-sh_d * sh_d * 90.0) * 0.5 * (1.0 - clamp((y - horizon) / max(hop_h, 0.05), 0.0, 1.0));
    }
    return vec4<f32>(col, 1.0);
}
