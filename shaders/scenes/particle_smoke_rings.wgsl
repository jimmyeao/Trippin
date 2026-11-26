// @sim 90000 @bloom 0.5 @tonemap agx
// @title Smoke Rings
// Smoke Rings: glowing vortex rings of 90,000 motes rising up the frame,
// each rolling round its own core as it climbs and widens. The rings fatten
// with the slow bass presence (shape), they climb and roll on the smooth
// energy clock (motion), and their path leans one way, then the other, over
// eight bars (direction). Five rings in flight, always five, staggered.
// Every mote is simulated (sim.rs, the shape-scene pattern): kicks puff each
// ring out from its own centre and it pulls back in, snares wrench them
// round, the mids tear wisps off the cores and the bass rolls them tight;
// in breakdowns the rings smoulder into drifting smoke.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

struct Ring {
    centre: vec3<f32>,
    w: vec3<f32>,
    life: f32,
    b: f32,
};

// Mote i on its ring.
fn ring_of(i: u32) -> Ring {
    var o: Ring;
    let cell = vec2<f32>(f32(i % 1024u), f32(i / 1024u));
    let h = hash22(cell + vec2<f32>(3.3, 6.6));
    let h2 = hash22(cell + vec2<f32>(5.5, 2.1));
    let ring = i % 5u;
    let fr = f32(ring);
    let ph = u.clock4.x * 0.035 + fr / 5.0;
    let life = fract(ph);
    let gen = floor(ph);
    let rh = hash22(vec2<f32>(fr, gen * 3.0 + 7.0));
    // The ring's centre climbs; its radius grows as it goes.
    let lean = 0.4 * sin(TAU * u.flow / 32.0);
    let cy = -1.4 + life * 2.9;
    let cx = (rh.x - 0.5) * 1.2 + lean * life;
    let big = 0.25 + 0.5 * life;
    let core = (0.06 + 0.04 * u.pres4.x + 0.05 * u.lvl4.y) * (0.6 + 0.6 * life);
    // A point on the torus: round the ring (a) and round its core (b), the
    // core angle rolling as it climbs.
    let a = h.x * TAU;
    let b = h.y * TAU + u.clock4.x * 0.6;
    let cr = core * sqrt(h2.x);
    let rr = big + cr * cos(b);
    var w = vec3<f32>(cos(a) * rr, cr * sin(b), sin(a) * rr);
    // Tilt the ring a little toward the camera.
    let wt = rot(0.35 + (rh.y - 0.5) * 0.4) * w.yz;
    o.w = vec3<f32>(w.x + cx, wt.x + cy, wt.y);
    o.centre = vec3<f32>(cx, cy, 0.0);
    o.life = life;
    o.b = b;
    return o;
}

fn sim_spawn(i: u32) -> Particle {
    return sim_shape_spawn();
}

fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    let r = ring_of(i);
    // A ring just born starts clean.
    if r.life < 0.01 {
        return sim_shape_spawn();
    }
    return sim_shape_step(i, p, r.w, r.centre, dt, 0.25 + 0.5 * r.life);
}

fn view(w: vec3<f32>) -> vec3<f32> {
    let wp = rot(0.08) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 4.0);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let r = ring_of(ii);
    let pp = sim_p[ii];
    let w = r.w + pp.pos.xyz;
    let a = view(w);
    let sq = sim_quad(a, view(w - pp.vel.xyz * 0.04), vi, 0.005, 1.7);
    o.pos = sq.pos;
    o.q = sq.q;
    let fr = f32(ii % 5u);
    let fade = smoothstep(0.0, 0.12, r.life) * smoothstep(1.0, 0.7, r.life);
    let c = palette(0.5 + 0.12 * fr + 0.15 * cos(r.b));
    let bri = 0.13 * fade * (0.7 + 0.5 * sin(r.b) * sin(r.b)) * clamp(5.0 - a.z, 0.4, 1.6);
    o.col = c * c * bri * sq.dim;
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
    var col = prev(in.uv) * 0.6;
    col += vec3<f32>(0.005, 0.004, 0.012) * (1.0 - 0.4 * length(p));
    return vec4<f32>(finite(col), 1.0);
}
