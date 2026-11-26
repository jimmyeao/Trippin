// @particles 90000 @bloom 0.5 @tonemap agx
// @title Particle Knot
// Particle Knot: 90,000 motes streaming along a torus knot, which unties and
// re-ties itself into a different knot every eight bars. The tube thickens
// with the slow mid presence and the knot swells with the bass (shape), the
// motes stream along it and it turns on the smooth energy clock (motion), and
// each morph turns the knot the other way (direction). Stateless.

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

fn knot(phi: f32, p: f32, q: f32) -> vec3<f32> {
    let r = 2.0 + cos(q * phi);
    return vec3<f32>(r * cos(p * phi), sin(q * phi), r * sin(p * phi)) * 0.42;
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    let cell = vec2<f32>(f32(ii % 1024u), f32(ii / 1024u));
    let h = hash22(cell + vec2<f32>(4.2, 7.7));
    let h2 = hash22(cell + vec2<f32>(8.3, 0.4));
    let s = fract(h.x + u.clock4.x * 0.01);
    let phi = s * TAU;
    // The knot sequence: (2,3) (3,4) (2,5) (3,5), morphing over two beats at
    // each eight-bar line.
    let k = floor(u.flow / 32.0);
    let m = smoothstep(0.0, 2.0, u.flow - k * 32.0);
    let ia = u32(k) % 4u;
    let ib = (ia + 3u) % 4u;
    let pqs = array<vec2<f32>, 4>(vec2<f32>(2.0, 3.0), vec2<f32>(3.0, 4.0), vec2<f32>(2.0, 5.0), vec2<f32>(3.0, 5.0));
    let a = pqs[ia];
    let b = pqs[ib];
    var w = mix(knot(phi, b.x, b.y), knot(phi, a.x, a.y), m);
    // The kick swells the knot and fattens its tube; the mids thicken it.
    let kick = kick_swell();
    w *= 1.0 + 0.12 * u.pres4.x + 0.1 * kick;
    // The tube: a random offset round the curve.
    let tube = 0.05 + 0.05 * u.pres4.y + 0.06 * u.lvl4.y + 0.12 * kick;
    let off = vec3<f32>(h2.x - 0.5, h2.y - 0.5, fract(h2.x * 13.7 + h2.y * 3.1) - 0.5);
    w += normalize(off + vec3<f32>(1e-4)) * tube * sqrt(fract(h.y * 5.3));
    // Turning: direction flips with each knot.
    let dir = select(1.0, -1.0, (u32(k) & 1u) == 1u);
    let wy = rot(u.clock4.x * 0.1 * dir + k * 0.7) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.5) * w.yz;
    w = vec3<f32>(w.x, wp.x, wp.y + 3.4);
    let c = palette(s * 2.0 % 1.0 * 0.6 + 0.1);
    let b2 = 0.13 * clamp(4.2 - w.z, 0.4, 1.6);
    return emit(w, vi, 0.0045, c * c * b2);
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
    col += vec3<f32>(0.005, 0.004, 0.012) * (1.0 - 0.4 * length(p));
    return vec4<f32>(finite(col), 1.0);
}
