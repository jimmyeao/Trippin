// @bloom 0.6
// @title Ink Bloom
// Coloured ink billowing through water (video clip). The bass swells a swirling warp through the fluid,
// the swirl turns one way then the other with the phrase, the footage
// flows faster as the track builds, and the palette re-colours it.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let swing = sin(u.flow / 32.0 * TAU);
    // Slow rotation whose direction follows the phrase.
    let ang = 0.12 * swing + 0.04 * sin(u.clock4.x * 0.05);
    let q = rot(ang) * p;
    // Curl-noise warp, deepened by the bass presence (smooth, ~1.5 s).
    let t = u.clock4.y * 0.06;
    let e = 0.02;
    let n0 = fbm(q * 1.4 + t);
    let nx = fbm(q * 1.4 + vec2<f32>(e, 0.0) + t);
    let ny = fbm(q * 1.4 + vec2<f32>(0.0, e) + t);
    let curl = vec2<f32>(ny - n0, n0 - nx) / e;
    let warp = curl * (0.004 + 0.03 * u.pres4.x + 0.01 * u.lvl4.x) * (1.0 - 0.5 * u.calm);
    let uv = uncentred(q) + warp;
    let base = vid_cam(uv, 1.1 + 0.04 * u.pres4.y, vec2<f32>(0.0));
    var c = untonemap(vid_grade(base, u.hue + dot(base, vec3<f32>(0.3)) * 0.6 + u.flow * 0.01, 0.35));
    // A whisper of feedback so the swirl leaves a soft wake.
    c = mix(c, prev(in.uv + warp * 0.5), 0.18);
    return vec4<f32>(c, 1.0);
}
