// Heart-monitor trace: a synthetic PQRST waveform locked to the beat —
// one full heartbeat per beat, scrolling right-to-left. Kick amplitude
// scales the QRS spike; the whole trace glows brighter with energy. Slow,
// readable, and clearly on the tempo.

// One heartbeat shape over phase 0..1: P bump, QRS spike, T bump.
fn ecg(ph: f32, amp: f32) -> f32 {
    let p = exp(-pow((ph - 0.18) * 14.0, 2.0)) * 0.12 * amp;          // P wave
    let qrs = (exp(-pow((ph - 0.38) * 60.0, 2.0)) * -0.15
             + exp(-pow((ph - 0.42) * 30.0, 2.0)) * 0.85
             + exp(-pow((ph - 0.46) * 60.0, 2.0)) * -0.2) * amp;    // QRS
    let t = exp(-pow((ph - 0.68) * 12.0, 2.0)) * 0.18 * amp;         // T wave
    return p + qrs + t;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);

    // Scroll: window slides right-to-left; ~5 beats on screen.
    let beats_on = 5.0;
    let bx = (p.x / aspect() * 0.5 + 0.5) * beats_on - u.beat;
    let ph = fract(-bx); // beat phase at this column

    // Amplitude rides the kick + energy.
    let amp = 0.35 + u.kick * 0.8 + u.energy * 0.3;
    let y_sig = -ecg(ph, amp); // up-screen is -p.y; invert so spike rises
    let dy = p.y + y_sig;

    let trace = exp(-abs(dy) * 80.0);
    let halo = exp(-abs(dy) * 10.0) * 0.08;
    // Older history dims to the left.
    let age = 0.45 + 0.55 * clamp(bx / beats_on + 0.5, 0.0, 1.0);

    var col = vec3<f32>(0.004, 0.014, 0.008);
    let green = vec3<f32>(0.15, 1.0, 0.4);
    col += green * (trace * age * (0.6 + u.energy * 0.8) + halo);
    // Grid backdrop.
    let g = max(smoothstep(0.015, 0.008, abs(fract(p.x * 4.0) - 0.5)),
                smoothstep(0.015, 0.008, abs(fract(p.y * 4.0) - 0.5)));
    col += green * g * 0.05;
    // Beat markers along the top.
    let mark = exp(-abs(fract(-bx) - 0.42) * 60.0) * exp(-abs(p.y + 0.85) * 30.0);
    col += green * mark * 0.4;
    return vec4<f32>(finite(col), 1.0);
}
