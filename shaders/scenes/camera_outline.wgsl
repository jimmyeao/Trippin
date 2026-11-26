// @title Camera Outline
// @bloom 0.6
// @no-dancer
// Camera Outline: the webcam (Settings > Camera) traced as glowing lines of
// light: the DJ's own outline drawn in the palette, mirrored like a selfie,
// with trails streaming out from the centre. The lines thicken with the
// bass presence and swell on the kick (shape), ripple along a flowing field
// that the mids stir up (motion), and the hue drifts down the figure on the
// energy clock. Only in rotation while a camera is sending frames.

// Its controls (Perform > scene controls): u.roto4 = glow, detail, trails,
// ripple; u.roto5.x = stream.

// The camera's luminance at uv (mirrored like a selfie), at a blur level:
// tracing the sharp image scribbles hair, skin and sensor noise.
fn cam_l(uv: vec2<f32>, lod: f32) -> f32 {
    let c = untonemap(cam_lod(vec2<f32>(1.0 - uv.x, uv.y), lod));
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let detail = clamp(u.roto4.y, 0.0, 1.0);
    let lod = mix(2.2, 0.3, detail);
    // The lines ripple along a flowing field; the mids stir it.
    let t = u.clock4.z * 0.05;
    let warp = vec2<f32>(
        noise(uv * vec2<f32>(6.0, 4.0) + vec2<f32>(t, 0.0)) - 0.5,
        noise(uv * vec2<f32>(4.0, 6.0) + vec2<f32>(0.0, t) + 7.0) - 0.5,
    ) * (0.003 + 0.016 * u.lvl4.y) * u.roto4.w;
    let q = uv + warp;
    // Sobel on the blurred luminance; the step (line weight) follows the
    // bass presence and the kick.
    let px = exp2(lod) * (1.0 + 1.2 * u.pres4.x + 0.8 * kick_swell()) / vec2<f32>(1280.0, 720.0);
    let tl = cam_l(q + vec2<f32>(-px.x, -px.y), lod);
    let tc = cam_l(q + vec2<f32>(0.0, -px.y), lod);
    let tr = cam_l(q + vec2<f32>(px.x, -px.y), lod);
    let ml = cam_l(q + vec2<f32>(-px.x, 0.0), lod);
    let mr = cam_l(q + vec2<f32>(px.x, 0.0), lod);
    let bl = cam_l(q + vec2<f32>(-px.x, px.y), lod);
    let bc = cam_l(q + vec2<f32>(0.0, px.y), lod);
    let br = cam_l(q + vec2<f32>(px.x, px.y), lod);
    let gx = (tr + 2.0 * mr + br) - (tl + 2.0 * ml + bl);
    let gy = (bl + 2.0 * bc + br) - (tl + 2.0 * tc + tr);
    let thr = mix(0.18, 0.07, detail);
    let edge = smoothstep(thr, thr * 2.5, length(vec2<f32>(gx, gy)));
    let c = palette(fract(uv.y * 0.6 + u.clock4.x * 0.02));
    let line = c * c * edge * 1.3 * u.roto4.x;
    // Trails stream out from the centre.
    let p = uv - 0.5;
    let back = prev(0.5 + p * (1.0 - 0.015 * u.roto5.x)) * clamp(u.roto4.z, 0.0, 0.95);
    return vec4<f32>(finite(back + line + vec3<f32>(0.002, 0.002, 0.005)), 1.0);
}
