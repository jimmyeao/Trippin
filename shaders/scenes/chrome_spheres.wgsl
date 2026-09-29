// @heavy — Synesthesia-style abstract. @bloom 0.8 @tonemap agx
// Gliding through an infinite lattice of mirror-chrome spheres. Every
// sphere reflects every other (two bounces), so the space fills with
// curved reflections; a sparse scattering of spheres are lit from inside
// like lamps, one spectrum band each, and their light bounces through the
// whole lattice.
// Analytic: per ray, a 3D DDA through the sphere grid (exact sphere hits,
// no marching), then the same for one reflection bounce.
// Audio vocabulary:
//  - glide on the whole-mix energy clock, lattice slowly rolling on the mid
//    clock;
//  - lamp spheres glow with their band level, flash on that band's hits;
//  - sphere size breathes with bass presence.

const CELL: f32 = 2.0;

fn lamp(c: vec3<f32>) -> vec4<f32> {
    // (colour, intensity) of the sphere in cell c; intensity 0 = chrome.
    let h = hash22(c.xy + c.z * 17.3);
    if h.x > 0.045 {
        return vec4<f32>(0.0);
    }
    let band = i32(h.y * 4.0);
    var lv = u.lvl4.x;
    var hit = u.hits4.x;
    if band == 1 {
        lv = u.lvl4.y;
        hit = u.hits4.y;
    } else if band == 2 {
        lv = u.lvl4.z;
        hit = u.hits4.z;
    } else if band == 3 {
        lv = u.lvl4.w;
        hit = u.hits4.w;
    }
    let col = palette(f32(band) * 0.22 + u.hue + c.z * 0.01);
    return vec4<f32>(col, 0.3 + 1.6 * lv + 2.5 * hit);
}

struct Hit {
    t: f32,
    n: vec3<f32>,
    cell: vec3<f32>,
};

fn trace(ro: vec3<f32>, rd: vec3<f32>, rad: f32, steps: i32) -> Hit {
    var h: Hit;
    h.t = -1.0;
    var cell = floor(ro / CELL);
    let stp = sign(rd);
    let inv = 1.0 / max(abs(rd), vec3<f32>(1e-5));
    var tm = ((cell + max(stp, vec3<f32>(0.0))) * CELL - ro) / rd;
    let td = CELL * inv;
    for (var i = 0; i < steps; i++) {
        let c = (cell + 0.5) * CELL;
        let oc = ro - c;
        let b = dot(oc, rd);
        let cc = dot(oc, oc) - rad * rad;
        let d = b * b - cc;
        if d > 0.0 {
            let t = -b - sqrt(d);
            if t > 0.001 {
                h.t = t;
                h.n = normalize(ro + rd * t - c);
                h.cell = cell;
                return h;
            }
        }
        if tm.x < tm.y && tm.x < tm.z {
            cell.x += stp.x;
            tm.x += td.x;
        } else if tm.y < tm.z {
            cell.y += stp.y;
            tm.y += td.y;
        } else {
            cell.z += stp.z;
            tm.z += td.z;
        }
    }
    return h;
}

fn sky(rd: vec3<f32>) -> vec3<f32> {
    return palette(0.6 + u.hue) * 0.015 * (0.5 + 0.5 * rd.y);
}

// Light arriving at a chrome point from nearby lamp spheres (3x3x3 cells).
fn lamp_light(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var l = vec3<f32>(0.0);
    let c0 = floor(p / CELL);
    for (var k = 0; k < 27; k++) {
        let c = c0 + vec3<f32>(f32(k % 3) - 1.0, f32((k / 3) % 3) - 1.0, f32(k / 9) - 1.0);
        let lm = lamp(c);
        if lm.w > 0.0 {
            let v = (c + 0.5) * CELL - p;
            let d2 = dot(v, v);
            l += lm.rgb * lm.w * max(dot(n, v / sqrt(d2)), 0.0) / (1.0 + d2 * 0.4) * 0.7;
        }
    }
    return l;
}

fn shade(ro: vec3<f32>, rd: vec3<f32>, h: Hit, rad: f32) -> vec3<f32> {
    let lm = lamp(h.cell);
    if lm.w > 0.0 {
        // Lamp sphere: glowing, hotter at the centre of its disc.
        let core = pow(max(dot(h.n, -rd), 0.0), 1.5);
        return lm.rgb * lm.w * (0.4 + 0.8 * core);
    }
    let p = ro + rd * h.t;
    return lamp_light(p, h.n) * 0.25;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.clock4.x * 0.9;
    // Fly along a gap between sphere rows (x,y on cell boundaries).
    let ro = vec3<f32>(CELL * 0.5 * sin(z * 0.07) + CELL, CELL * 0.5 * cos(z * 0.05) + CELL, z);
    let ta = ro + vec3<f32>(0.3 * sin(z * 0.11), 0.2 * cos(z * 0.09), 1.0);
    let rd = cam_ray(p, ro, ta, u.clock4.z * 0.02, 1.2);
    let rad = 0.62 + 0.06 * u.pres4.x;

    var col = sky(rd);
    let h = trace(ro, rd, rad, 40);
    if h.t > 0.0 {
        col = shade(ro, rd, h, rad);
        if lamp(h.cell).w <= 0.0 {
            // Chrome: one reflection bounce.
            let p1 = ro + rd * h.t;
            let r = reflect(rd, h.n);
            let h2 = trace(p1 + h.n * 0.01, r, rad, 20);
            var refl = sky(r);
            if h2.t > 0.0 {
                refl = shade(p1, r, h2, rad);
                if lamp(h2.cell).w <= 0.0 {
                    refl *= 0.6;
                }
                refl *= exp(-h2.t * 0.08);
            }
            let fr = fresnel(0.7, dot(h.n, -rd));
            col += refl * fr * vec3<f32>(0.95, 0.96, 1.0);
        }
        col *= exp(-h.t * 0.05);
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
