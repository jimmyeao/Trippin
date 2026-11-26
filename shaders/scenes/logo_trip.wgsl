// @title Logo Trip
// @bloom 0.35
// Logo Trip: the DJ's own logo (or any picked image: Stream > Branding >
// Logo scene; else the Trippin logo) run through a rotating set of
// treatments: a droste spiral, a melt, halftone, VHS and a kaleidoscope of
// tiles. A new treatment every eight bars, morphing in over two beats (as the
// laser formations do). The image's shape follows the music: the spiral
// turns on the energy clock and winds the other way each phrase, the melt
// runs deeper with the bass presence, the halftone dots swell with the mids,
// the VHS tracking frays with the highs, and the tiles breathe with the bass.

// The logo, centred, `half` its half-height, at p (centred(), y down):
// premultiplied colour and coverage.
fn logo_at(p: vec2<f32>, half: f32) -> vec4<f32> {
    let c = logo(p / (2.0 * half) + 0.5);
    return vec4<f32>(c.rgb * c.a, c.a);
}

// The logo's own colours, lifted a little toward the palette so it sits in
// the show's light.
fn tint(c: vec4<f32>, t: f32) -> vec3<f32> {
    let pal = palette(t);
    return mix(c.rgb, pal * pal * c.a * 1.2, 0.25) * 1.15;
}

// 0 · Droste: the logo inside itself, spiralling in for ever. Log-polar,
// with copies a factor S apart; it zooms in on the energy clock and the
// whole spiral swings round one way, then the other, every eight bars. (The
// twist must stay a whole number, or the spiral tears where the angle wraps.)
fn droste(p_in: vec2<f32>) -> vec3<f32> {
    let p = rot(1.2 * sin(TAU * u.flow / 32.0)) * p_in;
    let s_ln = log(2.6);
    let r = max(length(p), 1e-4);
    let a = angle(p);
    let t = u.clock4.x * 0.12;
    let lr = log(r) + a / TAU * s_ln - t * s_ln * 0.5;
    let band = fract(lr / s_ln);
    let rr = exp(band * s_ln) * 0.32;
    let q = vec2<f32>(cos(a), sin(a)) * rr;
    let c = logo_at(q, 0.62);
    // Fade the deepest copies into the dark at the centre.
    let depth = smoothstep(0.0, 0.25, r);
    return tint(c, 0.15 + band * 0.5) * depth;
}

// 1 · Melt: the logo runs down in drips whose length follows the bass
// presence; the drips crawl on the energy clock.
fn melt(p: vec2<f32>) -> vec3<f32> {
    let x = p.x * 3.0;
    let n = noise(vec2<f32>(x, u.clock4.x * 0.06)) * 0.7 + noise(vec2<f32>(x * 2.7, u.clock4.x * 0.1 + 3.0)) * 0.3;
    let run = smoothstep(0.3, 0.85, n);
    let depth = (0.3 + 0.6 * u.pres4.x) * run;
    // Pull from above: the further down, the longer the run behind it.
    let fall = clamp((p.y + 0.7) / 1.4, 0.0, 1.0);
    let q = vec2<f32>(p.x + 0.03 * sin(p.y * 6.0 + u.clock4.x * 0.3), p.y - depth * fall);
    let c = logo_at(q, 0.62);
    return tint(c, 0.3 + 0.3 * run);
}

// 2 · Halftone: the logo in printed dots on a turning grid; the dots swell
// with the mids.
fn halftone(p: vec2<f32>) -> vec3<f32> {
    let cell = 0.045;
    let ang = 0.35 + 0.25 * sin(TAU * u.flow / 64.0);
    let q = rot(ang) * p;
    let id = floor(q / cell);
    let f = fract(q / cell) - 0.5;
    let centre = rot(-ang) * ((id + 0.5) * cell);
    let c = logo_at(centre, 0.62);
    let ink = dot(c.rgb, vec3<f32>(0.3, 0.5, 0.2)) * 0.6 + c.a * 0.4;
    let rad = sqrt(clamp(ink, 0.0, 1.0)) * (0.42 + 0.22 * u.lvl4.y);
    let dotm = 1.0 - smoothstep(rad - 0.06, rad + 0.02, length(f));
    let col = select(c.rgb / max(c.a, 1e-3), palette(0.5), c.a < 0.02);
    return mix(col, palette(0.4 + 0.2 * ink) * palette(0.4), 0.3) * dotm * c.a * 1.2;
}

