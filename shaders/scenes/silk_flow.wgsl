// @bloom 0.6 @tonemap agx
// Synesthesia-style abstract: flowing silk. Layers of domain-warped noise
// fold into liquid ribbons of fabric, shaded as if lit from above with a
// thin-film iridescent sheen (colour shifts with the fold angle, like oil
// on water). 2D and cheap — runs on anything (no @heavy).
// Audio vocabulary:
//  - the flow advances on the mid energy clock (drifts in breakdowns,
//    streams on drops); warp strength swells with bass presence;
//  - fold creases catch light on high hits; a soft pressure wave ripples
//    out from the centre on bass hits;
//  - iridescence hue drifts on the high clock.

fn fbm3(p: vec2<f32>, t: f32) -> f32 {
    // Three octaves from the baked noise volume (time runs through z).
    // B channel spans ~0.37..0.64 — stretch it to ~0..1 around 0.5.
    let v = tnoise(vec3<f32>(p * 0.08, t)).b * 0.65
        + tnoise(vec3<f32>(p * 0.17 + 0.31, t * 1.3)).b * 0.35;
    return clamp((v - 0.5) * 3.2 + 0.5, 0.0, 1.0);
}

// Height of the silk surface: two rounds of domain warping.
fn silk(p: vec2<f32>, t: f32, warp: f32) -> f32 {
    let q = vec2<f32>(fbm3(p, t), fbm3(p + vec2<f32>(5.2, 1.3), t));
    let r = vec2<f32>(fbm3(p + warp * q + vec2<f32>(1.7, 9.2), t), fbm3(p + warp * q + vec2<f32>(8.3, 2.8), t));
    return fbm3(p + warp * r * 1.2, t);
}

// Thin-film interference tint for a given optical thickness.
fn film(x: f32) -> vec3<f32> {
    return 0.5 + 0.5 * cos(TAU * (x + vec3<f32>(0.0, 0.33, 0.67)));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * 1.6;
    let t = u.clock4.z * 0.012 + u.seed * 0.01;
    let warp = 1.1 + 0.9 * u.pres4.x;
    // Bass hits send a soft ripple out from the centre.
    let r = length(p);
    let ripple = sin(r * 7.0 - (1.0 - u.hits4.x) * 5.0) * u.hits4.x * 0.06 * exp(-r * 0.8);
    let pp = p + normalize(p + 1e-4) * ripple;

    let e = 0.03;
    let h = silk(pp, t, warp);
    let hx = silk(pp + vec2<f32>(e, 0.0), t, warp);
    let hy = silk(pp + vec2<f32>(0.0, e), t, warp);
    let n = normalize(vec3<f32>((h - hx) / e, (h - hy) / e, 1.6));
    let l = normalize(vec3<f32>(-0.4, -0.6, 1.0));
    let diff = max(dot(n, l), 0.0);
    let v = vec3<f32>(0.0, 0.0, 1.0);
    let hv = normalize(l + v);
    let spec = pow(max(dot(n, hv), 0.0), 60.0);
    // Folds: where the surface tilts hardest.
    let fold = 1.0 - n.z;

    let hue = u.clock4.w * 0.01 + u.hue;
    let base = palette(h * 1.4 + hue);
    let sheen = film(fold * 2.5 + h * 1.5 + hue * 2.0);
    let drive = 0.5 + 0.8 * u.intensity;
    var col = base * base * (0.04 + 0.96 * diff * diff) * 0.9;
    col = mix(col, sheen * base * 1.2, smoothstep(0.08, 0.4, fold) * 0.45);
    col += vec3<f32>(1.0) * spec * (0.5 + 1.5 * u.hits4.w) * 0.8;
    col *= drive;
    // Soft vignette into deep shadow between the ribbons.
    col *= smoothstep(0.2, 0.75, h);
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
