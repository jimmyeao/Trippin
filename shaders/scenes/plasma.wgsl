// Classic demoscene plasma, modernised: four interfering sine fields with
// the palette drifting through the track's hue and the frequency riding
// the mids. Feedback smear keeps it buttery.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Direction: the field rotates one way, then back; energy: it churns
    // faster as the mids drive.
    var p = rot(0.8 * sin(u.clock4.x * 0.02)) * centred(in.uv);
    let t = u.clock4.z * 0.3;

    var v = sin(p.x * 3.0 + t);
    v += sin((p.y * 2.2 + t) * 0.8);
    v += sin((p.x + p.y) * 2.4 + t * 0.7);
    let c = length(p - vec2<f32>(sin(t * 0.4) * 0.7, cos(t * 0.3) * 0.5));
    // Shape: the ring interference tightens with bass presence.
    v += sin(c * (4.0 + u.pres4.x * 8.0) - t);
    v *= 0.25;

    var col = palette(v * 0.5 + u.hue);
    col *= 0.25 + u.energy * 0.8 + u.bass * 0.3;
    // Beat ripples push the field.
    col += palette(v + 0.5 + u.hue) * beat_pulse(4.0) * 0.12;

    // Feedback zoom-blur adds motion smear.
    let trail = prev(uncentred(centred(in.uv) * (0.985 - u.kick * 0.01)));
    col = max(col, trail * 0.93);
    return vec4<f32>(col, 1.0);
}
