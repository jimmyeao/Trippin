// @heavy — infinite cube lattice flythrough: the camera rides through a
// repeating lattice of glowing cube frames. Bass expands the cell size,
// onsets flash whole cells at once.

fn map(p: vec3<f32>) -> vec2<f32> {
    let s = 1.4 + u.bass * 0.4;
    let id = floor(p / s + 0.5);
    let h = hash21(id.xz + id.y * 3.0);
    let c = p - id * s;
    // Cube frame: box minus thinner box → edges only.
    let b = abs(c) - 0.22;
    let box_d = max(max(b.x, b.y), b.z);
    let inner = max(max(abs(c.x), abs(c.y)), abs(c.z)) - 0.16;
    let frame = max(box_d, -inner);
    return vec2<f32>(frame, h);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.flow * 3.0;
    let ro = vec3<f32>(sin(z * 0.06) * 0.7, -0.7 + sin(z * 0.05) * 0.3, z);
    var rd = normalize(vec3<f32>(p.x * 0.8, -p.y * 0.7, 1.0));
    rd = vec3<f32>(rot(sin(z * 0.04) * 0.15) * rd.xy, rd.z);

    var t = 0.0;
    var hit = 0.0;
    var pos = ro;
    for (var i = 0; i < 80; i++) {
        pos = ro + rd * t;
        let m = map(pos);
        if m.x < 0.012 { hit = m.y; break; }
        t += m.x * 0.7;
        if t > 30.0 { break; }
    }

    var col = vec3<f32>(0.008, 0.01, 0.025);
    if t < 30.0 {
        let fade = exp(-t * 0.2);
        let flash = step(0.8, fract(hit + u.beat * 0.125)) * u.onset * 2.0;
        col = mix(col, palette(hit * 1.3 + u.hue) * (0.5 + u.energy + flash), fade);
    }
    // Distance fog toward the vanishing point.
    col += palette(0.7 + u.hue) * (1.0 - exp(-t * 0.1)) * 0.15;
    return vec4<f32>(col, 1.0);
}
