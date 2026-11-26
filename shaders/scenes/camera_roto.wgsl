// @title Camera Rotoscope
// @bloom 0.3
// @no-dancer
// Camera Rotoscope: the webcam (Settings > Camera) drawn as a living cartoon,
// mirrored like a selfie: flat cel-shaded bands in the palette's colours and
// bold ink outlines, with the lines "boiling" gently as hand-traced frames
// do. The ink thickens with the bass presence and on the kick (shape), the
// lines boil faster as the mids get busy (motion), and the bands' colours
// drift round the palette on the energy clock. Only in rotation while a
// camera is sending frames.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(1.0 - in.uv.x, in.uv.y);
    // Line boil: a small wander of the trace, quicker with the mids.
    let t = u.clock4.z * (0.06 + 0.1 * u.lvl4.y);
    let boil = vec2<f32>(noise(uv * 7.0 + vec2<f32>(t, 1.3)) - 0.5, noise(uv * 7.0 + vec2<f32>(4.1, t)) - 0.5) * 0.003;
    // The ink thickens with the bass presence and on the kick, on top of
    // the Rotoscope controls (Dancer & FX).
    let col = cam_roto(uv + boil, 0.5 * u.pres4.x + 0.5 * kick_swell());
    return vec4<f32>(finite(untonemap(col)), 1.0);
}
