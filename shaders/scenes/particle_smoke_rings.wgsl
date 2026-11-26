// @particles 90000 @bloom 0.5 @tonemap agx
// @title Smoke Rings
// Smoke Rings: glowing vortex rings of 90,000 motes rising up the frame,
// each rolling round its own core as it climbs and widens. The rings fatten
// with the slow bass presence (shape), they climb and roll on the smooth
// energy clock (motion), and their path leans one way, then the other, over
// eight bars (direction). Five rings in flight, always five, staggered.

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
    let h = hash22(cell + vec2<f32>(3.3, 6.6));
    let h2 = hash22(cell + vec2<f32>(5.5, 2.1));
    let ring = ii % 5u;
    let fr = f32(ring);
    let ph = u.clock4.x * 0.035 + fr / 5.0;
    let life = fract(ph);
    let gen = floor(ph);
    let rh = hash22(vec2<f32>(fr, gen * 3.0 + 7.0));
    // The ring's centre climbs; its radius grows as it goes.
    let lean = 0.4 * sin(TAU * u.flow / 32.0);
    let cy = -1.4 + life * 2.9;
    let cx = (rh.x - 0.5) * 1.2 + lean * life;
    // Kicks puff every ring wider and fatter; the mids thicken the cores.
    let kick = kick_swell();
    let big = (0.25 + 0.5 * life) * (1.0 + 0.16 * kick);
    let core = (0.06 + 0.04 * u.pres4.x + 0.05 * u.lvl4.y + 0.06 * kick) * (0.6 + 0.6 * life);
    // A point on the torus: round the ring (a) and round its core (b), the
    // core angle rolling as it climbs.
    let a = h.x * TAU;
    let b = h.y * TAU + u.clock4.x * 0.6;
    let cr = core * sqrt(h2.x);
    let rr = big + cr * cos(b);
    var w = vec3<f32>(cos(a) * rr, cr * sin(b), sin(a) * rr);
    // Tilt the ring a little toward the camera.
    let wt = rot(0.35 + (rh.y - 0.5) * 0.4) * w.yz;
    w = vec3<f32>(w.x + cx, wt.x + cy, wt.y);
    let wp = rot(0.08) * w.yz;
    w = vec3<f32>(w.x, wp.x, wp.y + 4.0);
    let fade = smoothstep(0.0, 0.12, life) * smoothstep(1.0, 0.7, life);
    let c = palette(0.5 + 0.12 * fr + 0.15 * cos(b));
    let bri = 0.13 * fade * (0.7 + 0.5 * sin(b) * sin(b)) * clamp(5.0 - w.z, 0.4, 1.6);
    return emit(w, vi, 0.005, c * c * bri);
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
    var col = prev(in.uv) * 0.6;
    col += vec3<f32>(0.005, 0.004, 0.012) * (1.0 - 0.4 * length(p));
    return vec4<f32>(finite(col), 1.0);
}
