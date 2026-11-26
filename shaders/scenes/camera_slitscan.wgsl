// @title Camera Slit-Scan
// @no-dancer
// Camera Slit-Scan: the webcam (Settings > Camera) live on the right of the
// frame, and everything it shows smeared away to the left over time, so
// every move leaves a long ribbon of itself. The smear runs faster with the
// loudness (motion), waves up and down with the bass presence (shape), and
// the ribbons pick up the palette as they age. Only in rotation while a
// camera is sending frames.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    // The live strip: the camera, mirrored, its centre at x = 0.78.
    let cam_uv = vec2<f32>(1.0 - (uv.x - 0.28), uv.y);
    let live = untonemap(cam(cam_uv));
    let mask = smoothstep(0.6, 0.72, uv.x);
    // The smear: last frame, pulled in from the right. Speed follows the
    // loudness (eased), as a fraction of the width per second.
    let loud = dot(u.lvl4, vec4<f32>(0.45, 0.3, 0.15, 0.1));
    let speed = (0.08 + 0.35 * loud) * clamp(u.dt, 0.0, 0.05);
    let wave = (0.0015 + 0.004 * u.pres4.x) * sin(uv.x * 14.0 + u.clock4.x * 0.4);
    var old = prev(uv + vec2<f32>(speed, wave)) * 0.995;
    // Ageing ribbons drift toward the palette.
    let pal = palette(fract(uv.x * 0.5 + u.clock4.x * 0.01));
    let l = dot(old, vec3<f32>(0.3, 0.5, 0.2));
    old = mix(old, pal * pal * l * 1.6, 0.012);
    return vec4<f32>(finite(mix(old, live, mask)), 1.0);
}
