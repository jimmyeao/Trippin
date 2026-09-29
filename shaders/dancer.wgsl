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
    style: f32,    // 0 shadow, 1 neon, 2 strobe
    count: f32,    // 1, or 3 for the canon
    scale: f32,    // main dancer height as a fraction of screen height
    trail: f32,    // 1 = ghost echoes of earlier frames trail her movement
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

// A dancer with her motion echoes layered behind her: earlier frames of the
// same routine sampled at a lag, hue-shifted and dimmer — cheap "video echo"
// trails that work under every look.
fn shaded_with_trails(
    p: vec2<f32>, slot: i32, x: f32, size: f32, flip: bool, tint: f32,
) -> vec4<f32> {
    var acc = shade(dancer_at(p, slot, x, size, flip), tint);
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
    if d.count > 1.5 {
        // Canon: a companion either side, smaller, each with its own routine.
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
            out = over(shaded_with_trails(p, i, sx, side, i == 1, f32(i) * 0.33), out);
        }
    }
    out = over(shaded_with_trails(p, 0, 0.0, 1.0, false, 0.0), out);
    return out;
}
