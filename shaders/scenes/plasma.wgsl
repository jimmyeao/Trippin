// Classic demoscene plasma, modernised: four interfering sine fields with
// the palette drifting through the track's hue and the frequency riding
// the mids. Feedback smear keeps it buttery.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var p = centred(in.uv);
    let t = u.time * 0.6;

    var v = sin(p.x * 3.0 + t);
    v += sin((p.y * 2.2 + t) * 0.8);
    v += sin((p.x + p.y) * 2.4 + t * 0.7);
    let c = length(p - vec2<f32>(sin(t * 0.4) * 0.7, cos(t * 0.3) * 0.5));
    v += sin(c * (5.0 + u.mid * 6.0) - t);
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
