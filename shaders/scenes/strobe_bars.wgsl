// Strobe towers: a wall of light columns that GROW upward on their
// spectrum band — audio drives the movement itself (columns rise and fall
// with their band), not just brightness. Onsets punch a column to full
// height and fire a crown flash.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let up = -p.y * 0.5 + 0.5; // 0 at bottom edge, 1 at top

    let cols = 20.0;
    let cx = floor((p.x / aspect() + 0.5) * cols);
    let fx = fract((p.x / aspect() + 0.5) * cols);
    let bar_t = cx / cols;

    // Column height = its spectrum band, plus a kick lift and onset spike.
    let band = spec(bar_t);
    let onset_cell = step(0.7, hash21(vec2<f32>(cx, floor(u.beat * 4.0))));
    let h = 0.08 + band * (0.55 + u.energy * 0.35) + u.kick * 0.15
          + u.onset * onset_cell * 0.25;

    let bar_w = smoothstep(0.0, 0.12, fx) * smoothstep(1.0, 0.88, fx);
    let inside = step(up, h);
    // Bright core + hot top edge + a flare above the lip.
    let core = inside * (0.25 + band * 0.9);
    let edge = exp(-abs(up - h) * 60.0) * (0.6 + u.kick);
    // Flare hugging the lip of each column.
    let flare = exp(-abs(up - h) * 12.0) * 0.3;

    var col = vec3<f32>(0.006, 0.006, 0.018);
    col += palette(bar_t + u.hue) * bar_w * (core + edge + flare);
    // Ground glow where the columns meet the floor.
    let refl = exp(-up * 6.0) * bar_w * band * 0.2;
    col += palette(bar_t + u.hue) * refl;
    return vec4<f32>(finite(col), 1.0);
}
