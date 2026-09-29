// Still water seen from above: ring ripples expand outward from random
// points, spawning on each beat and flaring on onsets. Ring colour comes
// from the spectrum — the surface itself stays deep and dark.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);

    // Water: deep gradient + slow drifting sheen.
    var col = vec3<f32>(0.008, 0.015, 0.03);
    col += palette(0.55 + u.hue) * fbm(p * 2.0 + u.clock4.z * 0.03) * 0.06;

    // Ring sources: 8 slots, one fires per beat (hashed position).
    for (var i = 0; i < 8; i++) {
        let fi = f32(i);
        let src_beat = floor(u.beat) - fi;
        let hp = vec2<f32>(hash21(vec2<f32>(src_beat, 3.0)) - 0.5,
                           hash21(vec2<f32>(src_beat, 9.0)) - 0.5)
               * vec2<f32>(aspect(), 1.0) * 1.5;
        // Age of this ring in beats — expands steadily, decays.
        let age = u.beat - src_beat;
        if (age < 0.0 || age > 4.0) { continue; }
        let rr = age * 0.35;
        let d = abs(length(p - hp) - rr);
        let fade = exp(-age * 0.9);
        let band = spec(fract(fi * 0.13 + src_beat * 0.11));
        col += palette(fract(fi * 0.13 + 0.5) + u.hue)
             * exp(-d * d * 900.0) * fade * (0.5 + band + u.kick * 0.5);
    }

    // Onset: a bright flash ring from centre.
    let orad = (1.0 - u.onset) * 1.4;
    let od = abs(length(p) - orad);
    col += palette(0.1 + u.hue) * exp(-od * od * 400.0) * u.onset * 1.5;

    col += prev(in.uv) * 0.15;
    return vec4<f32>(finite(col), 1.0);
}
