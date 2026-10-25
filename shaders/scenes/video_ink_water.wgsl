// @bloom 0.6
// @title Ink Bloom
// Coloured ink billowing through water (video clip). The bass swells a slow swirling warp through the
// fluid, the swirl turns one way then the other with the phrase, the
// footage flows faster as the track builds, and the palette tints it.

// Smooth two-octave noise: the warp must be broad, or the footage wrinkles.
fn soft(p: vec2<f32>) -> f32 {
    return noise(p) * 0.7 + noise(p * 2.1 + 7.3) * 0.3;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let swing = sin(u.flow / 32.0 * TAU);
    // Slow rotation whose direction follows the phrase.
    let ang = 0.08 * swing;
    let q = rot(ang) * p;
    // Curl of a broad noise field, deepened by the bass presence (~1.5 s).
    let t = u.clock4.y * 0.04;
    let e = 0.05;
    let s = q * 0.9 + vec2<f32>(t, -0.6 * t);
    let n0 = soft(s);
    let curl = vec2<f32>(soft(s + vec2<f32>(0.0, e)) - n0, n0 - soft(s + vec2<f32>(e, 0.0))) / e;
    let warp = curl * (0.002 + 0.014 * u.pres4.x + 0.004 * u.lvl4.x) * (1.0 - 0.5 * u.calm);
    let uv = uncentred(q) + warp;
    let base = vid_cam(uv, 1.2 + 0.03 * u.pres4.y, vec2<f32>(0.0));
    let c = untonemap(vid_grade(base, u.hue + dot(base, vec3<f32>(0.3)) * 0.6 + u.flow * 0.01, 0.2));
    return vec4<f32>(c, 1.0);
}
