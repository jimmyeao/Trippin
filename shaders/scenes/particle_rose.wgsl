// @particles 80000 @bloom 0.55 @tonemap agx
// @title Particle Rose
// Particle Rose: a flower of 80,000 motes drawn as a polar rose, its petals
// cupped up into a bowl and seen from above at an angle. It grows a
// different number of petals every eight bars, morphing over two beats, and
// each petal reaches out with its band of the music (shape); the flower
// turns on the smooth energy clock and ripples rise through the bowl
// (motion), its spin reversing with each new flower (direction). Stateless.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

fn emit(w: vec3<f32>, vi: u32, size_w: f32, col: vec3<f32>) -> POut {
    var o: POut;
    o.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    if w.z < 0.3 {
        return o;
    }
    let fl = 1.7;
    let sp = w.xy * fl / w.z;
    let corners = array<vec2<f32>, 3>(vec2<f32>(-1.732, -1.0), vec2<f32>(1.732, -1.0), vec2<f32>(0.0, 2.0));
    let q = corners[vi % 3u];
    let size = max(size_w * fl / w.z, 2.0 / u.res_y);
    let p = sp + q * size;
    o.pos = vec4<f32>(p.x / aspect(), p.y, 0.0, 1.0);
    o.q = q;
    o.col = col;
    return o;
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    let cell = vec2<f32>(f32(ii % 1024u), f32(ii / 1024u));
    let h = hash22(cell + vec2<f32>(2.4, 5.6));
    let h2 = hash22(cell + vec2<f32>(7.2, 1.3));
    let th = h.x * TAU;
    let k = floor(u.flow / 32.0);
    let m = smoothstep(0.0, 2.0, u.flow - k * 32.0);
    let ks = array<f32, 4>(3.0, 4.0, 5.0, 7.0);
    let ka = ks[u32(k) % 4u];
    let kb = ks[(u32(k) + 3u) % 4u];
    let ra = abs(cos(ka * th * 0.5));
    let rb = abs(cos(kb * th * 0.5));
    let shape = mix(rb, ra, m);
    // Each petal reaches with a band (petals cycle bass, mid, mid-high, high).
    let petal = u32(floor(th * ka / TAU + 0.5)) % 4u;
    let reach = 1.0 + 0.4 * u.pres4[petal] + 0.15 * u.lvl4[petal];
    // Most motes on the petal edge, the rest filling it.
    var f = 0.92 + 0.08 * h.y;
    if (ii % 3u) == 0u {
        f = sqrt(h.y);
    }
    let rad = shape * f * reach * 0.95;
    var w = vec3<f32>(cos(th) * rad, 0.0, sin(th) * rad);
    // Cupped into a bowl, with ripples rising through it.
    w.y = 0.35 * rad * rad + 0.05 * sin(rad * 9.0 - u.clock4.x * 0.6) + (h2.x - 0.5) * 0.02;
    let dir = select(1.0, -1.0, (u32(k) & 1u) == 1u);
    let wy = rot(u.clock4.x * 0.07 * dir) * w.xz;
    w = vec3<f32>(wy.x, w.y - 0.15, wy.y);
    let wp = rot(-0.75) * w.yz;
    w = vec3<f32>(w.x, wp.x, wp.y + 3.2);
    let c = palette(0.75 + 0.25 * rad + 0.05 * f32(petal));
    let b = (0.12 + 0.08 * f) * clamp(4.2 - w.z, 0.4, 1.6);
    return emit(w, vi, 0.0045, c * c * b);
}

@fragment
fn fs_particles(in: POut) -> @location(0) vec4<f32> {
    let r2 = dot(in.q, in.q);
    if r2 > 1.0 {
        discard;
    }
    let g = 1.0 - r2;
    return vec4<f32>(in.col * g * g, 0.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = prev(in.uv) * 0.5;
    col += palette(0.9) * palette(0.9) * exp(-length(p) * 6.0) * 0.05;
    return vec4<f32>(finite(col), 1.0);
}
