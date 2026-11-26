// @sim 72000 @bloom 0.55 @tonemap agx
// @title Particle Dandelion
// Particle Dandelion: a glowing dandelion clock of 72,000 motes, 600 seeds on
// fine stalks, simulated on the GPU (sim.rs). Seeds break loose, ride the
// wind away and regrow. The head swells with the slow bass presence (shape)
// and turns on the smooth energy clock (motion); the wind swings from one
// side to the other over eight bars (direction). The loudness sets how hard
// it blows and the mids how gusty; every kick shakes the whole clock and
// sends a gust through the loose seeds (momentum); the highs open the tufts.

const SEEDS: u32 = 600u;

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

struct Mote {
    w: vec3<f32>,
    dir: vec3<f32>,
    loose: f32,
    life: f32,
    tuft: bool,
    hs: vec2<f32>,
    t: f32,
};

// Mote i attached to the head (where it is while its seed holds on).
fn mote(i: u32) -> Mote {
    var o: Mote;
    let seed = i % SEEDS;
    let k = i / SEEDS; // 0..119: 0-69 the stalk, 70-119 the tuft
    let fs = f32(seed);
    // Seed directions: a Fibonacci sphere.
    let y = 1.0 - 2.0 * (fs + 0.5) / f32(SEEDS);
    let rr = sqrt(max(1.0 - y * y, 0.0));
    let phi = fs * 2.399963;
    let dir = vec3<f32>(cos(phi) * rr, y, sin(phi) * rr);
    let hs = hash22(vec2<f32>(fs, 3.0));
    let h = hash22(vec2<f32>(f32(i % 1024u), f32(i / 1024u)) + vec2<f32>(1.3, 8.8));
    let head = 0.75 * (1.0 + 0.08 * u.pres4.x);
    // Seed life: grows in, stays, then breaks loose and drifts away.
    let life = fract(u.clock4.x * 0.02 + hs.x);
    let grow = smoothstep(0.0, 0.1, life);
    o.dir = dir;
    o.hs = hs;
    o.life = life;
    o.loose = smoothstep(0.72, 0.76, life);
    o.tuft = k >= 70u;
    if !o.tuft {
        // The stalk, from near the centre to the tip; it stays when the seed
        // leaves.
        let t = f32(k) / 70.0;
        o.t = t;
        o.w = dir * mix(0.12, head, t) * (0.9 + 0.1 * grow) + (vec3<f32>(h.x, h.y, fract(h.x * 9.1)) - 0.5) * 0.004;
        o.loose = 0.0;
    } else {
        // The tuft: an umbrella of fine rays at the tip, opening with the highs.
        let a = h.x * TAU;
        let side = normalize(cross(dir, vec3<f32>(0.0, 1.0, 0.3)));
        let up = cross(dir, side);
        let open = 1.0 + 0.9 * u.lvl4.w;
        let ray = (cos(a) * side + sin(a) * up) * h.y * 0.09 * open + dir * h.y * 0.05;
        o.w = dir * head * grow + ray * grow;
    }
    return o;
}

fn sim_spawn(i: u32) -> Particle {
    return sim_shape_spawn();
}

// Attached motes keep their offset from the head (the shape pattern); a
// loose seed's motes are free (vel.w = 1) and pos is where they are.
fn sim_step(i: u32, p_in: Particle, dt: f32) -> Particle {
    var p = p_in;
    let m = mote(i);
    let free = p.vel.w > 0.5;
    if m.loose < 0.5 {
        if free {
            // Its seed has regrown: back on the head.
            return sim_shape_spawn();
        }
        return sim_shape_step(i, p, m.w, vec3<f32>(0.0), dt, 0.7);
    }
    if !free {
        // Breaking loose: from offset to free, keeping its speed.
        p.pos = vec4<f32>(m.w + p.pos.xyz, p.pos.w);
        p.vel = vec4<f32>(p.vel.xyz + m.dir * 0.3, 1.0);
    }
    let x = p.pos.xyz;
    // The wind: its side swings over eight bars, the loudness sets how hard
    // it blows and the mids how gusty it is; the seeds float up a little.
    let side = sin(TAU * u.flow / 32.0);
    let blow = (0.35 + 1.1 * sim_loud()) * (1.0 - 0.4 * u.calm);
    let wind = vec3<f32>(side * blow, 0.12 + 0.08 * sin(m.hs.y * 6.0), (m.hs.y - 0.5) * 0.4);
    var tv = wind + sim_curl(x * 0.6 + vec3<f32>(u.clock4.z * 0.02, 0.0, 0.0)) * (0.1 + 1.2 * u.lvl4.y);
    var v = p.vel.xyz + (tv - p.vel.xyz) * 1.5 * dt;
    // Kick: a gust through the loose seeds.
    v += vec3<f32>(side, 0.3, 0.0) * simc.onset.x * 0.9 * (1.0 - 0.8 * u.calm);
    p.vel = vec4<f32>(v, 1.0);
    p.pos = vec4<f32>(x + v * dt, p.pos.w + dt);
    return p;
}

// The head turns slowly on the energy clock; then the camera.
fn view(w_in: vec3<f32>) -> vec3<f32> {
    var w = w_in;
    let wy = rot(u.clock4.x * 0.05) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.15) * w.yz;
    return vec3<f32>(w.x, wp.x + 0.1, wp.y + 3.3);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let m = mote(ii);
    let pp = sim_p[ii];
    var w = m.w + pp.pos.xyz;
    if pp.vel.w > 0.5 {
        w = pp.pos.xyz;
    }
    let a = view(w);
    let sq = sim_quad(a, view(w - pp.vel.xyz * 0.04), vi, 0.004, 1.7);
    o.pos = sq.pos;
    o.q = sq.q;
    var bri = 0.05;
    var c = palette(0.3 + 0.1 * m.t);
    if m.tuft {
        bri = 0.12 * (1.0 - smoothstep(0.85, 1.0, m.life));
        c = palette(0.55 + 0.25 * m.hs.y);
    }
    o.col = c * c * bri * clamp(4.2 - a.z, 0.4, 1.6) * sq.dim;
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
    // The stem, down from the head, and a soft glow behind the head.
    let stem = exp(-abs(p.x + 0.02 * p.y) * 300.0) * step(0.1, p.y) * 0.12;
    col += palette(0.3) * palette(0.3) * stem;
    col += palette(0.6) * palette(0.6) * exp(-length(p + vec2<f32>(0.0, 0.05)) * 3.5) * 0.03;
    return vec4<f32>(finite(col), 1.0);
}
