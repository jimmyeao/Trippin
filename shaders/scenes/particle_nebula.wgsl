// @sim 150000 @bloom 0.6 @tonemap agx
// @title Particle Nebula
// Particle Nebula: a cloud of 150,000 glowing motes simulated on the GPU
// (sim.rs): each keeps its position and speed, swirls through a curl-noise
// flow and is sprung to its own place in the cloud, which a slow camera
// circles. The bass pulls the cloud dense and tight and breakdowns let it
// drift wide (density); the mids speed and churn the swirl (flow); every
// kick blasts a shock out from the heart that springs back (momentum); the
// highs shimmer the high-band motes. Each quarter of the motes answers its
// own band, in its own colour. After the GPU particle systems of Imaginando VS.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

// Each mote's home: a point in the ball, pulled along a slowly drifting
// noise field into filaments (w: how dense the field is there, so thin
// parts fade into dark lanes).
fn home(i: u32) -> vec4<f32> {
    // On the unit sphere, then out to a radius denser toward the heart.
    let b = sim_ball(sim_rand(i, 0u), sim_rand(i, 1u), 1.0);
    let w = b * pow(sim_rand(i, 2u), 0.6) * 1.6;
    // The field flows on the mid clock: busy mids churn the filaments.
    let t = u.clock4.z * 0.02;
    let n = tnoise(w * 0.22 + vec3<f32>(t, 0.3 * t, -0.5 * t));
    return vec4<f32>(w + (n.rgb - vec3<f32>(0.5)) * 1.4, smoothstep(0.38, 0.62, n.a));
}

fn sim_spawn(i: u32) -> Particle {
    var p: Particle;
    p.pos = vec4<f32>(home(i).xyz, 0.0);
    p.vel = vec4<f32>(0.0);
    return p;
}

fn sim_step(i: u32, p_in: Particle, dt: f32) -> Particle {
    var p = p_in;
    let band = sim_band(i);
    let x = p.pos.xyz;
    let r = length(x);
    let dir = x / max(r, 1e-3);
    let calm = u.calm;
    // Swirl: the flow the motes relax toward, drifting on the mid clock.
    // The mids make it faster and more turbulent; breakdowns slow it.
    let t = u.clock4.z * 0.015;
    let fs = 0.22 + 0.1 * u.lvl4.y;
    let swirl = (0.3 + 1.4 * u.lvl4.y + 0.5 * u.pres4.y + select(0.0, 0.8 * u.lvl4.y, band == 1u)) * (1.0 - 0.55 * calm);
    let flow = sim_curl(x * fs + vec3<f32>(t, 0.37 * t, -0.61 * t)) * swirl * 0.22;
    var acc = (flow - p.vel.xyz) * 5.0;
    // Density: a spring to the mote's home, scaled by how tight the cloud
    // is: the bass draws it in, breakdowns let it spread.
    let tight = 1.08 + 0.35 * calm - 0.2 * u.pres4.x - 0.12 * u.lvl4.x;
    let h = home(i).xyz * tight;
    acc += (h - x) * (11.0 + 4.0 * u.pres4.x);
    // Kick: a shock out from the heart that springs back. Every mote takes
    // the same push, so the filaments blow out whole instead of blurring.
    var kick = dir * simc.onset.x * 2.8 * exp(-0.4 * r) * (1.0 - 0.85 * calm);
    // Snares and claps (mid-high hits) twist the cloud round its axis.
    kick += vec3<f32>(-x.z, 0.0, x.x) * simc.onset.z * 1.2 * (1.0 - 0.6 * calm);
    // Highs: the high-band motes shimmer.
    if band == 3u {
        let j = vec3<f32>(sim_rand_f(i, 0u), sim_rand_f(i, 1u), sim_rand_f(i, 2u)) - vec3<f32>(0.5);
        acc += j * u.lvl4.w * 8.0;
    }
    p.vel = vec4<f32>(p.vel.xyz + acc * dt + kick, p.vel.w);
    p.pos = vec4<f32>(x + p.vel.xyz * dt, p.pos.w + dt);
    return p;
}

// World to camera: circles once every sixteen bars; its height swings.
fn view(w_in: vec3<f32>) -> vec3<f32> {
    var w = w_in;
    let yaw = TAU * u.flow / 64.0;
    let wy = rot(yaw) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let pitch = 0.25 * sin(TAU * u.flow / 64.0);
    let wp = rot(pitch) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 3.0);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    o.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    o.q = vec2<f32>(0.0);
    o.col = vec3<f32>(0.0);
    let pp = sim_p[ii];
    let a = view(pp.pos.xyz);
    let b = view(pp.pos.xyz - pp.vel.xyz * 0.05);
    if a.z < 0.3 || b.z < 0.3 {
        return o;
    }
    let fl = 1.6;
    let sa = a.xy * fl / a.z;
    let sb = b.xy * fl / b.z;
    // A streak along its motion on screen: fast motes smear.
    let d = sa - sb;
    let dl = length(d);
    let tang = select(vec2<f32>(1.0, 0.0), d / max(dl, 1e-6), dl > 1e-5);
    let nrm = vec2<f32>(-tang.y, tang.x);
    let size = max(0.0042 * fl / a.z, 2.0 / u.res_y);
    let len = size + 0.5 * dl;
    let corners = array<vec2<f32>, 3>(vec2<f32>(-1.732, -1.0), vec2<f32>(1.732, -1.0), vec2<f32>(0.0, 2.0));
    let q = corners[vi % 3u];
    let p = sa - 0.5 * d + tang * q.x * len + nrm * q.y * size;
    o.pos = vec4<f32>(p.x / aspect(), p.y, 0.0, 1.0);
    o.q = q;
    let band = sim_band(ii);
    let speed = length(pp.vel.xyz);
    let r = length(pp.pos.xyz);
    let c = palette(f32(band) * 0.07 + r * 0.22 + 0.05);
    // Brightness spread over the streak, so speed reads as shape.
    let fade = smoothstep(0.0, 0.6, pp.pos.w) * home(ii).w;
    o.col = c * c * (0.22 + 0.12 * u.pres4.y + 0.08 * min(speed, 2.0)) * (size / len) * fade * clamp(4.2 - a.z, 0.4, 1.6);
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
    var col = prev(in.uv) * 0.45;
    // Faint glow behind the cloud's heart.
    col += palette(0.3) * palette(0.3) * exp(-length(p) * 3.0) * 0.05;
    col += vec3<f32>(0.003, 0.004, 0.01);
    return vec4<f32>(finite(col), 1.0);
}
