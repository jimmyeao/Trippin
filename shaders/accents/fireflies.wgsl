// Fireflies (accent layer): soft points of light wandering on smooth loops,
// each glowing up and going dark on its own slow cycle (never on the beat).
// They wander on the smooth energy clock (motion), the swarm drifts left and
// right over eight bars (direction), and their glow widens with the slow mid
// presence (shape). Twenty-four flies, always twenty-four, mostly low in the
// frame.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.0);
    let t = u.clock4.x * 0.22;
    let drift = 0.25 * sin(TAU * u.flow / 32.0);
    let rad = 0.010 + 0.006 * u.pres4.y;
    for (var i = 0; i < 24; i++) {
        let fi = f32(i);
        let h = hash22(vec2<f32>(fi, 77.0));
        let w = 0.5 + hash22(vec2<f32>(fi, 13.0));
        var pos = vec2<f32>((h.x - 0.5) * 2.0 * aspect() * 0.95 + drift, 0.85 - h.y * h.y * 1.4);
        pos += vec2<f32>(0.16 * sin(t * w.x + fi * 1.7), 0.09 * sin(t * w.y * 1.3 + fi * 2.9));
        let d = length(p - pos);
        if d > 0.25 {
            continue;
        }
        let b0 = sin(u.time * (0.6 + 0.5 * w.x) + fi * 4.1);
        let blink = smoothstep(0.1, 0.9, b0);
        if blink <= 0.0 {
            continue;
        }
        let core = smoothstep(rad, rad * 0.2, d);
        let glow = exp(-d / (rad * 3.5)) * 0.35 + exp(-d * 18.0) * 0.08;
        let tone = palette(0.28 + 0.12 * h.x);
        col += (tone * tone * glow + mix(tone, vec3<f32>(1.0), 0.6) * core) * blink;
    }
    return vec4<f32>(finite(col), 1.0);
}
