// @sim 90000 @bloom 0.5 @tonemap agx
// @title Particle Knot
// Particle Knot: 90,000 motes streaming along a torus knot, which unties and
// re-ties itself into a different knot every eight bars. The tube thickens
// with the slow mid presence and the knot swells with the bass (shape), the
// motes stream along it and it turns on the smooth energy clock (motion), and
// each morph turns the knot the other way (direction). Every mote is
// simulated (sim.rs, the shape-scene pattern): kicks blast the tube open and
// it springs back, snares wrench it round, the mids fray it, the bass pulls
// it tight, and in breakdowns it loosens into a drifting haze.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

fn knot(phi: f32, p: f32, q: f32) -> vec3<f32> {
    let r = 2.0 + cos(q * phi);
    return vec3<f32>(r * cos(p * phi), sin(q * phi), r * sin(p * phi)) * 0.42;
}

// Mote i's place in the tube (w: where along the knot, for colour).
fn home(i: u32) -> vec4<f32> {
    let cell = vec2<f32>(f32(i % 1024u), f32(i / 1024u));
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
    w *= 1.0 + 0.12 * u.pres4.x;
    // The tube: a random offset round the curve; the mids thicken it.
    let tube = 0.05 + 0.05 * u.pres4.y + 0.05 * u.lvl4.y;
    let off = vec3<f32>(h2.x - 0.5, h2.y - 0.5, fract(h2.x * 13.7 + h2.y * 3.1) - 0.5);
    w += normalize(off + vec3<f32>(1e-4)) * tube * sqrt(fract(h.y * 5.3));
    return vec4<f32>(w, s);
}

fn sim_spawn(i: u32) -> Particle {
    return sim_shape_spawn();
}

fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    return sim_shape_step(i, p, home(i).xyz, vec3<f32>(0.0), dt, 1.1);
}

// Turning (the direction flips with each knot), then the camera.
fn view(w_in: vec3<f32>) -> vec3<f32> {
    var w = w_in;
    let k = floor(u.flow / 32.0);
    let dir = select(1.0, -1.0, (u32(k) & 1u) == 1u);
    let wy = rot(u.clock4.x * 0.1 * dir + k * 0.7) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.5) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 3.4);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let h = home(ii);
    let pp = sim_p[ii];
    let w = h.xyz + pp.pos.xyz;
    let a = view(w);
    let sq = sim_quad(a, view(w - pp.vel.xyz * 0.04), vi, 0.0045, 1.7);
    o.pos = sq.pos;
    o.q = sq.q;
    let loose = min(length(pp.pos.xyz) * 3.0, 1.0);
    let c = palette(h.w * 2.0 % 1.0 * 0.6 + 0.1 + 0.15 * loose);
    o.col = c * c * (0.13 + 0.08 * loose) * clamp(4.2 - a.z, 0.4, 1.6) * sq.dim;
    return o;
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
