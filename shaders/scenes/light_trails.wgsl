// Light trails: long-exposure headlight streaks sweeping the frame — the
// festival-screen timelapse look. Dozens of lanes, each with its own speed,
// hue and direction; energy drives speed and kick flares the trail heads.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.0);

    // Night sky wash.
    col += vec3<f32>(0.012, 0.014, 0.03) * (1.0 + u.intensity);

    for (var lane = 0; lane < 28; lane++) {
        let fl = f32(lane);
        let h = hash21(vec2<f32>(fl * 3.7, 1.3));
        let h2 = hash21(vec2<f32>(fl * 5.1, 9.2));
        let h3 = hash21(vec2<f32>(fl * 7.9, 4.4));

        // Lane position/height, direction and speed.
        let y = mix(-0.85, 0.85, h) + sin(u.time * 0.2 + fl) * 0.02;
        let dir = select(1.0, -1.0, h2 > 0.5);
        let speed = (0.25 + h3 * 1.1) * (0.4 + u.intensity * 1.4);
        let head = fract(h2 * 7.0 + u.flow * speed * 0.18) * 4.4 - 2.2;
        let hx = head * dir * aspect();

        // Trail: bright head, exponential tail behind it.
        let dx = (p.x - hx) * dir;
        let tail = exp(-max(-dx, 0.0) * (2.0 + h3 * 4.0));
        let ahead = exp(-max(dx, 0.0) * 24.0);
        let along = min(tail, ahead);

        // Thin streak with a soft glow around the lane.
        let dy = abs(p.y - y);
        let streak = exp(-dy * dy * (2600.0 + h * 3800.0));
        let halo = exp(-dy * 26.0) * 0.12;

        let hue = fract(h * 3.0 + u.hue);
        let lcol = mix(palette(hue), vec3<f32>(1.0, 0.95, 0.85), h2 * 0.4);
        col += lcol * (streak + halo) * along * (0.35 + u.kick * 0.5 + spec(h) * 0.8);
    }

    // Slight persistence sells the long-exposure feel.
    col += prev(in.uv) * 0.42;
    return vec4<f32>(col, 1.0);
}
