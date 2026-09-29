// Kaleidoscope over domain-warped noise. Rotation snaps on every beat, the
// pattern pumps with the kick, segment count changes on each cut.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var p = centred(in.uv);
    let snap = floor(u.beat) + smoothstep(0.0, 0.2, u.beat_phase);
    p = rot(u.clock4.z * 0.025 + snap * 0.15) * p;

    let n = 6.0 + 2.0 * floor(u.seed % 4.0);
    let r = length(p);
    var a = angle(p);
    let wedge = TAU / n;
    a = abs((a % wedge + wedge) % wedge - wedge * 0.5);
    var q = vec2<f32>(cos(a), sin(a)) * r;

    q *= 1.6 - 0.35 * beat_pulse(6.0) * (0.4 + u.bass);
    let t = u.clock4.x * 0.06 + u.seed;
    let warp = vec2<f32>(fbm(q * 1.5 + t), fbm(q * 1.5 - t + 5.2));
    let v = fbm(q * 2.0 + warp * (1.5 + u.mid * 2.0));

    let bands = abs(fract(v * 5.0 - u.beat * 0.25) - 0.5);
    let lines = smoothstep(0.08 + 0.1 * u.high, 0.0, bands);
    var c = palette(v * 1.2 + r * 0.25) * (0.25 + v * 0.6) * (0.4 + u.energy);
    c += palette(v + 0.5) * lines * (0.6 + 1.5 * u.onset);
    c *= smoothstep(1.9, 0.2, r);

    let fb = uncentred(rot(-0.01) * centred(in.uv) * 0.99);
    c = mix(c, max(c, prev(fb) * 0.85), 0.6);
    return vec4<f32>(c, 1.0);
}
