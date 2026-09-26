// Scrolling spectrogram: the spectrum written to the top row every frame and
// drifting downward — the track's fingerprint. Thermal palette, beat ruler.

fn thermal(t: f32) -> vec3<f32> {
    let c = clamp(t, 0.0, 1.0);
    var col = mix(vec3<f32>(0.0), vec3<f32>(0.35, 0.0, 0.55), smoothstep(0.0, 0.35, c));
    col = mix(col, vec3<f32>(0.95, 0.25, 0.1), smoothstep(0.35, 0.65, c));
    col = mix(col, vec3<f32>(1.0, 0.95, 0.6), smoothstep(0.65, 0.95, c));
    return col;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    // New spectrum row at the top; history scrolls down.
    let dy = u.dt * 0.22;
    var col = prev(vec2<f32>(uv.x, uv.y + dy)) * 0.995;

    if uv.y > 1.0 - dy * 2.0 {
        col = thermal(spec(uv.x) * (1.0 + u.intensity * 0.4));
    }

    // A beat ruler line crawls down with the history.
    let beat_row = step(fract(uv.y * 40.0 - u.beat * 0.25), 0.5 / 40.0);
    col += vec3<f32>(0.06) * beat_row * beat_pulse(8.0);

    // Column grid: bin separators.
    let bin = step(fract(uv.x * 32.0), 0.06);
    col *= 1.0 - bin * 0.35;

    // Bottom fade so the oldest history dies off.
    col *= smoothstep(0.0, 0.15, uv.y);

    return vec4<f32>(finite(col), 1.0);
}
