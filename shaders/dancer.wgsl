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
    burst: f32,      // disintegrate envelope: 0 whole, 1 scattered into dust
    burst_seed: f32, // 0..1, re-rolled per burst: the swirl's direction and field
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
    // Only where it's shaded: shade never drops below 0.22, so ungated strokes
    // hatched the lit side too and the body read as an engraving texture (M2
    // render, 1080p). A lighter warp keeps them straight like pencil strokes.
    let hp = (p.x + p.y) * 64.0 + noise(p * 30.0 + step_t) * 0.25;
    let h1 = (1.0 - smoothstep(0.0, 0.04 + 0.3 * shade, abs(fract(hp) - 0.5))) * smoothstep(0.35, 0.6, shade);
    let hp2 = (p.x - p.y) * 64.0 + noise(p * 30.0 + 9.0 + step_t) * 0.25;
    let h2 = 1.0 - smoothstep(0.0, 0.6 * max(shade - 0.55, 0.0), abs(fract(hp2) - 0.5));
    let hatch = clamp(h1 + h2, 0.0, 1.0) * (0.55 + 0.45 * noise(p * 140.0));
    let paper = vec3<f32>(0.94, 0.91, 0.83);
    // ~2x the first value: at 0.1 + 0.3i the paper barely tinted while the track
    // drove (torso saturation 0.125 vs 0.111 in a breakdown, M2 render).
    let wash_amt = (1.0 - u.calm) * (0.2 + 0.5 * u.intensity);
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
    // Half the first value: ~16 px between the red and cyan edges at 1080p read as
    // an anaglyph glitch rather than neon CGI (M2 render).
    let split = 0.002 + 0.004 * u.pres4.x;
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
    let luv = dancer_luv(p, slot, x, size, flip);
    if d.trail > 0.5 {
        let frame = d.slots[slot].frame;
        for (var g = 1; g <= 3; g++) {
            let gm = mask_at_frame(slot, luv, frame - f32(g) * 5.0);
            let a = gm * d.opacity * 0.5 / f32(g);
            let col = palette(tint + f32(g) * 0.08 + u.beat * 0.04) * a * (1.0 + u.kick);
            acc = over(acc, vec4<f32>(col, a));
        }
    }
    // Disintegrate: each cell of the body hides as its particle leaves.
    return acc * burst_keep(luv);
}

// Where a slot dances: centre x, size (fraction of the main dancer), mirrored,
// palette tint and fade. Slot 0 is the main dancer; in the canon, slots 1 and
// 2 dance either side, smaller, each with its own routine.
struct Place {
    x: f32,
    size: f32,
    flip: bool,
    tint: f32,
    fade: f32,
};

fn place(slot: i32) -> Place {
    if slot == 0 {
        return Place(0.0, 1.0, false, 0.0, 1.0);
    }
    let side = 0.8;
    let w0 = 2.0 * d.scale * d.slots[0].aspect;
    let wi = 2.0 * d.scale * side * d.slots[slot].aspect;
    // Clear of the main dancer, but kept inside the screen edges.
    let x = min((w0 * 0.5 + wi * 0.5) * 1.05, aspect() - wi * 0.5);
    return Place(select(x, -x, slot == 1), side, slot == 1, f32(slot) * 0.33, d.canon_fade);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var out = vec4<f32>(0.0);
    if d.count > 1.5 && d.canon_fade > 0.001 {
        // canon_fade eases the companions in/out when the toggle flips.
        for (var i = 1; i < 3; i++) {
            if d.slots[i].frames < 0.5 {
                continue;
            }
            let pl = place(i);
            let c = shaded_with_trails(p, i, pl.x, pl.size, pl.flip, pl.tint)
                  * vec4<f32>(vec3<f32>(pl.fade), pl.fade);
            out = over(c, out);
        }
    }
    out = over(shaded_with_trails(p, 0, 0.0, 1.0, false, 0.0), out);
    return out;
}

// ---- Disintegrate -----------------------------------------------------------
// On a drop the silhouette breaks into glowing dust and then re-forms (after
// the Disintegrate particle system in Imaginando's VS). One particle per cell
// of a P_W x P_H grid over each slot's mask, drawn as instanced quads straight
// from the mask (no particle state: every particle's position is a function
// of its cell, the envelope and the flow clock, so nothing can drift or
// accumulate). The head goes first and the feet last; on the way back the
// feet land first, and the dust lands on the *live* pose, so she is dancing
// again as she re-forms. Motion rides `u.flow` and a phrase-length swing,
// never raw audio; the slow bass presence only widens the cloud (shape).

const P_W: f32 = 160.0;
const P_H: f32 = 320.0;

fn burst_cell(luv: vec2<f32>) -> vec2<f32> {
    return floor(luv * vec2<f32>(P_W, P_H));
}

