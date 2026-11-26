// @sim 60000 @bloom 0.5 @tonemap agx
// @title Particle DNA
// Particle DNA: a double helix of 60,000 motes lying across the frame, with
// rungs of light between its strands. The helix's girth along its length is
// the spectrum, bass at the left and highs at the right (shape), it spins on
// the smooth energy clock (motion), and its axis sways and the camera swings
// round it over eight bars (direction). Every mote is simulated (sim.rs, the
// shape-scene pattern): kicks burst the helix outward from its axis and it
// springs back, snares wrench it, the mids fray the strands and the bass
// pulls them tight; in breakdowns it loosens into drifting haze.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

// The axis height at x (it sways in a slow wave).
fn axis_y(x: f32) -> f32 {
    return 0.25 * sin(x * 0.7 + TAU * u.flow / 32.0);
}

// Mote i on the helix (w: 0 strand A, 1 strand B, 2 rung).
fn home(i: u32) -> vec4<f32> {
    let cell = vec2<f32>(f32(i % 1024u), f32(i / 1024u));
    let h = hash22(cell + vec2<f32>(5.3, 1.1));
    let h2 = hash22(cell + vec2<f32>(2.9, 8.7));
    let kind = i % 4u; // 0,1: strands; 2,3: rungs
    var s = h.x;
    if kind >= 2u {
        // Rungs sit at fixed stations along the helix.
        s = (floor(h.x * 44.0) + 0.5) / 44.0;
    }
    let x = (s - 0.5) * 6.4;
    let girth = 0.42 * (1.0 + 0.9 * sqrt(clamp(spec(s * 0.85), 0.0, 1.0))) * (0.9 + 0.2 * u.pres4.x);
    let ang = s * 13.0 + u.clock4.x * 0.35;
    var a = ang;
    if kind == 1u {
        a = ang + PI;
    }
    var w: vec3<f32>;
    let jit = (vec3<f32>(h2.x, h2.y, fract(h2.x * 7.3 + h2.y)) - 0.5) * 0.06;
    if kind < 2u {
        w = vec3<f32>(x, cos(a) * girth, sin(a) * girth) + jit;
    } else {
        let pa = vec3<f32>(x, cos(ang) * girth, sin(ang) * girth);
        let pb = vec3<f32>(x, -cos(ang) * girth, -sin(ang) * girth);
        w = mix(pa, pb, h2.y) + jit * 0.4;
    }
    w.y += axis_y(x);
    return vec4<f32>(w, f32(min(kind, 2u)));
}

fn sim_spawn(i: u32) -> Particle {
    return sim_shape_spawn();
}

fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    let h = home(i).xyz;
    // Kicks burst out from the helix's own axis.
    return sim_shape_step(i, p, h, vec3<f32>(h.x, axis_y(h.x), 0.0), dt, 0.55);
}

// Camera: yaws round the helix and back over eight bars.
fn view(w_in: vec3<f32>) -> vec3<f32> {
    var w = w_in;
    let yaw = 0.55 * sin(TAU * u.flow / 32.0 + 0.6);
    let wy = rot(yaw) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.18) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 4.3);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let h = home(ii);
    let pp = sim_p[ii];
    let w = h.xyz + pp.pos.xyz;
    let a = view(w);
    let sq = sim_quad(a, view(w - pp.vel.xyz * 0.04), vi, 0.006, 1.7);
    o.pos = sq.pos;
    o.q = sq.q;
    let s = h.x / 6.4 + 0.5;
    let loose = min(length(pp.pos.xyz) * 4.0, 1.0);
    var c = palette(0.08 + 0.45 * f32(h.w == 1.0) + 0.1 * s + 0.15 * loose);
    var b = 0.3;
    if h.w > 1.5 {
        c = palette(0.3 + 0.2 * fract(f32(ii) * 0.618));
        b = 0.14;
    }
    o.col = c * c * b * clamp(5.6 - a.z, 0.4, 1.5) * sq.dim;
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
    col += vec3<f32>(0.006, 0.004, 0.014) * (1.0 - 0.5 * abs(p.y));
    return vec4<f32>(finite(col), 1.0);
}
