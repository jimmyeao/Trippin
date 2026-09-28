// Chase lights around the frame edge — like a venue trim of LEDs racing
// the beat. Four runners orbit the border on the beat clock; their tails
// stretch with energy.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);

    // Border distance and param around the frame.
    let ax = abs(p.x) / aspect();
    let ay = abs(p.y);
    let edge_d = min(1.0 - ay, 1.0 - ax);
    // Param around the perimeter: use the dominant axis.
    var t: f32;
    if ax > ay {
        t = p.y * 0.5 + 0.5 + select(0.0, 1.0, p.x > 0.0);
    } else {
        t = p.x * 0.5 + 0.5 + select(0.0, 1.0, p.y < 0.0);
    }
    t = fract(t * 0.5);

    var col = vec3<f32>(0.008, 0.006, 0.02);
    let edge_glow = exp(-edge_d * 8.0) * 0.15;
    col += palette(0.5 + u.hue) * edge_glow;

    for (var i = 0; i < 4; i++) {
        let fi = f32(i);
        let head = fract(u.beat * (0.25 + fi * 0.06) + fi * 0.25);
        let behind = fract(head - t);
        let trail = smoothstep(0.30 - u.energy * 0.1, 0.0, behind);
        let bulb = smoothstep(0.03, 0.0, behind);
        let dc = edge_d;
        col += palette(fi * 0.25 + u.hue) * (trail * 0.5 + bulb * 2.0)
               * exp(-dc * 14.0) * (0.4 + u.energy * 1.4);
    }
    // Centre wash so the frame isn't empty.
    col += palette(u.hue) * exp(-length(p) * 2.5) * (0.1 + u.bass * 0.5);
    return vec4<f32>(col, 1.0);
}
