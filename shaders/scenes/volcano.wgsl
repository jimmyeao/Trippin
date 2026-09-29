// @heavy — 2026 tier. @bloom 0.9 @tonemap agx
// A volcano at night: lava rivers wind down black slopes, an ash plume
// towers out of the crater lit orange from beneath, volcanic lightning
// crackles inside the plume, and on the drops the crater throws up
// fountains of glowing lava bombs.
// Heightfield march (72 steps) for the terrain, a 20-step volume march
// through the plume cylinder, and ballistic lava bombs that are pure poses
// of the beat phase (launched on each beat, never accumulated).
// The camera circles slowly on the tempo clock; the lava breathes with the
// bass, lightning fires on onsets, eruptions scale with intensity.

const VC: vec2<f32> = vec2<f32>(0.0, 0.0);   // volcano centre (xz)
const SUMMIT: f32 = 20.0;
const BASE_R: f32 = 45.0;

fn terrain(x: vec2<f32>) -> f32 {
    let r = length(x - VC);
    let cone = SUMMIT * pow(max(1.0 - r / BASE_R, 0.0), 1.4);
    let crater = smoothstep(5.5, 2.0, r) * 4.5;
    let n = tnoise(vec3<f32>(x * 0.02, 0.2));
    let ridges = (1.0 - abs(n.b * 2.0 - 1.0)) * 2.5 * smoothstep(BASE_R * 1.2, 8.0, r);
    let rough = (tnoise(vec3<f32>(x * 0.12, 0.7)).r - 0.22) * 1.2;
    return cone - crater + ridges + rough;
}

// Lava channels: isolines of a noise field running downhill, brighter
// toward the vent; the glow texture flows outward over time.
fn lava(x: vec2<f32>) -> f32 {
    let d = x - VC;
    let r = length(d);
    let a = angle(d);
    let ca = vec2<f32>(cos(a), sin(a));
    let n = tnoise(vec3<f32>(ca * 1.3, r * 0.012)).b;
    let chan = smoothstep(0.012, 0.0, abs(n - 0.5) - 0.004 * (1.0 + r * 0.02));
    let flow_tex = tnoise(vec3<f32>(ca * 3.0, r * 0.05 - u.time * 0.02)).g;
    let near = smoothstep(BASE_R * 0.9, 6.0, r);
    let pool = smoothstep(4.0, 1.5, r);
    return (chan * near * (0.4 + 0.9 * flow_tex) + pool * 1.5);
}

fn plume_density(p: vec3<f32>) -> f32 {
    let h = p.y - SUMMIT + 2.0;
    if h < 0.0 {
        return 0.0;
    }
    // Widening, leaning column.
    let lean = vec2<f32>(h * 0.25, h * 0.08);
    let r = length(p.xz - VC - lean);
    let w = 3.0 + h * 0.45;
    let n = tnoise(vec3<f32>(p.x, p.y - u.time * 0.8, p.z) * 0.04).r;
    let n2 = tnoise(p * 0.12 + vec3<f32>(0.0, -u.time * 0.1, 0.0)).g;
    return max((1.0 - r / w) * 1.4 + (n - 0.22) * 1.8 + (n2 - 0.5) * 0.4 - 0.3, 0.0) * smoothstep(60.0, 20.0, h);
}

