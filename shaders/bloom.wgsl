// Bloom chain (see src/gfx.rs). Standalone — not prepended with common.wgsl,
// and not hot-reloaded (embedded at build time).
// Downsample: 13-tap filter (Jimenez, "Next Generation Post Processing in
// Call of Duty: Advanced Warfare"). The first pass uses a Karis average so a
// single sparkling raymarch pixel can't bloom into a flickering blob.
// Upsample: 3×3 tent, additively blended into the next level up.

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VsOut;
    o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

fn tap(uv: vec2<f32>) -> vec3<f32> {
    let c = textureSampleLevel(src, samp, uv, 0.0).rgb;
    // NaN/Inf guard, and a ceiling so a single hot pixel stays bounded.
    let bad = (c != c) | (abs(c) > vec3<f32>(1e4));
    return min(select(max(c, vec3<f32>(0.0)), vec3<f32>(0.0), bad), vec3<f32>(64.0));
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn karis(a: vec3<f32>, b: vec3<f32>, c: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    let wa = 1.0 / (1.0 + luma(a));
    let wb = 1.0 / (1.0 + luma(b));
    let wc = 1.0 / (1.0 + luma(c));
    let wd = 1.0 / (1.0 + luma(d));
    return (a * wa + b * wb + c * wc + d * wd) / (wa + wb + wc + wd);
}

fn down13(uv: vec2<f32>, first: bool) -> vec3<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(src));
    let a = tap(uv + t * vec2<f32>(-2.0, -2.0));
    let b = tap(uv + t * vec2<f32>(0.0, -2.0));
    let c = tap(uv + t * vec2<f32>(2.0, -2.0));
    let d = tap(uv + t * vec2<f32>(-2.0, 0.0));
    let e = tap(uv);
    let f = tap(uv + t * vec2<f32>(2.0, 0.0));
    let g = tap(uv + t * vec2<f32>(-2.0, 2.0));
    let h = tap(uv + t * vec2<f32>(0.0, 2.0));
    let i = tap(uv + t * vec2<f32>(2.0, 2.0));
    let j = tap(uv + t * vec2<f32>(-1.0, -1.0));
    let k = tap(uv + t * vec2<f32>(1.0, -1.0));
    let l = tap(uv + t * vec2<f32>(-1.0, 1.0));
    let m = tap(uv + t * vec2<f32>(1.0, 1.0));
    if first {
        // Karis average per 2×2 block group.
        let g0 = karis(j, k, l, m);
        let g1 = karis(a, b, d, e);
        let g2 = karis(b, c, e, f);
        let g3 = karis(d, e, g, h);
        let g4 = karis(e, f, h, i);
        return g0 * 0.5 + (g1 + g2 + g3 + g4) * 0.125;
    }
    return e * 0.125 + (a + c + g + i) * 0.03125 + (b + d + f + h) * 0.0625 + (j + k + l + m) * 0.125;
}

@fragment
fn fs_down_first(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(down13(in.uv, true), 1.0);
}

@fragment
fn fs_down(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(down13(in.uv, false), 1.0);
}

@fragment
fn fs_up(in: VsOut) -> @location(0) vec4<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(src));
    var c = tap(uv_off(in.uv, t, 0.0, 0.0)) * 4.0;
    c += (tap(uv_off(in.uv, t, -1.0, 0.0)) + tap(uv_off(in.uv, t, 1.0, 0.0))
        + tap(uv_off(in.uv, t, 0.0, -1.0)) + tap(uv_off(in.uv, t, 0.0, 1.0))) * 2.0;
    c += tap(uv_off(in.uv, t, -1.0, -1.0)) + tap(uv_off(in.uv, t, 1.0, -1.0))
        + tap(uv_off(in.uv, t, -1.0, 1.0)) + tap(uv_off(in.uv, t, 1.0, 1.0));
    return vec4<f32>(c / 16.0, 1.0);
}

fn uv_off(uv: vec2<f32>, t: vec2<f32>, x: f32, y: f32) -> vec2<f32> {
    return uv + t * vec2<f32>(x, y);
}
