// Hypno-disc: a classic spinning spiral, high contrast. Rotation speed
// rides the beat clock so it locks to the music, arm count steps with
// energy. Stares straight back at the crowd.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var p = centred(in.uv);
    p *= 1.0 - beat_pulse(5.0) * 0.05; // breathe on the kick
    let r = length(p);
    let a = angle(p);

    let arms = 2.0 + floor(u.energy * 3.0);
    // Log spiral: angle + k*log(r) gives the classic hypnosis band.
    let band = sin((a + u.flow * 0.5) * arms + log(r + 0.02) * 9.0);
    let b = smoothstep(-0.3, 0.3, band);

    var col = mix(vec3<f32>(0.01), palette(band * 0.1 + r * 0.2 + u.hue), b);
    col *= exp(-r * 0.6);
    col += palette(0.5 + u.hue) * exp(-r * 9.0) * (0.4 + u.kick * 1.5);
    // Edge vignette.
    col *= smoothstep(1.5, 0.7, r);
    return vec4<f32>(col, 1.0);
}
