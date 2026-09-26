// Post-process the scene's HDR frame onto the window: kick-driven chromatic
// aberration, tone mapping, vignette, cut flash and a little film grain.

fn aces(x: vec3<f32>) -> vec3<f32> {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let dir = uv - 0.5;
    let ca = 0.004 + 0.012 * u.kick * u.intensity;
    var col = vec3<f32>(
        textureSampleLevel(prev_tex, samp, uv + dir * ca, 0.0).r,
        textureSampleLevel(prev_tex, samp, uv, 0.0).g,
        textureSampleLevel(prev_tex, samp, uv - dir * ca, 0.0).b
    );
    col = aces(finite(col) * (0.75 + 0.35 * u.intensity));
    col *= 1.0 - 0.9 * dot(dir, dir) * 1.6;
    col += u.flash * u.flash * 0.35;
    col *= u.master;
    col += (hash21(uv * vec2<f32>(u.res_x, u.res_y) + floor(fract(u.time * 7.0) * 997.0)) - 0.5) * 0.012;
    // The surface is usually sRGB, so let the hardware do the encode.
    return vec4<f32>(max(col, vec3<f32>(0.0)), 1.0);
}
