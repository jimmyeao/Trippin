// Skyline equaliser: a city of towers whose heights are the spectrum. Windows
// flicker per band; the sky pulses with the beat like distant heat lightning.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let tw = 24.0;
    let i = floor(uv.x * tw);
    let fx = (i + 0.5) / tw;
    let v = spec(fx);

    // Tower height with a per-tower jitter so it isn't a perfect ramp.
    let jitter = 0.7 + 0.6 * hash21(vec2<f32>(i, u.seed));
    let h = 0.10 + v * jitter * (0.55 + 0.2 * u.intensity);

    // Sky: deep gradient, bass-lit glow at the horizon, lightning on drops.
    var col = mix(vec3<f32>(0.02, 0.01, 0.05), palette(0.72) * 0.18, pow(uv.y, 1.5));
    col += palette(0.05) * exp(-uv.y * 4.0) * (0.25 + u.bass * 0.6);
    col += palette(0.9) * u.flash * 0.5 * exp(-uv.y * 2.0);

    // Stars fading out near the glow.
    let star = step(0.9975, hash21(floor(uv * vec2<f32>(220.0, 120.0))));
    col += vec3<f32>(0.7, 0.8, 1.0) * star * smoothstep(0.3, 0.7, uv.y) * (0.4 + u.high);

    if uv.y < h {
        // Tower face: dark with lit windows; window grid brightness = band.
        let wx = fract(uv.x * tw * 4.0);
        let wy = fract(uv.y * 40.0);
        let win = step(0.25, wx) * step(0.35, wy) * step(wx, 0.75) * step(wy, 0.8);
        let lit = step(0.55, hash21(vec2<f32>(i * 7.0 + floor(uv.y * 40.0), u.seed)) * (0.4 + v));
        let wcol = mix(vec3<f32>(1.0, 0.75, 0.35), palette(fx + 0.4), 0.3);
        col = vec3<f32>(0.015, 0.01, 0.03) + wcol * win * lit * (0.5 + u.energy);
        // Roof edge light.
        col += palette(fx + 0.2) * smoothstep(0.015, 0.0, h - uv.y) * (0.6 + v);
    } else {
        // Antenna blink on the tallest towers.
        let blink = step(0.6, sin(u.beat * PI + i * 2.4)) * step(h, 0.3);
        col += vec3<f32>(1.0, 0.15, 0.1) * blink * exp(-abs(uv.y - h - 0.01) * 120.0) * step(abs(uv.x * tw - i - 0.5), 0.06);
    }

    // Ground reflection: smear of the city lights.
    if uv.y < 0.06 {
        let refl = spec(uv.x) * 0.5;
        col = mix(col, palette(uv.x + 0.3) * refl, 0.5) * (1.0 - uv.y * 6.0);
    }

    return vec4<f32>(finite(col), 1.0);
}
