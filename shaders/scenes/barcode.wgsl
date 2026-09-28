// A living barcode: vertical strips whose widths drift with time and whose
// brightness is the spectrum under them. Onsets scroll it; the whole frame
// is a data-strip equaliser.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let scroll = p.x + u.flow * 0.15;

    // Irregular strip boundaries via hashed cells.
    let scale = 40.0;
    let cell = floor(scroll * scale);
    let w = hash21(vec2<f32>(cell, floor(u.seed)));
    let inside = fract(scroll * scale);

    let on = step(w, 0.62);
    let bin = spec(fract(scroll * 0.35 + 0.5));
    var amp = bin * (0.5 + u.energy * 1.5);

    // Horizontal scanline modulation for texture.
    let scan = 0.8 + 0.2 * sin(p.y * 90.0);
    let strip_col = palette(w * 3.0 + u.hue);
    var col = vec3<f32>(0.006, 0.006, 0.015);
    col += strip_col * on * smoothstep(0.0, 0.06, inside) * smoothstep(1.0, 0.94, inside) * amp * scan * 2.2;

    // Bright scan sweep on each beat.
    let sweep = exp(-abs(fract(scroll * 0.5 - u.beat * 0.25) - 0.5) * 30.0);
    col += vec3<f32>(1.0) * sweep * 0.3 * u.intensity;
    return vec4<f32>(col, 1.0);
}
