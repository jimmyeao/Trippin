// Skyline equaliser: a layered city whose tower tops ARE the spectrum.
// uv.y = 0 is screen TOP — `up = 1.0 - uv.y` is used throughout so towers
// rise from the bottom edge. Back rows sit dimmer behind the front row;
// windows are warm sodium light, the horizon glows with the bass.

fn tower_h(x: f32, layer: f32) -> f32 {
    let i = floor(x * 20.0 + layer * 13.0);
    let fx = fract((i + 0.5) / 20.0);
    let v = spec(fx);
    let jit = 0.55 + 0.45 * hash21(vec2<f32>(i, layer));
    return 0.08 + v * jit * (0.55 + 0.25 * u.intensity);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let up = 1.0 - in.uv.y;
    let x = in.uv.x;

    // Sky: deep dusk above, warm smog glow sinking to the horizon.
    var col = mix(vec3<f32>(0.012, 0.008, 0.03), palette(u.hue) * 0.35,
                  pow(1.0 - up, 2.0));
    col += palette(0.05 + u.hue) * exp(-up * 5.0) * (0.3 + u.bass * 0.7);
    // Heat-lightning wash on drops.
    col += palette(0.6 + u.hue) * u.onset * 0.25 * exp(-up * 1.5);

    // Back row of towers — taller spread, dimmer, hue-shifted.
    let h2 = tower_h(x, 1.0) * 1.15;
    if up < h2 {
        let edge2 = smoothstep(0.012, 0.0, h2 - up);
        col = vec3<f32>(0.03, 0.02, 0.05);
        col += palette(0.4 + u.hue) * edge2 * 0.5;
        let win2 = step(0.75, hash21(floor(vec2<f32>(x * 80.0, up * 60.0)) + 7.0));
        col += vec3<f32>(0.6, 0.5, 0.3) * win2 * 0.15 * step(0.05, up);
    }

    // Front row — darker silhouettes, lit windows, glowing rooflines.
    let h = tower_h(x, 0.0);
    if up < h {
        col = vec3<f32>(0.012, 0.008, 0.022);
        // Windows: warm panes that flicker with the band under them.
        let wx = floor(x * 90.0);
        let wy = floor(up * 70.0);
        let v = spec(fract((wx + 0.5) / 90.0));
        let lit = step(0.62, hash21(vec2<f32>(wx, wy)) * (0.5 + v * 0.9));
        let pane = step(0.15, fract(x * 90.0)) * step(fract(x * 90.0), 0.7)
                 * step(0.2, fract(up * 70.0)) * step(fract(up * 70.0), 0.8);
        col += mix(vec3<f32>(1.0, 0.72, 0.3), palette(u.hue + 0.2), 0.25)
               * pane * lit * (0.35 + u.energy * 0.9);
        // Hot roofline riding the spectrum.
        let roof = smoothstep(0.014, 0.0, h - up);
        col += palette(v + u.hue) * roof * (0.8 + u.kick * 1.5 + v);
    }

    // Street-level haze strip at the very bottom.
    col += palette(0.1 + u.hue) * exp(-up * 30.0) * (0.15 + u.bass * 0.3);
    return vec4<f32>(finite(col), 1.0);
}
