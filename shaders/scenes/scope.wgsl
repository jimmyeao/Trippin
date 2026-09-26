// Oscilloscope: the spectrum drawn as a glowing phosphor trace with feedback
// trails, plus a mirrored ghost. CRT grid, soft scan flicker on the beat.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Feedback: the previous frame smeared slightly — phosphor decay.
    var col = prev(uv * 0.998 + vec2<f32>(0.001, 0.0)) * 0.90;

    // The trace: amplitude curve across the screen, centred vertically.
    let v = spec(uv.x);
    let y1 = 0.5 + (v - 0.25) * (0.7 + 0.3 * u.intensity);
    let y2 = 0.5 - (v - 0.25) * (0.7 + 0.3 * u.intensity); // mirrored ghost

    // (Square manually: pow() of a negative base is undefined in WGSL.)
    let dy1 = (uv.y - y1) * 90.0;
    let dy2 = (uv.y - y2) * 90.0;
    let g1 = exp(-dy1 * dy1);
    let g2 = exp(-dy2 * dy2) * 0.35;

    // Phosphor green tinted a little by the palette so hue still rotates.
    let trace_col = mix(vec3<f32>(0.25, 1.0, 0.55), palette(0.35), 0.35);
    col += trace_col * (g1 * (0.6 + v) + g2);

    // Beat marker: a bright blob rides the trace where the kick lands.
    let cx = fract(u.beat_phase);
    let cv = spec(cx);
    let blob = exp(-length((uv - vec2<f32>(cx, 0.5 + (cv - 0.25) * 0.8)) * vec2<f32>(14.0, 14.0)) * 3.0);
    col += vec3<f32>(1.0, 0.9, 0.6) * blob * (0.4 + u.kick);

    // Graticule.
    let gx = smoothstep(0.02, 0.0, abs(fract(uv.x * 10.0) - 0.5) - 0.48);
    let gy = smoothstep(0.02, 0.0, abs(fract(uv.y * 10.0) - 0.5) - 0.48);
    col += vec3<f32>(0.05, 0.12, 0.07) * max(gx, gy);
    col += vec3<f32>(0.1, 0.2, 0.12) * exp(-abs(uv.y - 0.5) * 120.0);

    // Scan flicker with the beat.
    col *= 0.92 + 0.08 * sin(uv.y * 900.0 + u.time * 30.0) * (0.3 + u.beat_phase * 0.7);

    return vec4<f32>(finite(col), 1.0);
}
