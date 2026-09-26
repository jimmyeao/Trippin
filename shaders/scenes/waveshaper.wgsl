// A fat neon ribbon folded by the spectrum, mirrored in a dark floor like
// water. The seam between ribbon and reflection catches the kick.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let mirror = uv.y < 0.42;
    let yy = select(uv.y, 0.84 - uv.y, mirror); // reflect below the waterline

    // Spectrum folded into a height field with a slow drift.
    let v = spec(uv.x) * (0.9 + u.intensity * 0.3);
    let drift = fbm(vec2<f32>(uv.x * 3.0 - u.flow * 0.1, u.seed)) * 0.06;
    let h = 0.55 + v * 0.32 + drift;

    // Ribbon body and its glowing edge.
    let d = yy - h;
    let body = smoothstep(0.01, -0.15, d);
    let edge = exp(-abs(d) * 60.0) * (0.7 + u.kick * 0.5);

    var col = palette(uv.x * 0.6 + 0.2) * body * (0.15 + v * 0.9);
    col += palette(uv.x * 0.6 + 0.6) * edge;

    // Horizontal filaments inside the ribbon — a layered aurora look.
    let fil = sin((yy - h) * 120.0 + u.flow * 2.0 + v * 20.0) * 0.5 + 0.5;
    col += palette(uv.x + 0.4) * body * fil * v * 0.35;

    if mirror {
        col *= exp(-(0.42 - uv.y) * 3.5) * 0.55; // depth fade
        // Ripple distortion bands.
        col *= 0.85 + 0.15 * sin(uv.y * 300.0 + u.flow * 3.0);
    }

    // Waterline shimmer.
    col += vec3<f32>(0.5, 0.6, 0.8) * exp(-abs(uv.y - 0.42) * 200.0) * (0.1 + u.kick * 0.25);

    // Sky haze above.
    col += palette(0.7) * smoothstep(h, 1.0, yy) * 0.03 * (0.5 + u.high);

    return vec4<f32>(finite(col), 1.0);
}
