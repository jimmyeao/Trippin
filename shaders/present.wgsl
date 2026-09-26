// Post-process the scene's HDR frame onto the window: FX transform (mirror /
// kaleido / invert, per u.fx), kick-driven chromatic aberration, tone mapping,
// vignette, cut flash and a little film grain.

fn aces(x: vec3<f32>) -> vec3<f32> {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

// u.fx modes: 1 mirror X, 2 mirror Y, 3 quad mirror, 4 kaleido 6, 5 kaleido 8.
// The fold axis is the screen centre, so a centred subject (the dancer) stays
// whole and symmetric rather than being split out to the screen edges.
fn fx_uv(uv: vec2<f32>, m: i32) -> vec2<f32> {
    if m == 1 {
        return vec2<f32>(0.5 - abs(uv.x - 0.5), uv.y);
    }
    if m == 2 {
        return vec2<f32>(uv.x, 0.5 - abs(uv.y - 0.5));
    }
    if m == 3 {
        return 0.5 - abs(uv - 0.5);
    }
    if m == 4 || m == 5 {
        let asp = aspect();
        let seg = select(8.0, 6.0, m == 4);
        let d = (uv - 0.5) * vec2<f32>(asp, 1.0);
        let r = length(d);
        let w = TAU / seg;
        let a = abs(fract(angle(d) / w) * w - w * 0.5);
        return 0.5 + vec2<f32>(cos(a), sin(a)) * r / vec2<f32>(asp, 1.0);
    }
    return uv;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let fxm = i32(u.fx + 0.5);
    let uv = fx_uv(in.uv, fxm);
    let dir = uv - 0.5;
    let ca = 0.004 + 0.012 * u.kick * u.intensity;
    var col = vec3<f32>(
        textureSampleLevel(prev_tex, samp, uv + dir * ca, 0.0).r,
        textureSampleLevel(prev_tex, samp, uv, 0.0).g,
        textureSampleLevel(prev_tex, samp, uv - dir * ca, 0.0).b
    );
    col = aces(finite(col) * (0.75 + 0.35 * u.intensity));
    // Vignette and grain use screen-space uv so edges darken the same under FX.
    let scr = in.uv - 0.5;
    col *= 1.0 - 0.9 * dot(scr, scr) * 1.6;
    col += u.flash * u.flash * 0.35;
    col *= u.master;
    col += (hash21(in.uv * vec2<f32>(u.res_x, u.res_y) + floor(fract(u.time * 7.0) * 997.0)) - 0.5) * 0.012;
    // The surface is usually sRGB, so let the hardware do the encode.
    return vec4<f32>(max(col, vec3<f32>(0.0)), 1.0);
}
