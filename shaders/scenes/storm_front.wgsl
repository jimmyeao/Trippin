// @heavy — 2026 tier. @bloom 0.7 @tonemap agx
// Skimming a dark sea toward a supercell at dusk: a towering shelf cloud
// with a bright strip of sky beneath it, rain curtains hanging from the
// base, and lightning that lights the cloud from inside on the big hits.
// Volumetric: 44 blue-noise-jittered steps through a Perlin-Worley density
// built from the baked 3D noise (2 fetches/step + 1 for the light sample),
// so it stays inside an M2 budget at the default 75% scale.
// Forward flight follows the tempo clock; lightning fires on onsets while
// the track is driving — each strike's position is hashed per beat and its
// brightness is a decay envelope of the beat phase (pose, not integrator).

const BASE: f32 = 2.4;
const TOP: f32 = 9.0;

fn strike_pos(beat: f32) -> vec3<f32> {
    let h = hash22(vec2<f32>(beat, 3.7));
    return vec3<f32>((h.x - 0.5) * 16.0, 3.0 + h.y * 2.5, u.flow * 0.6 + 14.0 + h.y * 10.0);
}

// Lightning envelope for the current beat: fires on some beats when driving.
fn strike_amt() -> f32 {
    let b = floor(u.beat);
    let roll = hash21(vec2<f32>(b, 9.1));
    let chance = smoothstep(0.45, 0.95, u.intensity) * 0.8 + u.onset * 0.1;
    let on = step(roll, chance);
    // Double-flicker decay, like a real return stroke.
    let ph = u.beat_phase;
    return on * (exp(-ph * 9.0) + 0.6 * exp(-abs(ph - 0.12) * 40.0));
}

fn density(p: vec3<f32>) -> f32 {
    let hf = (p.y - BASE) / (TOP - BASE);
    if hf < 0.0 || hf > 1.0 {
        return 0.0;
    }
    // The storm wall sits ahead; coverage ramps up with distance.
    let wall = smoothstep(6.0, 20.0, p.z - u.flow * 0.6);
    let q = p * vec3<f32>(0.032, 0.05, 0.032) + vec3<f32>(0.0, 0.0, u.time * 0.003);
    let n = tnoise(q);
    // Height profile: flat dark base, billowing tower tops.
    let prof = smoothstep(0.0, 0.05, hf) * smoothstep(1.0, 0.45 + 0.4 * n.b, hf);
    var d = (n.r * 1.5 + n.b * 0.7 - 0.74 + wall * 0.6) * prof;
    if d <= 0.0 {
        return 0.0;
    }
    // Erode with two finer Worley octaves — crisp cauliflower edges.
    let e1 = tnoise(p * 0.13 + vec3<f32>(u.time * 0.008, 0.0, 0.0)).g;
    let e2 = tnoise(p * 0.37 + vec3<f32>(0.0, u.time * 0.01, 0.0)).a;
    // HZD-style remap: erosion carves the edge without thinning the core.
    let ero = (1.0 - e1) * 0.55 * (1.0 - hf * 0.4) + (1.0 - e2) * 0.18;
    let dd = clamp((d - ero) / max(1.0 - ero, 0.05), 0.0, 1.0);
    return dd * 14.0;
}

// Rain curtains below the base: streaky vertical density.
fn rain(p: vec3<f32>) -> f32 {
    if p.y > BASE || p.y < 0.0 {
        return 0.0;
    }
    let wall = smoothstep(10.0, 22.0, p.z - u.flow * 0.6);
    let n = tnoise(vec3<f32>(p.x * 0.05, 0.1, p.z * 0.05));
    let streak = tnoise(vec3<f32>(p.x * 0.6, p.y * 0.02 + u.time * 0.05, p.z * 0.6)).a;
    return smoothstep(0.52, 0.7, n.b) * wall * (0.5 + streak) * 0.25 * (0.6 + 0.6 * u.intensity);
}

