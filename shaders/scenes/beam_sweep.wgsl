// Festival searchlights: beams pivot from mounts along the bottom edge
// and sweep the sky. Sweep direction flips on the beat; haze brightens
// with energy, heads flash on onsets. World up = -p.y.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);

    var col = vec3<f32>(0.008, 0.01, 0.025) * (0.7 - p.y * 0.3);

    let mounts = 5;
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let mx = mix(-aspect() * 0.85, aspect() * 0.85, fi / 4.0);
        let mount = vec2<f32>(mx, 1.05); // just below the bottom edge

        // Beam pivots: slow wander + a snap on each beat. Direction stays
        // continuous (it's a pose), so motion is smooth but clearly beat-led.
        // Energy: sweeps quicken with the mix; shape: the fan opens wider
        // with bass presence and gathers/spreads with the mids.
        let swing = sin(u.clock4.x * (0.3 + fi * 0.04) + fi * 1.9) * (0.35 + 0.35 * u.pres4.x)
                  + (fi - 2.0) * 0.12 * (u.pres4.y * 2.0 - 0.6)
                  + sin(u.beat_phase * PI) * 0.15 * (fi - 2.0) * 0.3;
        let ang = -PI * 0.5 + swing; // pointing up-screen, ±swing

        // Beam strip: distance from the ray through the mount at angle
        // `ang`. dir = (cos, sin); along = rel·dir, perp = rel⊥dir.
        let rel = p - mount;
        let along = rel.x * cos(ang) + rel.y * sin(ang);
        let perp = rel.y * cos(ang) - rel.x * sin(ang);
        let width = (0.03 + max(along, 0.0) * 0.018) * (0.7 + 0.8 * u.lvl4.x);
        let band = spec(fi / 5.0);
        let front = smoothstep(0.0, 0.15, along);       // only above the mount
        let beam = smoothstep(0.04, 0.0, abs(perp) - width) * front;
        let haze = exp(-abs(perp) * 6.0) * front * 0.2;

        col += palette(fi * 0.19 + u.hue) * (beam * (0.5 + band * 1.3)
             + haze * (0.3 + u.energy)) * (0.7 + u.kick * 0.6);
        // Mount glow.
        col += palette(fi * 0.19 + u.hue) * exp(-length(rel) * 25.0) * 0.6;
    }
    col += prev(in.uv) * 0.12;
    return vec4<f32>(finite(col), 1.0);
}
