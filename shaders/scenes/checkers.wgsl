// A checkerboard that won't sit still: cell size steps with energy, the
// board wobbles like a flag, alternating cells light with bass and highs.
// Op-art strobe feel.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var p = centred(in.uv);
    // Flag wobble.
    p.y += sin(p.x * 4.0 + u.flow * 1.5) * 0.05 * (0.5 + u.bass);
    p = rot(sin(u.time * 0.13) * 0.12) * p;

    let n = 6.0 + floor(u.energy * 4.0) * 2.0;
    let cell = floor(p * n);
    let parity = abs(i32(cell.x + cell.y) % 2);
    let local = fract(p * n) - 0.5;

    // Alternating cells get different band drives.
    let a = f32(parity) * u.bass + (1.0 - f32(parity)) * u.high;
    let border = smoothstep(0.5, 0.45, max(abs(local.x), abs(local.y)));

    var col = vec3<f32>(0.008, 0.008, 0.02);
    col += palette(f32(parity) * 0.5 + u.hue) * border * (0.15 + a * 2.2 + u.onset * 0.6);
    col += prev(in.uv) * 0.12;
    return vec4<f32>(col, 1.0);
}
