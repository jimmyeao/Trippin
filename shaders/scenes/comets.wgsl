// Comets arcing across a deep field — each follows a curved path on a
// beat-synced cycle, dragging a glowing tail. Onsets launch extra comets.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.004, 0.004, 0.014);

    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let h1 = hash21(vec2<f32>(fi, 1.0));
        let h2 = hash21(vec2<f32>(fi, 7.0));
        // Each comet repeats over a few beats, phase-offset from the rest.
        let cyc = fract(u.beat_phase + fi * 0.37 + h1);
        // Direction: alternate comets cross the other way.
        let dir = select(1.0, -1.0, (i & 1) == 1);
        let cx = mix(-aspect(), aspect(), cyc) * dir;
        // Shape: arcs loft higher with mid presence.
        let cy = (h2 - 0.5) * 1.4 - cyc * 0.4 + sin(cyc * PI) * (0.2 + 0.5 * u.pres4.y); // arc
        let d = length(p - vec2<f32>(cx, cy));

        let band = spec(fract(fi * 0.17 + 0.1));
        // Head.
        col += palette(fi * 0.15 + u.hue) * exp(-d * 30.0) * (0.6 + band * 1.5);
        // Tail: stretched glow behind the head's motion.
        // Energy: tails stretch with the bass level.
        let td = length(vec2<f32>(p.x - cx + dir * (0.1 + 0.25 * u.lvl4.x), (p.y - cy) * 0.5));
        col += palette(fi * 0.15 + u.hue) * exp(-td * 8.0) * 0.35 * band;
    }

    // Onset: brief screen-edge shimmer.
    col += palette(0.6 + u.hue) * u.onset * exp(-abs(p.x) * 2.0) * 0.1;
    col += prev(in.uv) * 0.35;
    return vec4<f32>(finite(col), 1.0);
}
