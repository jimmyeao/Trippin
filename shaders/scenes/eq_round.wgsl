// Radial spectrum analyser: bars fan out from a pulsing core, ring ticks mark
// the beat. The whole ring breathes with the kick.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * 0.55;
    let r = length(p);
    // Direction: the ring turns one way, then back.
    let a = fract(angle(p) / TAU + 0.5 + 0.08 * sin(u.clock4.x * 0.03)); // 0..1 around the circle

    // Mirror the spectrum across the circle: left half = reversed right half.
    let fx = abs(a * 2.0 - 1.0);
    let v = spec(fx);

    // Shape: the core swells with bass presence and jolts on kicks.
    let r0 = 0.12 + 0.07 * u.pres4.x + 0.03 * u.hits4.x;
    let reach = 0.42 + 0.10 * u.intensity;
    let len = v * reach;

    // Bar body between r0 and r0+len, with angular gap between columns.
    let cols = 64.0;
    let cell = abs(fract(a * cols) - 0.5);
    let side = smoothstep(0.5, 0.32, cell);
    let body = smoothstep(0.0, 0.01, r - r0) * smoothstep(len, len - 0.02, r - r0);

    // Tip ring where the bar ends.
    let tip = exp(-abs(r - r0 - len) * 40.0);

    var col = palette(fx * 0.9 + 0.1) * (body * side * (0.5 + v) + tip * 0.8 * side);

    // Pulsing core.
    col += palette(0.6) * smoothstep(r0, r0 * 0.4, r) * (0.6 + u.kick * 1.2);
    col += palette(0.6) * exp(-abs(r - r0) * 30.0) * 0.35;

    // Beat ticks: four short markers on the quarter-beats.
    let tick = step(fract(a * 4.0), 0.02) * smoothstep(0.02, 0.0, abs(r - r0 - reach - 0.05));
    col += palette(0.8) * tick * beat_pulse(4.0);

    // Subtle inner darkness so the ring reads against scenery bleed.
    col *= 0.4 + 0.6 * smoothstep(0.02, 0.1, r);

    return vec4<f32>(finite(col), 1.0);
}
