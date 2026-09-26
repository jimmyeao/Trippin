// Spectrum-driven aurora: the curtains ARE the analyser — each ribbon layer's
// fold depth and height follow its band of the spectrum, striation brightness
// rides the same band, and the kick swells the whole display. Mountains and
// lake reflection remain, but nothing is static anymore.

fn curtain(pt: vec2<f32>, t: f32) -> f32 {
    // Thin ribbons winding across the ground plane; every layer above samples
    // the same ribbon, which draws it as a tall curtain of vertical rays.
    var v = 0.0;
    for (var k = 0; k < 2; k++) {
        let fk = f32(k);
        // This layer listens to a different slice of the spectrum: the ground
        // coordinate picks a bin, and that band sets fold depth and lift.
        let s = spec(clamp(pt.x * 0.035 + 0.5, 0.0, 1.0) * (0.55 + fk * 0.45));
        let wind = (2.2 + s * 7.0) * sin(pt.x * 0.18 + t * 0.05 + fk * 2.0) + 1.4 * fbm(vec2<f32>(pt.x * 0.12, fk * 5.0 + t * 0.02));
        let d = pt.y - (14.0 + fk * 7.0 + s * 9.0) - wind;
        let band = exp(-d * d * 0.35);
        // Fine vertical striations whose shimmer rate and brightness ride the band.
        let rays = 0.35 + 0.65 * noise(vec2<f32>(pt.x * 2.5 + t * (0.3 + s * 0.9), fk * 3.0)) * (0.4 + s * 1.5);
        v += band * rays * (0.7 + 0.3 * fk);
    }
    return v * (0.8 + u.onset * 0.5);
}

fn aurora(ro: vec3<f32>, rd: vec3<f32>, t: f32) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    for (var i = 0; i < 36; i++) {
        let fi = f32(i);
        let h = 0.6 + fi * 0.09;                     // layer height
        let dist = (h - ro.y) / rd.y;
        let pt = ro.xz + rd.xz * dist;
        let v = curtain(pt, t);
        // Green at the base of the curtain, magenta toward the top; loud highs
        // push more magenta into the upper sky.
        let tint = mix(vec3<f32>(0.1, 1.0, 0.45), vec3<f32>(0.75, 0.2, 0.9), smoothstep(8.0, 30.0, fi) * (0.5 + u.high * 0.7));
        col += tint * v * exp(-fi * 0.07) * 0.09;
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
    let a = aurora(vec3<f32>(0.0, 0.0, t * 0.05), rd, t) * (0.55 + 0.7 * u.intensity + 0.6 * u.kick);
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
