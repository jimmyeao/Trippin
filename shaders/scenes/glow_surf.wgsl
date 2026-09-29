// @heavy — 2026 tier. @bloom 0.9 @tonemap agx
// Bioluminescent surf at night: waves roll in under a low moon and their
// breaking crests light up electric blue — plankton flaring as the water is
// churned — then the glow runs up the wet sand in the swash and fades.
// Heightfield march (56 steps, one noise fetch per step) for the sea and
// beach; waves are an analytic crest profile, one arriving per bar.
// Wave timing is a pose of the tempo clock; the glow flares on the kick and
// the foam sparkles on the hi-hats.

fn shore(x: f32) -> f32 {
    return 7.0 + sin(x * 0.05) * 3.0 + sin(x * 0.13 + 1.0) * 0.8;
}

// Wave phase at a point: 0..1 per wave, waves travel toward −z (the beach).
fn wave_w(p: vec2<f32>) -> f32 {
    return fract((p.y - shore(p.x)) * 0.075 + u.clock4.x * 0.25 + sin(p.x * 0.04) * 0.15);
}

// Crest profile: a steep front (low w side toward the beach) and a long back.
fn crest(w: f32) -> f32 {
    let x = w - 0.12;
    return select(exp(-x * x / 0.0015), exp(-x * x / 0.02), x > 0.0);
}

fn height(p: vec2<f32>) -> f32 {
    let dz = p.y - shore(p.x);
    if dz < 0.0 {
        // Beach: rises gently landward.
        return -dz * 0.06 - 0.02;
    }
    // Waves steepen as they near the shore, then collapse in the shallows.
    // Shape: the swell grows with bass presence.
    let amp = smoothstep(40.0, 10.0, dz) * smoothstep(0.5, 5.0, dz) * 1.15 * (0.6 + 0.8 * u.pres4.x) + 0.05;
    let n = tnoise(vec3<f32>(p * 0.08, u.time * 0.03)).b - 0.5;
    return crest(wave_w(p)) * amp + n * 0.18 - dz * 0.004;
}

// Bioluminescent glow at a sea/beach point.
fn biolum(p: vec2<f32>) -> f32 {
    let dz = p.y - shore(p.x);
    let w = wave_w(p);
    // Breaking crest: the front of the wave inside the surf zone.
    let front = smoothstep(0.02, 0.1, w) * smoothstep(0.2, 0.1, w) * smoothstep(18.0, 7.0, dz) * smoothstep(0.0, 2.0, dz);
    // White water behind a break, fading as it spreads.
    let wash = smoothstep(0.1, 0.02, w) * smoothstep(0.0, 0.25, w + 0.0) * 0.0
        + smoothstep(0.35, 0.1, w) * step(0.1, w) * smoothstep(9.0, 3.0, dz) * smoothstep(-0.5, 1.0, dz) * 0.5;
    // Swash: a sheet running up the sand once per bar, glowing at its edge.
    let reach = sin(clamp(u.bar_phase * 1.4, 0.0, 1.0) * PI) * 5.0;
    let edge = exp(-abs(dz + reach) * 3.0) * step(dz, 0.5) * smoothstep(-6.0, -0.5, dz - reach * 0.2);
    let foam = tnoise(vec3<f32>(p * 0.6, u.time * 0.1)).g;
    return (front * 1.6 + wash + edge * 0.8) * (0.15 + 2.2 * foam * foam);
}

