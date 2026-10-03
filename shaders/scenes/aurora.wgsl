// Spectrum aurora: the curtains' lower edges ARE the analyser — looking
// across the sky, each ribbon layer's edge traces a slice of the spectrum,
// so loud bands literally raise the lights. Vertical striations stream
// upward on the energy clock, and kicks deepen the folds (shape, not flashing).
//
// Each layer's ground-x walks back and forth (mirrored, not clamped) across
// the active 3/4 of the spectrum: the old clamp parked far layers on the top
// bins, which sit near zero in most music, so much of the sky never moved.
// Cost: the wind and rays use the baked noise volume (a 5-octave fbm plus a
// hash noise per layer per step was most of the ~95 ms/frame at 1080p on an
// M2 Pro), the ray fetch is skipped away from each ribbon's edge, and 22
// height steps cover the same span as the old 36.

fn curtain(pt: vec2<f32>) -> f32 {
    // Thin ribbons winding across the ground plane; every layer above samples
    // the same ribbon, which draws it as a tall curtain of vertical rays.
    var v = 0.0;
    // Kick transients deepen the folds; slow bass presence sets their depth.
    let swell = (0.7 + 0.6 * u.pres4.x) * (1.0 + 0.9 * u.hits4.x);
    for (var k = 0; k < 3; k++) {
        let fk = f32(k);
        let xs = pt.x * (0.015 + fk * 0.006) + 0.25 + fk * 0.17;
        let s = spec(abs(fract(xs) * 2.0 - 1.0) * 0.75);
        // Wind bends the ribbon on the energy clock; folds ripple faster as the
        // highs drive.
        // Noise from the baked volume (one fetch instead of 4 hashes), remapped
        // from the channels' measured p05..p95 (B .37-.64, G .29-.68).
        let wn = clamp((tnoise(vec3<f32>(pt.x * 0.025, fk * 0.31 + u.clock4.z * 0.01, 0.5)).b - 0.37) / 0.27, 0.0, 1.0);
        let wind = (1.2 + s * 2.5) * swell * sin(pt.x * 0.15 + u.clock4.z * 0.35 + fk * 2.3) + 1.2 * wn;
        // Loud bands lift the edge, but less than before: the upper layers are
        // drawn fainter, so a big lift read as the sky dimming on every kick.
        let edge = 7.0 + fk * 5.5 + s * 17.0 + wind;
        let d = pt.y - edge;
        // Far from the ribbon edge the band is < 2%: skip the ray fetch.
        if abs(d) > 3.7 {
            continue;
        }
        let band = exp(-d * d * 0.30);
        // Vertical striations streaming upward, faster and brighter when loud.
        let rn = tnoise(vec3<f32>(pt.x * 0.7 + fk * 1.75, pt.y * 0.055 - u.clock4.w * (0.7 + s * 1.8) * 0.25, 0.2)).g;
        let rays = clamp((rn - 0.29) / 0.39, 0.0, 1.0);
        v += band * (0.2 + 0.8 * rays) * (0.25 + s * 2.8);
    }
    return v * 0.85;
}

// `layers` height steps over the span of the original 36 (22 for the sky; the
// darkened, rippled lake mirror gets by with 11).
// `jit` (0..1, static blue noise per pixel) offsets the layer heights: with 22
// steps the layers' rays otherwise show as a fan of discrete streaks.
fn aurora(ro: vec3<f32>, rd: vec3<f32>, t: f32, layers: i32, jit: f32) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    let step = 36.0 / f32(layers);
    for (var i = 0; i < layers; i++) {
        let fi = (f32(i) + jit) * step;
        let h = 0.6 + fi * 0.11;                     // layer height
        let dist = (h - ro.y) / rd.y;
        let pt = ro.xz + rd.xz * dist;
        let v = curtain(pt);
        // Green at the base of the curtain, magenta toward the top; loud highs
        // push more magenta into the upper sky.
        let tint = mix(vec3<f32>(0.1, 1.0, 0.45), vec3<f32>(0.75, 0.2, 0.9), smoothstep(6.0, 30.0, fi) * (0.4 + u.high * 0.8));
        col += tint * v * exp(-fi * 0.06) * 0.11 * step;
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

fn sky(rd: vec3<f32>, t: f32, layers: i32, jit: f32) -> vec3<f32> {
    var col = mix(vec3<f32>(0.02, 0.04, 0.08), vec3<f32>(0.0, 0.005, 0.02), clamp(rd.y * 1.5, 0.0, 1.0));
    col += stars(rd);
    let a = aurora(vec3<f32>(0.0, 0.0, t * 0.05), rd, t, layers, jit) * (0.55 + 0.8 * u.intensity);
    return col + a;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let t = u.time;
    // Static (not re-rolled per frame) so the dither never shimmers.
    let jit = textureLoad(blue_tex, vec2<i32>(in.pos.xy) & vec2<i32>(63), 0).r;
    let horizon = 0.12;                              // screen y (downward) of the lake shore
    let rd_sky = normalize(vec3<f32>(p.x, -(p.y - horizon) + 0.02, 1.4));

    var col: vec3<f32>;
    let ridge = mountains(p.x * 0.5 + t * 0.002);
    if p.y < horizon {
        col = sky(rd_sky, t, 22, jit);
        // Mountains: dark rock, a dusting of snow on the peaks catching aurora light.
        let above = horizon - p.y;
        if above < ridge {
            let snow = smoothstep(ridge - 0.04, ridge - 0.005, above) * smoothstep(0.12, 0.18, ridge);
            // The snow catches the sky already computed for this pixel (a second
            // aurora pass here doubled the cost of every mountain pixel).
            col = vec3<f32>(0.01, 0.012, 0.02) + snow * (vec3<f32>(0.12, 0.14, 0.18) + col * 0.9);
        }
    } else {
        // Lake: mirror the scene about the shore, darkened, with gentle ripples.
        let below = p.y - horizon;
        let ripple = (noise(vec2<f32>(p.x * 30.0, below * 120.0 - t * 0.6)) - 0.5) * 0.004 * (1.0 + below * 6.0);
        let my = horizon - below + ripple;
        let rd_m = normalize(vec3<f32>(p.x + ripple, -(my - horizon) + 0.02, 1.4));
        col = sky(rd_m, t, 11, jit);
        let above = horizon - my;
        if above < mountains(p.x * 0.5 + t * 0.002) {
            col = vec3<f32>(0.008, 0.01, 0.016);
        }
        col *= 0.55 * (1.0 - smoothstep(0.0, 0.9, below) * 0.5);
    }
    return vec4<f32>(col, 1.0);
}
