// A wall of LEDs: each dot's brightness rides the spectrum bin under it,
// its size pumps with the kick. Festival LED-wall look.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let n = 24.0; // dots across the short axis
    let cell = floor(p * n);
    let c = (cell + 0.5) / n;
    let f = p * n - cell - 0.5;

    // Column spectrum drives the dot; rows fall off with height.
    let bin = spec((c.x / aspect() + 0.5) * 0.8 + 0.1);
    let row_wave = sin(c.y * 8.0 - u.flow * 2.0 + cell.x * 0.4);
    let amp = bin * (0.55 + 0.45 * row_wave) * (0.7 + u.energy);
    let r = 0.16 + 0.30 * amp + u.kick * 0.08;
    let d = length(f);
    let dot = smoothstep(r, r - 0.08, d);

    let col_dot = palette(c.x * 0.5 + c.y * 0.15 + u.hue);
    var col = vec3<f32>(0.005, 0.005, 0.012);
    col += col_dot * dot * (0.25 + amp * 2.4);
    col += prev(in.uv) * 0.25; // short phosphor trail
    return vec4<f32>(col, 1.0);
}