fn sky(rd: vec3<f32>) -> vec3<f32> {
    // Bright strip of clear dusk sky on the horizon, bruised purple above.
    let h = rd.y;
    var c = mix(vec3<f32>(0.5, 0.28, 0.14), vec3<f32>(0.05, 0.05, 0.09), smoothstep(0.0, 0.18, h));
    c = mix(c, vec3<f32>(0.01, 0.012, 0.02), smoothstep(0.15, 0.6, h));
    return c * 0.6;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let ro = vec3<f32>(sin(u.flow * 0.01) * 1.5, 0.45 + 0.08 * sin(u.flow * 0.02), u.flow * 0.6);
    let rd = cam_ray(p, ro, ro + vec3<f32>(sin(u.flow * 0.008) * 0.2, 0.2, 1.0), sin(u.flow * 0.006) * 0.03, 1.4);

    let sun = normalize(vec3<f32>(-0.2, 0.08, 1.0));
    let amb_top = vec3<f32>(0.045, 0.05, 0.07);
    let sun_col = vec3<f32>(1.0, 0.55, 0.3) * 0.45;
    let strike = strike_amt();
    let sp = strike_pos(floor(u.beat));
    let bolt_col = vec3<f32>(0.75, 0.8, 1.0);

    // --- Sea ---------------------------------------------------------------
    var bg = sky(rd);
    let t_sea = select(1e5, -ro.y / rd.y, rd.y < 0.0);
    if t_sea < 1e4 {
        let hp = ro + rd * t_sea;
        let w = (tnoise(vec3<f32>(hp.xz * 0.08, u.time * 0.05)).b - 0.5) + (tnoise(vec3<f32>(hp.xz * 0.35, u.time * 0.09)).b - 0.5) * 0.5;
        let n = normalize(vec3<f32>(w * 0.25, 1.0, (tnoise(vec3<f32>(hp.xz * 0.08 + 0.5, u.time * 0.05)).b - 0.5) * 0.25));
        let rr = reflect(rd, n);
        let fr = fresnel(0.02, dot(-rd, n));
        let refl = sky(rr) * vec3<f32>(0.6, 0.75, 1.0);
        bg = refl * fr * 0.8 + vec3<f32>(0.004, 0.006, 0.008) * (1.0 - fr);
        // Lightning glints on the water.
        bg += bolt_col * strike * 0.3 * fr * exp(-length(hp.xz - sp.xz) * 0.08);
    }

    // --- Volume ------------------------------------------------------------
    // March the slab between the sea and the cloud tops.
    let t_top = select(60.0, (TOP - ro.y) / rd.y, rd.y > 0.001);
    let t_end = min(min(t_top, 60.0), t_sea);
    // Distance-scaled steps: fine near the camera, coarse far away.
    var t = 0.1 + 0.3 * bluen(in.pos.xy);
    var trans = 1.0;
    var acc = vec3<f32>(0.0);
    for (var i = 0; i < 56; i++) {
        if t > t_end {
            break;
        }
        let dt = 0.25 + t * 0.045;
        let pos = ro + rd * t;
        let dc = density(pos);
        let dr = rain(pos);
        let dsum = dc + dr;
        if dsum > 0.001 {
            // One light sample toward the sun for self-shadowing.
            let ls = density(pos + sun * 0.8);
            let light = exp(-ls * 0.9);
            let hf = clamp((pos.y - BASE) / (TOP - BASE), 0.0, 1.0);
            // Powder term: thin edges glow, dense cores stay dark.
            let powder = 1.0 - exp(-dc * 0.6);
            var l = sun_col * light * (0.25 + 0.75 * hf) * (0.4 + 0.6 * powder) + amb_top * (0.12 + 0.88 * hf * hf);
            let ld = length(pos - sp);
            l += bolt_col * strike * 9.0 / (1.0 + ld * ld * 0.25);
            let a = 1.0 - exp(-dsum * dt * 0.9);
            acc += trans * a * l;
            trans *= 1.0 - a;
            if trans < 0.02 {
                break;
            }
        }
        t += dt;
    }
    var col = bg * trans + acc;

    // --- Bolt ----------------------------------------------------------------
    // Cloud-to-sea channel: a jagged polyline down from the strike point.
    if strike > 0.05 {
        var a = sp;
        var bolt = 0.0;
        for (var k = 0; k < 7; k++) {
            let h = hash22(vec2<f32>(floor(u.beat) * 7.0 + f32(k), 1.3)) - 0.5;
            let b = a + vec3<f32>(h.x * 1.2, -sp.y / 7.0, h.y * 0.8);
            // Distance from the view ray to segment a-b.
            let ba = b - a;
            let oa = ro - a;
            let dd = dot(rd, ba);
            let den = max(dot(ba, ba) - dd * dd, 1e-4);
            let s = clamp((dot(oa, ba) - dot(oa, rd) * dd) / den, 0.0, 1.0);
            let pt = a + ba * s;
            let tt = max(dot(pt - ro, rd), 0.0);
            let dist = length(ro + rd * tt - pt) / max(tt, 1.0);
            bolt = max(bolt, exp(-dist * 900.0) + exp(-dist * 120.0) * 0.15);
            a = b;
        }
        col += bolt_col * bolt * strike * 3.0 * trans;
    }
    col += (bluen(in.pos.xy + 17.0) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
