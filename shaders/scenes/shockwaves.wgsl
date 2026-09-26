// Shockwaves: a ring bursts outward on every beat; onsets fire extra smaller
// rings from off-centre points. Trails linger via the feedback buffer.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * 0.55;
    var col = prev(in.uv) * 0.93;

    // Beat ring: radius grows through the beat, colour hashed by beat number.
    let t = u.beat_phase;
    let rr = t * t * 1.2;                    // accelerate outward
    let hue_b = hash21(vec2<f32>(floor(u.beat), u.seed));
    let ring = exp(-abs(length(p) - rr) * 60.0) * (1.0 - t) * 1.4;
    col += palette(hue_b) * ring;

    // Onset rings from two mirrored side points (hi-hats feel placed).
    for (var i = 0; i < 2; i++) {
        let side = select(-0.55, 0.55, i == 1);
        let o = vec2<f32>(side * aspect() * 0.5, -0.1);
        let or_ = u.onset * u.onset * 0.9;
        let oring = exp(-abs(length(p - o) - or_) * 90.0) * u.onset;
        col += palette(hash21(vec2<f32>(floor(u.beat * 8.0) + f32(i) * 31.0, u.seed))) * oring * 0.8;
    }

    // Constant faint heart: brightness rides the bass.
    col += palette(u.hue + 0.5) * exp(-length(p) * 6.0) * (0.25 + u.bass * 1.2);

    // Floor line reflection of the central ring for depth.
    let floor_ = smoothstep(-0.25, -0.6, p.y);
    col += palette(0.4) * floor_ * u.kick * 0.08;

    return vec4<f32>(finite(col), 1.0);
}
