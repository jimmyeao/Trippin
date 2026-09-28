// @heavy — clockwork chamber: giant gears turning around the camera.
// Gears are revolved 2D SDFs — cheap to march. Teeth count steps with
// energy; rotation locks to the beat clock so it visibly *counts* time.

fn gear2d(q: vec2<f32>, teeth: f32, r_in: f32, r_out: f32) -> f32 {
    let a = angle(q) * teeth / PI;
    let tooth = abs(fract(a) - 0.5) > 0.3;
    let r = select(r_in, r_out, tooth);
    return length(q) - r;
}

fn map(p: vec3<f32>) -> vec2<f32> {
    var d = 1e5;
    var id = 0.0;
    // Two gear planes along z, plus a big ring gear.
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let zoff = (fi - 1.0) * 1.4;
        let spin = u.beat * (0.125 + fi * 0.05) * select(1.0, -1.0, i % 2 == 1);
        let q = rot(spin * TAU) * p.xy;
        let slab = abs(p.z - zoff) - 0.15;
        let teeth = 6.0 + floor(u.energy * 4.0) + fi * 2.0;
        let g = max(gear2d(q, teeth, 0.5 + fi * 0.3, 0.7 + fi * 0.3), slab);
        let hole = max(-(length(p.xy) - 0.15 - fi * 0.1), g);
        if hole < d { d = hole; id = fi; }
    }
    return vec2<f32>(d, id);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let ro = vec3<f32>(0.0, 0.0, -3.2 + sin(u.time * 0.2) * 0.4);
    var rd = normalize(vec3<f32>(p.x * 0.9, -p.y * 0.9, 1.0));

    var t = 0.0;
    var hit = -1.0;
    var pos = ro;
    for (var i = 0; i < 64; i++) {
        pos = ro + rd * t;
        let m = map(pos);
        if m.x < 0.01 { hit = m.y; break; }
        t += m.x * 0.7;
        if t > 20.0 { break; }
    }

    var col = vec3<f32>(0.01, 0.008, 0.02);
    // Ambient machine glow.
    col += palette(0.8 + u.hue) * exp(-length(rd.xy) * 2.0) * (0.1 + u.bass * 0.4);
    if hit >= 0.0 {
        let e = vec2<f32>(0.01, 0.0);
        let n = normalize(vec3<f32>(
            map(pos + e.xyy).x - map(pos - e.xyy).x,
            map(pos + e.yxy).x - map(pos - e.yxy).x,
            map(pos + e.yyx).x - map(pos - e.yyx).x));
        let dif = clamp(dot(n, normalize(vec3<f32>(0.4, 0.7, 0.5))), 0.0, 1.0);
        let gc = palette(hit * 0.33 + u.hue);
        col = gc * (0.1 + dif * (0.5 + u.energy * 0.8));
        col += gc * pow(1.0 - abs(dot(n, -rd)), 3.0) * u.high;
    }
    return vec4<f32>(col, 1.0);
}
