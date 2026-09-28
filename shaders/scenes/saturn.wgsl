// A ringed planet hanging over the horizon: analytic sphere shading + a
// tilted ring disc, beat lights in the rings. Big, slow, iconic — a
// breather scene for breakdowns.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Starfield backdrop.
    var col = vec3<f32>(0.004, 0.004, 0.012);
    let cell = floor(p * 70.0);
    let st = hash21(cell);
    if st > 0.98 {
        let sp = fract(p * 70.0) - 0.5;
        col += vec3<f32>(0.9) * smoothstep(0.3, 0.05, length(sp))
               * (0.3 + 0.7 * sin(u.time * 2.0 + st * 40.0)) * (0.3 + u.high * 0.8);
    }

    // Planet: sphere at centre, slightly below frame centre.
    let c = vec2<f32>(0.0, -0.08); // world offset; -p.y is up so -0.08 lifts it
    let pr = vec2<f32>(p.x, -p.y) - c;
    let r = length(pr);
    let R = 0.45 + u.bass * 0.03;
    if r < R {
        let nz = sqrt(R * R - r * r) / R;
        let n = normalize(vec3<f32>(pr / R, nz));
        let l = normalize(vec3<f32>(-0.6, 0.5, 0.7));
        let dif = max(dot(n, l), 0.0);
        let band = fbm(vec2<f32>(pr.y * 8.0 + u.flow * 0.02, 0.0));
        col = palette(band * 0.3 + u.hue) * (0.15 + dif * (0.6 + u.energy * 0.7));
        // Rim light on the beat.
        col += palette(0.5 + u.hue) * pow(1.0 - nz, 3.0) * beat_pulse(4.0);
    }

    // Rings: ellipse band around the planet — tilted, occluded behind the
    // top half so they read as behind/in-front correctly (up is +world y).
    let rp = vec2<f32>(pr.x, pr.y * 3.2);
    let rr = length(rp);
    let ring_in = smoothstep(0.55, 0.58, rr) * smoothstep(0.98, 0.95, rr);
    let ring_pat = 0.5 + 0.5 * sin(rr * 60.0 + u.flow * 0.5);
    let front = step(pr.y, 0.0) + step(R, r); // in front below centre-line or outside planet
    col += palette(0.6 + u.hue) * ring_in * ring_pat * front
           * (0.3 + u.high * 0.8 + u.kick * 0.6);
    return vec4<f32>(col, 1.0);
}
