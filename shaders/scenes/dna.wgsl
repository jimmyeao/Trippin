// A double helix rotating upright through the frame — rungs glow on the
// spectrum bands, the whole thing sways with the phrase. World up is -p.y.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.006, 0.008, 0.02);

    // Helix winds along the vertical axis; +screen y is down, so "up the
    // helix" is -p.y.
    let hy = -p.y;
    let tilt = rot(sin(u.time * 0.2) * 0.15);
    let pp = tilt * p;

    let turns = 2.0;
    for (var s = 0; s < 2; s++) {
        let fs = f32(s);
        // Strand phase: y drives the sine, the second strand is pi off.
        let ph = hy * turns * TAU + u.flow * 1.5 + fs * PI;
        let sx = sin(ph) * 0.4;
        let depth = cos(ph); // -1..1 front-back
        let dx = pp.x - sx;
        let strand = exp(-dx * dx * 900.0);
        // Front strands brighter; back strands dimmed for depth.
        let shade = mix(0.3, 1.0, depth * 0.5 + 0.5);
        col += palette(fs * 0.5 + u.hue) * strand * shade * (0.4 + u.energy * 1.3);
    }

    // Rungs: horizontal links where both strands approach.
    let rung_y = fract(hy * turns + u.flow * 0.24);
    let rung = smoothstep(0.06, 0.02, abs(rung_y - 0.5));
    let width = abs(cos(hy * turns * TAU + u.flow * 1.5)) * 0.4;
    let rung_in = smoothstep(width, width * 0.5, abs(pp.x));
    let band = spec(fract(hy * 0.5 + 0.5));
    col += palette(0.25 + u.hue) * rung * rung_in * (0.2 + band * 1.8);
    col += prev(in.uv) * 0.15;
    return vec4<f32>(col, 1.0);
}
