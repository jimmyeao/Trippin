// Julia set seen through a kaleidoscope. Its constant orbits the edge of the
// Mandelbrot set, stepping on every beat, so the fractal morphs with the
// music; the view breathes in and out on each bar.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var p = centred(in.uv);
    p = rot(u.time * 0.04) * p;

    // Six-fold mirror for the mandala look.
    let seg = TAU / 6.0;
    var a = angle(p);
    a = abs(((a % seg) + seg) % seg - seg * 0.5);
    p = vec2<f32>(cos(a), sin(a)) * length(p);

    let bar = u.bar_phase;
    let zoom = 0.75 - 0.12 * sin(bar * TAU) - 0.08 * u.bass;
    var z = p * zoom;

    // Julia constants on the Mandelbrot boundary (connected, filament-rich
    // sets); glide from one to the next over each bar, easing on the beat.
    var cs = array<vec2<f32>, 6>(
        vec2<f32>(-0.8, 0.156), vec2<f32>(-0.7269, 0.1889), vec2<f32>(0.285, 0.01),
        vec2<f32>(-0.4, 0.6), vec2<f32>(0.355, 0.355), vec2<f32>(-0.54, 0.54));
    let pos = u.beat / 4.0 + floor(u.seed);
    let k = i32(floor(pos)) % 6;
    let beat_ease = (floor(u.beat % 4.0) + smoothstep(0.0, 0.35, u.beat_phase)) / 4.0;
    let c = mix(cs[k], cs[(k + 1) % 6], beat_ease);

    var i = 0;
    var trap = 1e9;
    loop {
        if i >= 96 || dot(z, z) > 64.0 {
            break;
        }
        z = vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
        trap = min(trap, abs(length(z) - 0.5));
        i++;
    }
    var col: vec3<f32>;
    if i >= 96 {
        // Inside the set: dark, lit by the orbit trap.
        col = palette(trap * 3.0 + 0.5) * exp(-trap * 10.0) * 0.35;
    } else {
        // Smooth escape-time colouring.
        // Fast escapes stay dark; only the boundary filaments light up.
        let sm = f32(i) - log2(max(log2(dot(z, z)), 1e-4)) + 4.0;
        let t = clamp(sm / 40.0, 0.0, 1.0);
        col = palette(sm * 0.04 + u.time * 0.03) * pow(t, 2.2) * 2.0;
    }
    col *= 0.7 + 0.6 * u.intensity + 0.5 * beat_pulse(6.0) * u.bass;
    col += prev(uncentred(rot(0.004) * centred(in.uv) * 0.99)) * 0.15;
    return vec4<f32>(col, 1.0);
}
