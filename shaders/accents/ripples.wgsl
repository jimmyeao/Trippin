// Ripples (accent layer): soft rings of light spreading out across the frame
// and fading, like rain on still water. Each ring grows on the smooth bass
// energy clock (motion) and reaches further as the bass presence swells
// (shape); every ring is born somewhere new. Six rings in flight, always six,
// on staggered phases so they never pulse together.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.0);
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        // Each ring's life: one cycle every 4 beats of the bass clock.
        let ph = u.clock4.y * 0.25 + fi / 6.0;
        let life = fract(ph);
        let gen = floor(ph);
        let h = hash22(vec2<f32>(gen * 7.0 + fi, fi * 3.0));
        let c = (h - 0.5) * vec2<f32>(2.0 * aspect() * 0.8, 1.6);
        let reach = 0.55 + 0.35 * u.pres4.x;
        let r = life * reach;
        let d = abs(length(p - c) - r);
        let ring = exp(-d * 70.0) + exp(-d * 18.0) * 0.25;
        // In from nothing, out to nothing.
        let fade = smoothstep(0.0, 0.1, life) * (1.0 - life) * (1.0 - life);
        let tone = palette(0.45 + h.x * 0.3);
        col += tone * tone * ring * fade * 0.8;
    }
    return vec4<f32>(finite(col), 1.0);
}
