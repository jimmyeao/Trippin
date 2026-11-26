// @sim 30000 @bloom 0.4
// @title Combed Flow
// Combed Flow: 30,000 streaks of light simulated on the GPU (sim.rs), each
// carried along wavy streamlines like light drawn through hair and keeping
// its own speed. The loudness sets how fast the stream runs and the streaks
// stretch with their speed (flow); the bass combs them into tight bright
// strands and breakdowns loosen them into a haze (density); the mids stir
// eddies through the stream; every kick surges it forward and shakes the
// strands apart before they settle back (momentum). The stream's angle and
// its waves' phase swing back and forth over eight bars (direction).
// After VS's Combed Flow particle system.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

const YS: f32 = 1.2;

fn span() -> f32 {
    return 2.0 * aspect() + 0.6;
}

// The stream function: streamlines are its level sets, three waves whose
// depth follows the music. Returns (psi, d psi/dx, d psi/dy).
fn psi(q: vec2<f32>) -> vec3<f32> {
    let swing = sin(TAU * u.flow / 32.0);
    let a1 = 0.12 + 0.14 * (u.pres4.x * 0.7 + u.lvl4.x * 0.3);
    let a2 = 0.06 + 0.08 * (u.pres4.y * 0.7 + u.lvl4.y * 0.3);
    let a3 = 0.03;
    let p1 = q.x * 1.3 + q.y * 2.0 + swing * 1.5;
    let p2 = q.x * 3.1 - q.y * 3.0 - swing * 2.2 + 1.7;
    let p3 = q.x * 7.0 + q.y * 9.0 + u.flow * 0.2;
    // psi = y - waves, so the streamlines are y = y0 + waves.
    let v = q.y - a1 * sin(p1) - a2 * sin(p2) - a3 * sin(p3);
    let dx = -a1 * 1.3 * cos(p1) - a2 * 3.1 * cos(p2) - a3 * 7.0 * cos(p3);
    let dy = 1.0 - a1 * 2.0 * cos(p1) + a2 * 3.0 * cos(p2) - a3 * 9.0 * cos(p3);
    return vec3<f32>(v, dx, dy);
}

fn spawn_at(i: u32, x: f32, k: u32) -> Particle {
    var p: Particle;
    p.pos = vec4<f32>(x, (sim_rand_f(i, k) - 0.5) * 2.0 * YS, 0.0, 0.0);
    p.vel = vec4<f32>(0.0);
    return p;
}

fn sim_spawn(i: u32) -> Particle {
    var p = spawn_at(i, (sim_rand(i, 0u) - 0.5) * span(), 1u);
    // Staggered lifetimes, so the first respawns don't all land together.
    p.vel.w = 6.0 * sim_rand(i, 8u);
    p.pos.w = 3.0 * sim_rand(i, 9u);
    return p;
}

// The stream's velocity at x for particle i (xy), and the downstream
// direction (zw).
fn stream_v(i: u32, x: vec2<f32>) -> vec4<f32> {
    let calm = u.calm;
    let loud = sim_loud();
    // The stream runs at an angle that swings over eight bars.
    let ang = 0.3 * sin(TAU * u.flow / 32.0 + 1.0);
    let q = rot(-ang) * x;
    let s = psi(q);
    let g = vec2<f32>(s.y, s.z);
    // Along the streamlines (perpendicular to grad psi), at the loudness's speed.
    let speed = (0.18 + 0.95 * loud) * (1.0 - 0.45 * calm) * (0.8 + 0.4 * sim_rand(i, 5u));
    var vq = vec2<f32>(g.y, -g.x) * speed;
    // Density: combed onto strands every 0.06 of psi. The bass pulls them
    // tight; breakdowns let go.
    let lane = 0.06;
    let comb = (0.4 + 2.6 * u.pres4.x + 1.4 * u.lvl4.x) * (1.0 - 0.75 * calm);
    vq -= g / max(dot(g, g), 0.2) * sin(TAU * s.x / lane) * lane / TAU * comb * 6.0;
    var v = rot(ang) * vq;
    // The mids stir eddies through the stream.
    v += sim_curl2(x * 0.45, u.clock4.z * 0.01) * (0.04 + 0.5 * u.lvl4.y);
    return vec4<f32>(v, rot(ang) * normalize(vec2<f32>(g.y, -g.x)));
}

