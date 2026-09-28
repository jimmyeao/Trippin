// @heavy — clockwork chamber: giant gears meshing around the camera,
// turning visibly on the beat clock. Hot rim light, spatter of sparks on
// onsets, molten glow behind.

fn gear2d(q: vec2<f32>, teeth: f32, r_in: f32, r_out: f32, spin: f32) -> f32 {
    let a = angle(q) + spin;
    let tooth = abs(fract(a * teeth / PI) - 0.5) > 0.30;
    let r = select(r_in, r_out, tooth);
    // Round the tooth profile a touch so it reads as machined.
    let flank = min(abs(fract(a * teeth / PI) - 0.5) - 0.30, 0.0);
    return length(q) - r + flank * 0.4;
}

fn map(p: vec3<f32>) -> vec2<f32> {
    var d = 1e5;
    var id = 0.0;
    // Interleaved gear planes; adjacent gears counter-rotate (meshed).
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let zoff = (fi - 1.0) * 1.3;
        let dir = select(1.0, -1.0, i % 2 == 1);
        // Meshing: rotation ratio inversely proportional to size.
        let spin = u.beat * 0.25 * dir / (1.0 + fi * 0.5);
        let qc = vec2<f32>(p.x + (fi - 1.0) * 1.1, p.y);
        let slab = abs(p.z - zoff) - 0.14;
        let teeth = 7.0 + fi * 3.0;
        let g = max(gear2d(rot(spin * TAU) * qc, teeth, 0.6 + fi * 0.35,
                           0.82 + fi * 0.35, spin), slab);
        let hole = max(-(length(qc) - 0.12 - fi * 0.08), g);
        if hole < d { d = hole; id = fi; }
    }
    return vec2<f32>(d, id);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let ro = vec3<f32>(sin(u.time * 0.11) * 0.5, sin(u.time * 0.09) * 0.3,
                       -3.4 + sin(u.time * 0.2) * 0.4);
    var rd = normalize(vec3<f32>(p.x * 0.9, -p.y * 0.9, 1.0));

    var t = 0.0;
    var hit = -1.0;
    var pos = ro;
    for (var i = 0; i < 72; i++) {
        pos = ro + rd * t;
        let m = map(pos);
        if m.x < 0.008 { hit = m.y; break; }
        t += m.x * 0.75;
        if t > 20.0 { break; }
    }

    // Molten furnace glow behind the works.
    var col = vec3<f32>(0.015, 0.008, 0.02);
    col += palette(0.95 + u.hue) * exp(-length(p + vec2<f32>(0.0, -0.3)) * 2.2)
           * (0.3 + u.bass * 0.7);

    if hit >= 0.0 {
        let e = vec2<f32>(0.008, 0.0);
        let n = normalize(vec3<f32>(
            map(pos + e.xyy).x - map(pos - e.xyy).x,
            map(pos + e.yxy).x - map(pos - e.yxy).x,
            map(pos + e.yyx).x - map(pos - e.yyx).x));
        let l = normalize(vec3<f32>(0.3, 0.5, -0.6));
        let dif = clamp(dot(n, l), 0.0, 1.0);
        let spec_h = pow(clamp(dot(reflect(rd, n), l), 0.0, 1.0), 24.0);
        let fres = pow(1.0 - clamp(dot(n, -rd), 0.0, 1.0), 3.0);
        let gc = palette(hit * 0.3 + u.hue);
        col = gc * (0.06 + dif * (0.35 + u.energy * 0.5));
        col += vec3<f32>(1.0, 0.9, 0.8) * spec_h * (0.6 + u.high);
        col += gc * fres * (0.8 + u.kick * 1.5);
        col *= exp(-t * 0.06);
        // Sparks at the mesh points on onsets.
        col += vec3<f32>(1.0, 0.6, 0.2) * u.onset * exp(-abs(pos.y) * 6.0) * 0.4;
    }
    return vec4<f32>(col, 1.0);
}
