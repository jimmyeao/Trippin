// Text overlays — rasterised glyph masks composited over the finished
// frame in the present pass, so post FX (mirror/flip/kaleido) never garble
// the lettering. One mask texture per text slot (the editor's two text
// lanes); colour and motion are procedural per style. Must match
// `TextSlotU`/`TextUniforms` in src/text.rs.

struct TS {
    quad: vec4<f32>,   // centre xy + half-size wh, centred coords
    aspect: f32,       // mask width / height; <0.01 = empty slot
    style: f32,        // 0 neon · 1 fire · 2 wave · 3 glitch · 4 pulse · 5 chrome
    opacity: f32,      // fade envelope from the render loop
    born: f32,         // u.time when the block started
    life: f32,         // block length in seconds (0 = untimed)
    hue: f32,          // colour seed
    anim: f32,         // 0 fade · 1 rise · 2 drop · 3 slide · 4 zoom · 5 type
    _p1: f32,
};

struct T {
    slots: array<TS, 2>,
};

@group(1) @binding(0) var mask0: texture_2d<f32>;
@group(1) @binding(1) var mask1: texture_2d<f32>;
@group(1) @binding(2) var<uniform> t: T;

fn mask_at(slot: i32, luv: vec2<f32>) -> f32 {
    if slot == 1 {
        return textureSampleLevel(mask1, samp, luv, 0.0).r;
    }
    return textureSampleLevel(mask0, samp, luv, 0.0).r;
}

fn mask_dim(slot: i32) -> vec2<f32> {
    if slot == 1 {
        return vec2<f32>(textureDimensions(mask1));
    }
    return vec2<f32>(textureDimensions(mask0));
}

