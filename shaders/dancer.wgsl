// Silhouette dancer, alpha-blended (premultiplied) over the scene. Written into
// the feedback target, so scenes with trails leave echoes of the dancer.
// Must match `DancerUniforms` in src/dancer.rs.

struct D {
    frame: f32,    // fractional frame index into the loop
    frames: f32,
    opacity: f32,
    style: f32,    // 0 shadow, 1 neon, 2 fill, 3 strobe
    count: f32,    // 1, or 3 for the mirrored canon
    aspect: f32,   // mask width / height
    scale: f32,    // dancer height as a fraction of screen height
    lag: f32,      // frames the side dancers trail by
};

@group(1) @binding(0) var masks: texture_2d_array<f32>;
@group(1) @binding(1) var<uniform> d: D;

fn wrap_frame(f: f32) -> f32 {
    return ((f % d.frames) + d.frames) % d.frames;
}

// Mask coverage at mask-space uv, blending neighbouring frames.
fn mask_at(luv: vec2<f32>, f: f32) -> f32 {
    let inside = all(luv >= vec2<f32>(0.0)) && all(luv <= vec2<f32>(1.0));
    let fa = floor(wrap_frame(f));
    let fb = wrap_frame(fa + 1.0);
    let a = textureSampleLevel(masks, samp, luv, i32(fa), 0.0).r;
    let b = textureSampleLevel(masks, samp, luv, i32(fb), 0.0).r;
    return select(0.0, mix(a, b, fract(f)), inside);
}

struct Hit {
    m: f32,
    edge: f32,
    luv: vec2<f32>,
};

fn dancer_at(p: vec2<f32>, x: f32, size: f32, flip: bool, f: f32) -> Hit {
    let h = 2.0 * d.scale * size;
    let w = h * d.aspect;
    let bottom = 0.98;
    var luv = vec2<f32>((p.x - x) / w + 0.5, (p.y - (bottom - h)) / h);
    if flip {
        luv.x = 1.0 - luv.x;
    }
    let texel = 2.0 / vec2<f32>(textureDimensions(masks).xy);
    let gx = mask_at(luv + vec2<f32>(texel.x, 0.0), f) - mask_at(luv - vec2<f32>(texel.x, 0.0), f);
    let gy = mask_at(luv + vec2<f32>(0.0, texel.y), f) - mask_at(luv - vec2<f32>(0.0, texel.y), f);
    var hit: Hit;
    hit.m = mask_at(luv, f);
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
        // Posterised colour fill, bands scrolling with the beat.
        let band = floor(fract(hit.luv.y * 3.0 - u.beat * 0.5) * 4.0) / 4.0;
        let col = palette(band * 0.5 + 0.3 + tint) * (0.8 + 1.2 * beat_pulse(6.0));
        return vec4<f32>(col * m, m);
    }
    if style == 3 {
        // Strobe: a white flash of the body on each beat, outline in between.
        let flash = beat_pulse(10.0);
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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var out = vec4<f32>(0.0);
    if d.count > 1.5 {
        // Canon: a mirrored pair either side, smaller and a half-beat behind.
        let w = 2.0 * d.scale * d.aspect;
        let side = d.frame - d.lag;
        // Keep wide clips (leaps, arabesques) inside the screen edges.
        let x = min(w * 0.95, aspect() - w * 0.8 * 0.5);
        out = over(shade(dancer_at(p, -x, 0.8, true, side), 0.33), out);
        out = over(shade(dancer_at(p, x, 0.8, false, side), 0.66), out);
    }
    out = over(shade(dancer_at(p, 0.0, 1.0, false, d.frame), 0.0), out);
    return out;
}
