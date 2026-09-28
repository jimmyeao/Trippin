// Deep-space nebula: domain-warped fbm in the track palette, star field
// behind. Flows continuously with the phrase clock; energy stirs the gas.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let q = p * 1.6 + vec2<f32>(u.flow * 0.05, u.flow * 0.03);

    // Domain warp for billowy structure.
    let warp = vec2<f32>(fbm(q + vec2<f32>(0.0, u.time * 0.04)),
                         fbm(q + vec2<f32>(5.2, 1.3) - u.time * 0.03));
    let n = fbm(q + warp * (1.2 + u.energy * 0.8));

    let c1 = palette(u.hue);
    let c2 = palette(0.33 + u.hue);
    let c3 = palette(0.66 + u.hue);
    var col = mix(vec3<f32>(0.005, 0.005, 0.015), c1 * 0.5, smoothstep(0.3, 0.7, n));
    col += c2 * smoothstep(0.55, 0.85, fbm(q - warp)) * (0.3 + u.bass * 0.7);
    col += c3 * smoothstep(0.7, 0.95, n) * u.mid;

    // Stars: sparse hashed cells that twinkle.
    let cell = floor(p * 90.0);
    let star = hash21(cell);
    if star > 0.985 {
        let sp = fract(p * 90.0) - 0.5;
        let tw = 0.4 + 0.6 * sin(u.time * (2.0 + star * 8.0) + star * 50.0);
        col += vec3<f32>(0.9) * smoothstep(0.25, 0.05, length(sp)) * tw * (0.4 + u.high);
    }
    return vec4<f32>(col, 1.0);
}
