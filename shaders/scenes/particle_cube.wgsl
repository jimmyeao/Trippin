// @sim 64000 @bloom 0.5 @tonemap agx
// @title Particle Cube
// Particle Cube: a wireframe lattice of 64,000 points that twists layer by layer,
// one way and then the other over eight bars (direction). A wave of the bass
// presence ripples out from the centre through the lattice (shape), and the
// cube turns on the smooth energy clock (motion). Every point is simulated
// (sim.rs, the shape-scene pattern): each kick blasts the lattice apart and
// it springs back, snares wrench it round and the bass pulls it taut; its
// lines stay straight (no swirl).

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

// The lattice point for particle i (x: position; w: the ripple, for colour).
// 48 lattice lines (4x4 along each axis), 1333 points each.
fn home(i: u32) -> vec4<f32> {
    let line = min(i / 1333u, 47u);
    let t = f32(i % 1333u) / 1332.0;
    let axis = line / 16u;
    let la = f32((line % 16u) / 4u) / 3.0;
    let lb = f32(line % 4u) / 3.0;
    var g = vec3<f32>(t, la, lb);
    if axis == 1u {
        g = vec3<f32>(la, t, lb);
    } else if axis == 2u {
        g = vec3<f32>(la, lb, t);
    }
    var w = (g - 0.5) * 1.9;
    // Twist each horizontal layer by its height; the mids wind it tighter.
    let twist = (1.0 + 0.8 * u.lvl4.y) * sin(TAU * u.flow / 32.0);
    let tw = rot(w.y * twist) * w.xz;
    w = vec3<f32>(tw.x, w.y, tw.y);
    // A ripple from the centre, its height the bass presence.
    let r = length(w);
    let wave = sin(r * 5.0 - u.clock4.x * 0.8);
    w *= 1.0 + (0.03 + 0.07 * u.pres4.x) * wave * smoothstep(0.0, 0.4, r);
    return vec4<f32>(w, wave);
}

fn sim_spawn(i: u32) -> Particle {
    return sim_shape_spawn();
}

fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    // Straight lines: no swirl (its fine grain ripples them even at a tenth).
    return sim_shape_step_k(i, p, home(i).xyz, vec3<f32>(0.0), dt, 0.95, 0.0);
}

// Turn on the energy clock, then the camera.
fn view(w_in: vec3<f32>) -> vec3<f32> {
    var w = w_in;
    let wy = rot(u.clock4.x * 0.08 + 0.6) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.45 + 0.1 * sin(TAU * u.flow / 64.0)) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 3.9);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let h = home(ii);
    let pp = sim_p[ii];
    let w = h.xyz + pp.pos.xyz;
    let a = view(w);
    let s = sim_quad(a, view(w - pp.vel.xyz * 0.04), vi, 0.006, 1.7);
    o.pos = s.pos;
    o.q = s.q;
    // Lines brighten where they're shaken loose.
    let loose = min(length(pp.pos.xyz) * 3.0, 1.0);
    let g = (h.y / 1.9 + 0.5);
    let c = palette(0.1 + 0.35 * g + 0.15 * h.w + 0.2 * loose);
    let b = (0.22 + 0.2 * max(h.w, 0.0) + 0.15 * loose) * clamp(5.0 - a.z, 0.4, 1.6);
    o.col = c * c * b * s.dim;
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
    var col = prev(in.uv) * 0.4;
    col += vec3<f32>(0.004, 0.004, 0.01) * (1.0 - 0.4 * length(p));
    return vec4<f32>(finite(col), 1.0);
}
