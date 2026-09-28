// Counter-rotating pinwheels: two interleaved fans spinning in opposite
// directions, their blades bending with the mids. Mesmerising mid-energy
// filler that still moves with the track.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var p = centred(in.uv);
    p *= 1.0 + u.kick * 0.06;
    let r = length(p);
    let a = angle(p);

    let blades = 5.0;
    let bend = sin(r * 9.0 - u.flow * 0.6) * 0.3 * (0.4 + u.mid);
    let a1 = sin((a + bend + u.flow * 0.3) * blades);
    let a2 = sin((a - bend - u.flow * 0.22) * blades);

    var col = vec3<f32>(0.01, 0.008, 0.02);
    let b1 = smoothstep(0.1, 0.5, a1) * exp(-r * 1.1);
    let b2 = smoothstep(0.1, 0.5, a2) * exp(-r * 1.4);
    col += palette(u.hue) * b1 * (0.3 + u.energy * 1.2);
    col += palette(0.5 + u.hue) * b2 * (0.3 + u.energy * 1.0);
    // Hub.
    col += vec3<f32>(0.8, 0.85, 1.0) * exp(-r * 12.0) * (0.5 + u.kick * 2.0);
    col *= smoothstep(1.5, 0.8, r);
    return vec4<f32>(col, 1.0);
}
