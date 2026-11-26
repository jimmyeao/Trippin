// @particles 100000 @bloom 0.5 @tonemap agx
// @title Particle Tornado
// Particle Tornado: a funnel of 100,000 motes spiralling upward, fast in the
// narrow throat and slow at the wide top. The funnel's waist bulges with the
// slow bass presence (shape), the motes spin and climb on the smooth energy
// clock (motion), and the funnel leans one way, then the other, over eight
// bars (direction). Stateless (render.rs `@particles`).

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
    let h = hash22(cell + vec2<f32>(1.7, 4.4));
    let h2 = hash22(cell + vec2<f32>(9.1, 3.3));
    // Height 0..1, climbing on the energy clock.
    let t = fract(h.x + u.clock4.x * 0.025);
    let y = -1.35 + t * 2.8;
    let waist = 1.0 + 0.45 * u.pres4.x * sin(t * PI);
    var r = (0.08 + 0.95 * t * t) * waist + (h2.x - 0.5) * (0.05 + 0.2 * t);
    r = max(r, 0.02);
    let a = h.y * TAU + u.clock4.x * 0.9 / (0.35 + r);
    var w = vec3<f32>(cos(a) * r, y, sin(a) * r);
    // The funnel's axis bends, leaning with an eight-bar swing.
    let lean = 0.45 * sin(TAU * u.flow / 32.0);
    w.x += lean * t * t + 0.12 * sin(t * 5.0 + u.clock4.x * 0.2);
    let wp = rot(0.12) * w.yz;
    w = vec3<f32>(w.x, wp.x, wp.y + 4.0);
    let fade = smoothstep(0.0, 0.08, t) * smoothstep(1.0, 0.82, t);
    let c = palette(0.05 + t * 0.55 + h2.y * 0.08);
    let b = (0.16 + 0.1 * u.pres4.x) * fade * clamp(5.0 - w.z, 0.4, 1.5);
    return emit(w, vi, 0.0045, c * c * b);
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
