// Endless zoom through nested portals. Log-polar space repeats every `P`, so
// each portal contains a scaled copy of the whole picture (a Droste effect).
// The seam between levels is hidden under a glowing polygon rim, and the zoom
// advances one portal per bar.

const P: f32 = 1.2; // log-scale between successive portals

// Kali-style fractal filigree: fold and invert, accumulating orbit detail.
fn filigree(p_in: vec2<f32>, c: vec2<f32>) -> vec3<f32> {
    var p = p_in;
    var acc = 0.0;
    var trap = 10.0;
    for (var i = 0; i < 9; i++) {
        p = abs(p) / max(dot(p, p), 0.02) - c;
        acc += exp(-abs(length(p) - 0.6) * 14.0);
        trap = min(trap, abs(p.x * p.y));
    }
    // Dark ground, bright filaments: keep values low so tonemapping keeps colour.
    let glow = acc / 9.0;
    return palette(glow * 2.0 + trap * 0.8) * pow(glow, 1.6) * 1.8 + palette(0.6) * exp(-trap * 60.0) * 0.35;
}

// Radius scaled so the level boundary is an n-gon rather than a circle.
fn polygon_factor(a: f32, n: f32) -> f32 {
    let seg = TAU / n;
    return cos(PI / n) / cos(((a % seg) + seg) % seg - seg * 0.5);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let n = 5.0 + floor(u.seed % 4.0);
    let spin = u.clock4.z * 0.08;
    let q = rot(spin) * p;
    let a = angle(q);
    let r = max(length(q), 1e-4) / polygon_factor(a, n);

    // One portal per bar, gliding smoothly.
    // Zoom speed follows the energy clock — the dive quickens on drops.
    let zoom = (u.clock4.x / 4.0) * P;
    let lr = log(r) + zoom;
    let level = floor(lr / P);
    let f = lr / P - level;               // 0 at a portal's inner edge, 1 at its outer
    let twist = a + level * 0.6;          // each level rotated a little further

    // Point inside the annulus, in the fractal's frame.
    let rr = exp(f * P);
    let local = vec2<f32>(cos(twist), sin(twist)) * rr * 0.55;
    let c = vec2<f32>(0.62 + 0.06 * sin(u.time * 0.21) + 0.05 * u.bass, 0.48 + 0.05 * cos(u.time * 0.17));
    var col = filigree(local, c) * (0.6 + 0.9 * u.intensity);

    // Glowing rim over the seam between levels, pulsing on the beat.
    let rim_d = min(f, 1.0 - f);
    col += palette(level * 0.13 + 0.3) * exp(-rim_d * 60.0) * (0.6 + 0.6 * u.intensity);
    col *= palette(level * 0.07) * 0.6 + 0.4;

    // Deep levels fade into the centre so it reads as a tunnel.
    col *= smoothstep(0.0, 0.18, length(p));
    col += prev(uncentred(p * 0.985)) * 0.2;
    return vec4<f32>(col, 1.0);
}
