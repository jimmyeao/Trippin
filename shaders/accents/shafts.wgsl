// Shafts (accent layer): god rays — soft shafts of light falling from above
// the frame, with fine dust glinting in them. The shafts' angle swings over
// eight bars (direction), they widen with the slow bass presence (shape), and
// the dust drifts down through them on the smooth energy clock (motion).
// Four shafts, always four. (Thresholded value noise for the dust came out
// as chunky blocks: the dust is sparse points on a hashed grid instead.)

// Fine dust: a sparse grid of tiny points drifting down.
fn dust(p: vec2<f32>) -> f32 {
    let q = (p + vec2<f32>(0.0, -u.clock4.x * 0.05)) * 55.0;
    let cell = floor(q);
    let h = hash22(cell + vec2<f32>(4.0, 1.0));
    if h.x > 0.12 {
        return 0.0;
    }
    let c = cell + 0.5 + (hash22(cell + vec2<f32>(9.0, 2.0)) - 0.5) * 0.7;
    let d = length(q - c);
    let tw = 0.6 + 0.4 * sin(u.time * (1.0 + 2.0 * h.y) + h.x * 50.0);
    return smoothstep(0.22, 0.0, d) * tw;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.0);
    let swing = 0.3 * sin(TAU * u.flow / 32.0);
    let dd = dust(p);
    for (var i = 0; i < 4; i++) {
        let fi = f32(i);
        let h = hash22(vec2<f32>(fi, 9.1));
        // Each shaft starts above the frame and leans with the swing.
        let x0 = (fi - 1.5) / 1.5 * aspect() * 0.7 + (h.x - 0.5) * 0.4;
        let ang = swing + (h.y - 0.5) * 0.35;
        let dir = normalize(vec2<f32>(sin(ang), 1.0));
        let n = vec2<f32>(dir.y, -dir.x);
        let rel = p - vec2<f32>(x0, -1.25);
        let along = dot(rel, dir);
        let across = dot(rel, n);
        // Widening as it falls, and with the bass presence.
        let wid = (0.07 + 0.05 * h.x) * (1.0 + 0.5 * u.pres4.x) * (0.5 + 0.45 * along);
        let beam = exp(-(across * across) / max(wid * wid, 1e-4))
                 * smoothstep(0.0, 0.5, along) * exp(-along * 0.45);
        let tone = palette(0.08 + fi * 0.07);
        col += tone * tone * beam * (0.22 + 1.6 * dd);
    }
    return vec4<f32>(finite(col), 1.0);
}
