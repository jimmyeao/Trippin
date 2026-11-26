// @sim 70000 @bloom 0.5 @tonemap agx
// @title Particle Fountain
// Particle Fountain: five jets of sparks arcing up from the foot of the frame
// and falling away, simulated on the GPU (sim.rs): every spark is thrown,
// falls under gravity and feels the air. The loudness sets how hard the jets
// throw (flow: a loud drop sends them soaring); the mids fan the jets out
// and stir the falling sparks into eddies; every kick surges the jets, so a
// burst of faster sparks climbs each one and rains down (momentum), and
// jolts the sparks in the air outward; the highs make the sparks fizz. The fan swivels left and right over eight bars (direction).

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

const G: f32 = 3.2;
const FLOOR: f32 = -1.7;

// Jet j's launch velocity for spark i (fresh randoms k picks).
fn launch(i: u32, j: u32, k: u32) -> vec3<f32> {
    let fj = f32(j);
    let swivel = 0.35 * sin(TAU * u.flow / 32.0);
    // The mids fan the jets out.
    let spread = 0.24 + 0.06 * u.pres4.y + 0.16 * u.lvl4.y;
    let ang = (fj - 2.0) * spread + swivel + (sim_rand_f(i, k) - 0.5) * 0.12;
    let az = (sim_rand_f(i, k + 1u) - 0.5) * 0.5;
    // The loudness throws them higher, and the jets surge on each kick: a
    // spark keeps the push it was launched with, so a kick climbs the jet.
    let v0 = (2.1 + 0.9 * sim_loud() + 0.35 * u.pres4.x + 1.9 * u.hits4.x) * (1.0 - 0.3 * u.calm) * (0.9 + 0.2 * sim_rand_f(i, k + 2u));
    return vec3<f32>(sin(ang) * v0, cos(ang) * v0, sin(az) * v0 * 0.4);
}

fn base(j: u32) -> vec3<f32> {
    return vec3<f32>((f32(j) - 2.0) * 0.18, -1.25, 0.0);
}

fn sim_spawn(i: u32) -> Particle {
    // Already in flight: a launch, some way along its arc.
    let j = i % 5u;
    let v = launch(i, j, 10u);
    let t = sim_rand(i, 3u) * 2.0 * v.y / G;
    var p: Particle;
    p.pos = vec4<f32>(base(j) + v * t - vec3<f32>(0.0, 0.5 * G * t * t, 0.0), t);
    p.vel = vec4<f32>(v - vec3<f32>(0.0, G * t, 0.0), 0.0);
    return p;
}

fn sim_step(i: u32, p_in: Particle, dt: f32) -> Particle {
    var p = p_in;
    let j = i % 5u;
    // Down past the floor (or lost): thrown again from its jet.
    if p.pos.y < FLOOR || p.pos.w > 4.0 {
        p.pos = vec4<f32>(base(j), 0.0);
        p.vel = vec4<f32>(launch(i, j, 20u), 0.0);
        return p;
    }
    let x = p.pos.xyz;
    var acc = vec3<f32>(0.0, -G, 0.0) - p.vel.xyz * 0.25;
    // The mids stir the falling sparks.
    acc += sim_curl(x * 0.5 + vec3<f32>(0.0, u.clock4.z * 0.02, 0.0)) * (0.2 + 3.0 * u.lvl4.y) * smoothstep(0.1, 0.8, p.pos.w);
    // The highs make them fizz.
    let jt = vec3<f32>(sim_rand_f(i, 0u), sim_rand_f(i, 1u), sim_rand_f(i, 2u)) - vec3<f32>(0.5);
    acc += jt * u.lvl4.w * 10.0;
    var v = p.vel.xyz + acc * dt;
    // Kick: the sparks in the air jolt outward.
    let k = simc.onset.x * (1.0 - 0.8 * u.calm);
    v += vec3<f32>((x.x - base(j).x) * 1.2, 0.5, 0.0) * k;
    p.vel = vec4<f32>(v, p.vel.w);
    p.pos = vec4<f32>(x + v * dt, p.pos.w + dt);
    return p;
}

fn view(w: vec3<f32>) -> vec3<f32> {
    let wp = rot(0.1) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 4.2);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let pp = sim_p[ii];
    let w = pp.pos.xyz;
    let a = view(w);
    let age = pp.pos.w;
    let sq = sim_quad(a, view(w - pp.vel.xyz * 0.035), vi, 0.006 * (1.0 - 0.15 * min(age, 2.0)), 1.7);
    o.pos = sq.pos;
    o.q = sq.q;
    let fj = f32(ii % 5u);
    let life = clamp(age / 2.2, 0.0, 1.0);
    let fade = smoothstep(0.0, 0.05, age) * smoothstep(FLOOR, FLOOR + 0.4, w.y) * smoothstep(1.0, 0.6, life);
    let c = palette(0.1 + fj * 0.17 + life * 0.1);
    o.col = c * c * 0.42 * fade * (1.0 - 0.5 * life) * sq.dim;
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
    // Trails: the sparks streak as they fly.
    var col = prev(in.uv) * 0.7;
    col += palette(0.1) * palette(0.1) * exp(-length((p - vec2<f32>(0.0, 0.75)) * vec2<f32>(0.5, 2.5)) * 3.0) * 0.04;
    return vec4<f32>(finite(col), 1.0);
}
