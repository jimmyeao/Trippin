// Lens-flare ring bursts: concentric rings expand from centre on each
// beat with chromatic edge dispersion — a drop-friendly shockwave look.
// The ring domain is mirrored so the polar wrap can't leave a seam.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * 0.6;
    let r = length(p);
    let a = angle(p) / TAU + 0.5;
    let am = abs(a - 0.5) * 2.0; // seam-safe mirrored angle

    var col = vec3<f32>(0.008, 0.006, 0.018);

    // Three trailing beat rings: ages 0, 0.33, 0.66 beats behind.
    for (var i = 0; i < 3; i++) {
        let age = fract(u.beat_phase + f32(i) / 3.0);
        let rr = age * 0.9 + 0.05;
        let w = 0.015 + age * 0.05;
        // Spectrum around the ring wobbles the radius.
        // Shape: the rings buckle into petals with the spectrum, more so
        // as the bass builds.
        let wob = spec(am) * (0.04 + 0.1 * u.pres4.x) + sin(am * TAU * 3.0 + u.clock4.z * 0.3) * 0.02 * u.pres4.y;
        let ring = exp(-pow(abs(r - rr - wob) / w, 2.0));
        let fade = (1.0 - age) * (1.0 - age);
        // Chromatic dispersion: three offset rings of colour.
        col.r += ring * fade * (0.9 + u.kick);
        col.g += exp(-pow(abs(r - rr - wob - 0.008) / w, 2.0)) * fade;
        col.b += exp(-pow(abs(r - rr - wob - 0.016) / w, 2.0)) * fade * (1.2 + u.high);
    }

    // Central lens glow + horizontal flare streak.
    col += palette(0.08 + u.hue) * exp(-r * 5.0) * (0.4 + u.kick * 1.2);
    col += vec3<f32>(0.5, 0.6, 1.0) * exp(-abs(p.y) * 60.0) * exp(-abs(p.x) * 3.0)
         * (0.15 + u.bass * 0.5);

    // Subtle petal halo.
    // Direction: the petal halo turns one way then back.
    let halo = 0.5 + 0.5 * sin(am * TAU * 6.0 + 6.0 * sin(u.clock4.x * 0.03));
    col += palette(am + u.hue) * halo * exp(-r * 3.0) * 0.15 * (0.5 + u.energy);
    return vec4<f32>(finite(col), 1.0);
}
