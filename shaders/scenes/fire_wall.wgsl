// A wall of stylised flame rising along the bottom of the frame —
// classic upscrolling noise fire, flame tips whip on the kick.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let up = -p.y * 0.5 + 0.5; // 0 bottom, 1 top

    // Fire domain: x wobbles, y rises — steady scroll, kick lifts the tips.
    let q = vec2<f32>(p.x * 2.5 + sin(up * 6.0 + u.flow * 0.8) * 0.15,
                      up * 3.0 - u.flow * 0.9);
    let n = fbm(q * 1.8);
    // Flame height per column rides the band under it.
    let band = spec(fract(p.x / aspect() * 0.5 + 0.5));
    let fh = 0.25 + band * 0.5 + u.kick * 0.15;
    let flame = smoothstep(fh, fh * 0.3, up) * (n * 0.9 + 0.25);

    // Fire palette: deep red → orange → pale yellow at the hot core.
    var col = mix(vec3<f32>(0.0), vec3<f32>(0.6, 0.05, 0.0), flame);
    col = mix(col, vec3<f32>(1.0, 0.5, 0.05), smoothstep(0.3, 0.7, flame));
    col = mix(col, vec3<f32>(1.0, 0.95, 0.6), smoothstep(0.75, 1.0, flame));
    col += vec3<f32>(0.8, 0.2, 0.0) * exp(-up * 4.0) * 0.3 * (0.5 + u.bass);
    col += prev(in.uv) * 0.25;
    return vec4<f32>(finite(col), 1.0);
}
