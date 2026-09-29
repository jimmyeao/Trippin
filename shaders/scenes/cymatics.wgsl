// @heavy — Synesthesia-style abstract. @bloom 0.8 @tonemap agx
// Cymatics: a backlit acrylic plate seen at an angle, vibrating with the
// music. Sand piles into ridges along the plate's nodal lines — Chladni
// figures, the sound made visible — casting real shadows, while the plate
// glows through the bare gaps. On every kick the sand leaps off the plate
// and rains back into the pattern.
// The mode numbers glide with the dominant band (bass-heavy = bold simple
// figures, busy highs = intricate ones), cross-faded smoothly.
// Rendering: a heightfield march over the sand (the plate is the base
// plane), a short shadow march toward the key light, airborne grains as
// three thin planes of sparks above the plate.
// Audio vocabulary: mode drift on the mid clock, pile height with the level,
// leap on bass hits, glints on high hits, the backlight on bass presence,
// the camera orbits on the whole-mix clock.

const HALF: f32 = 1.0;       // plate half-size
const PILE: f32 = 0.07;      // max sand ridge height

fn chladni(p: vec2<f32>, n: f32, m: f32) -> f32 {
    return cos(n * PI * p.x) * cos(m * PI * p.y) - cos(m * PI * p.x) * cos(n * PI * p.y);
}

// The blended mode field at plate point p (plate coords -1..1).
fn field(p: vec2<f32>) -> f32 {
    let lo = u.lvl4.x + u.lvl4.y * 0.5;
    let hi = u.lvl4.z + u.lvl4.w;
    // Busier highs AND more overall energy push toward the intricate family.
    let w = smoothstep(-0.3, 0.3, hi - lo + (u.pres4.x + u.pres4.z - 0.8) * 0.5);
    let drift = u.clock4.z * 0.02;
    let n1 = 2.0 + 2.0 * (0.5 + 0.5 * sin(drift));
    let m1 = 4.0 + 1.5 * (0.5 + 0.5 * cos(drift * 0.7));
    let n2 = 4.0 + 2.0 * (0.5 + 0.5 * sin(drift * 1.3));
    let m2 = 7.0 + 2.0 * (0.5 + 0.5 * cos(drift * 0.9));
    return mix(chladni(p, n1, m1), chladni(p, n2, m2), w);
}

// How much of the sand has been thrown up by the last kick (0 = settled).
fn airborne() -> f32 {
    return u.hits4.x;
}

// Sand height at plate point p.
fn sand(p: vec2<f32>) -> f32 {
    if max(abs(p.x), abs(p.y)) > HALF {
        return 0.0;
    }
    let f = field(p / HALF);
    let width = 0.1 + 0.08 * u.lvl4.x;
    let ridge = exp(-abs(f) / width);
    // Grain texture on the ridge surface.
    let g = tnoise(vec3<f32>(p * 3.0, 0.4)).a;
    let amount = (0.45 + 0.55 * u.lvl4.y + 0.3 * u.lvl4.x) * (1.0 - 0.6 * airborne());
    return PILE * ridge * ridge * amount * (0.8 + 0.4 * g);
}

// The plate physically flexes: its own standing wave, amplitude from the
// bass level (shape, not just light), oscillating at twice the beat rate.
fn disp(p: vec2<f32>) -> f32 {
    if max(abs(p.x), abs(p.y)) > HALF {
        return 0.0;
    }
    let osc = sin(u.flow * TAU * 2.0);
    let edge = smoothstep(HALF, HALF * 0.85, max(abs(p.x), abs(p.y)));
    return field(p / HALF) * osc * (0.012 + 0.05 * u.lvl4.x + 0.04 * u.hits4.x) * edge;
}

// Full surface height: flexing plate + sand riding on it.
fn surf(p: vec2<f32>) -> f32 {
    return disp(p) + sand(p);
}

