// Fog (accent layer): low haze rolling along the bottom of the frame, as a
// fog machine leaves on a dance floor, lit in the palette. It rolls sideways
// on the smooth energy clock (motion), the roll reverses over eight bars
// (direction), and the bank rises higher as the slow bass presence swells
// (shape).

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // +y is down: the bank's top edge, in centred units.
    let top = 0.35 - 0.3 * u.pres4.x;
    let h = p.y - top;
    if h < -0.35 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let roll = u.clock4.x * 0.03 + 0.4 * sin(TAU * u.flow / 32.0);
    let q = vec2<f32>(p.x * 0.9 + roll, p.y * 1.6 - u.clock4.x * 0.01);
    let n = fbm(q * 1.4 + vec2<f32>(0.0, u.time * 0.02));
    let n2 = noise(q * 3.0 - vec2<f32>(roll * 1.5, 0.0));
    // Denser low, wispy at the top edge.
    let edge = top + (n - 0.5) * 0.4;
    let dens = smoothstep(edge - 0.15, edge + 0.35, p.y) * (0.5 + 0.7 * n) * (0.7 + 0.5 * n2);
    let tone = palette(0.62 + 0.25 * n + 0.1 * p.x / aspect());
    return vec4<f32>(finite(tone * tone * dens * 0.6), 1.0);
}
