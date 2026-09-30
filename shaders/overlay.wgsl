// Stream overlays — the now-playing card, branding and the ticker, drawn
// CPU-side into straight-alpha sRGB images (src/overlay.rs) and composited
// over the finished frame in the present pass. Must match `OvLayerU` /
// `OvUniforms` in src/overlay.rs.

struct OL {
    quad: vec4<f32>,   // centre xy + half-size wh, centred coords (y down)
    uvx: vec4<f32>,    // x u-scale · y u-offset · z opacity · w 1 = ticker band
};

struct O {
    l: array<OL, 3>,
};

@group(1) @binding(0) var ov0: texture_2d<f32>;
@group(1) @binding(1) var ov1: texture_2d<f32>;
@group(1) @binding(2) var ov2: texture_2d<f32>;
@group(1) @binding(3) var<uniform> o: O;

fn tap(i: i32, uv: vec2<f32>, wrap: bool) -> vec4<f32> {
    var c: vec4<f32>;
    if i == 0 {
        c = textureSampleLevel(ov0, samp, uv, 0.0);
    } else if i == 1 {
        c = textureSampleLevel(ov1, samp, uv, 0.0);
    } else if wrap {
        c = textureSampleLevel(ov2, rsamp, uv, 0.0);
    } else {
        c = textureSampleLevel(ov2, samp, uv, 0.0);
    }
    return vec4<f32>(c.rgb * c.a, c.a);   // premultiply
}

fn over(front: vec4<f32>, back: vec4<f32>) -> vec4<f32> {
    return front + back * (1.0 - front.a);
}

fn layer(i: i32, p: vec2<f32>) -> vec4<f32> {
    let L = o.l[i];
    if L.uvx.z < 0.004 || L.quad.z <= 0.0 {
        return vec4<f32>(0.0);
    }
    let luv = (p - L.quad.xy) / L.quad.zw * 0.5 + 0.5;
    if any(luv < vec2<f32>(0.0)) || any(luv > vec2<f32>(1.0)) {
        return vec4<f32>(0.0);
    }
    var c: vec4<f32>;
    if L.uvx.w > 0.5 {
        // Ticker: a dark band with an accent hairline on top, the message
        // repeating across it.
        let txt = tap(i, vec2<f32>(luv.x * L.uvx.x + L.uvx.y, luv.y), true);
        let line = 1.0 - smoothstep(0.0, 0.05, luv.y);
        let band = vec4<f32>(0.01, 0.01, 0.02, 0.72);
        c = over(txt, over(vec4<f32>(palette(0.55) * line * 0.9, line * 0.9), band));
    } else {
        c = tap(i, luv, false);
    }
    return c * L.uvx.z * u.master;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var out = layer(2, p);
    out = over(layer(1, p), out);
    out = over(layer(0, p), out);
    return out;
}
