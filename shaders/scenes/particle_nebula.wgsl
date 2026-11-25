// @particles 150000 @bloom 0.6 @tonemap agx
// @title Particle Nebula
// Particle Nebula: a cloud of 150,000 glowing motes threaded into filaments
// by 3D noise, which a slow camera circles. The cloud breathes out with the
// bass presence and its heart swells on the bass (shape), the filaments drift
// on the smooth energy clock (motion), and the camera's height swings over
// sixteen bars (direction). After the GPU particle systems of Imaginando VS.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    o.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    o.q = vec2<f32>(0.0);
    o.col = vec3<f32>(0.0);
    let cell = vec2<f32>(f32(ii % 1024u), f32(ii / 1024u));
    let a = hash22(cell + vec2<f32>(1.3, 4.1));
    let b = hash22(cell + vec2<f32>(8.9, 2.7));
    // A point in the unit ball (denser toward the centre).
    let zc = a.x * 2.0 - 1.0;
    let ph = a.y * TAU;
    let s = sqrt(max(1.0 - zc * zc, 0.0));
    let r0 = pow(b.x, 0.6);
    var w = vec3<f32>(s * cos(ph), zc, s * sin(ph)) * r0 * 1.6;
    // Filaments: pull each mote along a smooth noise field that drifts slowly.
    let t = u.clock4.x * 0.03;
    let n = tnoise(w * 0.22 + vec3<f32>(t, 0.3 * t, -0.5 * t));
    w += (n.rgb - vec3<f32>(0.5)) * 1.4;
    // Density: motes in thin parts of the field fade, so the cloud has
    // dark lanes and bright threads.
    let dens = smoothstep(0.38, 0.62, n.a);
    if dens < 0.02 {
        return o;
    }
    // Breathing out with the bass presence; the heart swells on the bass.
    let rr = length(w);
    w *= 1.0 + 0.14 * u.pres4.x + 0.18 * u.lvl4.x * exp(-rr * 1.5);
    // Camera circles once every sixteen bars; its height swings.
    let yaw = TAU * u.flow / 64.0;
    let wy = rot(yaw) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let pitch = 0.25 * sin(TAU * u.flow / 64.0);
    let wp = rot(pitch) * w.yz;
    w = vec3<f32>(w.x, wp.x, wp.y);
    let z = w.z + 3.0;
    if z < 0.3 {
        return o;
    }
    let fl = 1.6;
    let sp = vec2<f32>(w.x, -w.y) * fl / z;
    let corners = array<vec2<f32>, 3>(vec2<f32>(-1.732, -1.0), vec2<f32>(1.732, -1.0), vec2<f32>(0.0, 2.0));
    let q = corners[vi % 3u];
    let size = max(0.0042 * fl / z, 2.0 / u.res_y);
    let p = sp + q * size;
    o.pos = vec4<f32>(p.x / aspect(), -p.y, 0.0, 1.0);
    o.q = q;
    let c = palette(rr * 0.25 + n.g * 0.4);
    o.col = c * c * dens * (0.22 + 0.12 * u.pres4.y) * clamp(4.2 - z, 0.4, 1.6);
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