fn sky(rd: vec3<f32>) -> vec3<f32> {
    let h = max(rd.y, 0.0);
    var c = mix(vec3<f32>(0.03, 0.012, 0.012), vec3<f32>(0.003, 0.003, 0.008), pow(h, 0.5));
    let g = rd * 220.0;
    let cell = floor(g);
    let r1 = hash22(cell.xy + cell.z * 17.3);
    let r2 = hash21(cell.yz + cell.x * 7.1);
    let sp = cell + vec3<f32>(r1.y, r2, fract(r2 * 13.1)) * 0.6 + 0.2;
    c += vec3<f32>(0.8, 0.85, 1.0) * step(0.92, r1.x) * (pow(r2, 8.0) * 1.5 + 0.03) * smoothstep(0.22, 0.0, length(g - sp)) * smoothstep(0.05, 0.2, rd.y);
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let ang = u.flow * 0.006 + u.seed;
    let rad = 62.0;
    let cxz = vec2<f32>(sin(ang), cos(ang)) * rad;
    let ro = vec3<f32>(cxz.x, max(terrain(cxz), 0.0) + 9.0, cxz.y);
    let ta = vec3<f32>(0.0, 16.0, 0.0);
    let rd = cam_ray(p, ro, ta, 0.0, 1.5);
    let lava_c = vec3<f32>(1.0, 0.32, 0.04);
    let drive = 0.5 + 0.8 * u.intensity;
    let lava_i = (0.7 + 1.3 * u.bass) * drive;

    // --- Terrain -----------------------------------------------------------
    var t = 1.0;
    var hit = false;
    for (var i = 0; i < 72; i++) {
        let q = ro + rd * t;
        let dh = q.y - terrain(q.xz);
        if dh < 0.01 * t {
            hit = true;
            break;
        }
        t += max(dh * 0.5, 0.05 + t * 0.01);
        if t > 260.0 {
            break;
        }
    }
    var col = sky(rd);
    if hit {
        let hp = ro + rd * t;
        let e = 0.15;
        let n = normalize(vec3<f32>(
            terrain(hp.xz - vec2<f32>(e, 0.0)) - terrain(hp.xz + vec2<f32>(e, 0.0)),
            2.0 * e,
            terrain(hp.xz - vec2<f32>(0.0, e)) - terrain(hp.xz + vec2<f32>(0.0, e))
        ));
        let rock = vec3<f32>(0.035, 0.03, 0.03) * (0.6 + 0.8 * tnoise(hp * 0.3).b);
        // Lit by the glow of the crater and the lava, plus faint starlight.
        let to_vent = vec3<f32>(0.0, SUMMIT + 4.0, 0.0) - hp;
        let vd = length(to_vent);
        let vent = lava_c * max(dot(n, to_vent / vd), 0.0) * 180.0 / (vd * vd + 40.0) * lava_i;
        col = rock * (vec3<f32>(0.02, 0.022, 0.03) * (0.4 + 0.6 * n.y) + vent);
        let lv = lava(hp.xz);
        // Hot core yellow-white, crust edges deep red.
        let hot = mix(vec3<f32>(0.9, 0.12, 0.02), vec3<f32>(1.0, 0.75, 0.3), smoothstep(0.6, 1.4, lv));
        col += hot * lv * lava_i * 2.0;
    }
    // Low orange haze from the lava field.
    let haze = 1.0 - exp(-t * 0.006);
    col = mix(col, vec3<f32>(0.05, 0.018, 0.01) * drive, haze * smoothstep(0.3, -0.05, rd.y));

    // --- Ash plume -------------------------------------------------------------
    // Intersect the view ray with the plume's bounding cylinder.
    let oc = ro.xz - VC - vec2<f32>(8.0, 2.5);
    let a = dot(rd.xz, rd.xz);
    let b = dot(oc, rd.xz);
    let cc = dot(oc, oc) - 26.0 * 26.0;
    let disc = b * b - a * cc;
    if disc > 0.0 {
        let s0 = max((-b - sqrt(disc)) / a, 0.0);
        let s1 = min((-b + sqrt(disc)) / a, select(1e4, t, hit));
        if s1 > s0 {
            let steps = 20;
            let dt = (s1 - s0) / f32(steps);
            var s = s0 + dt * bluen(in.pos.xy);
            var trans = 1.0;
            var acc = vec3<f32>(0.0);
            // Volcanic lightning in the plume on onsets (per-beat position).
            let lh = hash22(vec2<f32>(floor(u.beat), 4.4));
            let lp = vec3<f32>((lh.x - 0.5) * 12.0 + 5.0, SUMMIT + 10.0 + lh.y * 20.0, (lh.y - 0.5) * 8.0);
            let lf = step(0.5, hash21(vec2<f32>(floor(u.beat), 2.2))) * exp(-u.beat_phase * 8.0) * smoothstep(0.3, 0.8, u.intensity + u.onset * 0.3);
            for (var i = 0; i < steps; i++) {
                let q = ro + rd * s;
                let d = plume_density(q);
                if d > 0.001 {
                    let hh = q.y - SUMMIT;
                    // Underlit by the crater: orange near the vent, grey above.
                    var l = lava_c * 1.6 * exp(-max(hh, 0.0) * 0.12) * lava_i + vec3<f32>(0.01, 0.01, 0.013);
                    l += vec3<f32>(0.7, 0.75, 1.0) * lf * 10.0 / (1.0 + dot(q - lp, q - lp) * 0.08);
                    let aa = 1.0 - exp(-d * dt * 0.35);
                    acc += trans * aa * l;
                    trans *= 1.0 - aa;
                    if trans < 0.03 {
                        break;
                    }
                }
                s += dt;
            }
            col = col * trans + acc;
        }
    }

    // --- Lava bombs ------------------------------------------------------------
    // Launched each beat from the crater; ballistic arcs over one beat.
    let erupt = smoothstep(0.55, 0.9, u.intensity);
    if erupt > 0.0 {
        let fwd = normalize(ta - ro);
        let rgt = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), fwd));
        let upv = cross(fwd, rgt);
        for (var k = 0; k < 36; k++) {
            let fk = f32(k);
            let h = hash22(vec2<f32>(fk, floor(u.beat)));
            let tt = u.beat_phase * (0.8 + 0.4 * h.y) * 3.0;
            let dir = vec3<f32>(cos(h.x * TAU) * 0.35 * h.y, 1.0, sin(h.x * TAU) * 0.35 * h.y);
            let v0 = 14.0 * (0.6 + 0.6 * hash21(vec2<f32>(fk, 3.0)));
            let wp = vec3<f32>(0.0, SUMMIT - 1.0, 0.0) + dir * v0 * tt + vec3<f32>(0.0, -4.9, 0.0) * tt * tt;
            // Project to the screen.
            let v = wp - ro;
            let z = dot(v, fwd);
            if z < 1.0 {
                continue;
            }
            let sp = vec2<f32>(dot(v, rgt), -dot(v, upv)) / z * 1.5;
            let d = length(p - sp);
            let r = 0.006 + 0.3 / z;
            let cool = smoothstep(0.0, 1.0, tt / 3.0);
            col += mix(vec3<f32>(1.0, 0.7, 0.25), lava_c, cool) * exp(-d * d / (r * r)) * (1.0 - cool * 0.7) * erupt * 1.5;
        }
    }
    col += (bluen(in.pos.xy + 7.0) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
