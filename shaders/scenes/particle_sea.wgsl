// @particles 120000 @bloom 0.45 @tonemap agx
// @title Particle Sea
// Particle Sea: a sea of 120,000 points of light stretching to the horizon
// under a still camera. The long swell rises with the slow bass presence and
// the chop with the mids, and a crest of spectrum runs across it (shape);
// the waves roll on the smooth energy clock (motion), and the wind's
// direction swings over eight bars (direction). Not a flight: the camera
// stays put.

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
    let size = max(size_w * fl / w.z, 1.6 / u.res_y);
    let p = sp + q * size;
    o.pos = vec4<f32>(p.x / aspect(), p.y, 0.0, 1.0);
    o.q = q;
    o.col = col;
    return o;
}

fn sea_h(x: f32, z: f32) -> f32 {
    let t = u.clock4.x * 0.35;
    let wind = 0.6 * sin(TAU * u.flow / 32.0);
    let d1 = vec2<f32>(sin(wind), cos(wind));
    let d2 = vec2<f32>(sin(wind + 1.1), cos(wind + 1.1));
    let pxz = vec2<f32>(x, z);
    let swell = (0.12 + 0.22 * u.pres4.x) * sin(dot(pxz, d1) * 0.9 - t);
    let chop = (0.04 + 0.08 * u.pres4.y) * sin(dot(pxz, d2) * 2.3 - t * 1.6 + 1.0);
    // A crest whose height across the sea is the spectrum.
    let s = spec(clamp(x / 8.0 + 0.5, 0.0, 1.0));
    let crest = 0.25 * sqrt(clamp(s, 0.0, 1.0)) * exp(-(sin(z * 0.35 - t * 0.5) - 0.9) * (sin(z * 0.35 - t * 0.5) - 0.9) * 6.0);
    return swell + chop + crest;
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    let gx = f32(ii % 400u);
    let gz = f32(ii / 400u);
    let jit = hash22(vec2<f32>(gx, gz)) - 0.5;
    // Rows packed closer near the camera, wider far off.
    let fz = (gz + jit.y * 0.6) / 300.0;
    let z = 0.1 + fz * fz * 14.0;
    let x = ((gx + jit.x * 0.6) / 400.0 - 0.5) * (3.0 + z * 2.6);
    let y = sea_h(x, z);
    // Camera 1.1 up, pitched gently down.
    var w = vec3<f32>(x, y - 1.1, z + 0.6);
    let wp = rot(-0.35) * w.yz;
    w = vec3<f32>(w.x, wp.x, wp.y);
    let lift = clamp(y * 2.0 + 0.4, 0.0, 1.6);
    let c = palette(0.55 + 0.25 * lift + 0.03 * z);
    let far = exp(-z * 0.13) * smoothstep(14.1, 8.0, z);
    return emit(w, vi, 0.006, c * c * (0.12 + 0.3 * lift) * far);
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
