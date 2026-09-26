// Stardrive: a warp field of stars streaming out from centre — speed follows
// the beat clock, brightness streaks with the kick, hue drifts with energy.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * 0.6;
    let r = length(p) + 1e-4;
    let a = angle(p);

    var col = vec3<f32>(0.0);

    // Three parallax layers of streaking stars.
    for (var L = 0; L < 3; L++) {
        let fl = f32(L);
        let speed = (0.5 + fl * 0.5) * (0.5 + u.energy * 1.5);
        // Radial coordinate folds into cells; stars move outward with flow.
        let z = (1.0 / r) * (1.0 + fl * 0.3) - u.flow * speed * 0.5;
        let cell_a = floor(a / TAU * 24.0 + fl * 7.0);
        let cell_r = floor(z * 3.0);
        let h = hash22(vec2<f32>(cell_a, cell_r) + fl * 13.0);

        // Star position inside its cell; brightness by hash, streak on kick.
        let sr = fract(z * 3.0 + h.x) - 0.5;
        let sa = fract(a / TAU * 24.0 + fl * 7.0) - 0.5;
        let star_d = length(vec2<f32>(sa * 0.4, sr)) ;
        let streak = exp(-star_d * 18.0) * (0.3 + u.kick * 1.5);
        let sparkle = 0.5 + 0.5 * sin(u.time * (2.0 + h.y * 6.0) + h.x * 40.0);
        col += palette(h.x * 0.5 + fl * 0.2) * streak * sparkle * step(0.3, h.y) * (1.0 - r * 0.6);
    }

    // Centre glow.
    col += palette(u.hue) * exp(-r * 5.0) * (0.2 + u.bass * 0.8);

    // Tunnel vignette so edges feel deep.
    col *= smoothstep(1.4, 0.3, r) * 1.2;

    return vec4<f32>(finite(col), 1.0);
}
