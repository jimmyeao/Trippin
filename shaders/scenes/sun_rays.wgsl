// Sun rays: a blazing core with rays whose lengths are the spectrum mapped
// around the disc — the sun itself is the analyser.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Direction: the corona turns one way, then the other.
    let p = rot(0.8 * sin(u.clock4.x * 0.02)) * centred(in.uv) * 0.55;
    let r = length(p);
    let a = angle(p) / TAU + 0.5;

    // Mirrored angle — the polar wrap at ±π (screen left) must meet itself.
    let am = abs(a - 0.5) * 2.0;
    let v = spec(am);
    let ray_len = v * (0.30 + 0.15 * u.intensity);

    // Ray spikes around the core.
    // Shape: the sun swells with bass presence.
    let core_r = 0.1 + 0.07 * u.pres4.x + 0.02 * u.hits4.x;
    let ray = smoothstep(0.01, 0.0, r - core_r - ray_len) * smoothstep(core_r - 0.02, core_r + 0.03, r);
    let ray_edge = exp(-abs(r - core_r - ray_len) * 50.0) * step(r, core_r + ray_len + 0.02);

    var col = palette(am * 0.8 + u.clock4.w * 0.015) * (ray * (0.3 + v) + ray_edge * 0.5);

    // Blazing disc: hot centre, photosphere rim flicker by highs.
    let rim_noise = fbm(vec2<f32>(a * 8.0, u.clock4.z * 0.4)) * 0.03 * (0.4 + u.high);
    let disc = smoothstep(core_r + rim_noise, core_r - 0.06, r);
    col += mix(vec3<f32>(1.0, 0.55, 0.1), palette(0.08), 0.4) * disc * (1.2 + u.kick);
    col += vec3<f32>(1.0, 0.9, 0.6) * exp(-abs(r - core_r) * 18.0) * 0.8;

    // Corona wisps (mirrored domain so no seam at the wrap).
    let wisp = fbm(vec2<f32>(am * 6.0 + u.flow * 0.1, r * 4.0 - u.flow * 0.5));
    col += palette(0.15 + u.hue) * wisp * exp(-r * 2.2) * (0.3 + u.energy * 0.4);

    return vec4<f32>(finite(col), 1.0);
}
