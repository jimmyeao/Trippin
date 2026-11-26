// Sparkles (accent layer): four-pointed star glints scattered over the frame,
// each fading in and out on its own slow cycle (never on the beat). The
// spikes reach further as the slow high presence swells (shape), the stars
// turn on the smooth beat clock and the field drifts on the energy clock
// (motion). Sparse and sharp: glints on glass, not glitter.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.0);
    let scale = 3.4;
    let q = (p + vec2<f32>(u.clock4.x * 0.02, 0.01 * sin(TAU * u.flow / 32.0))) * scale;
    let base = floor(q);
    let reach = 0.55 + 0.6 * u.pres4.w;
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let cell = base + vec2<f32>(f32(i), f32(j));
            let h = hash22(cell + vec2<f32>(23.0, 7.0));
            if h.x > 0.35 {
                continue;
            }
            let c = cell + 0.5 + (hash22(cell + vec2<f32>(2.0, 11.0)) - 0.5) * 0.6;
            // Twinkle: mostly dark, a soft swell every few seconds.
            let tw0 = 0.5 + 0.5 * sin(u.time * (0.5 + 0.7 * h.y) + h.x * 60.0);
            let tw = tw0 * tw0 * tw0 * tw0;
            if tw < 0.01 {
                continue;
            }
            let r = rot(u.flow * 0.05 * (h.y - 0.5) + h.x * 3.0) * (q - c);
            let ax = abs(r.x);
            let ay = abs(r.y);
            let len = reach * (0.6 + 0.6 * h.y);
            let spikes = exp(-ax * 70.0) * exp(-ay * 4.0 / len) + exp(-ay * 70.0) * exp(-ax * 4.0 / len);
            // Fainter diagonals make it read as a lens star.
            let dg = rot(0.785) * r;
            let diag = (exp(-abs(dg.x) * 90.0) * exp(-abs(dg.y) * 9.0 / len)
                     + exp(-abs(dg.y) * 90.0) * exp(-abs(dg.x) * 9.0 / len)) * 0.35;
            let core = exp(-length(r) * 22.0);
            let tone = mix(vec3<f32>(1.0), palette(0.3 + h.y * 0.5), 0.55);
            col += tone * (spikes + diag + core * 1.2) * tw * 0.9;
        }
    }
    return vec4<f32>(finite(col), 1.0);
}
