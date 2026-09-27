// Bokeh lights: huge soft out-of-focus orbs drifting through the dark —
// the mellow crowd-lights look for breakdowns. Sizes and brightness ride
// the spectrum; kicks gently flare the whole field.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.008, 0.009, 0.02) * (1.0 + u.intensity * 0.5);

    for (var i = 0; i < 22; i++) {
        let fi = f32(i);
        let h = hash22(vec2<f32>(fi * 3.1, fi * 1.7));
        let h3 = hash21(vec2<f32>(fi * 5.3, 8.8));

        // Slow lissajous drift, each orb on its own path.
        let cx = sin(u.time * (0.08 + h3 * 0.15) + h.x * TAU) * aspect() * 0.8;
        let cy = cos(u.time * (0.06 + h.y * 0.12) + h.y * TAU) * 0.7;
        let d = length(p - vec2<f32>(cx, cy));

        // Big soft disc: gaussian core + faint rim like a defocused lens.
        let r = 0.12 + h3 * 0.30;
        let core = exp(-d * d / (r * r) * 3.0);
        let rim = smoothstep(r, r * 0.85, d) * smoothstep(r * 0.6, r * 0.85, d) * 0.6;

        let band = h.x;
        let glow = 0.10 + spec(band) * 1.1 + u.kick * 0.15;
        col += palette(fract(h.y * 2.0 + u.hue)) * (core * 0.5 + rim) * glow;
    }

    col += prev(in.uv) * 0.25;
    return vec4<f32>(col, 1.0);
}
