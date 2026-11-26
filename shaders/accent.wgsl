// Accent layer mix (render.rs, after the layers of Imaginando's VS): a second,
// light scene renders into its own feedback pair, and this pass adds it into
// the main scene target before the dancer, scaled by the eased amount in
// u.misc4.z (additive blend). Bound for this pass only: prev_tex is the main
// scene's last frame and ext_tex is the accent's newest frame (accents never
// run over Unity shows, the binding's usual owner).

fn lum(c: vec3<f32>) -> f32 {
    return max(c.r, max(c.g, c.b));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let a = finite(textureSampleLevel(ext_tex, samp, in.uv, 0.0).rgb);
    // Only the accent's lights add: its background lift (haze, star dust,
    // a dark-blue fill) would lay a grey veil over the base scene.
    let k = smoothstep(0.03, 0.15, lum(a));
    // Screen-like: the accent shows over the base's dark areas and fades
    // where it is bright (helix vanished into a glacier and washed plasma
    // out when added flat). The base is read wide, so the accent's own
    // small lights from last frame barely count and can't dim themselves.
    let r = 0.06;
    let ar = vec2<f32>(r / aspect(), r);
    var b = lum(prev(in.uv));
    b += lum(prev(in.uv + vec2<f32>(ar.x, 0.0)));
    b += lum(prev(in.uv - vec2<f32>(ar.x, 0.0)));
    b += lum(prev(in.uv + vec2<f32>(0.0, ar.y)));
    b += lum(prev(in.uv - vec2<f32>(0.0, ar.y)));
    let dark = 1.0 - clamp(b / 5.0, 0.0, 1.0);
    return vec4<f32>(a * k * u.misc4.z * dark * dark, 0.0);
}
