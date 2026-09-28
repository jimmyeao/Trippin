// Bouncing orbs on a floor plane — each sphere hops on its own subdivision
// of the beat, squash-and-stretch on landing, flash on touchdown. The kick
// launches every orb higher; energy lifts the whole show.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // World up = -p.y. Floor sits low in the frame.
    let horizon = 0.45; // floor height in -p.y units (below centre)
    var col = vec3<f32>(0.01, 0.008, 0.025);

    // Floor: perspective grid receding up toward the horizon line.
    if -p.y < horizon {
        let depth = horizon - (-p.y); // 0 at floor line → deeper down-screen
        let gp = vec2<f32>(p.x, -p.y) / max(depth * 0.8 + 0.05, 0.05);
        let g = max(smoothstep(0.06, 0.02, abs(fract(gp.x * 0.5 + u.flow * 0.15) - 0.5)),
                    smoothstep(0.06, 0.02, abs(fract(gp.y * 0.5) - 0.5)));
        col += palette(0.6 + u.hue) * g * exp(-depth * 2.0) * (0.25 + u.energy * 0.4);
    }

    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let h = hash21(vec2<f32>(fi, 4.0));
        let sx = (h - 0.5) * aspect() * 1.5;
        // Hop height jumps with energy + kick.
        let hop_h = (0.12 + h * 0.3) * (0.5 + u.energy * 1.2) + u.kick * 0.15;
        let rate = 0.5 + h; // hops per beat — locked to tempo
        let phase = fract(u.beat * rate + h * 3.0);
        // Parabolic arc.
        let y = horizon + hop_h * 4.0 * phase * (1.0 - phase);
        // Squash at touchdown, stretch mid-flight.
        let squash = 1.0 + exp(-phase * 10.0) * 0.45 - (1.0 - abs(phase - 0.5) * 2.0) * 0.12;
        let r = (0.09 + h * 0.05) * (1.0 + u.bass * 0.25);
        let d = length(vec2<f32>(p.x - sx, (-p.y - y) / squash));
        let ball = smoothstep(r, r - 0.02, d);
        // Shading: brighter top face.
        let shadel = clamp((-p.y - y) / r * 0.5 + 0.55, 0.2, 1.2);
        let bc = palette(h + u.hue);
        // Touchdown flash.
        let td = exp(-phase * 12.0) * (0.5 + u.kick);
        col = mix(col, bc * (shadel * (0.35 + u.energy * 1.3) + td), ball);
        // Landing ring on the floor.
        let ring_d = abs(length(vec2<f32>(p.x - sx, (-p.y - horizon) * 5.0)) - 0.25 * (1.0 - phase));
        col += bc * exp(-ring_d * 25.0) * td * 0.5;
        // Shadow shrinking under the orb.
        let sh_d = length(vec2<f32>(p.x - sx, (-p.y - horizon) * 4.0));
        let air = clamp((y - horizon) / max(hop_h, 0.05), 0.0, 1.0);
        col *= 1.0 - exp(-sh_d * sh_d * (90.0 + air * 200.0)) * 0.5 * (1.0 - air * 0.7);
    }
    return vec4<f32>(finite(col), 1.0);
}
