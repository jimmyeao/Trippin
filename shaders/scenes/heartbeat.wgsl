// Medical-monitor ECG trace: the real waveform amplified into a heartbeat
// line scrolling right-to-left, with a phosphor afterglow. Kick marks a
// sharp QRS spike.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);

    // Scrolling waveform — head sweeps left-to-right each bar.
    let head = fract(u.beat * 0.25);
    let x = (p.x / aspect() * 0.5 + 0.5);
    // wave() across a slightly offset window; newest sample on the right.
    let w = wave(fract(x - head * 0.5)) * 0.5;
    // QRS-ish spike at the head of the sweep, driven by the kick.
    let spike = exp(-abs(x - 0.98) * 25.0) * u.kick * 0.5;
    let y_sig = w + spike;
    let dy = p.y - y_sig; // +p.y is down-screen; trace sits mid-frame

    let trace = exp(-abs(dy) * 60.0);
    // Phosphor fade: brighter nearer the sweep head.
    let age = fract(x - head * 0.5);
    let phos = 0.3 + 0.7 * smoothstep(0.9, 0.0, age);

    var col = vec3<f32>(0.004, 0.012, 0.006);
    let green = vec3<f32>(0.1, 1.0, 0.35);
    col += green * trace * phos * (0.7 + u.energy * 1.2);
    col += green * exp(-abs(dy) * 8.0) * 0.05; // soft bloom
    // Grid backdrop.
    let g = max(smoothstep(0.02, 0.01, abs(fract(p.x * 4.0) - 0.5)),
                smoothstep(0.02, 0.01, abs(fract(p.y * 4.0) - 0.5)));
    col += green * g * 0.06;
    return vec4<f32>(col, 1.0);
}
