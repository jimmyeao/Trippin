// LED wall: a coarse grid of soft pixels, each column driven by its spectrum
// band. Looks like the side-screens at a festival; patterns pulse with energy.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let gx = 48.0;
    let gy = 27.0;
    let cell = vec2<f32>(floor(uv.x * gx), floor(uv.y * gy));
    let cuv = vec2<f32>(fract(uv.x * gx), fract(uv.y * gy)) - 0.5;

    // Each column's band; rows get a falling/bouncing waveform per column.
    let band = (cell.x + 0.5) / gx;
    let v = spec(band);
    let row = cell.y / gy;

    // Two waves: a slow amplitude swell and a fast beat ripple travelling up.
    let swell = v * (0.6 + 0.3 * u.intensity + 0.3 * u.pres4.x);
    let ripple = fract(u.beat_phase) * 1.4 - 0.2;
    let wave = step(row, swell) * (0.35 + 0.65 * exp(-abs(row - swell) * 6.0));
    let ring = exp(-abs(row - ripple) * 14.0) * u.kick * step(row, swell + 0.15);

    // Pixel: round dot with a visible gap — real LED walls have pitch.
    let dot_ = smoothstep(0.42, 0.30, length(cuv));
    var col = palette(band * 0.8 + row * 0.4) * dot_ * (wave + ring);

    // Beat-scrolling rainbow bands over the top at high energy.
    col += palette(uv.x + uv.y * 0.5 + 3.0 * sin(u.clock4.x * 0.02)) * dot_ * u.energy * 0.10;

    // Background grid haze.
    col += palette(0.5) * 0.015 * dot_;

    return vec4<f32>(finite(col), 1.0);
}
