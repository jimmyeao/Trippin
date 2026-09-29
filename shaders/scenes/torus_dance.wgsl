// @heavy — a torus knot dancing in place: raymarched trefoil knot spinning
// on its axis, band-lit by the spectrum. The knot tightens with energy —
// a sculptural centre-piece scene.

fn knot(q: vec3<f32>) -> f32 {
    // Torus-knot distance estimate: near a (2,3) curve laid on a torus.
    let r = length(q.xy);
    let t = vec2<f32>(r - 1.0, q.z);
    let a = atan2(q.y, q.x);
    // Twist the tube cross-section along the knot.
    // Shape: the tube thickens with bass presence and jolts on kicks.
    let cross = rot(a * 1.5 + u.clock4.z * 0.8) * t;
    return length(cross) - (0.24 + u.pres4.x * 0.14 + u.hits4.x * 0.06);
}

fn map(p: vec3<f32>) -> f32 {
    let s = 0.85 + u.energy * 0.2;
    var q = p;
    // Direction: spins one way, then back.
    let qxz = rot(4.0 * sin(u.clock4.x * 0.02) + u.clock4.x * 0.1) * q.xz;
    let qxy = rot(sin(u.time * 0.3) * 0.4) * vec2<f32>(qxz.x, q.y);
    q = vec3<f32>(qxy.x, qxy.y, qxz.y);
    return knot(q * s) / s;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let ro = vec3<f32>(0.0, 0.0, -3.0 - u.bass * 0.4);
    var rd = normalize(vec3<f32>(p.x * 0.85, -p.y * 0.85, 1.0));

    var t = 0.0;
    var hit = false;
    var pos = ro;
    for (var i = 0; i < 80; i++) {
        pos = ro + rd * t;
        let d = map(pos);
        if d < 0.008 { hit = true; break; }
        t += d * 0.8;
        if t > 12.0 { break; }
    }

    var col = vec3<f32>(0.008, 0.008, 0.02);
    col += palette(0.7 + u.hue) * exp(-length(p) * 2.0) * 0.1;
    if hit {
        let e = vec2<f32>(0.008, 0.0);
        let n = normalize(vec3<f32>(
            map(pos + e.xyy) - map(pos - e.xyy),
            map(pos + e.yxy) - map(pos - e.yxy),
            map(pos + e.yyx) - map(pos - e.yyx)));
        let dif = clamp(dot(n, normalize(vec3<f32>(0.6, 0.7, 0.4))), 0.0, 1.0);
        let fres = pow(1.0 - clamp(dot(n, -rd), 0.0, 1.0), 3.0);
        // Spectrum bands sweep around the knot.
        let band = spec(fract(pos.x * 0.4 + pos.y * 0.4 + u.flow * 0.1));
        col = palette(band + u.hue) * (0.15 + dif * (0.6 + u.energy));
        col += palette(0.5 + u.hue) * fres * (0.8 + u.high * 1.5);
        col += vec3<f32>(1.0) * fres * u.kick * 0.4;
    }
    return vec4<f32>(col, 1.0);
}
