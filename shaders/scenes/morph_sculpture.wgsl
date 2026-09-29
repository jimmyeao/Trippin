// @heavy — Synesthesia-style abstract. @bloom 0.7 @tonemap agx
// A polished sculpture floating in a dark studio, continuously morphing
// between forms — twisted torus knot-ish ring, rounded octahedral star,
// gyroid-carved sphere, stacked lotus of tori — each blended smoothly into
// the next over 8 beats. Half chrome, half iridescent glass: the surface
// reflects a soft studio environment and a thin-film sheen rolls over it.
// Audio vocabulary:
//  - morph progress and rotation on the mid energy clock (lingers on each
//    form in breakdowns, flows faster on drops);
//  - surface ripples driven by high presence; the whole form swells with
//    bass presence; rim light flares on bass hits.

fn sd_torus(p: vec3<f32>, r: f32, t: f32) -> f32 {
    return length(vec2<f32>(length(p.xz) - r, p.y)) - t;
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

fn form(p: vec3<f32>, i: i32) -> f32 {
    let k = ((i % 4) + 4) % 4;
    if k == 0 {
        // Twisted ring: a torus whose cross-section rotates around it.
        let a = angle(p.xz);
        var q = vec2<f32>(length(p.xz) - 0.85, p.y);
        q = rot(a * 1.5) * q;
        let c = abs(q) - vec2<f32>(0.28, 0.1);
        return length(max(c, vec2<f32>(0.0))) + min(max(c.x, c.y), 0.0) - 0.05;
    }
    if k == 1 {
        // Rounded octahedral star.
        let q = abs(p);
        let oct = (q.x + q.y + q.z - 1.0) * 0.57735;
        return smin(oct, length(p) - 0.62, 0.25) - 0.02;
    }
    if k == 2 {
        // Gyroid-carved sphere shell.
        let gy = abs(dot(sin(p * 5.0), cos(p.yzx * 5.0))) / 5.0 - 0.03;
        return max(length(p) - 0.95, gy);
    }
    // Lotus: three tori stacked and tilted.
    var d = sd_torus(p, 0.7, 0.12);
    let p2 = vec3<f32>(rot(1.05) * p.xy, p.z);
    d = smin(d, sd_torus(p2, 0.7, 0.12), 0.15);
    let p3 = vec3<f32>(rot(-1.05) * p.xy, p.z);
    return smin(d, sd_torus(p3, 0.7, 0.12), 0.15);
}

fn map(p_in: vec3<f32>) -> f32 {
    // Rotation on the mid clock.
    var p = p_in;
    let r1 = rot(u.clock4.z * 0.05) * p.xz;
    p = vec3<f32>(r1.x, p.y, r1.y);
    let r2 = rot(u.clock4.z * 0.031) * p.xy;
    p = vec3<f32>(r2, p.z);
    p /= 1.0 + 0.08 * u.pres4.x;
    // Morph: 8 beats per form (on the mid clock), with a smooth blend.
    let m = u.clock4.z / 8.0;
    let i = i32(floor(m));
    let f = smoothstep(0.55, 1.0, fract(m));
    var d = mix(form(p, i), form(p, i + 1), f);
    // Fine surface ripple from high presence.
    d += sin(p.x * 14.0 + u.clock4.w * 0.5) * sin(p.y * 14.0) * sin(p.z * 14.0) * 0.012 * u.pres4.w;
    return d * (1.0 + 0.08 * u.pres4.x) * 0.8;
}

fn normal(p: vec3<f32>) -> vec3<f32> {
    let e = 0.0015;
    let k = vec2<f32>(1.0, -1.0);
    return normalize(k.xyy * map(p + k.xyy * e) + k.yyx * map(p + k.yyx * e) + k.yxy * map(p + k.yxy * e) + k.xxx * map(p + k.xxx * e));
}

// Soft studio environment: dark gradient, a big overhead softbox, two
// coloured strips (palette) and a warm kicker.
fn env(d: vec3<f32>) -> vec3<f32> {
    var c = mix(vec3<f32>(0.005, 0.005, 0.01), vec3<f32>(0.05, 0.05, 0.07), d.y * 0.5 + 0.5);
    c += vec3<f32>(1.0, 0.97, 0.92) * 2.0 * smoothstep(0.8, 0.95, d.y);
    let az = angle(d.xz);
    let s1 = smoothstep(0.12, 0.04, abs(fract(az / TAU + 0.1) - 0.5)) * smoothstep(0.7, 0.3, abs(d.y));
    let s2 = smoothstep(0.08, 0.02, abs(fract(az / TAU + 0.6) - 0.5)) * smoothstep(0.7, 0.3, abs(d.y));
    c += palette(0.1 + u.hue) * s1 * 2.5 + palette(0.6 + u.hue) * s2 * 2.5;
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let ang = u.clock4.x * 0.015 + u.seed;
    let ro = vec3<f32>(sin(ang) * 2.6, 0.4 + 0.3 * sin(u.clock4.x * 0.011), cos(ang) * 2.6);
    let rd = cam_ray(p, ro, vec3<f32>(0.0), 0.0, 1.85);
    // Bounding sphere.
    let b = dot(ro, rd);
    let c = dot(ro, ro) - 1.6 * 1.6;
    let h = b * b - c;
    // Background: dark gradient + a soft palette glow behind the sculpture
    // (the studio lights only appear in reflections).
    var col = mix(vec3<f32>(0.004, 0.004, 0.008), vec3<f32>(0.02, 0.02, 0.03), rd.y * 0.5 + 0.5)
        + palette(0.55 + u.hue) * 0.06 * exp(-length(p) * 1.8) * (0.6 + 0.6 * u.pres4.x);
    if h > 0.0 {
        var t = max(-b - sqrt(h), 0.0);
        let tend = -b + sqrt(h);
        var hit = false;
        for (var i = 0; i < 90; i++) {
            let d = map(ro + rd * t);
            if d < 0.0005 {
                hit = true;
                break;
            }
            t += d;
            if t > tend {
                break;
            }
        }
        if hit {
            let pos = ro + rd * t;
            let n = normal(pos);
            let r = reflect(rd, n);
            let fr = fresnel(0.5, dot(n, -rd));
            // Thin-film sheen rolling over the surface.
            let film = 0.5 + 0.5 * cos(TAU * (dot(n, rd) * 1.5 + u.clock4.w * 0.02 + vec3<f32>(0.0, 0.33, 0.67)));
            let ao = clamp(map(pos + n * 0.12) / 0.12, 0.2, 1.0);
            col = env(r) * mix(vec3<f32>(0.95), film, 0.55) * (fr * 1.6 + 0.1) * ao;
            let rim = pow(1.0 - max(dot(n, -rd), 0.0), 4.0);
            col += palette(0.3 + u.hue) * rim * (0.3 + 2.0 * u.hits4.x) * (0.6 + 0.6 * u.intensity);
        }
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
