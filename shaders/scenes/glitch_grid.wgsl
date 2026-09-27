// Glitch wall: a grid of luminous tiles that tear sideways and strobe on
// onsets — the club-VJ "LED wall glitching" look. Rows shear on hard hits,
// random cells flash with the beat, and the spectrum sweeps the tile hues.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var uv = in.uv;
    let p = centred(uv);

    // Horizontal tears: whole rows shear on onsets.
    let row = floor(uv.y * 28.0);
    let tear_seed = vec2<f32>(row, floor(u.beat * 2.0));
    let tear = step(0.75, hash21(tear_seed)) * (hash21(tear_seed + 7.0) - 0.5) * u.onset * 0.6;
    uv.x += tear;

    // Tile grid.
    let g = uv * vec2<f32>(14.0, 8.0);
    let cell = floor(g);
    let f = fract(g) - 0.5;
    let h = hash21(cell);
    let h2 = hash21(cell + 91.7);

    // Rounded-square tile mask with a dark grout gap.
    let box_d = max(abs(f.x), abs(f.y));
    let tile = smoothstep(0.48, 0.40, box_d);

    // Each tile rides its own spectrum band; sparse tiles strobe on the beat.
    let band = h;
    var lit = 0.06 + spec(band) * 0.9;
    if h2 < 0.12 {
        lit += beat_pulse(9.0) * (1.0 + u.kick) * 2.0;
    }
    // Occasional dead tiles read as dropped pixels.
    if h2 > 0.94 {
        lit *= 0.15;
    }

    var col = palette(h * 0.7 + u.hue) * lit * tile;
    // Scanline shimmer.
    col *= 0.85 + 0.15 * sin(uv.y * u.res_y * 0.8 + u.time * 8.0);

    // Feedback: slight zoom-smear keeps the glitch residue for a frame.
    col += prev(uncentred(centred(in.uv) * 0.997)) * 0.28;
    return vec4<f32>(col, 1.0);
}
