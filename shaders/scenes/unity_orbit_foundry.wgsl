// Unity Orbit Foundry: a molten core in precessing gimbal rings under
// welding beams, rendered by the external Unity engine. Undo present's
// exposure and ACES for passthrough.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let y = clamp(textureSampleLevel(ext_tex, samp, in.uv, 0.0).rgb, vec3<f32>(0.0), vec3<f32>(0.985));
    let a = 2.51 - 2.43 * y;
    let b = 0.03 - 0.59 * y;
    let x = (-b + sqrt(b * b + 4.0 * a * 0.14 * y)) / (2.0 * a);
    let expo = 0.75 + 0.35 * u.intensity;
    return vec4<f32>(x / expo, 1.0);
}
