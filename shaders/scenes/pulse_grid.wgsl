// Pulse grid: an LED-tile floor. Each column's tiles light to its band energy
// and the whole floor hops on the kick — like a stadium floor in a drop.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Fake perspective: squash rows toward the horizon at uv.y = 0.5.
    let persp = uv.y * uv.y * 0.4 + uv.y * 0.6;      // denser tiles low down
    let cols = 24.0;
    let rows = 14.0;
    let cx = floor(uv.x * cols);
    let cy = floor(persp * rows);
    let f = vec2<f32>(fract(uv.x * cols), fract(persp * rows)) - 0.5;

    let band = (cx + 0.5) / cols;
    let v = spec(band);
    let row_level = cy / rows;

    // Tiles light from the front (bottom of screen) up to the band height.
    let lit = step(row_level, v * 1.15);
    // Kick bounce: tiles get rounder and hotter for a moment.
    let shrink = 0.06 + beat_pulse(10.0) * 0.10;
    let tile = smoothstep(0.5 - shrink, 0.5 - shrink - 0.08, max(abs(f.x), abs(f.y)));

    var col = palette(band * 0.7 + row_level * 0.3) * tile * lit * (0.3 + v * 0.9);
    // Unlit tiles keep a faint outline so the grid reads.
    col += palette(band) * tile * 0.03;

    // Travelling beat wavefront across the floor.
    let wf = fract(u.beat_phase);
    col += palette(0.5) * tile * exp(-abs(row_level - (1.0 - wf)) * 10.0) * 0.4;

    // Sky glow behind the floor.
    col += palette(0.75) * smoothstep(0.5, 1.0, uv.y) * (0.05 + u.high * 0.12);
    col *= 0.75 + 0.25 * smoothstep(0.0, 0.4, uv.y);

    return vec4<f32>(finite(col), 1.0);
}
