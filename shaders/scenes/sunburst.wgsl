// Radial ray fan spinning slowly — big top-of-drop look. Ray count steps
// up with energy, rays widen with bass, onsets snap the rotation.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var p = centred(in.uv);
    let r = length(p);
    let a = angle(p);

    let rays = 8.0 + floor(u.energy * 8.0) * 2.0;
    let spin = u.flow * 0.35 + u.onset * 0.2;
    let ray = abs(fract((a + spin) * rays / PI) - 0.5);
    let width = 0.32 + u.bass * 0.14;
    let beam = smoothstep(width, width * 0.35, ray);

    // Rays fade out with radius; a hot core anchors the middle.
    let fade = exp(-r * (1.2 - u.intensity * 0.5));
    let core = exp(-r * 6.0) * (0.5 + u.kick * 1.5);

    var col = vec3<f32>(0.01, 0.008, 0.02);
    let c1 = palette(u.hue);
    let c2 = palette(0.35 + u.hue);
    col += mix(c1, c2, fract((a + spin) * rays / PI * 0.5)) * beam * fade * (0.4 + u.energy * 1.6);
    col += c2 * core;
    // Occasional ring flash on the beat.
    col += c1 * exp(-abs(r - fract(u.beat * 0.25) * 1.6) * 20.0) * beat_pulse(5.0) * 0.5;
    return vec4<f32>(col, 1.0);
}
