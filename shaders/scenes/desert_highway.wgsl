// @heavy — 2026 tier. @bloom 0.8 @tonemap agx
// Night drive on an empty desert highway: headlights sweep the asphalt, the
// centre-line dashes rush past one per beat, sodium lamp posts pass by in
// pools of orange light, mesas stand black against the stars, and a storm
// flickers with lightning far off on the horizon.
// Analytic: ground plane, road markings, a few vertical cylinders for the
// lamp posts, procedural sky — cheap.
// The drive is locked to the tempo clock (one dash per beat); lamps glow with
// the bass, cat's-eyes flicker on the hi-hats, lightning fires on onsets.

const DASH: f32 = 4.0;        // dash period = distance travelled per beat
const LAMP_P: f32 = 36.0;     // lamp spacing

fn mesas(rd: vec3<f32>) -> f32 {
    let a = angle(rd.xz);
    let n = tnoise(vec3<f32>(cos(a) * 0.9, sin(a) * 0.9, 0.3)).b;
    // Flat-topped buttes: a quantised, clipped noise profile.
    let top = smoothstep(0.5, 0.56, n) * 0.05 + smoothstep(0.58, 0.61, n) * 0.035;
    return top + 0.004;
}

fn sky(rd: vec3<f32>, flash: f32, storm_dir: vec3<f32>) -> vec3<f32> {
    let h = max(rd.y, 0.0);
    var c = mix(vec3<f32>(0.03, 0.03, 0.05), vec3<f32>(0.004, 0.005, 0.014), sqrt(h));
    let g = rd * 240.0;
    let cell = floor(g);
    let r1 = hash22(cell.xy + cell.z * 17.3);
    let r2 = hash21(cell.yz + cell.x * 7.1);
    let sp = cell + vec3<f32>(r1.y, r2, fract(r2 * 13.1)) * 0.6 + 0.2;
    c += vec3<f32>(0.8, 0.85, 1.0) * step(0.88, r1.x) * (pow(r2, 8.0) * 2.0 + 0.05) * smoothstep(0.22, 0.0, length(g - sp)) * smoothstep(0.02, 0.15, rd.y);
    // Storm clouds low on the horizon toward the storm, lit by lightning.
    let toward = max(dot(normalize(vec3<f32>(rd.x, 0.0, rd.z)), storm_dir), 0.0);
    let cl = tnoise(vec3<f32>(rd.x * 3.0, rd.y * 8.0, rd.z * 3.0)).r;
    let band = smoothstep(0.2, 0.05, rd.y) * smoothstep(-0.01, 0.03, rd.y) * pow(toward, 3.0);
    c = mix(c, vec3<f32>(0.01, 0.01, 0.015) + vec3<f32>(0.6, 0.62, 0.8) * flash * cl * 1.5, band * smoothstep(0.15, 0.35, cl));
    // Mesa silhouettes.
    if rd.y < mesas(rd) {
        c = vec3<f32>(0.003, 0.003, 0.005);
    }
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.flow * DASH;
    let lane = 1.8;
    let ro = vec3<f32>(lane + sin(u.flow * 0.03) * 0.15, 1.25, z);
    let ta = ro + vec3<f32>(sin(u.flow * 0.011) * 0.12, -0.08, 1.0);
    let rd = cam_ray(p, ro, ta, sin(u.flow * 0.02) * 0.01, 1.3);
    let drive = 0.5 + 0.8 * u.intensity;
    let storm_dir = normalize(vec3<f32>(-0.6, 0.0, 1.0));
    let flash = step(0.55, hash21(vec2<f32>(floor(u.beat), 7.7))) * beat_pulse(7.0) * smoothstep(0.2, 0.7, u.intensity + u.onset * 0.3);
    let lamp_c = vec3<f32>(1.0, 0.55, 0.18);
    let lamp_i = (0.6 + 1.0 * u.bass) * drive;

    var col = sky(rd, flash, storm_dir);
    var t_hit = 1e5;

    // --- Ground --------------------------------------------------------------
    if rd.y < 0.0 {
        let t = -ro.y / rd.y;
        t_hit = t;
        let g = ro + rd * t;
        let on_road = step(abs(g.x), 4.0);
        let sand = vec3<f32>(0.09, 0.07, 0.05) * (0.6 + 0.8 * tnoise(vec3<f32>(g.xz * 0.1, 0.5)).b);
        let asphalt = vec3<f32>(0.025, 0.025, 0.027) * (0.8 + 0.4 * tnoise(vec3<f32>(g.xz * 2.0, 0.1)).a);
        var albedo = mix(sand, asphalt, on_road);
        // Markings: edge lines and the yellow centre dash (one per beat).
        let edge = smoothstep(0.1, 0.06, abs(abs(g.x) - 3.7));
        let dash = smoothstep(0.08, 0.05, abs(g.x)) * step(fract(g.z / DASH), 0.5);
        let mark_c = vec3<f32>(0.8) * edge + vec3<f32>(0.85, 0.65, 0.1) * dash;
        albedo = mix(albedo, mark_c, clamp(edge + dash, 0.0, 1.0));
        // Headlights: two cones forward from the car, bright near, fading.
        let rel = g - vec3<f32>(ro.x, 0.0, ro.z);
        var head = 0.0;
        if rel.z > 0.5 {
            let spread = abs(rel.x) / (rel.z * 0.35 + 0.8);
            head = smoothstep(1.0, 0.3, spread) * 26.0 / (rel.z * rel.z * 0.6 + 6.0);
        }
        var light = vec3<f32>(0.9, 0.92, 1.0) * head * (0.7 + 0.4 * u.intensity);
        // Sodium lamps (right side): pools of orange light.
        let k0 = round((g.z - 10.0) / LAMP_P);
        for (var j = -1; j <= 1; j++) {
            let lz = (k0 + f32(j)) * LAMP_P + 10.0;
            let lp = vec3<f32>(5.2, 7.0, lz);
            let v = lp - g;
            light += lamp_c * lamp_i * 45.0 / (dot(v, v) + 4.0);
        }
        // Starlight + a lightning wash from the storm side.
        light += vec3<f32>(0.02, 0.022, 0.03) + vec3<f32>(0.3, 0.3, 0.4) * flash * 0.15;
        col = albedo * light;
        // Retroreflective markings + cat's-eyes light up in the headlights.
        col += mark_c * head * 0.25 * clamp(edge + dash, 0.0, 1.0);
        let eye_z = fract(g.z / (DASH * 2.0) + 0.25);
        let eye = smoothstep(0.14, 0.05, length(vec2<f32>(abs(g.x) - 3.95, (eye_z - 0.5) * DASH * 2.0)));
        col += vec3<f32>(1.0, 0.3, 0.1) * eye * (0.3 + 1.5 * u.high) * smoothstep(80.0, 5.0, rel.z) * step(0.5, rel.z);
        // Distance haze.
        col = mix(col, sky(vec3<f32>(rd.x, 0.0005, rd.z), flash, storm_dir) * 0.8, smoothstep(40.0, 300.0, t));
    }

    // --- Lamp posts ------------------------------------------------------------
    let k0 = round((ro.z + 20.0 - 10.0) / LAMP_P);
    for (var j = 0; j < 5; j++) {
        let lz = (k0 + f32(j)) * LAMP_P + 10.0;
        // Pole: vertical cylinder at (6, lz), r 0.12, height 7.3.
        let oc = ro.xz - vec2<f32>(6.0, lz);
        let a = dot(rd.xz, rd.xz);
        let b = dot(oc, rd.xz);
        let c = dot(oc, oc) - 0.12 * 0.12;
        let d = b * b - a * c;
        if d > 0.0 {
            let tp = (-b - sqrt(d)) / a;
            let yy = ro.y + rd.y * tp;
            if tp > 0.0 && tp < t_hit && yy < 7.3 && yy > 0.0 {
                t_hit = tp;
                col = vec3<f32>(0.012) + lamp_c * lamp_i * 0.02;
            }
        }
        // Arm from the pole top out over the road (ray-to-segment distance).
        {
            let a0 = vec3<f32>(6.0, 7.25, lz);
            let ba = vec3<f32>(-0.95, -0.1, 0.0);
            let oa = ro - a0;
            let dd = dot(rd, ba);
            let den = max(dot(ba, ba) - dd * dd, 1e-4);
            let sa = clamp((dot(oa, ba) - dot(oa, rd) * dd) / den, 0.0, 1.0);
            let pt = a0 + ba * sa;
            let ta2 = dot(pt - ro, rd);
            if ta2 > 0.0 && ta2 < t_hit && length(ro + rd * ta2 - pt) < 0.07 {
                t_hit = ta2;
                col = vec3<f32>(0.012) + lamp_c * lamp_i * 0.02;
            }
        }
        // Lamp head glow (seen as a hot point with a halo in the dust).
        let lp = vec3<f32>(5.2, 7.0, lz);
        let v = lp - ro;
        let lt = dot(v, rd);
        if lt > 0.0 && lt < t_hit + 1.0 {
            let ld = length(v - rd * lt);
            col += lamp_c * lamp_i * (exp(-ld * ld * 40.0) * 3.0 + exp(-ld * 1.5) * 0.06);
        }
    }

    // --- Lightning bolt on the horizon -------------------------------------------
    if flash > 0.05 {
        let bh = hash21(vec2<f32>(floor(u.beat), 1.1));
        let bdir = normalize(storm_dir + vec3<f32>((bh - 0.5) * 0.6, 0.0, 0.0));
        let fwd = normalize(ta - ro);
        let rgt = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), fwd));
        let sx = dot(bdir, rgt) / max(dot(bdir, fwd), 0.1) * 1.3;
        if dot(bdir, fwd) > 0.0 {
            var x = sx;
            var bolt = 0.0;
            for (var k = 0; k < 6; k++) {
                let y0 = -0.35 + f32(k) * 0.07;
                let jx = (hash21(vec2<f32>(floor(u.beat) * 7.0 + f32(k), 3.0)) - 0.5) * 0.04;
                let seg_t = clamp((p.y - (-0.35 + f32(k) * 0.07)) / -0.07, 0.0, 1.0);
                let in_seg = step(y0, -p.y) * step(-p.y, y0 + 0.07);
                bolt = max(bolt, in_seg * exp(-abs(p.x - (x + jx * (1.0 - seg_t))) * 900.0));
                x += jx;
            }
            col += vec3<f32>(0.75, 0.8, 1.0) * bolt * flash * 3.0 * step(-0.02, -p.y - 0.0) * step(p.y, 0.0);
        }
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
