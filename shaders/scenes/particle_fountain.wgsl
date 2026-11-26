// @particles 70000 @bloom 0.5 @tonemap agx
// @title Particle Fountain
// Particle Fountain: five jets of sparks arcing up from the foot of the frame
// and falling away. The jets reach higher as the slow bass presence swells
// and fan wider with the mids (shape), the sparks fly on the smooth energy
// clock (motion), and the whole fan swivels left and right over eight bars
// (direction). Every spark is a ballistic arc: stateless.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

fn emit(w: vec3<f32>, vi: u32, size_w: f32, col: vec3<f32>) -> POut {
    var o: POut;
    o.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    if w.z < 0.3 {
        return o;
    }
    let fl = 1.7;
    let sp = w.xy * fl / w.z;
    let corners = array<vec2<f32>, 3>(vec2<f32>(-1.732, -1.0), vec2<f32>(1.732, -1.0), vec2<f32>(0.0, 2.0));
    let q = corners[vi % 3u];
    let size = max(size_w * fl / w.z, 2.0 / u.res_y);
    let p = sp + q * size;
    o.pos = vec4<f32>(p.x / aspect(), p.y, 0.0, 1.0);
    o.q = q;
    o.col = col;
    return o;
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    let cell = vec2<f32>(f32(ii % 1024u), f32(ii / 1024u));
    let h = hash22(cell + vec2<f32>(6.1, 2.2));
    let h2 = hash22(cell + vec2<f32>(3.7, 9.9));
    let jet = ii % 5u;
    let fj = f32(jet);
    // Each spark loops its flight on the energy clock.
    let life = fract(h.x + u.clock4.x * 0.16 * (0.85 + 0.3 * h2.x));
    let swivel = 0.35 * sin(TAU * u.flow / 32.0);
    // The mids fan the jets out; each kick pumps them higher.
    let spread = 0.26 + 0.06 * u.pres4.y + 0.14 * u.lvl4.y;
    let kick = kick_swell();
    let ang = (fj - 2.0) * spread + swivel + (h.y - 0.5) * 0.12;
    let az = (h2.y - 0.5) * 0.5;
    let v0 = (2.3 + 0.4 * u.pres4.x + 0.75 * kick) * (0.9 + 0.2 * h2.x);
    let g = 3.2;
    let tmax = 2.0 * v0 / g * 1.15;
    let tt = life * tmax;
    let vel = vec3<f32>(sin(ang) * v0, cos(ang) * v0, sin(az) * v0 * 0.4);
    var w = vel * tt;
    w.y -= 0.5 * g * tt * tt;
    w.y -= 1.25;
    // The jets' bases spread along the foot of the frame.
    w.x += (fj - 2.0) * 0.18;
    let wp = rot(0.1) * w.yz;
    w = vec3<f32>(w.x, wp.x, wp.y + 4.2);
    let fade = smoothstep(0.0, 0.05, life) * smoothstep(1.0, 0.6, life);
    let c = palette(0.1 + fj * 0.17 + life * 0.1);
    let b = 0.42 * fade * (1.0 - 0.5 * life);
    return emit(w, vi, 0.006 * (1.0 - 0.5 * life), c * c * b);
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
