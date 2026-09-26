// Sun rays: a blazing core with rays whose lengths are the spectrum mapped
// around the disc — the sun itself is the analyser.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * 0.55;
    let r = length(p);
    let a = angle(p) / TAU + 0.5;

    let v = spec(a);
    let ray_len = v * (0.30 + 0.15 * u.intensity);

    // Ray spikes around the core.
    let core_r = 0.13 + u.kick * 0.02;
    let ray = smoothstep(0.01, 0.0, r - core_r - ray_len) * smoothstep(core_r - 0.02, core_r + 0.03, r);
    let ray_edge = exp(-abs(r - core_r - ray_len) * 50.0) * step(r, core_r + ray_len + 0.02);

    var col = palette(a * 0.8 + u.flow * 0.03) * (ray * (0.3 + v) + ray_edge * 0.5);

    // Blazing disc: hot centre, photosphere rim flicker by highs.
    let rim_noise = fbm(vec2<f32>(a * 8.0, u.flow * 0.4)) * 0.03 * (0.4 + u.high);
    let disc = smoothstep(core_r + rim_noise, core_r - 0.06, r);
    col += mix(vec3<f32>(1.0, 0.55, 0.1), palette(0.08), 0.4) * disc * (1.2 + u.kick);
    col += vec3<f32>(1.0, 0.9, 0.6) * exp(-abs(r - core_r) * 18.0) * 0.8;

    // Corona wisps.
    let wisp = fbm(vec2<f32>(a * 6.0 + u.flow * 0.1, r * 4.0 - u.flow * 0.5));
    col += palette(0.15) * wisp * exp(-r * 2.2) * (0.3 + u.energy * 0.4);

    // Background space dust.
    col += vec3<f32>(0.4, 0.45, 0.7) * step(0.998, hash21(floor(in.uv * 350.0))) * 0.4;

    return vec4<f32>(finite(col), 1.0);
}
