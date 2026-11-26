// @sim 80000 @bloom 0.55 @tonemap agx
// @title Particle Rose
// Particle Rose: a flower of 80,000 motes drawn as a polar rose, its petals
// cupped up into a bowl and seen from above at an angle. It grows a
// different number of petals every eight bars, morphing over two beats, and
// each petal reaches out with its band of the music (shape); the flower
// turns on the smooth energy clock and ripples rise through the bowl
// (motion), its spin reversing with each new flower (direction). Every mote
// is simulated (sim.rs, the shape-scene pattern): kicks burst the petals
// open and they spring back, snares wrench the flower round, the mids blur
// its edges and the bass draws them crisp; in breakdowns it drifts apart.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

// Mote i on the flower (w: its radius, for colour).
fn home(i: u32) -> vec4<f32> {
    let cell = vec2<f32>(f32(i % 1024u), f32(i / 1024u));
    let h = hash22(cell + vec2<f32>(2.4, 5.6));
    let h2 = hash22(cell + vec2<f32>(7.2, 1.3));
    let th = h.x * TAU;
    let k = floor(u.flow / 32.0);
    let m = smoothstep(0.0, 2.0, u.flow - k * 32.0);
    let ks = array<f32, 4>(3.0, 4.0, 5.0, 7.0);
    let ka = ks[u32(k) % 4u];
    let kb = ks[(u32(k) + 3u) % 4u];
    let ra = abs(cos(ka * th * 0.5));
    let rb = abs(cos(kb * th * 0.5));
    let shape = mix(rb, ra, m);
    // Each petal reaches with a band (petals cycle bass, mid, mid-high, high).
    let petal = u32(floor(th * ka / TAU + 0.5)) % 4u;
    let reach = 1.0 + 0.25 * u.pres4[petal] + 0.4 * u.lvl4[petal];
    // Most motes on the petal edge, the rest filling it.
    var f = 0.92 + 0.08 * h.y;
    if (i % 3u) == 0u {
        f = sqrt(h.y);
    }
    let rad = shape * f * reach * 0.95;
    var w = vec3<f32>(cos(th) * rad, 0.0, sin(th) * rad);
    // Cupped into a bowl, with ripples rising through it.
    w.y = 0.35 * rad * rad + 0.05 * sin(rad * 9.0 - u.clock4.x * 0.6) + (h2.x - 0.5) * 0.02 - 0.15;
    return vec4<f32>(w, rad);
}

fn sim_spawn(i: u32) -> Particle {
    return sim_shape_spawn();
}

fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    // Kicks burst out from the heart of the bowl.
    return sim_shape_step(i, p, home(i).xyz, vec3<f32>(0.0, -0.25, 0.0), dt, 1.0);
}

// Spin (reversing with each flower), then the camera above at an angle.
fn view(w_in: vec3<f32>) -> vec3<f32> {
    var w = w_in;
    let k = floor(u.flow / 32.0);
    let dir = select(1.0, -1.0, (u32(k) & 1u) == 1u);
    let wy = rot(u.clock4.x * 0.07 * dir) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(-0.75) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 3.2);
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
    let rad = h.w;
    let loose = min(length(pp.pos.xyz) * 3.0, 1.0);
    let edge = select(1.0, 0.6, (ii % 3u) == 0u);
    let c = palette(0.75 + 0.25 * rad + 0.1 * loose);
    let b = (0.12 + 0.08 * edge + 0.05 * loose) * clamp(4.2 - a.z, 0.4, 1.6);
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
    var col = prev(in.uv) * 0.5;
    col += palette(0.9) * palette(0.9) * exp(-length(p) * 6.0) * 0.05;
    return vec4<f32>(finite(col), 1.0);
}
