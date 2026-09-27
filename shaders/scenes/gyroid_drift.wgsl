// @heavy — raymarched. A gyroid sculpture turning in space: an endless
// porous lattice that breathes with the bass, its chambers lit by the
// spectrum. The camera orbits slowly; every cut re-paints the palette.

const BOUND: f32 = 2.35;  // sphere containing the sculpture
const SCALE: f32 = 3.0;   // lattice frequency

fn gyr(p: vec3<f32>) -> f32 {
    // Wall thickness swells with the bass so the lattice inflates on kicks.
    let th = 0.06 + 0.14 * u.bass;
    return abs(dot(sin(p), cos(p.zxy))) * 0.55 - th;
}

fn tumbling(p: vec3<f32>) -> vec3<f32> {
    // The sculpture tumbles slowly and rolls a little with each phrase.
    let a = u.time * 0.09 + u.bar_phase * 0.5;
    var q = p;
    q = vec3<f32>(rot(a) * q.xy, q.z);
    return vec3<f32>(q.x, rot(a * 0.6 + 0.9) * q.yz);
}

fn map(p: vec3<f32>) -> f32 {
    return max(gyr(tumbling(p) * SCALE) / SCALE, length(p) - BOUND);
}

fn normal_at(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.0035, 0.0);
    return normalize(vec3<f32>(
        map(p + e.xyy) - map(p - e.xyy),
        map(p + e.yxy) - map(p - e.yxy),
        map(p + e.yyx) - map(p - e.yyx)));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let drift = u.time * 0.11 + u.flow * 0.10;
    let ro = vec3<f32>(sin(drift) * 3.6, sin(u.time * 0.17) * 1.1, cos(drift) * 3.6);
    let fw = normalize(-ro);
    let rt = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), fw));
    let up = cross(fw, rt);
    // centred() has +y pointing down the screen — negate so up is up.
    let rd = normalize(fw * 1.35 + rt * p.x - up * p.y);

    var t = 0.0;
    var hit = false;
    var steps = 0.0;
    for (var i = 0; i < 90; i++) {
        let pos = ro + rd * t;
        let d = map(pos);
        steps = f32(i);
        if d < 0.002 * (1.0 + t) {
            hit = true;
            break;
        }
        // Gyroid bounds aren't exact — stay conservative.
        t += max(d * 0.45, 0.008);
        if t > 9.0 {
            break;
        }
    }

    var col = vec3<f32>(0.0);
    if hit {
        let pos = ro + rd * t;
        let lattice = abs(gyr(tumbling(pos) * SCALE) / SCALE);
        if lattice < 0.02 {
            // Wall face: iridescent bands follow the lattice; mids shift the wash.
            let n = normal_at(pos);
            let hue = pos.x * 0.16 + pos.y * 0.13 + pos.z * 0.14 + u.mid * 0.25;
            let base = palette(hue);
            let lit_dir = normalize(vec3<f32>(0.6, 0.75, 0.4));
            let diff = 0.3 + 0.7 * max(dot(n, lit_dir), 0.0);
            let fres = pow(1.0 - max(dot(n, -rd), 0.0), 3.0);
            let occ = 1.0 - steps / 90.0; // deep crevices took more steps
            col = base * base * diff * (0.3 + 0.7 * occ) * (0.5 + 1.0 * u.intensity);
            col += palette(hue + 0.35) * fres * (0.5 + 0.9 * u.high);
            col *= exp(-t * 0.10);
        } else {
            // The bounding silhouette — rim it, don't shade a fake sphere.
            col = palette(pos.y * 0.2 + 0.6) * 0.05 * (0.4 + u.energy);
        }
    }
    // A dim aura so the sculpture never floats on dead black.
    col += palette(0.55) * exp(-length(p) * 2.6) * 0.08 * (0.4 + u.energy);
    col += prev(uncentred(centred(in.uv) * 0.99)) * 0.12;
    return vec4<f32>(col, 1.0);
}
