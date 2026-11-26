// @title Camera Outline
// @bloom 0.6
// @no-dancer
// Camera Outline: the webcam (Settings > Camera) traced as glowing lines of
// light: the DJ's own outline drawn in the palette, mirrored like a selfie,
// with trails streaming out from the centre. The lines thicken with the
// bass presence and swell on the kick (shape), ripple along a flowing field
// that the mids stir up (motion), and the hue drifts down the figure on the
// energy clock. Only in rotation while a camera is sending frames.

// The camera's luminance at uv (mirrored like a selfie).
fn cam_l(uv: vec2<f32>) -> f32 {
    let c = untonemap(vid(vec2<f32>(1.0 - uv.x, uv.y)));
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    // The lines ripple along a flowing field; the mids stir it.
    let t = u.clock4.z * 0.05;
    let warp = vec2<f32>(
        noise(uv * vec2<f32>(6.0, 4.0) + vec2<f32>(t, 0.0)) - 0.5,
        noise(uv * vec2<f32>(4.0, 6.0) + vec2<f32>(0.0, t) + 7.0) - 0.5,
    ) * (0.003 + 0.016 * u.lvl4.y);
    let q = uv + warp;
    // Sobel on the camera's luminance; the step (line weight) follows the
    // bass presence and the kick.
    let px = (1.4 + 2.2 * u.pres4.x + 1.5 * kick_swell()) / vec2<f32>(u.res_x, u.res_y);
    let tl = cam_l(q + vec2<f32>(-px.x, -px.y));
    let tc = cam_l(q + vec2<f32>(0.0, -px.y));
    let tr = cam_l(q + vec2<f32>(px.x, -px.y));
    let ml = cam_l(q + vec2<f32>(-px.x, 0.0));
    let mr = cam_l(q + vec2<f32>(px.x, 0.0));
    let bl = cam_l(q + vec2<f32>(-px.x, px.y));
    let bc = cam_l(q + vec2<f32>(0.0, px.y));
    let br = cam_l(q + vec2<f32>(px.x, px.y));
    let gx = (tr + 2.0 * mr + br) - (tl + 2.0 * ml + bl);
    let gy = (bl + 2.0 * bc + br) - (tl + 2.0 * tc + tr);
    let edge = smoothstep(0.08, 0.35, length(vec2<f32>(gx, gy)));
    let c = palette(fract(uv.y * 0.6 + u.clock4.x * 0.02));
    let line = c * c * edge * 1.3;
    // Trails stream out from the centre.
    let p = uv - 0.5;
    let back = prev(0.5 + p * 0.985) * 0.82;
    return vec4<f32>(finite(back + line + vec3<f32>(0.002, 0.002, 0.005)), 1.0);
}
