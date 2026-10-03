// Silhouette dancers, alpha-blended (premultiplied) over the scene. Written
// into the feedback target, so scenes with trails leave echoes of them.
// Slot 0 is the main dancer; in the canon, slots 1 and 2 dance either side,
// each with its own routine. Must match `DancerUniforms` in src/dancer.rs.

struct Slot {
    frame: f32,    // fractional frame index into this slot's loop
    frames: f32,   // 0 = nothing loaded in this slot
    aspect: f32,   // mask width / height
    _pad: f32,
};

struct D {
    slots: array<Slot, 3>,
    opacity: f32,
    style: f32,    // 0 shadow, 1 neon, 2 strobe, 3 comic, 4 wire
    count: f32,    // 1, or 3 for the canon
    scale: f32,    // main dancer height as a fraction of screen height
    trail: f32,    // 1 = ghost echoes of earlier frames trail her movement
    canon_fade: f32, // companions' fade 0..1 (eases with the canon toggle)
};

@group(1) @binding(0) var masks0: texture_2d_array<f32>;
@group(1) @binding(1) var masks1: texture_2d_array<f32>;
@group(1) @binding(2) var masks2: texture_2d_array<f32>;
@group(1) @binding(3) var<uniform> d: D;

fn sample_layer(slot: i32, luv: vec2<f32>, layer: i32) -> f32 {
    if slot == 1 {
        return textureSampleLevel(masks1, samp, luv, layer, 0.0).r;
    }
    if slot == 2 {
        return textureSampleLevel(masks2, samp, luv, layer, 0.0).r;
    }
    return textureSampleLevel(masks0, samp, luv, layer, 0.0).r;
}

fn mask_size(slot: i32) -> vec2<f32> {
    if slot == 1 {
        return vec2<f32>(textureDimensions(masks1).xy);
    }
    if slot == 2 {
        return vec2<f32>(textureDimensions(masks2).xy);
    }
    return vec2<f32>(textureDimensions(masks0).xy);
}

fn wrap_frame(f: f32, n: f32) -> f32 {
    return ((f % n) + n) % n;
}

// Mask coverage at mask-space uv at a (possibly fractional, wrapping) frame,
// blending neighbouring frames.
fn mask_at_frame(slot: i32, luv: vec2<f32>, frame: f32) -> f32 {
    let s = d.slots[slot];
    let inside = all(luv >= vec2<f32>(0.0)) && all(luv <= vec2<f32>(1.0));
    let fa = floor(wrap_frame(frame, s.frames));
    let fb = wrap_frame(fa + 1.0, s.frames);
    let a = sample_layer(slot, luv, i32(fa));
    let b = sample_layer(slot, luv, i32(fb));
    let m = select(0.0, mix(a, b, fract(frame)), inside);
    // Soft clip: some clips have her limbs leaving the sprite (cropped source
    // footage). Fade at the borders so they dissolve instead of slicing to a
    // hard edge — the edge gradient would draw that as a bright line.
    // The bottom edge stays hard: her feet are planted on the floor.
    let fade = smoothstep(0.0, 0.035, luv.x) * smoothstep(0.0, 0.035, 1.0 - luv.x)
             * smoothstep(0.0, 0.02, luv.y);
    return m * fade;
}

fn mask_at(slot: i32, luv: vec2<f32>) -> f32 {
    return mask_at_frame(slot, luv, d.slots[slot].frame);
}

struct Hit {
    m: f32,
    edge: f32,
    luv: vec2<f32>,
};

fn dancer_luv(p: vec2<f32>, slot: i32, x: f32, size: f32, flip: bool) -> vec2<f32> {
    let h = 2.0 * d.scale * size;
    let w = h * d.slots[slot].aspect;
    let bottom = 0.98;
    var luv = vec2<f32>((p.x - x) / w + 0.5, (p.y - (bottom - h)) / h);
    if flip {
        luv.x = 1.0 - luv.x;
    }
    return luv;
}

fn dancer_at(p: vec2<f32>, slot: i32, x: f32, size: f32, flip: bool) -> Hit {
    let luv = dancer_luv(p, slot, x, size, flip);
    let texel = 2.0 / mask_size(slot);
    let gx = mask_at(slot, luv + vec2<f32>(texel.x, 0.0)) - mask_at(slot, luv - vec2<f32>(texel.x, 0.0));
    let gy = mask_at(slot, luv + vec2<f32>(0.0, texel.y)) - mask_at(slot, luv - vec2<f32>(0.0, texel.y));
    var hit: Hit;
    hit.m = mask_at(slot, luv);
    hit.edge = clamp(length(vec2<f32>(gx, gy)) * 1.5, 0.0, 1.0);
    hit.luv = luv;
    return hit;
}

