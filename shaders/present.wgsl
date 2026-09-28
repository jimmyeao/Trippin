// Post-process the scene's HDR frame onto the window: FX transform (mirror /
// kaleido, per u.fx/u.fx_amt), kick-driven chromatic aberration, bloom (for
// scenes that opt in with `// @bloom`), tone mapping (ACES, or AgX for
// `// @tonemap agx`), vignette, cut flash and a little film grain.

fn aces(x: vec3<f32>) -> vec3<f32> {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

// u.fx modes: 1 mirror X, 2 mirror Y, 3 quad mirror, 4 kaleido 6, 5 kaleido 8.
// The fold axis is the screen centre, so a centred subject (the dancer) stays
// whole and symmetric rather than being split out to the screen edges.
fn fx_uv(uv: vec2<f32>, m: i32) -> vec2<f32> {
    if m == 1 {
        return vec2<f32>(0.5 - abs(uv.x - 0.5), uv.y);
    }
    if m == 2 {
        return vec2<f32>(uv.x, 0.5 - abs(uv.y - 0.5));
    }
    if m == 3 {
        return 0.5 - abs(uv - 0.5);
    }
    if m == 4 || m == 5 {
        let asp = aspect();
        let seg = select(8.0, 6.0, m == 4);
        let d = (uv - 0.5) * vec2<f32>(asp, 1.0);
        let r = length(d);
        let w = TAU / seg;
        let a = abs(fract(angle(d) / w) * w - w * 0.5);
        return 0.5 + vec2<f32>(cos(a), sin(a)) * r / vec2<f32>(asp, 1.0);
    }
    return uv;
}

// AgX (Troy Sobotka; Benjamin Wrensch's polynomial fit). Holds hue and
// desaturates toward white in the highlights instead of skewing to
// neon-yellow — the realistic scenes are lit for it.
fn agx(x: vec3<f32>) -> vec3<f32> {
    let m_in = mat3x3<f32>(
        vec3<f32>(0.842479062253094, 0.0423282422610123, 0.0423756549057051),
        vec3<f32>(0.0784335999999992, 0.878468636469772, 0.0784336),
        vec3<f32>(0.0792237451477643, 0.0791661274605434, 0.879142973793104)
    );
    let m_out = mat3x3<f32>(
        vec3<f32>(1.19687900512017, -0.0528968517574562, -0.0529716355144438),
        vec3<f32>(-0.0980208811401368, 1.15190312990417, -0.0980434501171241),
        vec3<f32>(-0.0990297440797205, -0.0989611768448433, 1.15107367264116)
    );
    let min_ev = -12.47393;
    let max_ev = 4.026069;
    var v = m_in * max(x, vec3<f32>(1e-10));
    v = clamp(log2(v), vec3<f32>(min_ev), vec3<f32>(max_ev));
    v = (v - min_ev) / (max_ev - min_ev);
    let v2 = v * v;
    let v4 = v2 * v2;
    v = 15.5 * v4 * v2 - 40.14 * v4 * v + 31.96 * v4 - 6.868 * v2 * v + 0.4298 * v2 + 0.1191 * v - 0.00232;
    // "Punchy" look (power 1.35, sat 1.4): base AgX lifts the shadows a lot,
    // which reads as grey on a club screen.
    v = pow(max(v, vec3<f32>(0.0)), vec3<f32>(1.35));
    let l = dot(v, vec3<f32>(0.2126, 0.7152, 0.0722));
    v = l + 1.4 * (v - l);
    v = m_out * v;
    // The fit outputs display-encoded values; the surface is sRGB, so undo
    // the 2.2 encode and let the hardware re-apply it.
    return pow(clamp(v, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(2.2));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let fxm = i32(u.fx + 0.5);
    let uv = mix(in.uv, fx_uv(in.uv, fxm), clamp(u.fx_amt, 0.0, 1.0));
    let dir = uv - 0.5;
    let ca = 0.004 + 0.012 * u.kick * u.intensity;
    var col = vec3<f32>(
        textureSampleLevel(prev_tex, samp, uv + dir * ca, 0.0).r,
        textureSampleLevel(prev_tex, samp, uv, 0.0).g,
        textureSampleLevel(prev_tex, samp, uv - dir * ca, 0.0).b
    );
    col = finite(col);
    if u.bloom > 0.0 {
        // The chain sums 6 levels; normalise, then blend (energy-conserving).
        let b = finite(textureSampleLevel(bloom_tex, samp, uv, 0.0).rgb) / 6.0;
        col = mix(col, b, clamp(u.bloom, 0.0, 1.0) * 0.25);
    }
    let expo = 0.75 + 0.35 * u.intensity;
    if u.tonemap > 0.5 {
        col = agx(col * expo * 1.25);
    } else {
        col = aces(col * expo);
    }
    // Vignette and grain use screen-space uv so edges darken the same under FX.
    let scr = in.uv - 0.5;
    col *= 1.0 - 0.9 * dot(scr, scr) * 1.6;
    col += u.flash * u.flash * 0.35;
    col *= u.master;
    // Grain fades out in the blacks — flat noise there shimmers as a dirty
    // texture, speckles a blacked-out screen, and costs encoders bits on
    // pure noise. Real film grain vanishes in deep shadow anyway.
    let lum = max(col.r, max(col.g, col.b));
    col += (hash21(in.uv * vec2<f32>(u.res_x, u.res_y) + floor(fract(u.time * 7.0) * 997.0)) - 0.5) * 0.012 * smoothstep(0.0, 0.08, lum);
    // The surface is usually sRGB, so let the hardware do the encode.
    return vec4<f32>(max(col, vec3<f32>(0.0)), 1.0);
}
