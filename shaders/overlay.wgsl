// Stream overlays — the now-playing card, branding and the ticker, drawn
// CPU-side into straight-alpha sRGB images (src/overlay.rs) and composited
// over the finished frame in the present pass. Must match `OvLayerU` /
// `OvUniforms` in src/overlay.rs.

struct OL {
    quad: vec4<f32>,   // centre xy + half-size wh, centred coords (y down)
    uvx: vec4<f32>,    // x u-scale · y u-offset · z opacity · w 1 = ticker band
};

struct O {
    l: array<OL, 4>,
    // x: the dancer's Disintegrate envelope (0 whole, 1 scattered), y: its
    // seed; the logo bursts with her (dancer::Burst). zw spare.
    burst: vec4<f32>,
};

@group(1) @binding(0) var ov0: texture_2d<f32>;
@group(1) @binding(1) var ov1: texture_2d<f32>;
@group(1) @binding(2) var ov2: texture_2d<f32>;
@group(1) @binding(3) var<uniform> o: O;
@group(1) @binding(4) var ov3: texture_2d<f32>;

fn tap(i: i32, uv: vec2<f32>, wrap: bool) -> vec4<f32> {
    var c: vec4<f32>;
    if i == 0 {
        c = textureSampleLevel(ov0, samp, uv, 0.0);
    } else if i == 1 {
        c = textureSampleLevel(ov1, samp, uv, 0.0);
    } else if i == 3 {
        c = textureSampleLevel(ov3, samp, uv, 0.0);
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
    if i == 1 {
        // The logo hides cell by cell as its dust leaves.
        c *= logo_keep(luv);
    }
    return c * L.uvx.z * u.master;
}

// ---- Logo Disintegrate ------------------------------------------------------
// The branding logo bursts into dust with the dancer (same envelope and seed,
// o.burst): one particle per cell of an LG x LG grid over the logo image,
// coloured from the logo's own pixels, flung out on a curl-noise swirl that
// drifts on the flow clock, then drawn back. Top first, like her head.
// Stateless, like the dancer's (dancer.wgsl vs_particles).

const LG: f32 = 128.0;

fn logo_local(cell: vec2<f32>) -> f32 {
    let y = clamp(cell.y / LG, 0.0, 1.0);
    let order = y * 0.6 + hash21(cell + vec2<f32>(o.burst.y * 977.0, 9.0)) * 0.4;
    let start = order * 0.55;
    return smoothstep(start, start + 0.45, o.burst.x);
}

fn logo_keep(luv: vec2<f32>) -> f32 {
    if o.burst.x <= 0.0 {
        return 1.0;
    }
    return 1.0 - smoothstep(0.0, 0.12, logo_local(floor(luv * LG)));
}

fn logo_field(q: vec2<f32>) -> f32 {
    return noise(q) + 0.5 * noise(q * 2.3 + vec2<f32>(5.2, 1.3));
}

struct LOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

@vertex
fn vs_logo(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> LOut {
    var out: LOut;
    out.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    out.q = vec2<f32>(0.0);
    out.col = vec3<f32>(0.0);
    let L = o.l[1];
    if L.uvx.z < 0.004 || L.quad.z <= 0.0 || o.burst.x <= 0.0 {
        return out;
    }
    let cell = vec2<f32>(f32(ii % u32(LG)), f32(ii / u32(LG)));
    let e = logo_local(cell);
    if e < 0.002 {
        return out;
    }
    let j = hash22(cell + vec2<f32>(17.0, 41.0));
    let luv = (cell + 0.15 + 0.7 * j) / LG;
    let c = textureSampleLevel(ov1, samp, luv, 0.0);
    if c.a < 0.3 {
        return out;
    }
    let home = L.quad.xy + (luv * 2.0 - 1.0) * L.quad.zw;
    let h = L.quad.w * 2.0;
    let rel = home - L.quad.xy;
    let r = hash21(cell + vec2<f32>(3.1, 7.7));
    let t = u.flow * 0.09 + o.burst.y * 40.0;
    let qn = home / h * 1.8 + vec2<f32>(t, -0.7 * t);
    let ep = 0.04;
    let dx = logo_field(qn + vec2<f32>(ep, 0.0)) - logo_field(qn - vec2<f32>(ep, 0.0));
    let dy = logo_field(qn + vec2<f32>(0.0, ep)) - logo_field(qn - vec2<f32>(0.0, ep));
    let curl = vec2<f32>(dy, -dx) / (2.0 * ep);
    // The logo is small, so its dust spreads further for its size than hers.
    let drift = rel * (0.3 + 0.8 * r)
              + curl * 0.12 * h
              + vec2<f32>(0.0, -(0.15 + 0.4 * r) * h);
    let dir = select(1.0, -1.0, o.burst.y < 0.5);
    let swing = 0.12 * sin(TAU * u.flow / 16.0 + o.burst.y * TAU);
    let turn = e * (dir * (0.2 + 0.3 * r) + swing);
    let pos = L.quad.xy + rot(turn) * (rel + drift * e);
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let q = corners[vi % 6u];
    let size = max(0.0026 + 0.003 * e * r, 2.5 / u.res_y);
    let pp = pos + q * size;
    out.pos = vec4<f32>(pp.x / aspect(), -pp.y, 0.0, 1.0);
    out.q = q;
    let a = smoothstep(0.3, 0.7, c.a) * smoothstep(0.0, 0.15, e) * L.uvx.z * u.master;
    // The logo's own colour, lifted so dark logos still glow.
    out.col = (c.rgb * 1.2 + vec3<f32>(0.08)) * a;
    return out;
}

@fragment
fn fs_logo(in: LOut) -> @location(0) vec4<f32> {
    let r2 = dot(in.q, in.q);
    if r2 > 1.0 {
        discard;
    }
    let g = 1.0 - r2;
    let col = in.col * g * g;
    return vec4<f32>(col, min(max(col.r, max(col.g, col.b)), 1.0) * 0.5);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var out = layer(3, p);
    out = over(layer(2, p), out);
    out = over(layer(1, p), out);
    out = over(layer(0, p), out);
    return out;
}