// Premultiplied colour + alpha for one dancer.
fn shade(hit: Hit, tint: f32) -> vec4<f32> {
    let m = hit.m * d.opacity;
    let e = hit.edge * d.opacity;
    let style = i32(d.style + 0.5);
    if style == 1 {
        // Neon outline around a dark body, so she stays readable even on
        // scenes that smear their feedback (fluid).
        let col = e * palette(hit.luv.y * 0.5 + u.time * 0.1 + tint) * (1.5 + 3.0 * u.kick);
        return vec4<f32>(col, max(m * 0.85, e * 0.3));
    }
    if style == 2 {
        // Strobe: a white flash of the body on each beat, outline in between.
        // It must always pulse — in a breakdown it slows to a softer flash
        // every two beats instead of the hard per-beat hit (not beat_pulse(),
        // which deliberately goes steady in breakdowns).
        let hard = exp(-u.beat_phase * 10.0);
        let soft = 0.2 + 0.8 * exp(-fract(u.beat * 0.5) * 4.0);
        let flash = mix(hard, soft, u.calm);
        let col = vec3<f32>(2.0) * flash * m + e * palette(0.5 + tint);
        return vec4<f32>(col, m * flash);
    }
    // Shadow: black cut-out with a rim glow that pumps with the kick.
    let rim = e * palette(0.1 + u.beat * 0.05 + tint) * (0.6 + 1.5 * u.kick);
    return vec4<f32>(rim, m);
}

fn over(front: vec4<f32>, back: vec4<f32>) -> vec4<f32> {
    return front + back * (1.0 - front.a);
}

// ---- Drawn looks: comic (a-ha, Take On Me) and wire (Dire Straits, Money for
// Nothing). Both sample the mask a few times around the pixel, so they bail
// out early outside the sprite.

// Screen-space edge strength of the mask at mask uv `luv`; `k` widens the
// line (the sampling distance, in 2-texel units).
fn edge_at(slot: i32, luv: vec2<f32>, k: f32) -> f32 {
    let texel = 2.0 * k / mask_size(slot);
    let gx = mask_at(slot, luv + vec2<f32>(texel.x, 0.0)) - mask_at(slot, luv - vec2<f32>(texel.x, 0.0));
    let gy = mask_at(slot, luv + vec2<f32>(0.0, texel.y)) - mask_at(slot, luv - vec2<f32>(0.0, texel.y));
    return clamp(length(vec2<f32>(gx, gy)) * 1.5, 0.0, 1.0);
}

// Cheap 5-tap blur of the mask: the centre plus a cross at radius `r`
// (a fraction of the sprite height, so the same on screen in x and y).
fn blur5(slot: i32, luv: vec2<f32>, r: f32) -> f32 {
    let rx = r / d.slots[slot].aspect;
    return (mask_at(slot, luv) * 2.0
        + mask_at(slot, luv + vec2<f32>(rx, 0.0)) + mask_at(slot, luv - vec2<f32>(rx, 0.0))
        + mask_at(slot, luv + vec2<f32>(0.0, r)) + mask_at(slot, luv - vec2<f32>(0.0, r))) / 6.0;
}

fn outside_sprite(luv: vec2<f32>) -> bool {
    return any(luv < vec2<f32>(-0.2)) || any(luv > vec2<f32>(1.2));
}

fn saturate_hue(c: vec3<f32>) -> vec3<f32> {
    return c / max(max(c.r, max(c.g, c.b)), 1e-3);
}

