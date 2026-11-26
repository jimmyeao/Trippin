// @particles 90000 @bloom 0.5 @tonemap agx
// @title Particle Sphere
// Particle Sphere: 90,000 motes spread evenly over a sphere that morphs with
// the music. Each latitude bulges with its band of the spectrum (bass at the
// poles, highs at the equator) and slow lobes roll across the surface
// (shape); it turns on the smooth energy clock (motion), and its tilt swings
// over eight bars (direction). Stateless (render.rs `@particles`).

const N: f32 = 90000.0;

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
    let fi = f32(ii);
    // Fibonacci sphere: even coverage with no poles bunching.
    let y = 1.0 - 2.0 * (fi + 0.5) / N;
    let rr = sqrt(max(1.0 - y * y, 0.0));
    let phi = fi * 2.399963;
    var n = vec3<f32>(cos(phi) * rr, y, sin(phi) * rr);
    // Shape: the spectrum by latitude plus slow rolling lobes.
    let band = spec(1.0 - abs(y) * 0.95);
    let t = u.clock4.x * 0.12;
    let lobes = sin(n.x * 3.0 + t) * sin(n.y * 2.5 - t * 0.7) * sin(n.z * 3.0 + t * 0.5);
    let disp = 0.32 * sqrt(clamp(band, 0.0, 1.0)) + (0.12 + 0.12 * u.pres4.y) * lobes;
    var w = n * (1.0 + 0.08 * u.pres4.x + 0.12 * kick_swell() + disp);
    // Turn on the energy clock; tilt swings over eight bars.
    let wy = rot(u.clock4.x * 0.12) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.35 * sin(TAU * u.flow / 32.0)) * w.yz;
    w = vec3<f32>(w.x, wp.x, wp.y + 3.6);
    let c = palette(0.15 + 0.9 * disp + 0.1 * y);
    // The far side dimmer, so it reads as a solid shell.
    let b = (0.14 + 0.2 * clamp(disp * 2.0, 0.0, 1.0)) * clamp(4.4 - w.z, 0.3, 1.4);
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
    var col = prev(in.uv) * 0.4;
    let core = exp(-length(p) * 3.0);
    col += palette(0.6) * palette(0.6) * core * 0.03;
    return vec4<f32>(finite(col), 1.0);
}
