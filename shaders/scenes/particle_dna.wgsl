// @particles 60000 @bloom 0.5 @tonemap agx
// @title Particle DNA
// Particle DNA: a double helix of 60,000 motes lying across the frame, with
// rungs of light between its strands. The helix's girth along its length is
// the spectrum, bass at the left and highs at the right (shape), it spins on
// the smooth energy clock (motion), and its axis sways and the camera swings
// round it over eight bars (direction). Stateless (render.rs `@particles`).

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

// Project a camera-space point (z = distance ahead) as one glow triangle.
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
    let h = hash22(cell + vec2<f32>(5.3, 1.1));
    let h2 = hash22(cell + vec2<f32>(2.9, 8.7));
    let kind = ii % 4u; // 0,1: strands; 2,3: rungs
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
    // The axis sways in a slow wave.
    w.y += 0.25 * sin(x * 0.7 + TAU * u.flow / 32.0);
    // Camera: yaws round the helix and back over eight bars.
    let yaw = 0.55 * sin(TAU * u.flow / 32.0 + 0.6);
    let wy = rot(yaw) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.18) * w.yz;
    w = vec3<f32>(w.x, wp.x, wp.y + 4.3);
    var c = palette(0.08 + 0.45 * f32(kind == 1u) + 0.1 * s);
    var b = 0.3;
    if kind >= 2u {
        c = palette(0.3 + 0.2 * h2.y);
        b = 0.14;
    }
    return emit(w, vi, 0.006, c * c * b * clamp(5.6 - w.z, 0.4, 1.5));
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