fn sky(rd: vec3<f32>, moon: vec3<f32>) -> vec3<f32> {
    let h = max(rd.y, 0.0);
    var c = mix(vec3<f32>(0.02, 0.028, 0.05), vec3<f32>(0.003, 0.005, 0.014), sqrt(h));
    let md = dot(rd, moon);
    c += vec3<f32>(1.0, 0.95, 0.85) * smoothstep(0.99975, 0.99985, md) * 4.0;
    c += vec3<f32>(0.3, 0.32, 0.4) * pow(max(md, 0.0), 300.0) * 0.4 + vec3<f32>(0.05, 0.06, 0.09) * pow(max(md, 0.0), 12.0);
    let g = rd * 230.0;
    let cell = floor(g);
    let r1 = hash22(cell.xy + cell.z * 17.3);
    let r2 = hash21(cell.yz + cell.x * 7.1);
    let sp = cell + vec3<f32>(r1.y, r2, fract(r2 * 13.1)) * 0.6 + 0.2;
    c += vec3<f32>(0.8, 0.85, 1.0) * step(0.9, r1.x) * (pow(r2, 8.0) * 2.0 + 0.04) * smoothstep(0.22, 0.0, length(g - sp)) * smoothstep(0.02, 0.1, rd.y);
    // Headland silhouette on the horizon.
    let a = angle(rd.xz);
    let ridge = 0.015 + 0.04 * max(sin(a * 3.0 + 2.0), 0.0) * (0.6 + 0.4 * sin(a * 11.0));
    c = select(c, vec3<f32>(0.004, 0.005, 0.009), rd.y < ridge && rd.z > 0.0);
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Direction: strolling the beach one way, then back.
    let bx = 12.0 * sin(u.clock4.x * 0.01);
    let ro = vec3<f32>(bx, 1.2 + 0.1 * sin(u.clock4.x * 0.013), shore(bx) - 2.5);
    let ta = ro + vec3<f32>(0.2 + cos(u.clock4.x * 0.01) * 0.5, -0.08, 1.0);
    let rd = cam_ray(p, ro, ta, 0.0, 1.5);
    let moon = normalize(vec3<f32>(0.1, 0.16, 1.0));
    let blue = vec3<f32>(0.1, 0.55, 1.0);
    let drive = 0.5 + 0.8 * u.intensity;

    var col = sky(rd, moon);
    // Heightfield march.
    var t = 0.2;
    var hit = false;
    for (var i = 0; i < 56; i++) {
        let q = ro + rd * t;
        let dh = q.y - height(q.xz);
        if dh < 0.004 * t {
            hit = true;
            break;
        }
        t += max(dh * 0.5, 0.02 + t * 0.012);
        if t > 150.0 {
            break;
        }
    }
    if hit {
        let hp = ro + rd * t;
        let e = 0.05;
        let n = normalize(vec3<f32>(
            height(hp.xz - vec2<f32>(e, 0.0)) - height(hp.xz + vec2<f32>(e, 0.0)),
            2.0 * e,
            height(hp.xz - vec2<f32>(0.0, e)) - height(hp.xz + vec2<f32>(0.0, e))
        ));
        let dz = hp.z - shore(hp.x);
        let wet = smoothstep(1.0, -4.0, dz - 0.0) ;
        let rr = reflect(rd, n);
        let fr = fresnel(0.02, dot(-rd, n));
        let refl = sky(rr, moon);
        var base: vec3<f32>;
        if dz > -0.3 {
            // Sea: dark water reflecting the moon and sky.
            base = vec3<f32>(0.001, 0.003, 0.006) + refl * (fr * 0.9 + 0.02);
        } else {
            // Sand: dry and dim further up, wet and mirror-ish near the water.
            let sand = vec3<f32>(0.05, 0.045, 0.04) * (0.7 + 0.5 * tnoise(vec3<f32>(hp.xz * 0.8, 0.2)).b);
            let moonlit = max(dot(n, moon), 0.0) * 0.4 + 0.08;
            let wet_s = smoothstep(-7.0, -1.0, dz);
            base = sand * moonlit * (1.0 - wet_s * 0.6) + refl * fr * wet_s * 0.8;
        }
        // Bioluminescence (+ a hat-driven sparkle in the foam).
        let g = biolum(hp.xz);
        let sg = hp.xz * 6.0;
        let cell = floor(sg);
        let sh = hash22(cell + floor(u.beat * 2.0) * 13.0);
        let spark = step(0.93, sh.x) * smoothstep(0.18, 0.0, length(fract(sg) - 0.5 - (sh - 0.5) * 0.5)) * u.high * 3.0;
        let glow = g * (0.6 + 1.6 * u.kick) + spark * g;
        col = base + blue * glow * drive * 1.4;
        // Glow on the sand reflects in the wet film just behind the swash.
        col = mix(col, sky(rd, moon) * 0.5, smoothstep(40.0, 150.0, t));
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
