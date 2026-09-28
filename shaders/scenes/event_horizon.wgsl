// @heavy — 2026 tier. @bloom 0.55 @tonemap agx
// A black hole with a lensed accretion disk (the Interstellar look): each
// pixel's photon is integrated through Schwarzschild-like bending
// (a = −1.5·h²·x/r⁵, units of the Schwarzschild radius), picking up disk
// emission every time it crosses the disk plane — so the far side of the
// disk arcs over the top and under the bottom, and a thin photon ring hugs
// the shadow. Doppler beaming brightens the approaching side.
// ~100 adaptive steps of cheap math per pixel; no textures in the loop
// except one noise fetch per disk crossing.
// The camera drifts on the tempo clock; the disk's heat follows the music.

const DISK_IN: f32 = 2.6;
const DISK_OUT: f32 = 13.0;

fn stars(d: vec3<f32>) -> vec3<f32> {
    let g = d * 260.0;
    let cell = floor(g);
    let r1 = hash22(cell.xy + cell.z * 17.3);
    let r2 = hash21(cell.yz + cell.x * 7.1);
    let sp = cell + vec3<f32>(r1.y, r2, fract(r2 * 13.1)) * 0.6 + 0.2;
    let b = step(0.9, r1.x) * (pow(r2, 7.0) * 2.0 + 0.03);
    var c = mix(vec3<f32>(0.7, 0.8, 1.0), vec3<f32>(1.0, 0.85, 0.7), r1.y) * b * smoothstep(0.24, 0.0, length(g - sp));
    // Faint nebula wash so the lensing distortion reads.
    let n = tnoise(d * 1.2 + 0.4);
    c += palette(n.b * 0.6 + 0.3) * smoothstep(0.3, 0.6, n.r) * 0.015;
    return c;
}

// Approximate blackbody tint for a normalised temperature 0..1+.
fn heat(t: f32) -> vec3<f32> {
    let x = clamp(t, 0.0, 1.6);
    return vec3<f32>(
        smoothstep(0.0, 0.35, x),
        smoothstep(0.15, 0.8, x) * 0.9,
        smoothstep(0.45, 1.4, x) * 0.95
    ) * (0.3 + x * x * 1.6);
}

// Emission + opacity of the disk at plane point q, seen along ray dir v.
fn disk(q: vec3<f32>, v: vec3<f32>) -> vec4<f32> {
    let r = length(q.xz);
    if r < DISK_IN || r > DISK_OUT {
        return vec4<f32>(0.0);
    }
    // Keplerian swirl: inner radii turn faster. Phase follows the tempo
    // clock (continuous), so it never jumps.
    let om = 1.6 * pow(r, -1.5);
    let a = angle(q.xz) + u.flow * om * 1.2;
    let uvw = vec3<f32>(cos(a) * r * 0.08, sin(a) * r * 0.08, r * 0.06);
    let n = tnoise(uvw + 0.3);
    let sn = clamp((n.b - 0.36) * 3.6, 0.0, 1.0);
    let streak = 0.12 + 1.5 * sn * sn * (0.5 + 0.9 * n.g);
    // Temperature falls off outward; inner edge feathered.
    let temp = pow(DISK_IN / r, 0.75) * (0.9 + 0.35 * u.bass + 0.25 * u.intensity);
    let edge = smoothstep(DISK_IN, DISK_IN + 0.5, r) * smoothstep(DISK_OUT, DISK_OUT * 0.6, r);
    // Doppler: disk orbits counter-clockwise seen from +y.
    let vel = normalize(vec3<f32>(-q.z, 0.0, q.x)) * clamp(sqrt(0.5 / max(r - 1.0, 0.3)), 0.0, 0.7);
    let beta = dot(vel, -v);
    let gamma = 1.0 / sqrt(max(1.0 - dot(vel, vel), 0.05));
    let dop = 1.0 / (gamma * (1.0 - beta));
    let boost = pow(dop, 3.0);
    let col = heat(temp * mix(1.0, dop, 0.5)) * streak * edge * boost;
    let alpha = clamp(edge * (0.25 + streak * 0.55), 0.0, 1.0);
    return vec4<f32>(col * 2.2, alpha);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let az = u.flow * 0.01 + u.seed;
    let el = 0.13 + 0.06 * sin(u.flow * 0.013);
    let dist = 17.0 + 2.0 * sin(u.flow * 0.009);
    let ro = vec3<f32>(sin(az) * cos(el), sin(el), cos(az) * cos(el)) * dist;
    var dir = cam_ray(p, ro, vec3<f32>(0.0), 0.12, 1.7);

    var pos = ro;
    var vel = dir;
    var col = vec3<f32>(0.0);
    var acc = 0.0;                          // accumulated disk opacity
    let h2 = dot(cross(pos, vel), cross(pos, vel));
    var captured = false;
    for (var i = 0; i < 110; i++) {
        let r2 = dot(pos, pos);
        let r = sqrt(r2);
        if r < 1.0 {
            captured = true;
            break;
        }
        if r > 40.0 && dot(pos, vel) > 0.0 {
            break;
        }
        // Step size: small near the hole, large far away.
        let dt = clamp(0.08 * r, 0.05, 1.5);
        // RK2 (midpoint) on x'' = −1.5·h²·x/r⁵.
        let a1 = -1.5 * h2 * pos / (r2 * r2 * r);
        let pm = pos + vel * dt * 0.5;
        let vm = vel + a1 * dt * 0.5;
        let rm2 = dot(pm, pm);
        let a2 = -1.5 * h2 * pm / (rm2 * rm2 * sqrt(rm2));
        let npos = pos + vm * dt;
        vel = normalize(vel + a2 * dt);
        // Disk-plane crossing.
        if pos.y * npos.y < 0.0 {
            let f = pos.y / (pos.y - npos.y);
            let q = mix(pos, npos, f);
            let e = disk(q, vel);
            col += (1.0 - acc) * e.rgb * e.a;
            acc += (1.0 - acc) * e.a;
            if acc > 0.97 {
                break;
            }
        }
        pos = npos;
    }
    if !captured && acc < 0.97 {
        col += (1.0 - acc) * stars(vel);
    }
    col *= 0.5 + 0.7 * u.intensity;
    col += (bluen(in.pos.xy) - 0.5) * 0.002;
    return vec4<f32>(col, 1.0);
}
