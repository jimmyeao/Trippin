// Flight through a ray-marched fractal: a folded, sphere-inverted structure
// repeated along the flight path, with a tunnel carved through it. Glow
// accumulates along each ray; speed follows the beat.

fn fractal(p0: vec3<f32>) -> f32 {
    var p = p0;
    // Repeat along the flight axis so the tunnel never ends.
    p.z = ((p.z % 4.0) + 4.0) % 4.0 - 2.0;
    var scale = 1.0;
    for (var i = 0; i < 6; i++) {
        p = abs(p) - vec3<f32>(0.9, 1.1, 0.7);
        let r2 = max(dot(p, p), 1e-3);
        let k = clamp(1.4 / r2, 0.5, 2.2);
        p *= k;
        scale *= k;
        let xy = rot(0.45 + 0.1 * u.mid) * p.xy;
        p = vec3<f32>(xy, p.z);
    }
    // Box-ish distance gives architectural struts rather than blobs.
    let q = abs(p) - vec3<f32>(0.6, 0.15, 0.6);
    return (length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0)) / scale;
}

fn scene_dist(p: vec3<f32>) -> f32 {
    // Carve the flight tunnel out of the fractal.
    let tunnel = 0.55 + 0.08 * sin(p.z * 0.7);
    return max(fractal(p), tunnel - length(p.xy));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let travel = u.flow * 1.2;
    let ro = vec3<f32>(0.0, 0.0, travel);
    var rd = normalize(vec3<f32>(p * 0.8, 1.0));
    let roll = rot(u.time * 0.1 + travel * 0.05);
    rd = vec3<f32>(roll * rd.xy, rd.z);

    var t = 0.0;
    var glow = vec3<f32>(0.0);
    var hit = false;
    var steps = 0.0;
    for (var i = 0; i < 80; i++) {
        let pos = ro + rd * t;
        let d = scene_dist(pos);
        // Squared palette = deeper, more saturated colour than the raw cosine palette.
        let pc = palette(pos.z * 0.05 + f32(i) * 0.01);
        glow += pc * pc * exp(-abs(d) * 40.0) * 0.008;
        steps = f32(i);
        if d < 0.002 * t {
            hit = true;
            break;
        }
        t += d * 0.7;
        if t > 20.0 {
            break;
        }
    }
    var col = glow * (0.7 + 0.8 * u.intensity);
    if hit {
        let pos = ro + rd * t;
        // Rays that needed many steps ended in crevices: shade them darker.
        let pc = palette(pos.z * 0.08 + 0.3);
        col += pc * pc * 0.6 * (1.0 - steps / 80.0) * exp(-t * 0.15);
    }
    col *= exp(-t * 0.06);
    col += prev(uncentred(p * 0.99)) * 0.15;
    return vec4<f32>(col, 1.0);
}
