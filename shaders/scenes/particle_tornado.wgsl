// @sim 100000 @bloom 0.5 @tonemap agx
// @title Particle Tornado
// Particle Tornado: a funnel of 100,000 motes simulated on the GPU (sim.rs),
// each swept round the funnel's axis, lifted up it and sprung to its wall,
// keeping its speed from frame to frame. The loudness drives the spin and
// the climb (flow); the bass presence swells the waist (shape); every kick
// blows the wall out and up, and the motes swing back in (momentum); the
// highs tear the edge into turbulence. The axis leans one way, then the
// other, over eight bars, and the motes lag behind it like a real whirl.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

const Y0: f32 = -1.35;
const YH: f32 = 2.8;

// The funnel's axis at height fraction t (it leans on an eight-bar swing).
fn axis(t: f32) -> f32 {
    let lean = 0.45 * sin(TAU * u.flow / 32.0);
    return lean * t * t + 0.12 * sin(t * 5.0 + u.clock4.x * 0.2);
}

// The funnel wall's radius at t for mote i.
fn wall(i: u32, t: f32) -> f32 {
    let waist = 1.0 + 0.4 * u.pres4.x * sin(t * PI);
    let own = (sim_rand(i, 3u) - 0.5) * (0.05 + 0.2 * t);
    return max((0.08 + 0.95 * t * t) * waist + own, 0.03);
}

fn spawn_at(i: u32, t: f32, k: u32) -> Particle {
    var p: Particle;
    let a = sim_rand_f(i, k) * TAU;
    let r = wall(i, t);
    p.pos = vec4<f32>(axis(t) + cos(a) * r, Y0 + t * YH, sin(a) * r, 0.0);
    p.vel = vec4<f32>(0.0);
    return p;
}

fn sim_spawn(i: u32) -> Particle {
    return spawn_at(i, sim_rand(i, 1u), 0u);
}

fn sim_step(i: u32, p_in: Particle, dt: f32) -> Particle {
    var p = p_in;
    let x = p.pos.xyz;
    let t = (x.y - Y0) / YH;
    // Out the top (or the bottom): back in at the foot.
    if t > 1.0 || t < -0.05 {
        return spawn_at(i, 0.02 * sim_rand_f(i, 5u), 4u);
    }
    let calm = u.calm;
    let loud = sim_loud();
    let rel = vec2<f32>(x.x - axis(t), x.z);
    let r = max(length(rel), 1e-3);
    let rad = rel / r;
    let tng = vec2<f32>(-rad.y, rad.x);
    // The flow each mote relaxes toward: spin (fastest in the throat), lift,
    // and a spring onto the wall.
    let spin = (0.7 + 2.2 * loud) * (1.0 - 0.5 * calm) * (0.55 + 0.6 * r) / (0.35 + r);
    let lift = (0.22 + 0.55 * loud) * (1.0 - 0.55 * calm) * (0.7 + 0.6 * sim_rand(i, 2u));
    let back = (wall(i, t) - r) * 6.0;
    var v = vec3<f32>(tng.x * spin + rad.x * back, lift, tng.y * spin + rad.y * back);
    // The highs tear the wall's edge into turbulence.
    v += sim_curl(x * 0.6 + vec3<f32>(0.0, -u.clock4.w * 0.04, 0.0)) * (0.08 + 0.9 * u.lvl4.w) * (0.3 + t);
    var acc = (v - p.vel.xyz) * 5.0;
    // Kick: the wall blows out and up; the spring and the drag bring it back.
    let k = simc.onset.x * (1.0 - 0.85 * calm) * 5.0;
    let kick = vec3<f32>(rad.x, 0.35, rad.y) * k * (0.5 + t);
    p.vel = vec4<f32>(p.vel.xyz + acc * dt + kick, p.vel.w);
    p.pos = vec4<f32>(x + p.vel.xyz * dt, p.pos.w + dt);
    return p;
}

// World to camera: tilted a little, four units back.
fn view(w: vec3<f32>) -> vec3<f32> {
    let wp = rot(0.12) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 4.0);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    o.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    o.q = vec2<f32>(0.0);
    o.col = vec3<f32>(0.0);
    let pp = sim_p[ii];
    let a = view(pp.pos.xyz);
    let b = view(pp.pos.xyz - pp.vel.xyz * 0.04);
    if a.z < 0.3 || b.z < 0.3 {
        return o;
    }
    let fl = 1.7;
    let sa = a.xy * fl / a.z;
    let sb = b.xy * fl / b.z;
    let d = sa - sb;
    let dl = length(d);
    let tang = select(vec2<f32>(1.0, 0.0), d / max(dl, 1e-6), dl > 1e-5);
    let nrm = vec2<f32>(-tang.y, tang.x);
    let size = max(0.0045 * fl / a.z, 2.0 / u.res_y);
    let len = size + 0.5 * dl;
    let corners = array<vec2<f32>, 3>(vec2<f32>(-1.732, -1.0), vec2<f32>(1.732, -1.0), vec2<f32>(0.0, 2.0));
    let q = corners[vi % 3u];
    let p = sa - 0.5 * d + tang * q.x * len + nrm * q.y * size;
    o.pos = vec4<f32>(p.x / aspect(), p.y, 0.0, 1.0);
    o.q = q;
    let t = clamp((pp.pos.y - Y0) / YH, 0.0, 1.0);
    let fade = smoothstep(0.0, 0.08, t) * smoothstep(1.0, 0.82, t) * smoothstep(0.0, 0.4, pp.pos.w);
    let c = palette(0.05 + t * 0.55 + sim_rand(ii, 4u) * 0.08);
    let br = (0.16 + 0.1 * u.pres4.x) * fade * clamp(5.0 - a.z, 0.4, 1.5) * (size / len);
    o.col = c * c * br;
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
    var col = prev(in.uv) * 0.55;
    // A dim ground glow under the funnel's foot.
    col += palette(0.05) * palette(0.05) * exp(-length((p - vec2<f32>(0.0, 0.85)) * vec2<f32>(0.6, 3.0)) * 3.0) * 0.05;
    return vec4<f32>(finite(col), 1.0);
}
