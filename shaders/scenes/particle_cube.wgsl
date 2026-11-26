// @particles 64000 @bloom 0.5 @tonemap agx
// @title Particle Cube
// Particle Cube: a wireframe lattice of 64,000 points that twists layer by layer,
// one way and then the other over eight bars (direction). A wave of the bass
// presence ripples out from the centre through the lattice, pushing it into
// a rounded swell (shape), and the cube turns and the ripple travels on the
// smooth energy clock (motion). Stateless.

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
    // 48 lattice lines (4x4 along each axis), 1333 points each.
    let line = min(ii / 1333u, 47u);
    let t = f32(ii % 1333u) / 1332.0;
    let axis = line / 16u;
    let la = f32((line % 16u) / 4u) / 3.0;
    let lb = f32(line % 4u) / 3.0;
    var g = vec3<f32>(t, la, lb);
    if axis == 1u {
        g = vec3<f32>(la, t, lb);
    } else if axis == 2u {
        g = vec3<f32>(la, lb, t);
    }
    var w = (g - 0.5) * 1.9;
    // Twist each horizontal layer by its height.
    let twist = 1.0 * sin(TAU * u.flow / 32.0);
    let tw = rot(w.y * twist) * w.xz;
    w = vec3<f32>(tw.x, w.y, tw.y);
    // A ripple from the centre, its height the bass presence.
    let r = length(w);
    let wave = sin(r * 5.0 - u.clock4.x * 0.8);
    w *= 1.0 + (0.03 + 0.07 * u.pres4.x) * wave * smoothstep(0.0, 0.4, r);
    // Turn on the energy clock.
    let wy = rot(u.clock4.x * 0.08 + 0.6) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.45 + 0.1 * sin(TAU * u.flow / 64.0)) * w.yz;
    w = vec3<f32>(w.x, wp.x, wp.y + 3.9);
    let c = palette(0.1 + 0.35 * g.y + 0.15 * wave);
    let b = (0.22 + 0.2 * max(wave, 0.0)) * clamp(5.0 - w.z, 0.4, 1.6);
    return emit(w, vi, 0.006, c * c * b);
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
    col += vec3<f32>(0.004, 0.004, 0.01) * (1.0 - 0.4 * length(p));
    return vec4<f32>(finite(col), 1.0);
}
