// Pulse grid: an LED-tile floor. Each column's tiles light to its band energy
// and the whole floor hops on the kick — like a stadium floor in a drop.
// (uv.y grows downward: the horizon sits mid-screen, tiles fill the bottom.)

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Depth 0 at the horizon (uv.y=0.45) to 1 at the bottom of the screen.
    let depth = clamp((uv.y - 0.45) / 0.55, 0.0, 1.0);
    // More rows packed near the horizon than at the front.
    let persp = pow(depth, 0.75);
    let cols = 24.0;
    let rows = 14.0;
    let cx = floor(uv.x * cols);
    let cy = floor(persp * rows);
    let f = vec2<f32>(fract(uv.x * cols), fract(persp * rows)) - 0.5;

    let band = (cx + 0.5) / cols;
    let v = spec(band);
    let row_level = cy / rows; // 0 back (horizon) -> 1 front (bottom)

    // Tiles light from the front (bottom of screen) up to the band height.
    let lit = step(1.0 - v * 1.15, row_level);
    // Kick bounce: tiles get rounder and hotter for a moment.
    let shrink = 0.06 + beat_pulse(10.0) * 0.10;
    let tile = smoothstep(0.5 - shrink, 0.5 - shrink - 0.08, max(abs(f.x), abs(f.y)));

    var col = palette(band * 0.7 + row_level * 0.3) * tile * lit * (0.3 + v * 0.9);
    // Unlit tiles keep a faint outline so the grid reads.
    col += palette(band) * tile * 0.03 * step(0.0, uv.y - 0.45);

    // Travelling beat wavefront rolling from the horizon to the front.
    col += palette(0.5) * tile * exp(-abs(row_level - u.beat_phase) * 10.0) * 0.4;

    // Sky glow above the horizon.
    col += palette(0.75) * smoothstep(0.45, 0.0, uv.y) * (0.08 + u.high * 0.15);
    // Dim the far rows toward the horizon haze.
    col *= 0.4 + 0.6 * smoothstep(0.0, 0.5, depth);

    return vec4<f32>(finite(col), 1.0);
}
