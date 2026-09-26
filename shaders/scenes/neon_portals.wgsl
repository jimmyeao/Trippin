// Fly down a corridor of neon polygon portals over a wet, reflective floor in
// blue haze. One portal passes per beat. Shape (triangle / square / hexagon)
// and twist change with each cut via the seed.

const SPACING: f32 = 3.0;
const R: f32 = 0.85;       // portal apothem; the floor sits under the bottom edge

fn shape_n() -> f32 {
    let s = floor(u.seed % 3.0);
    return select(select(6.0, 4.0, s < 1.5), 3.0, s < 0.5);
}

fn twist() -> f32 {
    return (floor(u.seed * 7.0) % 3.0) * 0.12;
}

// Signed distance to a regular n-gon with apothem r (after Inigo Quilez).
fn sd_ngon(p_in: vec2<f32>, r: f32, n: f32) -> f32 {
    let an = PI / n;
    let he = r * tan(an);
    var p = vec2<f32>(-p_in.y, -p_in.x);
    let bn = 2.0 * an * floor((angle(p) + an) / (2.0 * an));
    let cs = vec2<f32>(cos(bn), sin(bn));
    p = vec2<f32>(cs.x * p.x + cs.y * p.y, -cs.y * p.x + cs.x * p.y);
    return length(p - vec2<f32>(r, clamp(p.y, -he, he))) * sign(p.x - r);
}

struct Frame {
    d: f32,
    k: f32,
    ang: f32,
};

// Distance to the nearest portal tube.
fn portal(pos: vec3<f32>) -> Frame {
    let k = floor(pos.z / SPACING + 0.5);
    let z = pos.z - k * SPACING;
    // Flat edge down, standing just above the floor at -R.
    let q = rot(k * twist()) * pos.xy;
    let d2 = sd_ngon(q, R, shape_n());
    var f: Frame;
    f.d = length(vec2<f32>(d2, z)) - 0.03;
    f.k = k;
    f.ang = angle(q);
    return f;
}

fn neon(ang: f32, k: f32) -> vec3<f32> {
    let pink = vec3<f32>(1.0, 0.15, 0.7);
    let cyan = vec3<f32>(0.1, 0.7, 1.0);
    let m = 0.5 + 0.5 * sin(ang + k * 0.9 + u.hue * TAU);
    return mix(pink, cyan, m);
}

// March `ro`,`rd` up to `tmax`, accumulating neon glow; returns glow.
fn march_glow(ro: vec3<f32>, rd: vec3<f32>, tmax: f32, steps: i32) -> vec3<f32> {
    var t = 0.05;
    var glow = vec3<f32>(0.0);
    for (var i = 0; i < steps; i++) {
        let pos = ro + rd * t;
        let f = portal(pos);
        let fog = exp(-t * 0.07);
        let pulse = 0.85 + 0.35 * u.intensity;
        glow += neon(f.ang, f.k) * (0.0016 / (0.0012 + f.d * f.d)) * fog * pulse * 0.05;
        if f.d < 0.002 {
            // Hot, nearly white tube core.
            glow += (neon(f.ang, f.k) * 1.6 + vec3<f32>(0.3)) * fog * pulse;
            break;
        }
        t += max(f.d * 0.8, 0.01);
        if t > tmax {
            break;
        }
    }
    return glow;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let travel = u.flow * SPACING;
    let ro = vec3<f32>(0.0, 0.05, travel);
    let rd = normalize(vec3<f32>(p.x, -p.y, 1.7));

    let haze = mix(vec3<f32>(0.005, 0.008, 0.03), vec3<f32>(0.04, 0.05, 0.15), exp(-abs(rd.y) * 5.0));
    var col = haze;

    let t_floor = select(1e9, (-R - 0.06 - ro.y) / rd.y, rd.y < 0.0);
    col += march_glow(ro, rd, min(t_floor, 40.0), 90);

    if t_floor < 40.0 {
        // Wet floor: rippled mirror of the portals, fading with distance.
        let hit = ro + rd * t_floor;
        // Gentle, broad ripples: soft streaky reflections, not squiggles.
        let rip = vec2<f32>(noise(hit.xz * vec2<f32>(1.2, 0.4) + u.time * 0.3),
                            noise(hit.xz * vec2<f32>(1.2, 0.4) - u.time * 0.2 + 7.0)) - 0.5;
        let n = normalize(vec3<f32>(rip.x * 0.025, 1.0, rip.y * 0.012));
        let rd2 = reflect(rd, n);
        let fres = 0.25 + 0.6 * pow(1.0 - max(-rd.y, 0.0), 3.0);
        let refl = march_glow(hit, rd2, 30.0, 60);
        col = mix(col, vec3<f32>(0.01, 0.012, 0.03), 0.6) + refl * fres * exp(-t_floor * 0.05);
    }
    col += prev(in.uv) * 0.12;
    return vec4<f32>(col, 1.0);
}
