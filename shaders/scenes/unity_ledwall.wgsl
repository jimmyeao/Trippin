// @title Unity LED Wall
// Unity ledwall: a wall of LED cells showing live patterns. A screen-content show from the external Unity engine (Spout sender "Trippin Stage", see
// unity/README.md), driven live by Trippin's show-state feed. Only in rotation while frames
// are arriving. The frame is already tonemapped, so this undoes present's exposure + ACES
// to hand it through unchanged — dancer, text, strobe, FX and outputs still layer on top.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let y = clamp(textureSampleLevel(ext_tex, samp, in.uv, 0.0).rgb, vec3<f32>(0.0), vec3<f32>(0.985));
    // Inverse of aces(x) = x(2.51x+0.03) / (x(2.43x+0.59)+0.14): solve the
    // quadratic (2.51-2.43y)x^2 + (0.03-0.59y)x - 0.14y = 0 for x >= 0.
    let a = 2.51 - 2.43 * y;
    let b = 0.03 - 0.59 * y;
    let x = (-b + sqrt(b * b + 4.0 * a * 0.14 * y)) / (2.0 * a);
    let expo = 0.75 + 0.35 * u.intensity;
    return vec4<f32>(x / expo, 1.0);
}
