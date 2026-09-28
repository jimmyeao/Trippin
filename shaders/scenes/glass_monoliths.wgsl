// @heavy — 2026 tier. @bloom 0.6 @tonemap agx
// Five tall slabs of optical glass turning slowly on a black mirror floor in
// front of a wall of light bars. Rays refract in, march through the glass,
// and split into R/G/B with slightly different indices at the exit, so edges
// throw rainbow fringes. The light bars behind are a spectrum display — the
// glass bends and splits the music.
// Slab rotation is a pose of the tempo clock; audio drives light only.

const N_SLABS: i32 = 5;
const HALF: vec3<f32> = vec3<f32>(0.42, 1.5, 0.17);

fn slab_xf(k: i32) -> vec4<f32> {
    // (x, z, yaw, unused)
    let fk = f32(k) - 2.0;
    let a = fk * 0.42;
    let yaw = fk * 0.4 + sin(u.flow * 0.03 + f32(k) * 1.3) * 0.7 + u.flow * 0.01;
    return vec4<f32>(sin(a) * 3.4, 3.4 - cos(a) * 3.4, yaw, 0.0);
}

fn rbox(p: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let q = abs(p) - b + r;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

fn map(p: vec3<f32>) -> f32 {
    var d = 1e3;
    for (var k = 0; k < N_SLABS; k++) {
        let x = slab_xf(k);
        var q = p - vec3<f32>(x.x, HALF.y, x.y);
        let r2 = rot(x.z) * q.xz;
        q = vec3<f32>(r2.x, q.y, r2.y);
        d = min(d, rbox(q, HALF, 0.06));
    }
    return d;
}

fn normal(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.001, 0.0);
    return normalize(vec3<f32>(
        map(p + e.xyy) - map(p - e.xyy),
        map(p + e.yxy) - map(p - e.yxy),
        map(p + e.yyx) - map(p - e.yyx)
    ));
}

// The world outside the glass: black stage, a wall of vertical light bars
// (a spectrum display), a soft top light, and the black mirror floor.
fn env(ro: vec3<f32>, rd: vec3<f32>) -> vec3<f32> {
    var c = vec3<f32>(0.004, 0.004, 0.006);
    // Back wall at z = 7.
    if rd.z > 0.0 {
        let t = (7.0 - ro.z) / rd.z;
        let w = ro + rd * t;
        if w.y > -0.01 {
            let bars = 24.0;
            let bx = (w.x + 8.0) / 16.0 * bars;
            let id = floor(bx);
            let f = fract(bx);
            let lvl = spec(clamp(id / bars, 0.0, 1.0) * 0.85);
            let hgt = 0.6 + lvl * 4.2 * (0.6 + 0.6 * u.intensity);
            let on = smoothstep(0.42, 0.3, abs(f - 0.5)) * smoothstep(hgt + 0.05, hgt - 0.05, w.y) * step(abs(w.x), 8.0);
            let bc = palette(id / bars * 0.8);
            c += bc * on * (1.2 + 1.5 * u.intensity) * (0.6 + 0.4 * smoothstep(0.0, hgt, w.y));
            // Wall glow above the bars.
            c += bc * 0.03 * exp(-abs(w.y - hgt) * 1.5) * step(abs(w.x), 8.5);
        }
    }
    // Soft top light.
    c += vec3<f32>(1.0, 0.98, 0.95) * 0.25 * smoothstep(0.75, 0.95, rd.y);
    return c;
}

// Stage without the glass: whichever of floor / wall the ray meets first;
// the floor is a black mirror of the wall.
fn world(ro: vec3<f32>, rd: vec3<f32>) -> vec3<f32> {
    let tw = select(1e5, (7.0 - ro.z) / rd.z, rd.z > 0.0);
    let tf = select(1e5, -ro.y / rd.y, rd.y < 0.0);
    if tf < tw {
        let fp = ro + rd * tf;
        let rr = vec3<f32>(rd.x, -rd.y, rd.z);
        return env(fp, rr) * (0.1 + fresnel(0.05, -rd.y) * 0.6) * exp(-tf * 0.04)
            + palette(0.4) * 0.02 * exp(-abs(fp.z - 7.0) * 0.6) * (0.5 + u.intensity);
    }
    return env(ro, rd);
}

fn march(ro: vec3<f32>, rd: vec3<f32>, steps: i32, tmax: f32) -> f32 {
    var t = 0.0;
    for (var i = 0; i < steps; i++) {
        let d = map(ro + rd * t);
        if d < 0.0005 {
            return t;
        }
        t += d;
        if t > tmax {
            break;
        }
    }
    return -1.0;
}

// Shade a glass hit: reflection + refraction through the slab with
// dispersion at the exit face.
fn glass(p: vec3<f32>, rd: vec3<f32>) -> vec3<f32> {
    let n = normal(p);
    let fr = fresnel(0.04, dot(-rd, n));
    let refl = world(p + n * 0.002, reflect(rd, n));
    // Enter.
    let ior = 1.5;
    let rin = refract(rd, n, 1.0 / ior);
    var q = p - n * 0.004;
    // March the interior (negated field) to the exit face.
    var t = 0.0;
    for (var i = 0; i < 24; i++) {
        let d = -map(q + rin * t);
        if d < 0.0005 {
            break;
        }
        t += max(d, 0.004);
    }
    let pe = q + rin * t;
    let ne = -normal(pe);
    var col = vec3<f32>(0.0);
    // Exit: separate indices per channel.
    let iors = vec3<f32>(1.47, 1.5, 1.535);
    for (var c = 0; c < 3; c++) {
        var ro2 = refract(rin, ne, iors[c]);
        if dot(ro2, ro2) < 0.01 {
            ro2 = reflect(rin, ne);                 // total internal reflection
        }
        let e = world(pe - ne * 0.004, ro2);
        col[c] = e[c];
    }
    // Faint green-ish absorption through the thickness, like real glass.
    col *= exp(-vec3<f32>(0.25, 0.08, 0.15) * t);
    return mix(col, refl, fr) + vec3<f32>(0.02, 0.022, 0.025) * pow(1.0 - abs(dot(rd, n)), 4.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let ang = sin(u.flow * 0.012) * 0.35;
    let ro = vec3<f32>(sin(ang) * 6.5, 1.35 + 0.2 * sin(u.flow * 0.017), -6.5 * cos(ang) + 1.5);
    let rd = cam_ray(p, ro, vec3<f32>(0.0, 1.2, 1.8), 0.0, 1.8);

    var col: vec3<f32>;
    let t = march(ro, rd, 80, 20.0);
    if t > 0.0 {
        col = glass(ro + rd * t, rd);
    } else {
        col = world(ro, rd);
        // Mirror floor also reflects the slabs.
        let tf = select(1e5, -ro.y / rd.y, rd.y < 0.0);
        if tf < (7.0 - ro.z) / rd.z {
            let fp = ro + rd * tf;
            let rr = vec3<f32>(rd.x, -rd.y, rd.z);
            let rt = march(fp + rr * 0.003, rr, 48, 12.0);
            if rt > 0.0 {
                let hp = fp + rr * (rt + 0.003);
                let n = normal(hp);
                let g = env(hp, refract(rr, n, 0.67)) * 0.8 + env(hp, reflect(rr, n)) * 0.1;
                col = g * (0.1 + fresnel(0.05, -rd.y) * 0.6) * exp(-tf * 0.04);
            }
        }
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
