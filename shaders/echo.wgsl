// Echo (post effect, render.rs; after the "Render Output" feedback layer of
// Imaginando's VS): the finished scene frame, dancer and accent included,
// max-blended with its own last echo, which is zoomed out from the centre and
// turned a little each frame, so light leaves trails that follow the motion.
// Runs in its own feedback pair, so scenes that read prev() never see it.
// Bound for this pass: ext_tex = this frame, prev_tex = the last echo.
// Max-blend can't blow out (nothing gets brighter than its brightest input),
// unlike additive feedback. Amount in u.misc4.w.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let cur = textureSampleLevel(ext_tex, samp, in.uv, 0.0);
    // Per-frame rates, scaled to the frame time so 30 and 120 fps match.
    let k = clamp(u.dt * 60.0, 0.0, 4.0);
    // The echo drifts outward a touch faster with the slow bass presence
    // (shape, smoothed), and its turn swings direction over eight bars on
    // the smooth clock, never with raw audio.
    let zoom = 1.0 - (0.006 + 0.010 * u.pres4.x) * k;
    let turn = 0.004 * k * sin(TAU * u.flow / 32.0);
    let asp = vec2<f32>(aspect(), 1.0);
    let d = rot(turn) * ((in.uv - 0.5) * asp) * zoom;
    let puv = d / asp + 0.5;
    // Trails of ~half a second while the beat drives, long and dreamy in a
    // breakdown. (0.86 per frame faded in a sixth of a second: live, it read
    // as doing nothing.)
    let decay = pow(mix(0.92, 0.96, u.calm), k);
    let old = prev(puv) * decay;
    let echo = max(finite(cur.rgb), old);
    return vec4<f32>(mix(finite(cur.rgb), echo, u.misc4.w), cur.a);
}
