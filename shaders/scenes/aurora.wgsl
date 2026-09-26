// Spectrum aurora: the curtains' lower edges ARE the analyser — looking
// across the sky, each ribbon layer's edge traces a slice of the spectrum,
// so loud bands literally raise the lights. Vertical striations stream
// upward on the beat clock, onsets shimmer them, kicks swell the sky.

fn curtain(pt: vec2<f32>, t: f32) -> f32 {
    // Thin ribbons winding across the ground plane; every layer above samples
    // the same ribbon, which draws it as a tall curtain of vertical rays.
    var v = 0.0;
    for (var k = 0; k < 3; k++) {
        let fk = f32(k);
        // Each layer's ground-x spans a different slice of the spectrum, and
        // the band level IS the curtain's lower edge — the sky draws the music.
        let xp = clamp(pt.x * (0.030 + fk * 0.012) + 0.5, 0.0, 1.0);
        let s = spec(xp);
        // Wind still bends the ribbon, now on the beat clock.
        let wind = (1.2 + s * 2.5) * sin(pt.x * 0.15 + u.flow * 0.35 + fk * 2.3)
                 + 1.2 * fbm(vec2<f32>(pt.x * 0.10, fk * 5.0 + u.flow * 0.04));
        let edge = 7.0 + fk * 5.5 + s * 26.0 + wind;
        let d = pt.y - edge;
        let band = exp(-d * d * 0.30);
        // Vertical striations streaming upward, faster and brighter when loud.
        let rays = noise(vec2<f32>(pt.x * 2.8 + fk * 7.0, pt.y * 0.22 - u.flow * (0.7 + s * 1.8)));
        v += band * (0.2 + 0.8 * rays) * (0.30 + s * 1.7);
    }
    return v * (0.6 + u.onset * 0.8 + u.kick * 0.5);
}

fn aurora(ro: vec3<f32>, rd: vec3<f32>, t: f32) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    for (var i = 0; i < 36; i++) {
        let fi = f32(i);
        let h = 0.6 + fi * 0.11;                     // layer height
        let dist = (h - ro.y) / rd.y;
        let pt = ro.xz + rd.xz * dist;
        let v = curtain(pt, t);
        // Green at the base of the curtain, magenta toward the top; loud highs
        // push more magenta into the upper sky.
        let tint = mix(vec3<f32>(0.1, 1.0, 0.45), vec3<f32>(0.75, 0.2, 0.9), smoothstep(6.0, 30.0, fi) * (0.4 + u.high * 0.8));
        col += tint * v * exp(-fi * 0.06) * 0.11;
    }
    return col * smoothstep(0.0, 0.25, rd.y);
}

fn stars(rd: vec3<f32>) -> vec3<f32> {
    let q = rd.xy / max(rd.z, 0.1) * 260.0;
    let c = floor(q);
    let h = hash21(c);
    let d = length(fract(q) - 0.5);
    return vec3<f32>(0.9, 0.95, 1.0) * step(0.985, h) * smoothstep(0.35, 0.0, d) * (0.5 + 0.5 * hash21(c + 3.0));
}

fn mountains(x: f32) -> f32 {
    // Ridge height above the horizon for a view-space x.
    let big = fbm(vec2<f32>(x * 1.6, 4.0));
    return 0.02 + 0.26 * big * big + 0.05 * fbm(vec2<f32>(x * 8.0, 9.0));
}

fn sky(rd: vec3<f32>, t: f32) -> vec3<f32> {
    var col = mix(vec3<f32>(0.02, 0.04, 0.08), vec3<f32>(0.0, 0.005, 0.02), clamp(rd.y * 1.5, 0.0, 1.0));
    col += stars(rd);
    let a = aurora(vec3<f32>(0.0, 0.0, t * 0.05), rd, t) * (0.5 + 0.8 * u.intensity + 0.7 * u.kick);
    return col + a;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let t = u.time;
    let horizon = 0.12;                              // screen y (downward) of the lake shore
    let rd_sky = normalize(vec3<f32>(p.x, -(p.y - horizon) + 0.02, 1.4));

    var col: vec3<f32>;
    let ridge = mountains(p.x * 0.5 + t * 0.002);
    if p.y < horizon {
        col = sky(rd_sky, t);
        // Mountains: dark rock, a dusting of snow on the peaks catching aurora light.
        let above = horizon - p.y;
        if above < ridge {
            let snow = smoothstep(ridge - 0.04, ridge - 0.005, above) * smoothstep(0.12, 0.18, ridge);
            let glow = aurora(vec3<f32>(0.0, 0.0, t * 0.05), normalize(vec3<f32>(p.x, 0.35, 1.4)), t);
            col = vec3<f32>(0.01, 0.012, 0.02) + snow * (vec3<f32>(0.12, 0.14, 0.18) + glow * 0.6);
        }
    } else {
        // Lake: mirror the scene about the shore, darkened, with gentle ripples.
        let below = p.y - horizon;
        let ripple = (noise(vec2<f32>(p.x * 30.0, below * 120.0 - t * 0.6)) - 0.5) * 0.004 * (1.0 + below * 6.0);
        let my = horizon - below + ripple;
        let rd_m = normalize(vec3<f32>(p.x + ripple, -(my - horizon) + 0.02, 1.4));
        col = sky(rd_m, t);
        let above = horizon - my;
        if above < mountains(p.x * 0.5 + t * 0.002) {
            col = vec3<f32>(0.008, 0.01, 0.016);
        }
        col *= 0.55 * (1.0 - smoothstep(0.0, 0.9, below) * 0.5);
    }
    return vec4<f32>(col, 1.0);
}
