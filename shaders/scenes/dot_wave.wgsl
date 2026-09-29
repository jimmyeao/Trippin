// A dot grid lifted into waves — like a stadium crowd doing the wave.
// Column height rides the spectrum, wavefront rolls with the phrase.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let n = 34.0;
    let cell = floor(p * n);
    let f = p * n - cell - 0.5;
    let c = (cell + 0.5) / n;

    // Rolling wave front + spectrum lift per column.
    // Shape: wavelength tightens with mid presence; direction: the wave
    // rolls one way then back; energy sets its pace.
    let wave_h = sin(c.x * aspect() * (3.0 + 3.0 * u.pres4.y) - 8.0 * sin(u.clock4.x * 0.02) - u.clock4.x * 0.7) * 0.5 + 0.5;
    let band = spec(c.x / aspect() + 0.25);
    let lift = wave_h * 0.6 + band * 0.6;
    let r = 0.10 + lift * 0.38 + u.hits4.x * 0.06;
    let dot = smoothstep(r, r - 0.06, length(f));

    // Colour sweeps across columns with the wavefront.
    let hue_t = c.x * 0.4 + wave_h * 0.15 + u.hue;
    var col = vec3<f32>(0.008, 0.008, 0.02);
    col += palette(hue_t) * dot * (0.3 + lift * (0.8 + u.energy));
    col += prev(in.uv) * 0.18;
    return vec4<f32>(col, 1.0);
}
