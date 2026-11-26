// @sim 120000 @bloom 0.45 @tonemap agx
// @title Particle Sea
// Particle Sea: a sea of 120,000 points of light stretching to the horizon
// under a still camera. The long swell rises with the slow bass presence and
// the chop with the mids, and a crest of spectrum runs across it (shape);
// the waves roll on the smooth energy clock (motion), and the wind's
// direction swings over eight bars (direction). Every point is simulated
// (sim.rs, the shape-scene pattern): each kick sends a front rolling out to
// sea that throws the points it passes up into the air, and they bob back
// down in its wake; the mids scatter spray off the surface and the bass
// smooths it; in breakdowns it hangs in a mist. Not a flight: the camera
// stays put.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

fn sea_h(x: f32, z: f32) -> f32 {
    let t = u.clock4.x * 0.35;
    let wind = 0.6 * sin(TAU * u.flow / 32.0);
    let d1 = vec2<f32>(sin(wind), cos(wind));
    let d2 = vec2<f32>(sin(wind + 1.1), cos(wind + 1.1));
    let pxz = vec2<f32>(x, z);
    let swell = (0.12 + 0.22 * u.pres4.x) * sin(dot(pxz, d1) * 0.9 - t);
    let chop = (0.04 + 0.04 * u.pres4.y + 0.07 * u.lvl4.y) * sin(dot(pxz, d2) * 2.3 - t * 1.6 + 1.0);
    // A crest whose height across the sea is the spectrum.
    let s = spec(clamp(x / 8.0 + 0.5, 0.0, 1.0));
    let cz = sin(z * 0.35 - t * 0.5) - 0.9;
    let crest = 0.25 * sqrt(clamp(s, 0.0, 1.0)) * exp(-cz * cz * 6.0);
    return swell + chop + crest;
}

// Point i on the surface (x, height, z).
fn home(i: u32) -> vec3<f32> {
    let gx = f32(i % 400u);
    let gz = f32(i / 400u);
    let jit = hash22(vec2<f32>(gx, gz)) - 0.5;
    // Rows packed closer near the camera, wider far off.
    let fz = (gz + jit.y * 0.6) / 300.0;
    let z = 0.1 + fz * fz * 14.0;
    let x = ((gx + jit.x * 0.6) / 400.0 - 0.5) * (3.0 + z * 2.6);
    return vec3<f32>(x, sea_h(x, z), z);
}

fn sim_spawn(i: u32) -> Particle {
    return sim_shape_spawn();
}

fn sim_step(i: u32, p_in: Particle, dt: f32) -> Particle {
    let h = home(i);
    // No shock from a centre (the whole sea would jump): pass the point as
    // its own centre, and let a front roll out from the shore on each kick,
    // throwing up what it passes.
    var p = sim_shape_step(i, p_in, h, h, dt, 0.35 + 0.05 * h.z);
    let front = beat_ring(h.z / 12.0, 0.07);
    p.vel = vec4<f32>(p.vel.xyz + vec3<f32>(0.0, 60.0, 0.0) * front * dt, p.vel.w);
    return p;
}

// Camera 1.1 up, pitched gently down.
fn view(h: vec3<f32>) -> vec3<f32> {
    let w = vec3<f32>(h.x, h.y - 1.1, h.z + 0.6);
    let wp = rot(-0.35) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let h = home(ii);
    let pp = sim_p[ii];
    let w = h + pp.pos.xyz;
    let a = view(w);
    let sq = sim_quad(a, view(w - pp.vel.xyz * 0.04), vi, 0.006, 1.7);
    o.pos = sq.pos;
    o.q = sq.q;
    let lift = clamp(w.y * 2.0 + 0.4, 0.0, 1.6);
    let c = palette(0.55 + 0.25 * lift + 0.03 * h.z);
    let far = exp(-h.z * 0.13) * smoothstep(14.1, 8.0, h.z);
    o.col = c * c * (0.12 + 0.3 * lift) * far * sq.dim;
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
    var col = prev(in.uv) * 0.35;
    // A faint sky glow above the horizon.
    let sky = exp(-abs(p.y + 0.6) * 5.0);
    col += palette(0.7) * palette(0.7) * sky * 0.025;
    return vec4<f32>(finite(col), 1.0);
}
