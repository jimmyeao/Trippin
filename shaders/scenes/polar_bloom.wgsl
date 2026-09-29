// Polar bloom: a closed curve whose radius is the spectrum wrapped around a
// circle — a living flower that opens on loudness, petals sharp on highs.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * 0.55;
    let r = length(p);
    let a = angle(p) / TAU + 0.5;

    // Mirrored spectrum: a=0 and a=1 meet at the left edge, so spec(a)
    // alone leaves a seam there. Mirroring makes the wrap continuous.
    let v = spec(abs(a - 0.5) * 2.0);
    let edge_r = 0.22 + v * (0.28 + 0.12 * u.intensity) + u.hits4.x * 0.05;

    // Inside the curve: translucent fill, outside: darkness; rim: hot.
    let inside = smoothstep(0.02, -0.02, r - edge_r);
    let rim = exp(-abs(r - edge_r) * 45.0);

    // Petal modulation: sharper lobes when highs are loud.
    let lobes = 3.0 + floor(spec(0.5) * 6.0);
    // Direction: the petals turn one way, then the other.
    let petal = 0.5 + 0.5 * sin(a * TAU * lobes + 5.0 * sin(u.clock4.x * 0.03));

    // Palette also keys on the mirrored angle so its gradient is seam-free.
    let am = abs(a - 0.5) * 2.0;
    var col = palette(am + u.flow * 0.02) * inside * (0.10 + 0.35 * v) * (0.6 + petal * 0.4);
    col += palette(am + 0.5) * rim * (0.8 + v);
    col += palette(0.5) * smoothstep(0.10, 0.0, r) * (0.5 + u.kick * 1.4); // core

    // Slow echo bloom: the previous frame, zoomed slightly out, ghosts the rim.
    col += prev(uncentred(centred(in.uv) * 0.985)) * 0.25 * inside;

    return vec4<f32>(finite(col), 1.0);
}