// Text colours sit on top of a darkened outline so they read on bright
// scenes too.
fn shade(slot: i32, p: vec2<f32>) -> vec4<f32> {
    let s = t.slots[slot];
    if s.opacity < 0.005 || s.aspect < 0.01 {
        return vec4<f32>(0.0);
    }
    var luv = (p - s.quad.xy) / s.quad.zw * 0.5 + 0.5;
    let lt = u.time - s.born;
    let style = i32(s.style + 0.5);
    let anim = i32(s.anim + 0.5);

    // Entrance animation (video-editor style): warps the sampled mask during
    // the first ~0.7 s. luv is mask-space, so +y shifts the glyph UP-screen.
    if anim >= 1 && anim <= 4 {
        let e = 1.0 - pow(1.0 - clamp(lt / 0.7, 0.0, 1.0), 3.0);
        if anim == 1 { // rise: slides up into place
            luv.y -= (1.0 - e) * 0.55;
        } else if anim == 2 { // drop: falls from above
            luv.y += (1.0 - e) * 0.55;
        } else if anim == 3 { // slide: in from the left
            luv.x += (1.0 - e) * 0.7;
        } else { // zoom: pops from small to full
            let sc = 0.3 + 0.7 * e;
            luv = (luv - 0.5) / sc + 0.5;
        }
    }

    // uv warps first — they move the whole glyph, not the shading.
    if style == 2 {
        // Wave: the line rides a sine that also bobs on the beat.
        luv.y += sin(luv.x * 7.0 + lt * 2.6) * 0.045
               + sin(luv.x * 3.0 - lt * 1.1) * 0.02 * (0.5 + u.kick);
    }
    if style == 3 {
        // Glitch: horizontal slices jump sideways in bursts.
        let row = floor(luv.y * 22.0);
        let tick = floor(lt * 13.0);
        let gate = step(0.72, hash21(vec2(row * 3.1, tick * 0.37)));
        luv.x += (hash21(vec2(row, tick)) - 0.5) * 0.14 * gate;
    }
    if style == 4 {
        // Pulse: the whole quad breathes with the beat.
        let sc = 1.0 + 0.14 * beat_pulse(7.0);
        luv = (luv - 0.5) / sc + 0.5;
    }

    let inside = all(luv >= vec2<f32>(0.0)) && all(luv <= vec2<f32>(1.0));
    let m = select(0.0, mask_at(slot, luv), inside);
    // Gradient of the mask → rim/edge mask (same trick as the dancers).
    let texel = 2.0 / mask_dim(slot);
    let gx = mask_at(slot, luv + vec2<f32>(texel.x, 0.0))
           - mask_at(slot, luv - vec2<f32>(texel.x, 0.0));
    let gy = mask_at(slot, luv + vec2<f32>(0.0, texel.y))
           - mask_at(slot, luv - vec2<f32>(0.0, texel.y));
    let edge = select(0.0, clamp(length(vec2<f32>(gx, gy)) * 1.8, 0.0, 1.0), inside);

    var col = vec3<f32>(0.0);
    var a = m;
    if style == 0 {
        // Neon: hot core, saturated halo, faint mains flicker.
        let flick = 0.9 + 0.1 * hash21(vec2(floor(lt * 24.0), s.hue));
        let tint = palette(0.6 + s.hue + lt * 0.05);
        col = vec3<f32>(1.4) * m + tint * edge * (1.4 + 2.2 * u.kick);
        col *= flick;
        a = max(m, edge * 0.55);
    } else if style == 1 {
        // Fire: flame gradient scrolls up inside the letters.
        let n = noise(vec2(luv.x * 6.0, luv.y * 3.5 - lt * 2.4));
        let h = clamp(luv.y + n * 0.5, 0.0, 1.0);
        col = mix(vec3<f32>(1.6, 0.9, 0.1), vec3<f32>(1.2, 0.15, 0.02), h) * m;
        col += vec3<f32>(1.4, 0.5, 0.05) * edge * 0.8;
        a = max(m, edge * 0.4);
    } else if style == 2 {
        // Wave: palette scrolls left-to-right through the letters.
        col = palette(luv.x * 0.6 + lt * 0.22 + s.hue) * m * 1.5;
        col += palette(lt * 0.22 + s.hue + 0.4) * edge * 1.2;
        a = max(m, edge * 0.45);
    } else if style == 3 {
        // Glitch: RGB split + burst inversion.
        let off = 0.012 + 0.01 * u.kick;
        let mr = select(0.0, mask_at(slot, luv + vec2<f32>(off, 0.0)), inside);
        let mb = select(0.0, mask_at(slot, luv - vec2<f32>(off, 0.0)), inside);
        col = vec3<f32>(mr * 1.5, m * 0.9, mb * 1.5);
        let invert = step(0.93, hash21(vec2(floor(lt * 9.0), 7.0 + s.hue)));
        col = mix(col, vec3<f32>(1.0) - col, invert);
        a = max(m, max(mr, mb));
    } else if style == 4 {
        // Pulse: white-hot core that slams on each beat.
        let pump = beat_pulse(7.0);
        col = mix(palette(s.hue + luv.y * 0.25), vec3<f32>(2.2), pump * 0.8) * m;
        col += palette(s.hue + 0.5) * edge * (0.8 + 2.0 * u.kick);
        a = max(m, edge * 0.5);
    } else {
        // Chrome: brushed metal bands with a beat-brightened sheen.
        let band = sin(luv.y * 16.0 + lt * 0.7) * 0.5 + 0.5;
        let sheen = smoothstep(0.2, 0.8, fract(luv.x - lt * 0.11));
        let metal = mix(vec3<f32>(0.25), vec3<f32>(0.95, 1.0, 1.1), band * sheen);
        col = metal * m * (0.8 + 0.6 * u.energy);
        col += vec3<f32>(0.9, 0.95, 1.0) * edge * 0.9;
        a = max(m, edge * 0.5);
    }

    // Typewriter: left-to-right reveal with a blinking caret. Runs on the
    // final colour/alpha so it composes with every style above.
    if anim == 5 {
        let rev = clamp(lt / 1.2, 0.0, 1.0);
        let vis = 1.0 - smoothstep(rev - 0.02, rev + 0.02, luv.x);
        let caret = (1.0 - smoothstep(0.0, 0.012, abs(luv.x - rev)))
                  * step(rev, 0.995)
                  * step(0.15, luv.y) * step(luv.y, 0.85)
                  * step(0.5, fract(lt * 1.6));
        col = col * vis + palette(s.hue + 0.5) * caret * 1.6;
        a = max(a * vis, caret * 0.9);
    }

    a *= s.opacity * u.master;
    return vec4<f32>(col * s.opacity * u.master, a);
}

fn over(front: vec4<f32>, back: vec4<f32>) -> vec4<f32> {
    return front + back * (1.0 - front.a);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var out = shade(1, p);
    out = over(shade(0, p), out);
    return out;
}
