// @particles 72000 @bloom 0.55 @tonemap agx
// @title Particle Dandelion
// Particle Dandelion: a glowing dandelion clock of 72,000 motes, 600 seeds on
// fine stalks, with seeds breaking loose and drifting away on the wind and
// regrowing. The head swells with the slow bass presence (shape), it turns
// and the seeds drift on the smooth energy clock (motion), and the wind
// swings from one side to the other over eight bars (direction).

const SEEDS: u32 = 600u;

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
    let seed = ii % SEEDS;
    let k = ii / SEEDS; // 0..119: 0-69 the stalk, 70-119 the tuft
    let fs = f32(seed);
    // Seed directions: a Fibonacci sphere.
    let y = 1.0 - 2.0 * (fs + 0.5) / f32(SEEDS);
    let rr = sqrt(max(1.0 - y * y, 0.0));
    let phi = fs * 2.399963;
    let dir = vec3<f32>(cos(phi) * rr, y, sin(phi) * rr);
    let hs = hash22(vec2<f32>(fs, 3.0));
    let h = hash22(vec2<f32>(f32(ii % 1024u), f32(ii / 1024u)) + vec2<f32>(1.3, 8.8));
    // The head pumps with the kick.
    let kick = kick_swell();
    let head = 0.75 * (1.0 + 0.08 * u.pres4.x + 0.12 * kick);
    // Seed life: grows in, stays, then breaks loose and drifts away.
    let ph = u.clock4.x * 0.02 + hs.x;
    let life = fract(ph);
    let loose = smoothstep(0.72, 1.0, life);
    let grow = smoothstep(0.0, 0.1, life);
    var w: vec3<f32>;
    var bri = 0.0;
    var c = vec3<f32>(0.0);
    if k < 70u {
        // The stalk, from near the centre to the tip; it stays when the seed
        // leaves.
        let t = f32(k) / 70.0;
        w = dir * mix(0.12, head, t) * (0.9 + 0.1 * grow);
        w += (vec3<f32>(h.x, h.y, fract(h.x * 9.1)) - 0.5) * 0.004;
        bri = 0.05;
        c = palette(0.3 + 0.1 * t);
    } else {
        // The tuft: an umbrella of fine rays at the tip.
        let a = h.x * TAU;
        let side = normalize(cross(dir, vec3<f32>(0.0, 1.0, 0.3)));
        let up = cross(dir, side);
        // The tufts open with the highs and on each kick.
        let open = 1.0 + 0.9 * u.lvl4.w + 1.2 * kick;
        let ray = (cos(a) * side + sin(a) * up) * h.y * 0.09 * open + dir * h.y * 0.05;
        var tip = dir * head * grow + ray * grow;
        // Loose: carried off on the wind, rising and tumbling.
        let wind = sin(TAU * u.flow / 32.0);
        let drift = loose * loose * 3.2;
        tip += vec3<f32>(wind * drift, drift * 0.35 + 0.15 * sin(drift * 3.0 + hs.y * 6.0), (hs.y - 0.5) * drift);
        w = tip;
        bri = 0.12 * (1.0 - smoothstep(0.85, 1.0, life));
        c = palette(0.55 + 0.25 * hs.y);
    }
    // The head turns slowly on the energy clock.
    let wy = rot(u.clock4.x * 0.05) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.15) * w.yz;
    w = vec3<f32>(w.x, wp.x + 0.1, wp.y + 3.3);
    return emit(w, vi, 0.004, c * c * bri * clamp(4.2 - w.z, 0.4, 1.6));
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
    // The stem, down from the head, and a soft glow behind the head.
    let stem = exp(-abs(p.x + 0.02 * p.y) * 300.0) * step(0.1, p.y) * 0.12;
    col += palette(0.3) * palette(0.3) * stem;
    col += palette(0.6) * palette(0.6) * exp(-length(p + vec2<f32>(0.0, 0.05)) * 3.5) * 0.03;
    return vec4<f32>(finite(col), 1.0);
}