fn sim_step(i: u32, p_in: Particle, dt: f32) -> Particle {
    var p = p_in;
    let w = span();
    var x = p.pos.xy;
    // Off either end: back in at the other; streaks also live 3-9 s, so
    // the combing never drains the gaps between strands. They come in at
    // the stream's own speed (from rest they would bunch up at the edge).
    if x.x > 0.5 * w || x.x < -0.5 * w || abs(x.y) > YS + 0.3 || p.pos.w > p.vel.w + 3.0 {
        var n = spawn_at(i, select(0.5 * w, -0.5 * w, x.x > 0.0), 2u);
        if abs(x.x) < 0.5 * w {
            n = spawn_at(i, (sim_rand_f(i, 3u) - 0.5) * w, 2u);
        }
        let v0 = stream_v(i, n.pos.xy).xy;
        n.vel = vec4<f32>(v0, 0.0, 6.0 * sim_rand_f(i, 4u));
        return n;
    }
    let f = stream_v(i, x);
    var acc = (f.xy - p.vel.xy) * 5.0;
    // Kick: a surge downstream, and a shake across the strands.
    let along = f.zw;
    let kk = simc.onset.x * (1.0 - 0.8 * u.calm);
    let kick = along * kk * 1.8 + vec2<f32>(-along.y, along.x) * (sim_rand(i, 6u) - 0.5) * kk * 2.6;
    let nv = p.vel.xy + acc * dt + kick;
    p.vel = vec4<f32>(nv, 0.0, p.vel.w);
    x += nv * dt;
    p.pos = vec4<f32>(x, 0.0, p.pos.w + dt);
    return p;
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let pp = sim_p[ii];
    let asp = aspect();
    let w = span();
    let x = pp.pos.xy;
    let v = pp.vel.xy;
    let sp = length(v);
    let tang = select(vec2<f32>(1.0, 0.0), v / max(sp, 1e-6), sp > 1e-5);
    let nrm = vec2<f32>(-tang.y, tang.x);
    let corners = array<vec2<f32>, 3>(vec2<f32>(-1.732, -1.0), vec2<f32>(1.732, -1.0), vec2<f32>(0.0, 2.0));
    let q = corners[vi % 3u];
    // A streak as long as it travels in ~1/20 s: speed reads as length.
    let len = 0.014 + 0.012 * sim_rand(ii, 7u) + 0.05 * sp;
    let wid = max(0.0022, 1.5 / u.res_y);
    let p = x - tang * len * 0.5 + tang * q.x * len + nrm * q.y * wid;
    o.pos = vec4<f32>(p.x / asp, -p.y, 0.0, 1.0);
    o.q = q;
    // Fade in at birth and at the ends of the stream, so respawns are unseen.
    let edge = smoothstep(-0.5 * w, -0.5 * w + 0.3, x.x) * (1.0 - smoothstep(0.5 * w - 0.3, 0.5 * w, x.x));
    let life = smoothstep(0.0, 0.5, pp.pos.w) * (1.0 - smoothstep(pp.vel.w + 2.5, pp.vel.w + 3.0, pp.pos.w));
    let c = palette(clamp(x.y / (2.0 * YS) + 0.5, 0.0, 1.0) * 0.6 + 0.1);
    o.col = c * c * 0.3 * edge * life * (0.75 + 0.5 * min(sp, 1.0)) * (0.03 / len);
    return o;
}

@fragment
fn fs_particles(in: POut) -> @location(0) vec4<f32> {
    let r2 = dot(in.q, in.q);
    if r2 > 1.0 {
        discard;
    }
    // Bright along the streak's axis, soft at its ends.
    let a = (1.0 - in.q.x * in.q.x) * exp(-in.q.y * in.q.y * 3.0);
    return vec4<f32>(in.col * a, 0.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Silky trails: the streaks smear into continuous strands.
    var col = prev(in.uv) * 0.62;
    col += vec3<f32>(0.002, 0.002, 0.006);
    return vec4<f32>(finite(col), 1.0);
}