fn sky(rd: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(0.004, 0.004, 0.008) * (0.5 + 0.5 * rd.y);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Swing back and forth around the plate (direction reverses every
    // phrase or so), pace on the energy clock; dips lower when it's loud.
    let ang = u.seed + 1.1 * sin(u.clock4.x * 0.025);
    let ro = vec3<f32>(sin(ang) * 1.55, 0.95 + 0.2 * sin(u.clock4.x * 0.017) - 0.15 * u.pres4.x, cos(ang) * 1.55);
    let rd = cam_ray(p, ro, vec3<f32>(0.0, -0.1, 0.0), sin(u.clock4.x * 0.019) * 0.08, 1.2);
    let key = normalize(vec3<f32>(-0.5, 0.45, 0.35));     // low raking light: long shadows
    let hue = u.clock4.w * 0.008 + u.hue;
    let drive = 0.5 + 0.8 * u.intensity;
    var col = sky(rd);

    // Heightfield march between the sand-top bound and the plate plane.
    var hit = false;
    var t = 0.0;
    if rd.y < 0.0 {
        let t0 = (PILE + 0.12 - ro.y) / rd.y;
        let t1 = (-0.12 - ro.y) / rd.y;
        let steps = 56;
        let dt = (t1 - t0) / f32(steps);
        t = t0 + dt * bluen(in.pos.xy);
        var pt = t;
        for (var i = 0; i < steps; i++) {
            let q = ro + rd * t;
            if q.y < surf(q.xz) {
                // Refine between the last two samples.
                var a = pt;
                var b = t;
                for (var r = 0; r < 4; r++) {
                    let m = 0.5 * (a + b);
                    let qm = ro + rd * m;
                    if qm.y < surf(qm.xz) {
                        b = m;
                    } else {
                        a = m;
                    }
                }
                t = b;
                hit = true;
                break;
            }
            pt = t;
            t += dt;
        }
        if !hit {
            // Missed the plate: the dark floor below.
            t = (-0.4 - ro.y) / rd.y;
            hit = true;
        }
    }
    if hit {
        let q = ro + rd * t;
        let onplate = max(abs(q.x), abs(q.z)) < HALF;
        if onplate {
            let h = sand(q.xz);
            let e = 0.006;
            let n = normalize(vec3<f32>(surf(q.xz - vec2<f32>(e, 0.0)) - surf(q.xz + vec2<f32>(e, 0.0)), 2.0 * e, surf(q.xz - vec2<f32>(0.0, e)) - surf(q.xz + vec2<f32>(0.0, e))));
            // Shadow: march toward the key light over the sand.
            var sh = 1.0;
            for (var i = 1; i <= 10; i++) {
                let sp = q + key * f32(i) * 0.03;
                let d = sp.y - surf(sp.xz);
                sh = min(sh, clamp(d * 60.0 + 0.2, 0.0, 1.0));
            }
            let diff = max(dot(n, key), 0.0) * sh;
            let sand_c = mix(vec3<f32>(0.95, 0.9, 0.8), palette(hue + 0.1), 0.25);
            let cover = smoothstep(0.0, PILE * 0.15, h);
            // Backlit acrylic plate glowing through the gaps.
            let glow = palette(hue + 0.55 + length(q.xz) * 0.15) * (0.25 + 1.3 * u.pres4.x + 0.5 * u.lvl4.x) * drive;
            let fill = vec3<f32>(0.03, 0.03, 0.05);
            // The bare plate shows its flex too: brighter where it bows up.
            let bow = 1.0 + disp(q.xz) * 8.0;
            col = mix(glow * bow * (0.6 + 0.4 * max(dot(n, key), 0.0)), sand_c * (diff * 1.6 + 0.06) + fill * 0.3, cover);
            // Glints on high hits.
            let gl = hash21(floor(q.xz * 180.0) + floor(u.beat * 4.0));
            col += vec3<f32>(1.0) * step(0.992, gl) * cover * u.hits4.w * 3.0 * sh;
        } else {
            // Dark studio floor far below-ish: faint glow spill from the plate.
            let d = max(abs(q.x), abs(q.z)) - HALF;
            col = vec3<f32>(0.004, 0.004, 0.007) + palette(hue + 0.55) * 0.08 * exp(-d * 4.0) * (0.3 + u.pres4.x);
        }
        // Plate rim.
        let edge = max(abs(q.x), abs(q.z));
        col += palette(hue + 0.55) * smoothstep(0.012, 0.0, abs(edge - HALF)) * 0.8 * drive;
    }

    // Airborne grains after a kick: three thin planes of sparks above the
    // plate, rising and falling with the bass-hit envelope.
    let air = airborne();
    if air > 0.02 && rd.y < 0.0 {
        for (var k = 0; k < 3; k++) {
            let fk = f32(k);
            let hgt = (0.08 + 0.16 * fk) * air * (1.0 - air * 0.3);
            let tp = (hgt - ro.y) / rd.y;
            if tp > 0.0 && tp < t {
                let qp = ro + rd * tp;
                if max(abs(qp.x), abs(qp.z)) < HALF {
                    // Grains come from the ridges below them.
                    let src = exp(-abs(field(qp.xz / HALF)) / 0.12);
                    let gp = qp.xz * (140.0 - fk * 20.0);
                    let cell = floor(gp);
                    let hh = hash22(cell + fk * 13.0);
                    let dd = length(fract(gp) - 0.5 - (hh - 0.5) * 0.6);
                    let grain = step(0.75, hh.x) * smoothstep(0.25, 0.05, dd) * src;
                    col += vec3<f32>(1.0, 0.95, 0.85) * grain * air * 1.5;
                }
            }
        }
    }
    col += (bluen(in.pos.xy + 5.0) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
