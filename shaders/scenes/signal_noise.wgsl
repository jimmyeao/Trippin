// Corrupted broadcast: blocky static in the scene palette, tearing in
// horizontal slices. Energy controls how much signal survives the noise —
// quiet moments dissolve to static, drops resolve into clean bars.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var p = centred(in.uv);

    // Slice tearing: each horizontal band shifts a little.
    let band = floor(p.y * 18.0);
    let tear = (hash21(vec2<f32>(band, floor(u.time * 14.0))) - 0.5) * u.onset * 0.4;
    p.x += tear;

    // Block noise, refreshed ~15fps and on every onset.
    let tq = floor(u.time * 15.0) + floor(u.beat);
    let bn = hash21(floor(p * 28.0) + tq * 0.37);

    // Underlying "signal": clean vertical colour bars keyed to spectrum.
    let sig = palette(spec(p.x * 0.3 / aspect() + 0.5) + u.hue);
    let noise_col = palette(bn * 4.0 + u.hue) * bn;

    let blend_t = clamp(bn * (1.2 - u.energy) + u.onset * 0.4, 0.0, 1.0);
    var col = mix(sig * (0.2 + u.energy), noise_col, blend_t);

    // Rolling brightness bands.
    col *= 0.85 + 0.15 * sin(p.y * 40.0 + u.time * 8.0);
    // Luminance crush keeps it dark between spikes.
    col *= 0.35 + u.intensity * 0.9 + u.kick * 0.4;
    return vec4<f32>(col, 1.0);
}
