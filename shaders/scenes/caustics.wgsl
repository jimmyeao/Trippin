// Swimming-pool caustics: the classic tiled interference shimmer, drifting
// slowly. Mids sharpen the filaments, kick flashes the floor lights.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Direction: the water drifts one way then back; energy speeds it.
    var q = p * 3.0 + vec2<f32>(2.0 * sin(u.clock4.x * 0.02) + u.clock4.x * 0.03, u.clock4.x * 0.04);

    // Iterated interference — the caustic filament pattern.
    var v = 0.0;
    var amp = 1.0;
    var t = 0.0;
    for (var i = 0; i < 4; i++) {
        let fi = f32(i);
        // Shape: the filaments knot tighter with bass presence.
        let warp = 0.25 + 0.3 * u.pres4.x;
        q += vec2<f32>(sin(q.y * 1.7 + t) * warp, cos(q.x * 1.5 + t) * warp) * amp;
        v += abs(sin(q.x) * sin(q.y)) * amp;
        t += 1.7 + u.mid;
        amp *= 0.65;
        q = rot(0.5) * q * 1.4;
    }
    v = 1.0 - v / 2.34;
    let fil = pow(clamp(v, 0.0, 1.0), 7.0 - u.mid * 3.0);

    // Water colour grades by palette; deeper blue in the valleys.
    let depth = smoothstep(0.2, 0.9, fbm(p * 2.0 + u.flow * 0.02));
    var col = mix(vec3<f32>(0.005, 0.02, 0.05), palette(0.55 + u.hue) * 0.3, depth);
    col += palette(0.6 + u.hue) * fil * (0.4 + u.energy * 1.3);
    col += vec3<f32>(1.0) * fil * u.kick * 0.5;
    return vec4<f32>(col, 1.0);
}
