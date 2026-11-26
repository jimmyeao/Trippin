// @sim 90000 @bloom 0.5 @tonemap agx
// @title Particle Sphere
// Particle Sphere: 90,000 motes spread evenly over a sphere that morphs with
// the music. Each latitude bulges with its band of the spectrum (bass at the
// poles, highs at the equator) and slow lobes roll across the surface
// (shape); it turns on the smooth energy clock (motion), and its tilt swings
// over eight bars (direction). Every mote is simulated (sim.rs, the
// shape-scene pattern): kicks blast the shell outward and it springs back,
// snares wrench it round, the mids blur its surface into a swirl and the
// bass pulls it crisp; in breakdowns it hangs as a drifting haze.

const N: f32 = 90000.0;

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

// Mote i on the shell (w: its displacement, for colour).
fn home(i: u32) -> vec4<f32> {
    let fi = f32(i);
    // Fibonacci sphere: even coverage with no poles bunching.
    let y = 1.0 - 2.0 * (fi + 0.5) / N;
    let rr = sqrt(max(1.0 - y * y, 0.0));
    let phi = fi * 2.399963;
    let n = vec3<f32>(cos(phi) * rr, y, sin(phi) * rr);
    // Shape: the spectrum by latitude plus slow rolling lobes.
    let band = spec(1.0 - abs(y) * 0.95);
    let t = u.clock4.x * 0.12;
    let lobes = sin(n.x * 3.0 + t) * sin(n.y * 2.5 - t * 0.7) * sin(n.z * 3.0 + t * 0.5);
    let disp = 0.32 * sqrt(clamp(band, 0.0, 1.0)) + (0.12 + 0.12 * u.pres4.y) * lobes;
    return vec4<f32>(n * (1.0 + 0.08 * u.pres4.x + disp), disp);
}

fn sim_spawn(i: u32) -> Particle {
    return sim_shape_spawn();
}

fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    return sim_shape_step(i, p, home(i).xyz, vec3<f32>(0.0), dt, 1.0);
}

// Turn on the energy clock; the tilt swings over eight bars.
fn view(w_in: vec3<f32>) -> vec3<f32> {
    var w = w_in;
    let wy = rot(u.clock4.x * 0.12) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.35 * sin(TAU * u.flow / 32.0)) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 3.6);
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
    let disp = h.w;
    let loose = min(length(pp.pos.xyz) * 3.0, 1.0);
    let c = palette(0.15 + 0.9 * disp + 0.1 * (f32(ii) / N) + 0.15 * loose);
    // The far side dimmer, so it reads as a solid shell.
    let b = (0.14 + 0.2 * clamp(disp * 2.0, 0.0, 1.0) + 0.08 * loose) * clamp(4.4 - a.z, 0.3, 1.4);
    o.col = c * c * b * sq.dim;
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
    let core = exp(-length(p) * 3.0);
    col += palette(0.6) * palette(0.6) * core * 0.03;
    return vec4<f32>(finite(col), 1.0);
}
