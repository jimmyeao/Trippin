// Orbiters: particles on rotating rings, flung outward by their band — bass
// inner ring, highs outer. Feedback leaves curved ion trails.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * 0.55;

    // Trails decay with a slow rotation — short enough that each orb reads as
    // a comet tail rather than the four orbs per ring fusing into a band.
    let bp = centred(in.uv) * 0.985;
    var col = prev(uncentred(rot(0.006) * bp)) * 0.82;

    for (var i = 0; i < 16; i++) {
        let fi = f32(i);
        let ring = floor(fi / 4.0);            // 4 rings x 4 orbiters
        let band = ring / 3.0;                 // inner->bass, outer->highs
        let v = spec(band * 0.25 + 0.05);

        let speed = 0.3 + ring * 0.15;
        let dir = select(1.0, -1.0, i32(ring) % 2 == 1);
        // Energy: orbit speed rides the smooth energy clock (multiplying
        // the tempo clock by raw energy here made the orbits lurch).
        let ang = u.clock4.x * speed * dir + hash21(vec2<f32>(fi, u.seed)) * TAU;
        // Inner ring starts far enough out that the orbs' halos never overlap
        // the centre — that overlap smeared into a permanent disc before.
        // Shape: orbits widen with presence and jolt on kicks.
        let rad = 0.14 + ring * (0.12 + 0.08 * u.pres4.x) + v * 0.16 + u.hits4.x * 0.04;
        let pos = vec2<f32>(cos(ang), sin(ang)) * rad;

        let d = length(p - pos);
        let size = 0.007 + v * 0.010;
        let orb = smoothstep(size, size * 0.3, d);
        let glow = exp(-d * 34.0) * 0.10;

        col += palette(band * 0.5 + fi / 16.0 + u.hue) * (orb * (0.8 + v) + glow);
    }

    // Faint ember at the hub — driven by the music, dies in silence so the
    // feedback can't accumulate a permanent blob.
    col += palette(0.5) * exp(-length(p) * 18.0) * (u.kick * 0.5 + u.bass * 0.10);

    return vec4<f32>(finite(col), 1.0);
}
