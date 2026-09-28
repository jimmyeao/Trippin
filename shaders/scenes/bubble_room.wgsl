// @heavy — a room of floating spheres: grid of orbs that rise and fall on
// the spectrum, orbited by a slow camera. Marched spheres with sharp
// reflections faked by palette bands. Highs make them shimmer.

fn map(p: vec3<f32>) -> vec2<f32> {
    let id = floor(p / 3.0 + 0.5);
    let h = hash21(id.xz + id.y * 7.0);
    var c = p - id * 3.0;
    // Bob: each cell bobs on its own beat subdivision.
    c.y -= sin(u.beat * (0.5 + h) + h * TAU) * (0.3 + h * 0.5);
    let r = 0.32 + h * 0.28 + u.kick * 0.08;
    return vec2<f32>(length(c) - r, h);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let t = u.flow * 0.5;
    let ro = vec3<f32>(sin(t * 0.3) * 1.5, 3.0 + sin(t * 0.2) * 0.5, t * 4.0);
    var rd = normalize(vec3<f32>(p.x * 0.8, -p.y * 0.7 - 0.3, 1.0));

    var d_tot = 0.0;
    var hit = -1.0;
    var pos = ro;
    for (var i = 0; i < 80; i++) {
        pos = ro + rd * d_tot;
        let m = map(pos);
        if m.x < 0.01 { hit = m.y; break; }
        d_tot += m.x * 0.8;
        if d_tot > 30.0 { break; }
    }

    var col = mix(vec3<f32>(0.01, 0.012, 0.03), palette(u.hue) * 0.2,
                  exp(-abs(rd.y + 0.2) * 4.0));
    if hit >= 0.0 {
        // Cheap lighting: normal from gradient + banded reflection.
        let e = vec2<f32>(0.01, 0.0);
        let n = normalize(vec3<f32>(
            map(pos + e.xyy).x - map(pos - e.xyy).x,
            map(pos + e.yxy).x - map(pos - e.yxy).x,
            map(pos + e.yyx).x - map(pos - e.yyx).x));
        let dif = clamp(dot(n, normalize(vec3<f32>(0.5, 0.8, -0.3))), 0.0, 1.0);
        let fres = pow(1.0 - clamp(dot(n, -rd), 0.0, 1.0), 3.0);
        let bc = palette(hit + u.hue);
        col = bc * (0.15 + dif * (0.5 + u.energy));
        col += palette(0.5 + hit + u.hue) * fres * (0.6 + u.high * 1.5);
        col *= exp(-d_tot * 0.08);
    }
    return vec4<f32>(col, 1.0);
}
