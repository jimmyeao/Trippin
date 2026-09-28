// Liquid stripes: diagonal bands flowing and wobbling, widths modulated by
// the spectrum. A calm scene for mid-energy passages.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var p = centred(in.uv);
    let wob = noise(p * 2.0 + u.flow * 0.2) * 0.6;
    let d = p.x * 0.8 + p.y * 0.6 + wob + u.flow * 0.25;

    let bands = 10.0;
    let b = fract(d * bands);
    let width = 0.5 + (spec(b) - 0.5) * 0.4;
    let stripe = smoothstep(width, width - 0.15, abs(b - 0.5));

    let edge = smoothstep(0.05, 0.02, abs(b - 0.5) - width + 0.1);
    var col = vec3<f32>(0.01, 0.008, 0.02);
    col += palette(d * 0.4 + u.hue) * stripe * (0.2 + u.energy * 1.1);
    col += palette(0.5 + u.hue) * max(edge - stripe, 0.0) * u.kick * 1.5;
    col += prev(in.uv) * 0.18;
    return vec4<f32>(col, 1.0);
}