// Comic action lines radiating from behind the dancer: a fixed set of
// strokes whose reach follows the slow bass presence (and a little of the
// kick), so they swell and relax rather than flash.
fn comic_lines(p: vec2<f32>, x: f32, size: f32) -> vec4<f32> {
    let h = 2.0 * d.scale * size;
    let q = p - vec2<f32>(x, 0.98 - h * 0.5);
    let r = length(q) / h;
    let k = angle(q) / TAU * 56.0;
    let id = floor(k);
    let on = step(0.74, hash21(vec2<f32>(id, 11.0)));
    let drive = (0.6 * u.pres4.x + 0.4 * u.hits4.x) * (1.0 - 0.6 * u.calm);
    let reach = 0.12 + 0.35 * drive;
    let r0 = 0.58 + 0.2 * hash21(vec2<f32>(id, 5.0));
    let along = smoothstep(r0, r0 + 0.03, r) * (1.0 - smoothstep(r0 + 0.04, r0 + reach, r));
    let w = abs(fract(k) - 0.5) * 2.0;
    // Each stroke starts fat and narrows to a point.
    let width_here = clamp(0.32 - (r - r0) * 1.2, 0.02, 0.32);
    let taper = 1.0 - smoothstep(width_here * 0.6, width_here, w);
    let a = on * along * taper * 0.9 * (0.5 + 0.5 * (1.0 - u.calm));
    return vec4<f32>(vec3<f32>(0.95, 0.92, 0.85) * a, a) * d.opacity;
}

// Comic: a rotoscoped pencil sketch, a-ha "Take On Me". Ink outline, graphite
// hatching on the shadow side, a paper fill that picks up colour as the track
// drives (pure pencil in a breakdown), Ben-Day dots round the figure and a
// 12 fps line boil like hand-redrawn frames.
fn comic_look(
    slot: i32, p: vec2<f32>, x: f32, size: f32, flip: bool, tint: f32,
) -> vec4<f32> {
    let lines = comic_lines(p, x, size);
    let luv0 = dancer_luv(p, slot, x, size, flip);
    if outside_sprite(luv0) {
        return lines;
    }
    // Line boil: the sampling position wobbles, stepped at 12 fps.
    let step_t = floor(u.time * 12.0);
    let wob = (vec2<f32>(noise(luv0 * 22.0 + step_t * 1.7), noise(luv0 * 22.0 + 37.0 - step_t * 1.3)) - 0.5) * 0.014;
    let luv = luv0 + wob;
    let m = mask_at(slot, luv);
    let edge = edge_at(slot, luv, 2.5);
    let inner = blur5(slot, luv, 0.03);
    // Light from the left: the right-hand side of the figure is in shadow.
    let lit = mask_at(slot, luv - vec2<f32>(0.02, 0.0)) - mask_at(slot, luv + vec2<f32>(0.02, 0.0));
    let shade = clamp(0.22 + 0.5 * (1.0 - inner) + 0.5 * max(lit, 0.0), 0.0, 1.0);
    // Pencil hatching, cross-hatched in the darkest parts; stroke width follows the shade.
    let hp = (p.x + p.y) * 64.0 + noise(p * 30.0 + step_t) * 0.6;
    let h1 = 1.0 - smoothstep(0.0, 0.04 + 0.3 * shade, abs(fract(hp) - 0.5));
    let hp2 = (p.x - p.y) * 64.0 + noise(p * 30.0 + 9.0 + step_t) * 0.6;
    let h2 = 1.0 - smoothstep(0.0, 0.6 * max(shade - 0.55, 0.0), abs(fract(hp2) - 0.5));
    let hatch = clamp(h1 + h2, 0.0, 1.0) * (0.55 + 0.45 * noise(p * 140.0));
    let paper = vec3<f32>(0.94, 0.91, 0.83);
    let wash_amt = (1.0 - u.calm) * (0.1 + 0.3 * u.intensity);
    let wash = palette(tint + 0.55 + luv.y * 0.25);
    let base = mix(paper, paper * (0.35 + 1.1 * wash), wash_amt);
    let ink = clamp(edge * 2.2 + hatch * 1.0, 0.0, 1.0);
    let body = mix(base, vec3<f32>(0.03, 0.03, 0.05), ink);
    let a_body = max(m, edge);
    // Ben-Day dots in the halo outside the figure: dot size follows how far from the body.
    let halo = clamp(blur5(slot, luv, 0.07) - m, 0.0, 1.0);
    let cell = fract(rot(0.7854) * (p * 48.0)) - 0.5;
    let dot_r = 0.5 * sqrt(clamp(halo * 2.2, 0.0, 1.0));
    let dots = (1.0 - smoothstep(dot_r - 0.08, dot_r, length(cell))) * step(0.02, halo) * (1.0 - a_body);
    let dot_c = palette(tint + 0.1 + u.time * 0.03);
    let col = body * a_body + dot_c * dot_c * dots;
    let a = clamp(a_body + dots, 0.0, 1.0);
    return over(vec4<f32>(col, a) * d.opacity, lines);
}

