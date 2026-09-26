// Beat-locked tunnel: rings rush past once per beat, spokes shimmer with the
// highs, bass lights the core. Feedback zoom leaves light trails.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let r = max(length(p), 0.001);
    let a = angle(p);

    // Depth advances one ring per beat, faster when the track is driving.
    let z = 0.35 / r + u.flow * 0.75;
    let twist = a + 0.35 * sin(z * 0.4 + u.time * 0.3) + u.bar_phase * TAU / 8.0;
    let seg = 6.0 + 2.0 * floor(u.seed % 4.0);

    let rings = pow(abs(sin(z * PI)), 24.0) * (0.6 + 0.9 * u.intensity);
    let spokes = pow(abs(sin(twist * seg * 0.5)), 40.0) * u.high * 2.0;
    let tex = fbm(vec2<f32>(twist * 2.0, z * 1.5));
    var c = palette(z * 0.04 + tex * 0.5) * (rings * 0.9 + spokes * 0.6 + tex * tex * u.mid * 0.4);
    c *= smoothstep(0.0, 0.25, r);
    c += palette(0.55) * u.bass * 0.12 / (r * 6.0 + 0.2);

    let fb = uncentred(rot(0.004 * sin(u.time)) * p * 0.975);
    c += prev(fb) * (0.55 + 0.15 * u.intensity);
    return vec4<f32>(c, 1.0);
}
