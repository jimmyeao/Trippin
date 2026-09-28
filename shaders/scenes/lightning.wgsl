// Electric storm: jagged bolts arcing top-to-bottom (+y is down-screen).
// Bolts re-strike on every onset and fork with the highs. Between strikes
// the sky glows with charge.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.008, 0.01, 0.03);

    for (var b = 0; b < 3; b++) {
        let fb = f32(b);
        let seed = floor(u.beat * 0.5) + fb * 31.0;
        let x0 = (hash21(vec2<f32>(seed, fb)) - 0.5) * aspect() * 1.4;
        let flicker = fract(hash21(vec2<f32>(seed * 7.0, 1.0)) * 4.0 + u.beat_phase);
        let on = step(flicker, 0.55 + u.onset * 0.4);

        // Walk down the screen in segments; each segment jag is hashed.
        var x = x0;
        var d = 1e5;
        var seg = 0.0;
        let y_top = -1.0;
        let steps = 14.0;
        for (var i = 0; i < 14; i++) {
            let fi = f32(i);
            let y0 = y_top + (fi / steps) * 2.2 - 0.1;
            let y1 = y_top + ((fi + 1.0) / steps) * 2.2 - 0.1;
            let x1 = x + (hash21(vec2<f32>(seed, fi)) - 0.5) * 0.55;
            // Distance from p to segment (x,y0)-(x1,y1), only within segment slab.
            if p.y * -1.0 >= y0 && p.y * -1.0 < y1 { // screen-down is -p.y
                let t = (p.y * -1.0 - y0) / max(y1 - y0, 0.001);
                let px = mix(x, x1, t);
                d = min(d, abs(p.x - px));
            }
            x = x1;
            seg = fi;
        }
        let bolt = exp(-d * 220.0) + exp(-d * 30.0) * 0.35;
        let bc = mix(vec3<f32>(0.6, 0.7, 1.0), palette(0.75 + u.hue), fb * 0.3);
        col += bc * bolt * on * (0.5 + u.energy + u.onset);
    }

    // Sky charge glow between strikes.
    col += palette(0.8 + u.hue) * fbm(p * 3.0 + u.time * 0.05) * 0.12 * (0.5 + u.energy);
    return vec4<f32>(col, 1.0);
}
