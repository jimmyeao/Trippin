// @heavy — 2026 tier. @bloom 0.9 @tonemap agx
// A moonlit forest at night: walking a path between tall trunks, mist
// pooling on the ground, moonbeams slanting through gaps in the canopy, and
// fireflies drifting among the trees, blinking on and off.
// Domain-repeated trunk SDF (one cell, clamped step) + a 20-sample march
// that shares its samples between the moonbeam scattering (canopy-gap test
// along the moon direction) and firefly glow (one candidate per 3D cell).
// The walk follows the tempo clock. Fireflies blink in time — a new set
// lights on every half beat, more of them with the hi-hats — and the mist
// glows with the mids.

const CELL: f32 = 4.0;
const CANOPY: f32 = 14.0;

fn moon_dir() -> vec3<f32> {
    return normalize(vec3<f32>(0.3, 1.0, 0.45));   // toward the moon
}

fn ground(x: vec2<f32>) -> f32 {
    return (tnoise(vec3<f32>(x * 0.05, 0.4)).b - 0.5) * 1.4;
}

fn trunk(p: vec3<f32>) -> f32 {
    let c = floor(p.xz / CELL);
    let h = hash22(c);
    let ctr = (c + 0.5 + (h - 0.5) * 0.6) * CELL;
    // Keep the path (x≈0) clear.
    if abs(ctr.x) < 2.8 || h.x < 0.15 {
        return 1.5;
    }
    let r = 0.22 + h.y * 0.3;
    let lean = (h - 0.5) * 0.04 * p.y;
    let bark = sin(angle(p.xz - ctr) * 14.0 + p.y * 0.7 + h.x * 20.0) * 0.012;
    return length(p.xz - ctr - lean) - r - bark;
}

fn map(p: vec3<f32>) -> vec2<f32> {
    let g = p.y - ground(p.xz);
    let tr = trunk(p);
    if tr < g {
        return vec2<f32>(tr, 1.0);
    }
    return vec2<f32>(g, 0.0);
}

// 1 where moonlight gets through the canopy above point q.
fn canopy_gap(q: vec3<f32>) -> f32 {
    let L = moon_dir();
    let s = (CANOPY - q.y) / L.y;
    let w = q.xz + L.xz * s;
    let n = tnoise(vec3<f32>(w * 0.035, 0.6)).g + (tnoise(vec3<f32>(w * 0.12, 0.2)).a - 0.5) * 0.25;
    return smoothstep(0.6, 0.68, n);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.clock4.x * 0.35;
    let ro = vec3<f32>(sin(u.flow * 0.02) * 0.6, ground(vec2<f32>(0.0, z)) + 1.6 + 0.04 * sin(u.flow * PI), z);
    // Direction: the gaze wanders from side to side along the path.
    let ta = ro + vec3<f32>(sin(u.clock4.x * 0.03) * 0.8, -0.05 + sin(u.clock4.x * 0.02) * 0.1, 1.0);
    let rd = cam_ray(p, ro, ta, 0.0, 1.3);
    let L = moon_dir();
    let moon_c = vec3<f32>(0.55, 0.65, 0.9);
    let drive = 0.5 + 0.8 * u.intensity;

    var t = 0.05;
    var m = -1.0;
    for (var i = 0; i < 64; i++) {
        let h = map(ro + rd * t);
        if h.x < 0.002 * t {
            m = h.y;
            break;
        }
        t += min(h.x * 0.85, 1.5);
        if t > 50.0 {
            break;
        }
    }
    var col = vec3<f32>(0.004, 0.006, 0.012);
    if m >= 0.0 {
        let hp = ro + rd * t;
        let e = 0.01;
        let k = vec2<f32>(1.0, -1.0);
        let n = normalize(k.xyy * map(hp + k.xyy * e).x + k.yyx * map(hp + k.yyx * e).x + k.yxy * map(hp + k.yxy * e).x + k.xxx * map(hp + k.xxx * e).x);
        let gap = canopy_gap(hp);
        var albedo = vec3<f32>(0.05, 0.045, 0.035) * (0.6 + 0.8 * tnoise(hp * 0.8).b);
        if m < 0.5 {
            // Leaf litter and moss.
            albedo = mix(vec3<f32>(0.04, 0.035, 0.025), vec3<f32>(0.02, 0.05, 0.025), tnoise(vec3<f32>(hp.xz * 0.3, 0.1)).b);
        }
        let moon = moon_c * max(dot(n, L), 0.0) * gap * 1.2;
        col = albedo * (moon + vec3<f32>(0.01, 0.014, 0.025));
    }
    // --- Mist, moonbeams and fireflies (shared samples) ------------------------
    let tmax = min(t, 40.0);
    let steps = 16;
    let jit = bluen(in.pos.xy);
    var beams = 0.0;
    var mist = 0.0;
    var flies = vec3<f32>(0.0);
    var last = vec3<f32>(1e6);
    for (var i = 0; i < steps; i++) {
        let s = (f32(i) + jit) / f32(steps) * tmax;
        let q = ro + rd * s;
        let hg = q.y - ground(q.xz);
        let dens = exp(-max(hg, 0.0) * 1.2) * (0.5 + tnoise(q * 0.08 + vec3<f32>(u.time * 0.01, 0.0, 0.0)).r * 1.4);
        mist += dens;
        beams += canopy_gap(q) * (0.3 + dens);
        // Firefly in this sample's 3D cell (each cell counted once).
        let c = floor(q / 3.0);
        if any(c != last) {
            last = c;
            let h = hash22(c.xz + c.y * 17.0);
            let exists = step(hash21(c.xz * 1.7 + c.y * 3.0), 0.7) * step(c.y * 3.0, ground(c.xz * 3.0) + 3.0);
            let fp = (c + vec3<f32>(h.x, 0.2 + 0.5 * h.y, fract(h.x * 7.3 + h.y))) * 3.0;
            // Shape + energy: the swarm loops wider and faster as the
            // highs build.
            let tw = u.clock4.w * 0.4;
            let fp2 = fp + vec3<f32>(sin(tw * 0.7 + h.x * 9.0), sin(tw * 0.5 + h.y * 7.0) * 0.4, cos(tw * 0.6 + h.y * 5.0)) * (0.4 + 0.8 * u.pres4.z);
            let v = fp2 - ro;
            let tt = dot(v, rd);
            if tt > 0.2 && tt < t {
                let d = length(v - rd * tt);
                // Blink: a new set lights each half beat; more with the hats.
                let blink_h = hash21(c.xz * 3.1 + c.y + floor(u.beat * 2.0) * 0.37);
                let on = step(blink_h, 0.35 + 0.5 * u.high) * (0.25 + 0.75 * exp(-fract(u.beat * 2.0) * 2.5));
                let r = max(0.045, tt * 0.0025);
                let glow = exp(-d * d / (r * r)) * 1.5 + exp(-d * 2.5) * 0.04;
                flies += vec3<f32>(0.75, 1.0, 0.3) * on * glow * (0.03 / r) * exists;
            }
        }
    }
    let stp = tmax / f32(steps);
    var fog = 1.0 - exp(-mist * stp * 0.06 - tmax * 0.04);
    if m < 0.0 {
        fog = 1.0;                                   // nothing hit: lost in the mist
    }
    let mist_c = vec3<f32>(0.03, 0.04, 0.06) * (0.6 + 0.8 * u.mid);
    col = mix(col, mist_c, fog);
    col += moon_c * beams * stp * 0.03 * (0.6 + 0.6 * u.intensity);
    col += flies * drive * 1.2;
    col += (bluen(in.pos.xy + 5.0) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