// Wire: neon CGI figures, Dire Straits "Money for Nothing". A two-colour
// outline split by a small chromatic offset, nested contour lines inside the
// body, a soft halo and CRT scanlines, on a dark glass body.
fn wire_look(
    slot: i32, p: vec2<f32>, x: f32, size: f32, flip: bool, tint: f32,
) -> vec4<f32> {
    let luv = dancer_luv(p, slot, x, size, flip);
    if outside_sprite(luv) {
        return vec4<f32>(0.0);
    }
    let m = mask_at(slot, luv);
    let split = 0.004 + 0.008 * u.pres4.x;
    let e_a = edge_at(slot, luv + vec2<f32>(split, 0.0), 1.6);
    let e_b = edge_at(slot, luv - vec2<f32>(split, 0.0), 1.6);
    let b2 = blur5(slot, luv, 0.03);
    let halo = clamp(blur5(slot, luv, 0.07) - m * 0.6, 0.0, 1.0);
    // Contour lines inside the body: iso-lines of the blurred mask follow the shape.
    let c1 = 1.0 - smoothstep(0.0, 0.025, abs(b2 - 0.62));
    let c2 = 1.0 - smoothstep(0.0, 0.02, abs(b2 - 0.84));
    let contour = max(c1, c2 * 0.8) * m;
    let neon_a = saturate_hue(palette(tint));
    let neon_b = saturate_hue(palette(tint + 0.5));
    let outline = e_a * neon_a + e_b * neon_b;
    let inner = contour * mix(neon_b, neon_a, smoothstep(0.2, 0.8, luv.y));
    let glow = halo * 0.35 * mix(neon_a, neon_b, 0.5);
    let scan = 0.8 + 0.2 * sin(p.y * 440.0 + u.time * 3.0);
    let gain = 1.5 + 0.8 * u.pres4.x;
    let light = (outline * 1.6 + inner * 0.7 + glow) * scan * gain;
    let line_a = max(max(e_a, e_b), max(contour * 0.8, halo * 0.3));
    let a = clamp(m * 0.5 + line_a, 0.0, 1.0);
    return vec4<f32>(light, a) * d.opacity;
}

// A dancer with her motion echoes layered behind her: earlier frames of the
// same routine sampled at a lag, hue-shifted and dimmer — cheap "video echo"
// trails that work under every look.
fn shaded_with_trails(
    p: vec2<f32>, slot: i32, x: f32, size: f32, flip: bool, tint: f32,
) -> vec4<f32> {
    let style = i32(d.style + 0.5);
    var acc: vec4<f32>;
    if style == 3 {
        acc = comic_look(slot, p, x, size, flip, tint);
    } else if style == 4 {
        acc = wire_look(slot, p, x, size, flip, tint);
    } else {
        acc = shade(dancer_at(p, slot, x, size, flip), tint);
    }
    if d.trail > 0.5 {
        let luv = dancer_luv(p, slot, x, size, flip);
        let frame = d.slots[slot].frame;
        for (var g = 1; g <= 3; g++) {
            let gm = mask_at_frame(slot, luv, frame - f32(g) * 5.0);
            let a = gm * d.opacity * 0.5 / f32(g);
            let col = palette(tint + f32(g) * 0.08 + u.beat * 0.04) * a * (1.0 + u.kick);
            acc = over(acc, vec4<f32>(col, a));
        }
    }
    return acc;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var out = vec4<f32>(0.0);
    if d.count > 1.5 && d.canon_fade > 0.001 {
        // Canon: a companion either side, smaller, each with its own routine.
        // canon_fade eases them in/out when the toggle flips.
        let side = 0.8;
        let w0 = 2.0 * d.scale * d.slots[0].aspect;
        for (var i = 1; i < 3; i++) {
            if d.slots[i].frames < 0.5 {
                continue;
            }
            let wi = 2.0 * d.scale * side * d.slots[i].aspect;
            // Clear of the main dancer, but kept inside the screen edges.
            let x = min((w0 * 0.5 + wi * 0.5) * 1.05, aspect() - wi * 0.5);
            let sx = select(x, -x, i == 1);
            let c = shaded_with_trails(p, i, sx, side, i == 1, f32(i) * 0.33)
                  * vec4<f32>(vec3<f32>(d.canon_fade), d.canon_fade);
            out = over(c, out);
        }
    }
    out = over(shaded_with_trails(p, 0, 0.0, 1.0, false, 0.0), out);
    return out;
}
