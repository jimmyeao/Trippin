// Orbiters: particles on rotating rings, flung outward by their band — bass
// inner ring, highs outer. Feedback leaves curved ion trails.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * 0.55;

    // Trails decay with a slow rotation — orbits smear into rings.
    let bp = centred(in.uv) * 0.985;
    var col = prev(uncentred(rot(0.004) * bp)) * 0.90;

    for (var i = 0; i < 16; i++) {
        let fi = f32(i);
        let ring = floor(fi / 4.0);            // 4 rings x 4 orbiters
        let band = ring / 3.0;                 // inner->bass, outer->highs
        let v = spec(band * 0.25 + 0.05);

        let speed = (0.3 + ring * 0.15) * (0.5 + u.energy);
        let dir = select(1.0, -1.0, i32(ring) % 2 == 1);
        let ang = u.flow * speed * dir + hash21(vec2<f32>(fi, u.seed)) * TAU;
        let rad = 0.12 + ring * 0.16 + v * 0.20 + u.kick * 0.02;
        let pos = vec2<f32>(cos(ang), sin(ang)) * rad;

        let d = length(p - pos);
        let size = 0.006 + v * 0.010;
        let orb = smoothstep(size, size * 0.3, d);
        let glow = exp(-d * 20.0) * 0.25;

        col += palette(band * 0.5 + fi / 16.0 + u.hue) * (orb * (0.7 + v) + glow);
    }

    // Central glow pulsing with the kick.
    col += palette(0.5) * exp(-length(p) * 9.0) * (0.3 + u.kick);

    return vec4<f32>(finite(col), 1.0);
}
