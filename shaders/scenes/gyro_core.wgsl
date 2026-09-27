// @heavy — raymarched. A gyroscopic reactor: three nested rings spinning on
// different axes around a pulsing core, haloed in their own light. Ring
// speed follows the tempo; the core swells on the kick.

fn sd_torus(p: vec3<f32>, r: f32, t: f32) -> f32 {
    return length(vec2<f32>(length(p.xz) - r, p.y)) - t;
}

struct Hit {
    d: f32,
    part: f32, // 0..3 which ring, 4 core
};

fn map(p: vec3<f32>) -> Hit {
    var h: Hit;
    h.part = -1.0;
    // Core orb, breathing with the kick.
    h.d = length(p) - (0.42 + 0.06 * u.kick);
    h.part = 4.0;
    // Ring 0: equatorial, spins fastest.
    var q = p;
    q = vec3<f32>(q.x, rot(u.flow * 0.9) * q.yz);
    var d = sd_torus(q, 1.05, 0.055);
    if d < h.d { h.d = d; h.part = 0.0; }
    // Ring 1: tilted 60 degrees, counter-spinning.
    q = vec3<f32>(rot(1.05) * p.xy, p.z);
    q = vec3<f32>(q.x, rot(-u.flow * 0.6) * q.yz);
    d = sd_torus(q, 1.35, 0.045);
    if d < h.d { h.d = d; h.part = 1.0; }
    // Ring 2: steep tilt, slow stately spin.
    q = vec3<f32>(rot(2.3) * p.xy, p.z);
    q = vec3<f32>(rot(u.flow * 0.35) * q.xz, q.y);
    d = sd_torus(q, 1.65, 0.04);
    if d < h.d { h.d = d; h.part = 2.0; }
    return h;
}

fn normal_at(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.004, 0.0);
    return normalize(vec3<f32>(
        map(p + e.xyy).d - map(p - e.xyy).d,
        map(p + e.yxy).d - map(p - e.yxy).d,
        map(p + e.yyx).d - map(p - e.yyx).d));
}

fn part_col(part: f32) -> vec3<f32> {
    return palette(part * 0.22 + u.hue * 0.5);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let drift = u.time * 0.1;
    let ro = vec3<f32>(sin(drift) * 3.4, 1.1 + sin(u.time * 0.15) * 0.3, cos(drift) * 3.4);
    let fw = normalize(-ro);
    let rt = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), fw));
    let up = cross(fw, rt);
    // centred() has +y pointing down the screen — negate so up is up.
    let rd = normalize(fw * 1.35 + rt * p.x - up * p.y);

    var t = 0.0;
    var glow = vec3<f32>(0.0);
    var hit = false;
    var h: Hit;
    for (var i = 0; i < 80; i++) {
        h = map(ro + rd * t);
        if h.part >= 0.0 {
            glow += part_col(h.part) * exp(-max(h.d, 0.0) * 26.0) * 0.008;
        }
        if h.d < 0.0015 * (1.0 + t) {
            hit = true;
            break;
        }
        t += clamp(h.d * 0.8, 0.008, 0.4);
        if t > 9.0 {
            break;
        }
    }

    var col = glow * (0.7 + 0.9 * u.intensity);
    if hit {
        let pos = ro + rd * t;
        let n = normal_at(pos);
        let base = part_col(h.part);
        let lit_dir = normalize(vec3<f32>(0.5, 0.8, 0.6));
        let diff = 0.3 + 0.7 * max(dot(n, lit_dir), 0.0);
        if h.part == 4.0 {
            // The core: molten, bright, kicked.
            col += mix(vec3<f32>(1.0, 0.8, 0.5), base, 0.5)
                 * (0.9 + 0.8 * u.kick + 0.3 * diff);
        } else {
            // Rings: brushed metal catching their own colour.
            let fres = pow(1.0 - max(dot(n, -rd), 0.0), 3.0);
            col += base * diff * 0.35 + base * fres * (0.8 + u.high * 0.6);
        }
    }
    col *= exp(-t * 0.08);
    col += palette(0.6) * exp(-length(p) * 3.0) * 0.07 * (0.4 + u.energy);
    col += prev(uncentred(centred(in.uv) * 0.99)) * 0.12;
    return vec4<f32>(col, 1.0);
}
