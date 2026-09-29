// @heavy — Synesthesia-style abstract. @bloom 0.9 @tonemap agx
// A bioluminescent creature floating in the dark: a soft glowing core with
// a crown of long tentacles that curl and undulate, each lined with glowing
// beads that ripple from base to tip. Drawn as neon tubes (per-tentacle
// closest-approach glow along the view ray), so it's mostly light.
// Audio vocabulary:
//  - tentacle undulation on the mid energy clock; the crown slowly turns on
//    the whole-mix clock;
//  - curl (how tightly they coil) follows bass presence;
//  - light waves run up the tentacles on each bass hit; the tips sparkle on
//    high hits; the core pulses with the bass level.

const NT: i32 = 14;          // tentacles
const SEG: i32 = 8;         // segments per tentacle

// Point along tentacle k at parameter s in 0..1.
fn tent(k: i32, s: f32) -> vec3<f32> {
    let fk = f32(k);
    let base_a = fk / f32(NT) * TAU + u.clock4.x * 0.02;
    let tilt = 0.9 + 0.7 * sin(fk * 2.3);
    let dir = vec3<f32>(cos(base_a) * sin(tilt), cos(tilt), sin(base_a) * sin(tilt));
    let side = normalize(cross(dir, vec3<f32>(0.0, 1.0, 0.01)));
    let up = cross(side, dir);
    let len = 2.8 + 0.6 * sin(fk * 1.7);
    let w = u.clock4.z * 0.35 + fk * 1.3;
    let curl = 0.6 + 1.4 * u.pres4.x;
    let bend = s * s * curl;
    return dir * (0.35 + len * s)
        + side * sin(w - s * 5.0) * bend * 0.6
        + up * cos(w * 0.8 - s * 4.0) * bend * 0.5;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let ang = u.clock4.x * 0.01 + u.seed;
    let ro = vec3<f32>(sin(ang) * 5.0, 1.0 + 0.8 * sin(u.clock4.x * 0.007), cos(ang) * 5.0);
    let rd = cam_ray(p, ro, vec3<f32>(0.0, 0.8, 0.0), 0.0, 1.2);
    let hue = u.clock4.w * 0.008 + u.hue;
    let drive = 0.5 + 0.8 * u.intensity;
    var col = vec3<f32>(0.002, 0.003, 0.008);

    // Core: glow around the origin.
    let tc = max(dot(-ro, rd), 0.0);
    let dc = length(ro + rd * tc);
    col += palette(hue + 0.1) * (exp(-dc * 3.0) * (0.6 + 1.6 * u.lvl4.x) + exp(-dc * 12.0) * 2.0) * drive;

    // Tentacles: segment-by-segment closest approach of the ray.
    for (var k = 0; k < NT; k++) {
        // Bounding sphere around tentacle k (base..tip + max bend): skip the
        // whole tentacle if the ray passes nowhere near it.
        let mid = tent(k, 0.5);
        let tip = tent(k, 1.0);
        let br = max(length(tip - mid), length(mid)) + 0.9;
        let tb = dot(mid - ro, rd);
        if length(ro + rd * tb - mid) > br {
            continue;
        }
        let tcol = palette(hue + f32(k) / f32(NT) * 0.35);
        var a = tent(k, 0.0);
        for (var i = 1; i <= SEG; i++) {
            let s1 = f32(i) / f32(SEG);
            let b = tent(k, s1);
            // Ray vs segment a-b.
            let ba = b - a;
            let oa = ro - a;
            let dd = dot(rd, ba);
            let den = max(dot(ba, ba) - dd * dd, 1e-5);
            let sp = clamp((dot(oa, ba) - dot(oa, rd) * dd) / den, 0.0, 1.0);
            let pt = a + ba * sp;
            let tt = dot(pt - ro, rd);
            let dist = length(ro + rd * tt - pt);
            let s = s1 - (1.0 - sp) / f32(SEG);
            let thick = 0.09 * (1.0 - s * 0.8);
            // Far from this segment: nothing to add (skip the glow maths).
            if dist > thick * 14.0 || tt < 0.0 {
                a = b;
                continue;
            }
            // Wave of light running base -> tip after each bass hit.
            let wave = exp(-abs(s - (1.0 - u.hits4.x)) * 10.0) * u.hits4.x;
            // Beads along the tube.
            let bead = pow(0.5 + 0.5 * cos(s * 60.0 - u.clock4.z * 2.0), 8.0);
            let lvl = 0.25 + 0.6 * bead + 2.5 * wave + step(0.93, s) * u.hits4.w * 2.0;
            let line = exp(-dist * dist / (thick * thick)) + exp(-dist / (thick * 3.0)) * 0.15;
            col += tcol * line * lvl * 0.35 * drive * step(0.0, tt);
            a = b;
        }
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
