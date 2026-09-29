// @heavy — raymarched. @bloom 0.8
// A gyroscopic reactor in the cube_spin neon style: five nested rings of
// glowing tube light spinning on different axes around a plasma core.
// Rings are drawn as neon lines — the ray tracks its closest approach to
// each ring and lights it with a tight core + wide halo (like cube_spin's
// edges), so they glow rather than shade. Light pulses chase round each
// ring driven by its own spectrum band; the core swirls, rim-glows and
// punches on the kick; sparks orbit it. Spin follows the tempo clock.

const NR: i32 = 5;

fn sd_torus(p: vec3<f32>, r: f32) -> f32 {
    return length(vec2<f32>(length(p.xz) - r, p.y));
}

// Ring k's local frame: tilt + spin (poses of the tempo clock).
fn ring_local(p: vec3<f32>, k: i32) -> vec3<f32> {
    let fk = f32(k);
    let tilt = fk * 0.63 + 0.2;
    let spin = u.flow * (0.25 + 0.12 * fk) * select(1.0, -1.0, (k & 1) == 1);
    var q = vec3<f32>(rot(tilt) * p.xy, p.z);
    q = vec3<f32>(rot(fk * 1.1) * q.xz, q.y).xzy;
    q = vec3<f32>(rot(spin) * q.xz, q.y).xzy;
    return q;
}

fn ring_r(k: i32) -> f32 {
    return 0.95 + f32(k) * 0.26;
}

fn core_r() -> f32 {
    return 0.42 + 0.07 * u.kick;
}

fn ring_col(k: i32) -> vec3<f32> {
    return palette(f32(k) * 0.2 + u.hue);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let drift = u.flow * 0.02;
    let ro = vec3<f32>(sin(drift) * 4.2, 1.2 + sin(u.flow * 0.013) * 0.5, cos(drift) * 4.2);
    let rd = cam_ray(p, ro, vec3<f32>(0.0), 0.0, 1.5);

    // Closest approach of the ray to each ring (before hitting the core),
    // and the ring angle there (for the chasing pulses).
    var dmin = array<f32, 5>(9.0, 9.0, 9.0, 9.0, 9.0);
    var amin = array<f32, 5>(0.0, 0.0, 0.0, 0.0, 0.0);
    var t = 0.0;
    var hit_core = false;
    for (var i = 0; i < 56; i++) {
        let pos = ro + rd * t;
        var d = length(pos) - core_r();
        if d < 0.002 {
            hit_core = true;
            break;
        }
        for (var k = 0; k < NR; k++) {
            let q = ring_local(pos, k);
            let dr = sd_torus(q, ring_r(k));
            if dr < dmin[k] {
                dmin[k] = dr;
                amin[k] = angle(q.xz);
            }
            d = min(d, dr);
        }
        // Small steps near the rings so the minimum is accurate.
        t += clamp(d * 0.75, 0.006, 0.25);
        if t > 9.0 {
            break;
        }
    }

    var col = vec3<f32>(0.006, 0.006, 0.016);
    // Faint background dust.
    let g = rd * 90.0;
    let cell = floor(g);
    let rh = hash22(cell.xy + cell.z * 13.1);
    col += vec3<f32>(0.6, 0.7, 1.0) * step(0.93, rh.x) * smoothstep(0.3, 0.0, length(fract(g) - 0.5)) * 0.08;

    // Core.
    if hit_core {
        let pos = ro + rd * t;
        let n = normalize(pos);
        let fres = pow(1.0 - max(dot(n, -rd), 0.0), 2.5);
        let swirl = tnoise(vec3<f32>(n * 1.4) + vec3<f32>(u.flow * 0.05, 0.0, 0.0)).r;
        let hot = mix(vec3<f32>(1.0, 0.85, 0.6), palette(0.1 + u.hue), 0.4);
        col = hot * (0.15 + 1.1 * swirl * swirl) * (0.6 + 1.2 * u.kick) + palette(0.3 + u.hue) * fres * 2.0;
    }
    // Core halo (closest approach to the centre).
    let tc = max(dot(-ro, rd), 0.0);
    let dc = length(ro + rd * tc);
    col += mix(vec3<f32>(1.0, 0.8, 0.5), palette(0.1 + u.hue), 0.5) * exp(-max(dc - core_r(), 0.0) * 4.0) * (0.15 + 0.5 * u.kick);

    // Neon rings (behind the core only where the ray passed them before it).
    let drive = 0.5 + 0.9 * u.intensity;
    for (var k = 0; k < NR; k++) {
        let d = dmin[k];
        let lvl = 0.3 + 1.5 * spec(f32(k) / f32(NR) * 0.85);
        // Pulses chasing round the ring, speed with the tempo.
        let chase = pow(0.5 + 0.5 * sin(amin[k] * 4.0 - u.flow * PI * (1.0 + f32(k) * 0.25)), 6.0);
        let line = exp(-d * 180.0) * 1.3 + exp(-d * 22.0) * 0.22;
        col += ring_col(k) * line * (0.15 + lvl * (0.12 + 2.4 * chase)) * drive;
    }

    // Sparks orbiting the core.
    for (var s = 0; s < 24; s++) {
        let fs = f32(s);
        let h = hash22(vec2<f32>(fs, 3.1));
        let r = 0.6 + h.x * 2.0;
        let a = u.flow * (0.3 + h.y * 0.8) + fs * 2.4;
        let sp = vec3<f32>(cos(a) * r, (h.y - 0.5) * 1.6 * sin(a * 0.7 + fs), sin(a) * r);
        let v = sp - ro;
        let st = dot(v, rd);
        if st > 0.0 && (!hit_core || st < t) {
            let sd = length(v - rd * st);
            col += palette(h.x + u.hue) * exp(-sd * sd * 4000.0) * (0.4 + 1.2 * u.high);
        }
    }
    col += prev(uncentred(centred(in.uv) * 0.99)) * 0.1;
    return vec4<f32>(col, 1.0);
}
