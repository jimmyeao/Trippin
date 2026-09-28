// A synthwave sea at dusk: layered wave silhouettes roll under a glowing
// sun. Silhouettes are dark against the sky — the sun pulses to the beat
// and the waves heave with the bass.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // World up = -p.y (uv is y-down).

    // Sky: warm gradient above, dark below the horizon.
    let up = -p.y;
    var col = mix(vec3<f32>(0.02, 0.005, 0.05), palette(u.hue) * 0.6,
                  smoothstep(-0.3, 0.6, up));
    col = mix(palette(0.08 + u.hue) * 0.8, col, smoothstep(-0.05, 0.35, up));

    // Sun: a disc sitting on the horizon at the lower third-line, pulsing
    // on the beat, striped like the classic retro sun.
    let sun_p = p - vec2<f32>(0.0, 0.30);
    let sun_r = length(sun_p);
    let sun = smoothstep(0.32, 0.30, sun_r);
    let stripes = smoothstep(0.0, 0.02, abs(fract(sun_p.y * 12.0 + u.time * 0.3) - 0.5) - 0.28);
    let sun_col = mix(palette(0.9 + u.hue), palette(0.1 + u.hue), clamp(up * 2.0 + 0.5, 0.0, 1.0));
    col = mix(col, sun_col * (0.6 + beat_pulse(3.0) * 0.8), sun * stripes);
    col += palette(0.9 + u.hue) * exp(-sun_r * 4.0) * 0.4 * (0.5 + u.bass);

    // Wave layers: rule of thirds — the sea occupies only the bottom
    // third of the frame (up < -0.33), sky gets the other two.
    for (var i = 0; i < 4; i++) {
        let fi = f32(i);
        let depth = fi / 4.0;
        let amp = 0.02 + depth * 0.025 + u.bass * 0.03;
        let wv = sin(p.x * (5.0 - fi) + u.flow * (0.6 + fi * 0.4) + fi * 2.0) * amp
               + sin(p.x * 11.0 - u.flow * (0.9 + fi * 0.3)) * amp * 0.4;
        let level = -0.33 - depth * 0.45;
        let below = smoothstep(level + wv + 0.005, level + wv - 0.005, up);
        let layer_col = mix(palette(0.55 + u.hue) * 0.25, vec3<f32>(0.005, 0.002, 0.015), depth);
        // Sheen on the wave tops.
        let sheen = exp(-abs(up - (level + wv)) * 60.0) * (0.3 + u.kick);
        col = mix(col, layer_col + palette(u.hue) * sheen * 0.3, below);
    }
    return vec4<f32>(col, 1.0);
}
