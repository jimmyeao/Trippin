// Fly through a curving tunnel whose walls are real boxes jutting inward at
// random depths, toward a light at the far end. Two looks, picked per cut:
// red monochrome, or dark metal with glowing neon edges and lit tiles.

const R0: f32 = 1.7;      // radius of the solid wall behind the blocks
const DEPTH: f32 = 0.6;   // how far blocks can jut inward
const N: f32 = 22.0;      // blocks around the circumference
const L: f32 = 0.5;       // block length along the tunnel

fn neon_look() -> bool {
    return (floor(u.seed) % 2.0) > 0.5;
}

// The tunnel's centre line wanders so the flight curves.
fn centre(z: f32) -> vec2<f32> {
    return vec2<f32>(sin(z * 0.13) * 1.1, cos(z * 0.09) * 0.7);
}

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

struct Block {
    d: f32,
    id: vec2<f32>,
    local: vec3<f32>,
    half: vec3<f32>,
    depth: f32,   // 0..1 how far this block juts in
};

// Distance to the nearest block among the 3x3 neighbouring cells.
fn blocks(p: vec3<f32>) -> Block {
    let q = p.xy - centre(p.z);
    let cell = vec2<f32>(floor((angle(q) / TAU + 0.5) * N), floor(p.z / L));
    var best: Block;
    best.d = 1e9;
    for (var i = -1; i <= 1; i++) {
        for (var j = -1; j <= 1; j++) {
            let id = cell + vec2<f32>(f32(i), f32(j));
            let a = ((id.x + 0.5) / N - 0.5) * TAU;
            let radial = vec2<f32>(cos(a), sin(a));
            let tangent = vec2<f32>(-radial.y, radial.x);
            let h = hash21(vec2<f32>(((id.x % N) + N) % N, id.y));
            let inner = R0 - DEPTH * h;
            // Box from its inner face out past the wall.
            let half = vec3<f32>(PI * R0 / N * 0.97, (R0 + 0.3 - inner) * 0.5, L * 0.485);
            let mid_r = inner + half.y;
            let local = vec3<f32>(dot(q, tangent), dot(q, radial) - mid_r, p.z - (id.y + 0.5) * L);
            let d = sd_box(local, half);
            if d < best.d {
                best.d = d;
                best.id = id;
                best.local = local;
                best.half = half;
                best.depth = h;
            }
        }
    }
    return best;
}

fn dist(p: vec3<f32>) -> f32 {
    let wall = R0 - length(p.xy - centre(p.z));
    return min(blocks(p).d, wall);
}

fn normal_at(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.002, 0.0);
    return normalize(vec3<f32>(
        dist(p + e.xyy) - dist(p - e.xyy),
        dist(p + e.yxy) - dist(p - e.yxy),
        dist(p + e.yyx) - dist(p - e.yyx)));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.flow * 1.6;
    let ro = vec3<f32>(centre(z), z);
    let look_at = vec3<f32>(centre(z + 3.0), z + 3.0);
    let fw = normalize(look_at - ro);
    let rt = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), fw));
    let up = cross(fw, rt);
    let sp = rot(sin(u.time * 0.2) * 0.15) * vec2<f32>(p.x, -p.y);
    let rd = normalize(fw * 1.4 + rt * sp.x + up * sp.y);

    var t = 0.0;
    var hit = false;
    var steps = 0.0;
    for (var i = 0; i < 110; i++) {
        let d = dist(ro + rd * t);
        steps = f32(i);
        if d < 0.001 * (1.0 + t) {
            hit = true;
            break;
        }
        t += d * 0.9;
        if t > 28.0 {
            break;
        }
    }

    // Brightness follows the track's overall energy slowly; no beat flashes.
    let pulse = 0.85 + 0.3 * u.intensity;
    let neon = neon_look();
    let far_glow = select(vec3<f32>(0.9, 0.3, 0.25), vec3<f32>(0.25, 0.08, 0.45), neon);
    var col = far_glow * pulse;

    if hit {
        let pos = ro + rd * t;
        let n = normal_at(pos);
        let b = blocks(pos);
        // Warm light down the tunnel plus a dim headlight from the camera.
        let far_l = vec3<f32>(centre(z + 8.0), z + 8.0) - pos;
        let fl = length(far_l);
        let diff_far = max(dot(n, far_l / fl), 0.0) * 4.0 / (1.0 + fl * fl * 0.1);
        let diff_cam = max(dot(n, -rd), 0.0) * 0.12;
        let occ = 1.0 - steps / 110.0;                   // many steps = tucked in a crevice
        let light = (diff_far + diff_cam) * (0.4 + 0.6 * occ);
        if neon {
            col = vec3<f32>(0.06, 0.05, 0.14) * (0.15 + light * 0.8);
            // Glowing box edges: near two faces at once.
            let dd = b.half - abs(b.local);
            let mid = dd.x + dd.y + dd.z - min(dd.x, min(dd.y, dd.z)) - max(dd.x, max(dd.y, dd.z));
            let side = 0.5 + 0.5 * sin(angle(pos.xy - centre(pos.z)) + u.hue * TAU);
            let edge_col = mix(vec3<f32>(1.0, 0.12, 0.02), vec3<f32>(0.04, 0.25, 1.0), side);
            col += edge_col * smoothstep(0.02, 0.0, mid) * 0.9 * pulse;
            // A few blocks lit from within, flashing on the beat.
            if hash21(b.id + 17.0) > 0.95 && b.d < 0.01 {
                col += vec3<f32>(1.0, 0.32, 0.0) * (0.6 + 0.4 * u.intensity);
            }
        } else {
            let tone = 0.85 + 0.3 * hash21(b.id + 3.0);
            col = vec3<f32>(0.6, 0.07, 0.06) * (0.05 + light * 0.9) * tone;
        }
        // Haze toward the light at the end of the tunnel.
        col = mix(col, far_glow * pulse, 1.0 - exp(-t * 0.028));
    }
    col += prev(in.uv) * 0.1;
    return vec4<f32>(col, 1.0);
}