// This cell's own progress 0..1 through the burst: a staggered smoothstep of
// the envelope, head (luv.y 0) first, with jitter so the edge crumbles.
fn burst_local(cell: vec2<f32>) -> f32 {
    let y = clamp(cell.y / P_H, 0.0, 1.0);
    let order = y * 0.6 + hash21(cell + vec2<f32>(d.burst_seed * 977.0, 3.0)) * 0.4;
    let start = order * 0.55;
    return smoothstep(start, start + 0.45, d.burst);
}

// 1 while this cell of the body is still there, 0 once its particle left.
fn burst_keep(luv: vec2<f32>) -> f32 {
    if d.burst <= 0.0 {
        return 1.0;
    }
    return 1.0 - smoothstep(0.0, 0.12, burst_local(burst_cell(luv)));
}

// Two octaves of value noise, for the curl field.
fn burst_field(q: vec2<f32>) -> f32 {
    return noise(q) + 0.5 * noise(q * 2.3 + vec2<f32>(5.2, 1.3));
}

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    // Culled unless a particle is out: a degenerate quad off screen.
    o.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    o.q = vec2<f32>(0.0);
    o.col = vec3<f32>(0.0);
    let per = u32(P_W) * u32(P_H);
    let slot = i32(ii / per);
    let k = ii % per;
    if slot > 2 || d.slots[slot].frames < 0.5 {
        return o;
    }
    let pl = place(slot);
    if pl.fade < 0.001 {
        return o;
    }
    let cell = vec2<f32>(f32(k % u32(P_W)), f32(k / u32(P_W)));
    let e = burst_local(cell);
    if e < 0.002 {
        return o;
    }
    let j = hash22(cell + vec2<f32>(17.0, 41.0));
    let luv = (cell + 0.15 + 0.7 * j) / vec2<f32>(P_W, P_H);
    let m = mask_at(slot, luv);
    if m < 0.2 {
        return o;
    }
    // Home: where this cell sits on screen (dancer_luv inverted).
    let h = 2.0 * d.scale * pl.size;
    let w = h * d.slots[slot].aspect;
    let lx = select(luv.x, 1.0 - luv.x, pl.flip);
    let home = vec2<f32>(pl.x + (lx - 0.5) * w, 0.98 - h + luv.y * h);
    let c = vec2<f32>(pl.x, 0.98 - h * 0.55);
    let r = hash21(cell + vec2<f32>(3.1, 7.7));
    // Curl of a noise field that drifts on the flow clock: a divergence-free
    // swirl, so the dust eddies instead of clumping.
    let t = u.flow * 0.09 + d.burst_seed * 40.0;
    let qn = home / h * 1.8 + vec2<f32>(t, -0.7 * t);
    let ep = 0.04;
    let dx = burst_field(qn + vec2<f32>(ep, 0.0)) - burst_field(qn - vec2<f32>(ep, 0.0));
    let dy = burst_field(qn + vec2<f32>(0.0, ep)) - burst_field(qn - vec2<f32>(0.0, ep));
    let curl = vec2<f32>(dy, -dx) / (2.0 * ep);
    let rel = home - c;
    let reach = 0.85 + 0.5 * u.pres4.x;
    // Reach stays within about half her height: a cloud in her shape that
    // swirls, not confetti across the screen (first render filled the frame).
    // The outward push scales with the distance from her centre, so the
    // figure swells as a whole; a fixed-length push hollowed the middle out.
    let drift = rel * (0.15 + 0.45 * r)
              + curl * 0.05 * h
              + vec2<f32>(0.0, -(0.04 + 0.14 * r) * h);
    // The cloud turns as it flies out, one way or the other per burst, and
    // swings back and forth over a four-bar phrase while it hangs.
    let dir = select(1.0, -1.0, d.burst_seed < 0.5);
    let swing = 0.12 * sin(TAU * u.flow / 16.0 + d.burst_seed * TAU);
    let turn = e * (dir * (0.12 + 0.18 * r) + swing);
    let pos = c + rot(turn) * (rel + drift * e * reach);
    // A small round dot, a little larger once it is out.
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let q = corners[vi % 6u];
    let size = max(0.0032 + 0.0045 * e * r, 3.0 / u.res_y);
    let p = pos + q * size;
    o.pos = vec4<f32>(p.x / aspect(), -p.y, 0.0, 1.0);
    o.q = q;
    // Her own colour: the shadow look's rim hue (on the smooth clock), with
    // a narrow spread, so it reads as her dust and not confetti.
    let tone = palette(0.1 + u.flow * 0.05 + pl.tint + r * 0.08 + luv.y * 0.1);
    let a = smoothstep(0.2, 0.6, m) * smoothstep(0.0, 0.15, e) * d.opacity * pl.fade;
    o.col = tone * tone * 1.1 * a;
    return o;
}

@fragment
fn fs_particles(in: POut) -> @location(0) vec4<f32> {
    let r2 = dot(in.q, in.q);
    if r2 > 1.0 {
        discard;
    }
    let g = 1.0 - r2;
    let core = g * g;
    let col = in.col * core;
    return vec4<f32>(col, min(max(col.r, max(col.g, col.b)), 1.0) * 0.5);
}
