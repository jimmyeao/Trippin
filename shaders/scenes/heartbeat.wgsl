// Heartbeat monitor: a scrolling ECG-style trace drawn at the right edge and
// pushed left each frame — spike height follows energy with a kick thump.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let dx = u.dt * 0.30;

    // Scroll history left.
    var col = prev(vec2<f32>(uv.x + dx, uv.y)) * 0.99;

    // New sample at the right edge: ECG-ish spike on the kick, baseline wiggle
    // from the spectrum.
    let h = 0.5
        + (u.energy - 0.35) * 0.25
        + u.kick * 0.30 * exp(-u.beat_phase * 30.0)
        - u.kick * 0.14 * exp(-abs(u.beat_phase - 0.12) * 40.0)
        + (spec(uv.x * 0.5) - 0.3) * 0.08;
    let dy = (uv.y - h) * 120.0;
    let trace = exp(-dy * dy);
    let new_px = smoothstep(1.0 - dx * 2.5, 1.0, uv.x);
    col += vec3<f32>(0.15, 1.0, 0.5) * trace * new_px * 1.2;
    col += vec3<f32>(0.4, 0.9, 0.6) * trace * 0.25 * step(uv.x, 1.0 - dx * 2.5); // keep old line lit

    // Faint grid.
    let gx = smoothstep(0.015, 0.0, abs(fract(uv.x * 16.0) - 0.5) - 0.47);
    let gy = smoothstep(0.015, 0.0, abs(fract(uv.y * 9.0) - 0.5) - 0.47);
    col += vec3<f32>(0.04, 0.10, 0.06) * max(gx, gy);

    // Pulse digits glow: a brightness bar top-left follows the BPM.
    let bar = step(uv.x, u.bpm / 220.0 * 0.3) * step(0.92, uv.y) * step(uv.x, 0.32);
    col += palette(0.1) * bar * 0.3;

    return vec4<f32>(finite(col), 1.0);
}
