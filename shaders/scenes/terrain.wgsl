// Spectrum terrain: a wireframe valley scrolling toward the viewer — the near
// ridge IS the spectrum, older rows recede into noise-sculpted hills.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    var col = vec3<f32>(0.0);

    // Sky: simple dusk gradient.
    col = mix(vec3<f32>(0.02, 0.01, 0.04), palette(0.75) * 0.12, uv.y);

    // Perspective: screen y maps to depth; horizon at 0.55.
    let horizon = 0.55;
    if uv.y < horizon {
        let depth = (horizon - uv.y) / horizon;              // 1 near, 0 far
        let z = 1.0 / max(depth, 0.02);                       // world depth
        let travel = u.flow * 0.6;
        let wz = z * 6.0 - travel;                            // scrolling rows
        let wx = (uv.x - 0.5) * z * aspect() * 1.4;

        // Height = spectrum (near) blending into fbm hills (far).
        let near_band = clamp(uv.x, 0.0, 1.0);
        let spec_h = spec(near_band) * (0.5 + u.intensity * 0.3);
        let hills = fbm(vec2<f32>(wx * 0.4, wz * 0.3 + u.seed)) * 0.5;
        let h = mix(hills, spec_h, depth * depth) * 0.35;

        // Surface is where the height field crosses the screen y.
        let surf = smoothstep(0.012 * z, 0.0, uv.y - (horizon - depth * (0.55 + h)));

        // Wireframe grid on the terrain.
        let gx = abs(fract(wx * 2.0) - 0.5);
        let gz = abs(fract(wz) - 0.5);
        let wire = smoothstep(0.08, 0.0, min(gx, gz));

        let fog = exp(-z * 0.25);
        col += palette(near_band * 0.6 + 0.1) * wire * surf * fog * (0.5 + u.energy);
        col += palette(0.3) * surf * fog * 0.15;

        // Beacon pulse racing down the valley on the kick.
        col += palette(0.9) * exp(-abs(depth - (1.0 - u.beat_phase)) * 8.0) * surf * u.kick * 0.4;
    } else {
        // Stars above the horizon, twinkling on highs.
        let st = step(0.998, hash21(floor(uv * vec2<f32>(300.0, 160.0))));
        col += vec3<f32>(0.8, 0.85, 1.0) * st * (0.3 + u.high * 0.7) * smoothstep(horizon, 1.0, uv.y);
    }

    return vec4<f32>(finite(col), 1.0);
}