// 3 · VHS: a tape of the logo: lines wobble, colours split, scanlines, and
// a tracking band rolls down; the highs fray the tracking.
fn vhs(p: vec2<f32>) -> vec3<f32> {
    let y = p.y;
    let roll = fract(u.clock4.x * 0.02);
    let bd = (fract((y + 1.0) * 0.5 - roll) - 0.5) * 9.0;
    let band = exp(-bd * bd);
    let wob = (noise(vec2<f32>(y * 40.0, u.clock4.w * 0.3)) - 0.5) * (0.01 + 0.05 * band * (0.4 + u.lvl4.w));
    let sep = 0.012 + 0.02 * u.lvl4.w;
    let q = p + vec2<f32>(wob, 0.0);
    let r = logo_at(q + vec2<f32>(sep, 0.0), 0.62);
    let g = logo_at(q, 0.62);
    let b = logo_at(q - vec2<f32>(sep, 0.0), 0.62);
    var col = vec3<f32>(r.r, g.g, b.b) * 1.2;
    let scan = 0.75 + 0.25 * sin((y + 1.0) * u.res_y * 1.6);
    col *= scan;
    // The tracking band shows only faintly off the logo.
    col += vec3<f32>(0.01, 0.012, 0.018) * band;
    return col;
}

// 4 · Kaleidoscope tiles: the logo folded into six-fold mirrors and tiled;
// the tiles breathe with the bass presence and the pattern turns, one way
// then the other each phrase.
fn tiles(p: vec2<f32>) -> vec3<f32> {
    let spin = 0.25 * sin(TAU * u.flow / 32.0) + u.clock4.x * 0.01;
    var q = rot(spin) * p;
    let n = 1.7 - 0.35 * u.pres4.x;
    q *= n;
    // Hex-ish mirror tiling, then a six-fold fold in each tile.
    let cell = vec2<f32>(1.0, 1.7320508);
    let a = q - cell * floor(q / cell + 0.5);
    let b = (q - cell * 0.5) - cell * floor((q - cell * 0.5) / cell + 0.5);
    let g = select(b, a, dot(a, a) < dot(b, b));
    let w = TAU / 6.0;
    let ang = abs(fract(angle(g) / w) * w - w * 0.5);
    let k = vec2<f32>(cos(ang), sin(ang)) * length(g);
    let c = logo_at(k - vec2<f32>(0.22, 0.0), 0.3);
    return tint(c, 0.6 + 0.2 * length(g));
}

fn treatment(i: u32, p: vec2<f32>) -> vec3<f32> {
    switch i {
        case 0u: { return droste(p); }
        case 1u: { return melt(p); }
        case 2u: { return halftone(p); }
        case 3u: { return vhs(p); }
        default: { return tiles(p); }
    }
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // The whole picture breathes a little with the kick (shape, not light).
    let p = centred(in.uv) / (1.0 + 0.04 * kick_swell());
    // A new treatment every eight bars, morphing in over two beats.
    let k = floor(u.flow / 32.0);
    let m = smoothstep(0.0, 2.0, u.flow - k * 32.0);
    let ia = u32(k) % 5u;
    let ib = (ia + 4u) % 5u;
    var col = treatment(ia, p);
    if m < 1.0 {
        col = mix(treatment(ib, p), col, m);
    }
    // A dark palette ground with a soft glow behind the centre, and short
    // trails.
    let r = length(p);
    let g = palette(0.7 + 0.1 * sin(u.clock4.x * 0.05));
    var bg = g * g * 0.05 * exp(-r * 1.4) + vec3<f32>(0.004, 0.004, 0.01);
    bg += prev(in.uv) * 0.3;
    return vec4<f32>(finite(max(col, vec3<f32>(0.0)) + bg * (1.0 - min(dot(col, vec3<f32>(0.33)), 1.0))), 1.0);
}
