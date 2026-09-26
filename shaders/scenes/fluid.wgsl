// Pure feedback: the last frame is advected through a noise curl field while
// three emitters (bass / mids / highs) inject colour, and each beat sends a
// ripple out from the centre.

fn emitter(p: vec2<f32>, centre: vec2<f32>, size: f32) -> f32 {
    let d = length(p - centre);
    return exp(-d * d / (size * size));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let p = centred(uv);
    let t = u.time * 0.15 + u.seed * 0.1;

    let e = 0.02;
    let s = p * 1.2;
    let n0 = fbm(s + t);
    let nx = fbm(s + vec2<f32>(e, 0.0) + t);
    let ny = fbm(s + vec2<f32>(0.0, e) + t);
    let curl = vec2<f32>(ny - n0, n0 - nx) / e;

    let radial = normalize(p + 1e-4) * (u.kick * 0.012 + 0.001);
    let flow = curl * (0.0025 + 0.006 * u.intensity) + radial;
    var c = prev(uncentred(p - flow)) * (0.965 + 0.02 * u.intensity);

    let orbit = u.time * 0.4;
    let c1 = vec2<f32>(sin(orbit), cos(orbit * 0.7)) * 0.6;
    let c2 = vec2<f32>(sin(orbit * 1.3 + 2.0), cos(orbit * 0.9 + 1.0)) * 0.8;
    let c3 = vec2<f32>(sin(orbit * 0.8 + 4.0), cos(orbit * 1.1 + 3.0)) * 0.7;
    c += palette(0.0 + t * 0.1) * emitter(p, c1, 0.06 + 0.12 * u.bass) * u.bass * 0.25;
    c += palette(0.33 + t * 0.1) * emitter(p, c2, 0.05 + 0.08 * u.mid) * u.mid * 0.2;
    c += palette(0.66 + t * 0.1) * emitter(p, c3, 0.03 + 0.05 * u.high) * u.high * 0.2;

    let ring = abs(length(p) - u.beat_phase * 1.6);
    c += palette(u.beat * 0.1) * smoothstep(0.03, 0.0, ring) * beat_pulse(2.5) * (0.1 + 0.3 * u.bass);
    return vec4<f32>(min(c, vec3<f32>(6.0)), 1.0);
}
