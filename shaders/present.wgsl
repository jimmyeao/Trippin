// Post-process the scene's HDR frame onto the window: FX transform (mirror /
// kaleido, per u.fx/u.fx_amt), kick-driven chromatic aberration, bloom (for
// scenes that opt in with `// @bloom`), tone mapping (ACES, or AgX for
// `// @tonemap agx`), vignette, cut flash and a little film grain.

fn aces(x: vec3<f32>) -> vec3<f32> {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

// u.fx modes: 1 mirror X, 2 mirror Y, 3 quad mirror, 4 kaleido 6, 5 kaleido 8,
// then the lens effects: 6 glass ribs, 7 glass fan, 8 fly eye, 9 anamorphic,
// 10 melt, 11 pixels, 12 tiles, 13 twist (`Fx` in config.rs).
// The fold axis is the screen centre, so a centred subject (the dancer) stays
// whole and symmetric rather than being split out to the screen edges.
// Returns the uv to read the scene at, and z: a light factor for the lens
// effects' own highlights (glass edges, facet rims; 0 = none).
// The lens effects move like the scenes do (AGENTS §2): their shape follows
// the slow presence and the smooth clocks, swinging over phrases; nothing
// pulses on the beat.
fn fx_uv(uv: vec2<f32>, m: i32) -> vec3<f32> {
    if m == 1 {
        return vec3<f32>(0.5 - abs(uv.x - 0.5), uv.y, 0.0);
    }
    if m == 2 {
        return vec3<f32>(uv.x, 0.5 - abs(uv.y - 0.5), 0.0);
    }
    if m == 3 {
        return vec3<f32>(0.5 - abs(uv - 0.5), 0.0);
    }
    let asp = aspect();
    let d = (uv - 0.5) * vec2<f32>(asp, 1.0);
    let swing = sin(TAU * u.flow / 32.0);
    if m == 4 || m == 5 {
        let seg = select(8.0, 6.0, m == 4);
        let r = length(d);
        let w = TAU / seg;
        let a = abs(fract(angle(d) / w) * w - w * 0.5);
        return vec3<f32>(0.5 + vec2<f32>(cos(a), sin(a)) * r / vec2<f32>(asp, 1.0), 0.0);
    }
    if m == 6 {
        // Glass ribs: fluted glass, each rib a lens that shifts its slice of
        // the frame; the ribs lean with the phrase and refract harder with
        // the bass presence.
        let n = 16.0;
        let q = rot(0.18 * swing) * d;
        let f = fract(q.x * n / asp + 0.5) - 0.5;
        let k = (0.05 + 0.07 * u.pres4.x) * asp / n * 6.0;
        let off = rot(-0.18 * swing) * vec2<f32>(f * k, 0.0);
        let lite = 0.35 * smoothstep(0.32, 0.5, abs(f)) - 0.12 * (1.0 - abs(f) * 2.0);
        return vec3<f32>(uv - off / vec2<f32>(asp, 1.0), lite);
    }
    if m == 7 {
        // Glass fan: the same glass bent round the centre in wedges, the fan
        // turning slowly on the energy clock.
        let n = 18.0;
        let r = length(d);
        let a = angle(d) + u.clock4.x * 0.02;
        let f = fract(a * n / TAU + 0.5) - 0.5;
        let k = (0.25 + 0.35 * u.pres4.x) / n * TAU;
        let a2 = angle(d) - f * k;
        let q = vec2<f32>(cos(a2), sin(a2)) * r;
        let lite = 0.35 * smoothstep(0.32, 0.5, abs(f)) * smoothstep(0.02, 0.15, r);
        return vec3<f32>(0.5 + q / vec2<f32>(asp, 1.0), lite);
    }
    if m == 8 {
        // Fly eye: a honeycomb of lenses, each showing its own patch of the
        // frame magnified and turned over; the lenses grow with the bass
        // presence and the comb drifts on the energy clock.
        let size = 0.09 + 0.04 * u.pres4.x;
        let p = d / size + vec2<f32>(u.clock4.x * 0.03, 0.0);
        let s = vec2<f32>(1.0, 1.7320508);
        let a = p - s * floor(p / s + 0.5);
        let b = (p - s * 0.5) - s * floor((p - s * 0.5) / s + 0.5);
        let g = select(b, a, dot(a, a) < dot(b, b));
        let c = (p - g - vec2<f32>(u.clock4.x * 0.03, 0.0)) * size;
        let q = c - g * size * 0.9;
        let rim = smoothstep(0.38, 0.5, length(g));
        return vec3<f32>(0.5 + q / vec2<f32>(asp, 1.0), 0.35 * rim - 0.15 * rim * rim);
    }
    if m == 10 {
        // Melt: the frame drips down in runs whose length follows the bass
        // presence; the runs crawl on the energy clock.
        let x = uv.x * 9.0;
        let n = noise(vec2<f32>(x, u.clock4.x * 0.05)) * 0.7 + noise(vec2<f32>(x * 3.1, u.clock4.x * 0.08 + 4.0)) * 0.3;
        let run = smoothstep(0.3, 0.8, n);
        let depth = (0.12 + 0.22 * u.pres4.x) * run * (0.15 + 0.85 * uv.y);
        return vec3<f32>(uv.x, uv.y - depth, 0.0);
    }
    if m == 11 {
        // Pixels: big pixels whose size breathes with the bass presence.
        let px = (8.0 + 18.0 * u.pres4.x) / u.res_y;
        let c = (floor(d / px) + 0.5) * px;
        let e = abs(fract(d / px) - 0.5);
        let gap = smoothstep(0.42, 0.5, max(e.x, e.y));
        return vec3<f32>(0.5 + c / vec2<f32>(asp, 1.0), -0.35 * gap);
    }
    if m == 12 {
        // Tiles: the frame repeated in mirrored tiles (the sampler mirrors),
        // zooming with the bass presence and drifting on the energy clock,
        // the drift swinging round over phrases.
        let n = 2.6 + 0.8 * u.pres4.x;
        let drift = vec2<f32>(cos(swing * 1.2), sin(swing * 1.2)) * u.clock4.x * 0.01;
        return vec3<f32>(0.5 + d * n / vec2<f32>(asp, 1.0) * 0.5 + drift, 0.0);
    }
    if m == 13 {
        // Twist: a whirl round the centre that winds one way, then the other,
        // over eight bars, tighter with the bass presence.
        let r = length(d);
        let k = 1.6 * swing * (0.6 + 0.6 * u.pres4.x);
        let fall = 1.0 - smoothstep(0.0, 0.9, r);
        let q = rot(k * fall * fall) * d;
        return vec3<f32>(0.5 + q / vec2<f32>(asp, 1.0), 0.0);
    }
    return vec3<f32>(uv, 0.0);
}

// Anamorphic flare (u.fx 9): long horizontal streaks, tinted cool, off the
// lights that stand out from the frame (keyed to the frame's own average,
// so a bright full-frame scene doesn't white out and a dark one with a few
// lamps still flares); their length breathes with the bass presence. Read
// from the bloom chain (half res, already soft; the renderer runs it while
// this effect is on): 8 taps each side, jittered per pixel so no copies
// show. Full-res taps of the frame cost 0.4-0.7 ms at 1080p.
fn bloom_l(uv: vec2<f32>) -> f32 {
    let c = finite(textureSampleLevel(bloom_tex, samp, uv, 0.0).rgb) / 6.0;
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn anamorphic(uv: vec2<f32>, frag: vec2<f32>) -> vec3<f32> {
    // The frame's average, from nine fixed points (the same for every
    // pixel, so they stay in cache).
    var avg = 0.0;
    for (var gy = 0; gy < 3; gy++) {
        for (var gx = 0; gx < 3; gx++) {
            avg += bloom_l((vec2<f32>(f32(gx), f32(gy)) + 0.5) / 3.0);
        }
    }
    avg /= 9.0;
    let key = 0.02 + 1.6 * avg;
    let len = 0.55 + 0.3 * u.pres4.x;
    let j = bluen(frag);
    var s = 0.0;
    for (var i = 0; i < 8; i++) {
        let t = (f32(i) + j) / 8.0;
        let o = t * len;
        let w = exp(-2.5 * t);
        s += (max(bloom_l(uv + vec2<f32>(o, 0.0)) - key, 0.0) + max(bloom_l(uv - vec2<f32>(o, 0.0)) - key, 0.0)) * w;
    }
    s = s / 8.0 * 12.0;
    // A soft cap: streaks glow, they never fill the frame.
    return vec3<f32>(0.3, 0.55, 1.0) * (s / (1.0 + s)) * 0.6;
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

// The camera layer's key: how much of the camera shows at c (1 = all).
// Green/blue screen (chroma), dark background (luma), or none.
fn cam_key(c: vec3<f32>) -> f32 {
    let key = i32(u.cam4.z + 0.5);
    let tol = clamp(u.cam4.w, 0.0, 1.0);
    if key == 1 {
        let d = c.g - max(c.r, c.b);
        return 1.0 - smoothstep(0.02 + 0.12 * tol, 0.1 + 0.2 * tol, d);
    }
    if key == 2 {
        let d = c.b - max(c.r, c.g);
        return 1.0 - smoothstep(0.02 + 0.12 * tol, 0.1 + 0.2 * tol, d);
    }
    if key == 3 {
        let l = dot(c, vec3<f32>(0.299, 0.587, 0.114));
        return smoothstep(0.02 + 0.12 * tol, 0.08 + 0.2 * tol, l);
    }
    return 1.0;
}

// Photosensitive-safe limiter state (gfx::Statics::lum): the average
// luminance the frame may show, the gain that holds it there, on, and the
// rise allowed per second.
struct Lum {
    ref_l: f32,
    gain: f32,
    on: f32,
    rate: f32,
};
@group(0) @binding(11) var<storage, read_write> lum: Lum;

// The finished frame before grain: FX, bloom, tone map, vignette, cut flash
// and master. rgb = colour, a = the dancer's coverage (transparent mode).
// Shared by fs_main and the limiter's measure (fs_lum).
fn graded(scr_uv: vec2<f32>) -> vec4<f32> {
    let fxm = i32(u.fx + 0.5);
    let amt = clamp(u.fx_amt, 0.0, 1.0);
    let fx = fx_uv(scr_uv, fxm);
    let uv = mix(scr_uv, fx.xy, amt);
    let dir = uv - 0.5;
    let ca = 0.004 + 0.012 * u.kick * u.intensity;
    var col = vec3<f32>(
        textureSampleLevel(prev_tex, samp, uv + dir * ca, 0.0).r,
        textureSampleLevel(prev_tex, samp, uv, 0.0).g,
        textureSampleLevel(prev_tex, samp, uv - dir * ca, 0.0).b
    );
    col = finite(col);
    // The lens effects' own light (glass edges, facet rims) and flare.
    col *= max(1.0 + fx.z * amt, 0.0);
    if fxm == 9 {
        col += anamorphic(uv, scr_uv * vec2<f32>(u.res_x, u.res_y)) * amt;
    }
    let clear_bg = u.misc4.x > 0.5;
    // Transparent mode: the target holds the dancer premultiplied over
    // alpha 0 — carry its coverage through.
    let cover = textureSampleLevel(prev_tex, samp, uv, 0.0).a;
    if u.bloom > 0.0 {
        // The chain sums 6 levels; normalise, then blend (energy-conserving).
        let b = finite(textureSampleLevel(bloom_tex, samp, uv, 0.0).rgb) / 6.0;
        col = mix(col, b, clamp(u.bloom, 0.0, 1.0) * 0.12);
    }
    let expo = 0.75 + 0.35 * u.intensity;
    // AgX lifts black off zero — a grey veil over the camera in transparent
    // mode — so that mode always takes ACES (black stays black).
    if u.tonemap > 0.5 && !clear_bg {
        col = agx(col * expo * 1.25);
    } else {
        col = aces(col * expo);
    }
    // The camera layer, in display space after tone mapping so the camera
    // looks like itself on every scene (ACES or AgX): behind the scene (it
    // shows through the scene's dark parts) or keyed in front of it.
    let cmode = i32(u.cam4.x + 0.5);
    if cmode > 0 && !clear_bg {
        var c = cam(vec2<f32>(1.0 - uv.x, uv.y));
        let a = cam_key(c);
        // Green / blue screen: pull the screen's colour out of the edges.
        let key = i32(u.cam4.z + 0.5);
        if key == 1 {
            c.g = min(c.g, max(c.r, c.b) * 1.05);
        } else if key == 2 {
            c.b = min(c.b, max(c.r, c.g) * 1.05);
        }
        let k = clamp(u.cam4.y, 0.0, 1.0) * a;
        if cmode == 1 {
            let sl = dot(col, vec3<f32>(0.2126, 0.7152, 0.0722));
            col += c * k * (1.0 - smoothstep(0.04, 0.5, sl));
        } else {
            col = mix(col, c, k);
        }
    }
    // Vignette and grain use screen-space uv so edges darken the same under FX.
    let scr = scr_uv - 0.5;
    col *= 1.0 - 0.9 * dot(scr, scr) * 1.6;
    if !clear_bg {
        col += u.flash * u.flash * 0.35;
    }
    return vec4<f32>(col * u.master, cover);
}

// Photosensitive-safe measure, one pixel, run just before present while the
// switch is on: the frame's average displayed luminance on a 16x9 grid.
// Darkening passes freely; brightening may rise only `rate` a second, and
// the gain present applies holds the frame to that (WCAG 2.3.1: no more than
// three flashes a second). It catches every scene's own flashes, not only
// the strobe and flash cues the render loop turns off.
@fragment
fn fs_lum(in: VsOut) -> @location(0) vec4<f32> {
    var sum = 0.0;
    for (var j = 0; j < 9; j++) {
        for (var i = 0; i < 16; i++) {
            let p = (vec2<f32>(f32(i), f32(j)) + 0.5) / vec2<f32>(16.0, 9.0);
            let c = clamp(graded(p).rgb, vec3<f32>(0.0), vec3<f32>(1.0));
            sum += dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
        }
    }
    let l = sum / 144.0;
    var r = lum.ref_l;
    if r < 0.0 || r != r {
        r = l;
    }
    r = min(l, r + lum.rate * clamp(u.dt, 0.0, 0.1));
    lum.ref_l = r;
    lum.gain = select(1.0, clamp(r / l, 0.0, 1.0), l > r + 1e-4);
    return vec4<f32>(l, r, 0.0, 1.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let g = graded(in.uv);
    let gain = select(1.0, lum.gain, lum.on > 0.5);
    var col = g.rgb * gain;
    let cover = g.a * gain;
    let clear_bg = u.misc4.x > 0.5;
    // Grain fades out in the blacks — flat noise there shimmers as a dirty
    // texture, speckles a blacked-out screen, and costs encoders bits on
    // pure noise. Real film grain vanishes in deep shadow anyway.
    let peak = max(col.r, max(col.g, col.b));
    col += (hash21(in.uv * vec2<f32>(u.res_x, u.res_y) + floor(fract(u.time * 7.0) * 997.0)) - 0.5) * 0.012 * smoothstep(0.0, 0.08, peak);
    if clear_bg {
        // Premultiplied out: anything lit (glow, bloom) earns alpha too, so
        // additive light reads over a camera feed instead of going grey.
        col = max(col, vec3<f32>(0.0));
        let a = clamp(max(cover * u.master, max(col.r, max(col.g, col.b))), 0.0, 1.0);
        return vec4<f32>(col, a);
    }
    // The surface is usually sRGB, so let the hardware do the encode.
    return vec4<f32>(max(col, vec3<f32>(0.0)), 1.0);
}
