// @bloom 0.6 @tonemap agx
// Cymatics: a dark metal plate sprinkled with glowing sand, vibrating with
// the music. Sand settles on the plate's nodal lines — Chladni figures —
// so the pattern is the sound made visible. The mode numbers (n, m) are
// picked from the loudest band and glide between figures, so each part of
// the track draws its own geometry. 2D, cheap (not @heavy).
// Audio vocabulary:
//  - the Chladni mode drifts on the mid clock and jumps family with the
//    dominant band (smoothly cross-faded, never a snap);
//  - sand brightness follows the level; bass hits shake the sand (lines
//    blur and re-settle); high hits sparkle grains;
//  - the plate glows faintly with bass presence.

fn chladni(p: vec2<f32>, n: f32, m: f32) -> f32 {
    // Square-plate modes; |f| near 0 = nodal lines where sand collects.
    return cos(n * PI * p.x) * cos(m * PI * p.y) - cos(m * PI * p.x) * cos(n * PI * p.y);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let pc = centred(in.uv);
    // Plate occupies a centred square; slowly rotate it on the mid clock.
    let q = rot(u.clock4.z * 0.01) * pc / 0.72;
    let edge = max(abs(q.x), abs(q.y));
    var col = vec3<f32>(0.003, 0.003, 0.005);
    if edge < 1.0 {
        // Mode pair: two families, blended by which band dominates.
        let lo = u.lvl4.x + u.lvl4.y * 0.5;
        let hi = u.lvl4.z + u.lvl4.w;
        let w = smoothstep(-0.3, 0.3, hi - lo);
        let drift = u.clock4.z * 0.02;
        let n1 = 2.0 + 2.0 * (0.5 + 0.5 * sin(drift));
        let m1 = 5.0 + 1.5 * (0.5 + 0.5 * cos(drift * 0.7));
        let n2 = 5.0 + 2.0 * (0.5 + 0.5 * sin(drift * 1.3));
        let m2 = 9.0 + 2.0 * (0.5 + 0.5 * cos(drift * 0.9));
        // Bass hits shake the plate: jitter the sample point.
        let shake = u.hits4.x * 0.02 * vec2<f32>(sin(in.pos.y * 0.9 + u.time * 60.0), cos(in.pos.x * 0.8 + u.time * 55.0));
        let f = mix(chladni(q + shake, n1, m1), chladni(q + shake, n2, m2), w);
        // Sand: grains along the nodal lines; line width grows with level.
        let width = 0.05 + 0.1 * u.lvl4.x + 0.12 * u.hits4.x;
        let line = exp(-abs(f) / width);
        let grain = hash21(floor(in.pos.xy * 0.7));
        let sand = line * smoothstep(0.2, 0.9, grain + line * 0.5);
        let hue = u.clock4.w * 0.008 + u.hue;
        let drive = 0.5 + 0.8 * u.intensity;
        // Brushed dark plate with a faint bass-presence glow.
        let brushed = 0.6 + 0.4 * hash21(vec2<f32>(floor(in.pos.y), 3.0));
        col = vec3<f32>(0.012, 0.012, 0.016) * brushed + palette(hue + 0.6) * 0.03 * u.pres4.x;
        col += palette(hue + (1.0 - line) * 0.2) * sand * (0.4 + 1.4 * u.lvl4.y + 0.6 * u.lvl4.x) * drive;
        // Sparkling grains on high hits.
        col += vec3<f32>(1.0) * step(0.985, grain) * line * u.hits4.w * 2.0;
        // Plate rim.
        col += palette(hue) * smoothstep(0.015, 0.0, abs(edge - 0.99)) * 0.4;
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
