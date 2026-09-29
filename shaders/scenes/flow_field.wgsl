// @bloom 0.7 @tonemap agx
// Synesthesia-style abstract: a particle flow field. Sparks are seeded each
// frame and the whole image is advected along a curl-noise field, so every
// spark drags a glowing trail through the flow — thousands of luminous
// strands swirling around eddies. Pure feedback: 2D, cheap (not @heavy).
// Audio vocabulary:
//  - flow speed follows the whole-mix energy level (smoothly: strands crawl
//    in breakdowns, race on drops); the field itself evolves on the mid clock;
//  - spark density per band: bass seeds big slow embers near the centre,
//    highs seed fine fast glitter everywhere; bass hits burst a ring of
//    sparks outwards;
//  - colour by band; trails lengthen with presence.

// Divergence-free (curl) flow from the baked noise volume.
fn flow(p: vec2<f32>, t: f32) -> vec2<f32> {
    let e = 0.02;
    let s = 0.18;
    let a = tnoise(vec3<f32>((p + vec2<f32>(0.0, e)) * s, t)).b;
    let b = tnoise(vec3<f32>((p - vec2<f32>(0.0, e)) * s, t)).b;
    let c = tnoise(vec3<f32>((p + vec2<f32>(e, 0.0)) * s, t)).b;
    let d = tnoise(vec3<f32>((p - vec2<f32>(e, 0.0)) * s, t)).b;
    // Plus a gentle swirl around the centre so the image has a heart.
    let swirl = vec2<f32>(-p.y, p.x) * 0.15 / (1.0 + dot(p, p));
    return vec2<f32>(a - b, -(c - d)) / (2.0 * e) * 2.5 + swirl;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let t = u.clock4.z * 0.004 + u.seed * 0.01;
    let energy = clamp(u.lvl4.x * 0.45 + u.lvl4.y * 0.3 + u.lvl4.z * 0.15 + u.lvl4.w * 0.1, 0.0, 1.0);
    let speed = (0.006 + 0.02 * energy) * (u.bpm / 120.0);
    // Advect: fetch where this pixel's content came from.
    let v = flow(p, t);
    let src = p - v * speed;
    var col = prev(uncentred(src));
    let persist = 0.972 + 0.02 * max(u.pres4.x, u.pres4.z);
    col *= persist;

    // Seed sparks: per-pixel lottery, re-rolled every frame.
    let cell = floor(in.pos.xy);
    let h = hash22(cell + vec2<f32>(u.frame * 17.0, u.frame * 3.0));
    let r = length(p);
    let hue = u.clock4.w * 0.008 + u.hue;
    // Bass embers near the centre.
    if h.x < 0.0003 * u.lvl4.x * smoothstep(1.0, 0.1, r) {
        col += palette(0.05 + hue) * 6.0;
    }
    // Mid sparks mid-field.
    if h.y < 0.00018 * u.lvl4.y {
        col += palette(0.35 + hue) * 5.0;
    }
    // High glitter everywhere.
    if fract(h.x * 7.13 + h.y) < 0.00015 * u.lvl4.w {
        col += palette(0.7 + hue) * 4.0;
    }
    // Bass hit: a burst ring of sparks.
    let ring = abs(r - (0.15 + (1.0 - u.hits4.x) * 0.6));
    if u.hits4.x > 0.2 && ring < 0.008 && h.y < 0.04 {
        col += palette(0.1 + hue) * 5.0 * u.hits4.x;
    }
    return vec4<f32>(min(col, vec3<f32>(6.0)), 1.0);
}
