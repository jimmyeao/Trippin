// Halloween: a haunted house and graveyard on a foggy hill under a huge
// cratered moon, bats crossing it, lightning flickering inside the clouds on
// hard onsets, and jack-o'-lanterns whose candlelight follows the music (each
// rides a different band). A wrought-iron fence fills the foreground.
// Seasonal (see config.rs).

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

fn sd_box(p: vec2<f32>, b: vec2<f32>) -> f32 {
    let d = abs(p) - b;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

fn sd_tri(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> f32 {
    let e0 = b - a;
    let e1 = c - b;
    let e2 = a - c;
    let v0 = p - a;
    let v1 = p - b;
    let v2 = p - c;
    let pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    let pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    let pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
    let s = sign(e0.x * e2.y - e0.y * e2.x);
    let d = min(min(vec2<f32>(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x)),
                    vec2<f32>(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x))),
                vec2<f32>(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x)));
    return -sqrt(d.x) * sign(d.y);
}

// Dead tree: a trunk and a few forking, tapering branches.
fn tree(p: vec2<f32>) -> f32 {
    var d = sd_segment(p, vec2<f32>(0.0, 0.0), vec2<f32>(0.02, -0.42)) - mix(0.035, 0.012, clamp(-p.y / 0.42, 0.0, 1.0));
    d = min(d, sd_segment(p, vec2<f32>(0.015, -0.25), vec2<f32>(0.2, -0.45)) - 0.012);
    d = min(d, sd_segment(p, vec2<f32>(0.2, -0.45), vec2<f32>(0.3, -0.52)) - 0.006);
    d = min(d, sd_segment(p, vec2<f32>(0.2, -0.45), vec2<f32>(0.22, -0.58)) - 0.005);
    d = min(d, sd_segment(p, vec2<f32>(0.01, -0.18), vec2<f32>(-0.18, -0.36)) - 0.011);
    d = min(d, sd_segment(p, vec2<f32>(-0.18, -0.36), vec2<f32>(-0.3, -0.4)) - 0.006);
    d = min(d, sd_segment(p, vec2<f32>(-0.18, -0.36), vec2<f32>(-0.2, -0.5)) - 0.005);
    d = min(d, sd_segment(p, vec2<f32>(0.02, -0.38), vec2<f32>(0.08, -0.55)) - 0.007);
    d = min(d, sd_segment(p, vec2<f32>(0.02, -0.38), vec2<f32>(-0.06, -0.54)) - 0.006);
    return d;
}

// Haunted house: origin at the ground line, y negative is up. Returns
// (silhouette distance, lit-window mask).
fn house(p: vec2<f32>) -> vec2<f32> {
    var d = sd_box(p - vec2<f32>(0.0, -0.11), vec2<f32>(0.17, 0.11));
    d = min(d, sd_tri(p, vec2<f32>(-0.21, -0.21), vec2<f32>(0.21, -0.21), vec2<f32>(0.0, -0.37)));
    d = min(d, sd_box(p - vec2<f32>(0.11, -0.31), vec2<f32>(0.022, 0.06)));
    d = min(d, sd_box(p - vec2<f32>(-0.24, -0.09), vec2<f32>(0.06, 0.09)));
    d = min(d, sd_tri(p - vec2<f32>(-0.24, -0.18), vec2<f32>(-0.08, 0.0), vec2<f32>(0.08, 0.0), vec2<f32>(0.0, -0.08)));
    // One lit window in the side wing; two dark ones upstairs.
    let lit = sd_box(p - vec2<f32>(-0.24, -0.08), vec2<f32>(0.02, 0.032));
    return vec2<f32>(d, smoothstep(0.004, -0.002, lit));
}

// Bat silhouette with flapping, scalloped wings. `flap` in -1..1.
fn bat(p_in: vec2<f32>, flap: f32) -> f32 {
    var p = p_in;
    p.x = abs(p.x);
    let body = length(p / vec2<f32>(0.6, 1.0)) - 0.018;
    // Wing: rotate up/down about the shoulder, scallop the trailing edge.
    let q = rot(-flap * 0.6) * (p - vec2<f32>(0.01, 0.0));
    let span = 0.09;
    let top = -0.012 - q.x * 0.25;
    let bottom = 0.02 - 0.012 * abs(sin(q.x / span * 3.0 * PI)) + q.x * 0.1;
    let wing = max(max(top - q.y, q.y - bottom), max(-q.x, q.x - span));
    return min(body, wing);
}

// Jack-o'-lantern: ribbed pumpkin with a glowing carved face. Returns
// (silhouette distance, carved-glow amount).
fn pumpkin(p: vec2<f32>, size: f32) -> vec2<f32> {
    let q = p / size;
    let a = angle(q);
    let ribs = 1.0 + 0.05 * cos(a * 6.0);
    let body = length(q / vec2<f32>(1.25, 1.0)) - ribs;
    let stem = sd_box(q - vec2<f32>(0.05, -1.05), vec2<f32>(0.1, 0.18));
    var face = sd_tri(q, vec2<f32>(-0.55, -0.15), vec2<f32>(-0.2, -0.15), vec2<f32>(-0.37, -0.45));
    face = min(face, sd_tri(q, vec2<f32>(0.2, -0.15), vec2<f32>(0.55, -0.15), vec2<f32>(0.37, -0.45)));
    face = min(face, sd_tri(q, vec2<f32>(-0.1, 0.12), vec2<f32>(0.1, 0.12), vec2<f32>(0.0, -0.05)));
    // Jagged grin.
    let mx = q.x;
    let mouth_top = 0.3 + 0.08 * abs(fract(mx * 3.0) - 0.5);
    let mouth_bot = 0.52 - 0.12 * (1.0 - mx * mx * 2.5) + 0.08 * abs(fract(mx * 3.0 + 0.5) - 0.5);
    let mouth = max(max(mouth_top - q.y, q.y - mouth_bot), abs(mx) - 0.6);
    face = min(face, mouth);
    return vec2<f32>(min(body, stem) * size, smoothstep(0.03, -0.02, face) * step(body, 0.0));
}

// Height of the graveyard hill at view-space x.
fn hill_h(x: f32, asp: f32) -> f32 {
    return 0.42 - 0.1 * exp(-pow(x + asp * 0.25, 2.0) * 1.5) + 0.02 * fbm(vec2<f32>(x * 3.0, 1.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);            // y grows downward
    let t = u.time;
    let asp = aspect();
    let strike_k = floor(u.beat);

    // Sky: deep purple to black, with faint stars.
    var col = mix(vec3<f32>(0.10, 0.04, 0.16), vec3<f32>(0.01, 0.0, 0.03), clamp(-p.y * 0.8 + 0.3, 0.0, 1.0));
    let star = step(0.996, hash21(floor(in.uv * vec2<f32>(u.res_x, u.res_y) / 3.0)));
    col += star * (0.4 + 0.3 * sin(t * 2.0 + hash21(floor(in.uv * 300.0)) * 20.0)) * step(p.y, 0.2);

    // Moon with craters; the halo breathes gently on the beat.
    let mc = vec2<f32>(asp * 0.38, -0.42);
    let mr = 0.42;
    let md = length(p - mc);
    let crater = fbm((p - mc) * 6.0 + 3.0);
    let maria = smoothstep(0.45, 0.65, fbm((p - mc) * 2.5 + 11.0));      // darker "seas"
    let moon = vec3<f32>(0.72, 0.6, 0.42) * (0.55 + 0.45 * crater) * (1.0 - 0.3 * maria);
    let breathe = 1.0 + 0.35 * beat_pulse(3.0) * (0.3 + 0.7 * u.kick);
    col += vec3<f32>(1.0, 0.55, 0.2) * 0.25 * breathe / (1.0 + pow(max(md - mr, 0.0) * 6.0, 2.0));
    col = mix(col, moon, smoothstep(mr + 0.004, mr - 0.004, md));
    // Clouds drifting across the moon, faster when the track drives.
    let cl = fbm(vec2<f32>(p.x * 1.2 + t * (0.03 + 0.05 * u.energy), p.y * 3.0));
    let cloud_band = smoothstep(0.55, 0.8, cl) * smoothstep(0.3, -0.6, p.y);
    col = mix(col, vec3<f32>(0.08, 0.04, 0.1), cloud_band * 0.85);

    // Distant lightning on hard beats: a jagged bolt in the cloud band plus a
    // bloom that lights the clouds and rims the hill. Beat-hashed so it fires
    // sparsely in quiet sections and often in driven ones — never a strobe.
    let striking = step(1.0 - 0.45 * u.intensity, hash21(vec2<f32>(strike_k, 31.0)));
    let lflick = striking * exp(-u.beat_phase * 5.0)
               * (0.55 + 0.45 * sin(u.beat_phase * 80.0 + hash21(vec2<f32>(strike_k, 7.0)) * 20.0));
    let bolt_x = (hash21(vec2<f32>(strike_k, 5.0)) - 0.5) * 1.7 * asp;
    let band_i = floor((p.y + 1.1) * 9.0);
    let jag_x = bolt_x + (hash21(vec2<f32>(band_i, strike_k)) - 0.5) * 0.24;
    let bolt = smoothstep(0.004, 0.0, abs(p.x - jag_x))
             * step(-1.05, p.y) * step(p.y, 0.08 + 0.1 * hash21(vec2<f32>(strike_k, 3.0)));
    let lglow = exp(-abs(p.x - bolt_x) * 2.2) * smoothstep(0.3, -0.7, p.y);
    col += lflick * (vec3<f32>(0.85, 0.9, 1.0) * bolt * 3.0
                   + vec3<f32>(0.35, 0.38, 0.6) * lglow * (0.4 + 0.6 * cloud_band));

    // Bats across the sky: more of them and quicker as the energy climbs.
    let bat_count = 4 + i32(round(u.intensity * 6.0));
    for (var i = 0; i < 10; i++) {
        if i >= bat_count {
            break;
        }
        let fi = f32(i);
        let speed = (0.12 + 0.05 * hash21(vec2<f32>(fi, 3.0))) * (0.7 + 0.7 * u.energy);
        let x = ((t * speed + hash21(vec2<f32>(fi, 7.0)) * 4.0) % 4.0) - 2.0;
        let y = -0.75 + 0.5 * hash21(vec2<f32>(fi, 11.0)) + 0.05 * sin(t * 1.3 + fi);
        let size = 0.6 + 0.8 * hash21(vec2<f32>(fi, 13.0));
        let flap = sin(t * (9.0 + fi) * (0.8 + 0.4 * u.energy) + fi);
        let b = bat((p - vec2<f32>(x * asp, y)) / size, flap) * size;
        col = mix(col, vec3<f32>(0.0), smoothstep(0.004, 0.0, b));
    }

    // Graveyard hill with tombstones and the dead tree, in silhouette, plus
    // the haunted house on the right crest.
    let hill_y = hill_h(p.x, asp);
    var sil = hill_y - p.y;             // negative below the hill line (y grows downward)
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let gx = -asp * 0.6 + fi * 0.22 + 0.05 * hash21(vec2<f32>(fi, 2.0));
        let gh = 0.05 + 0.04 * hash21(vec2<f32>(fi, 5.0));
        let lean = rot((hash21(vec2<f32>(fi, 9.0)) - 0.5) * 0.3);
        var g = sd_box(lean * (p - vec2<f32>(gx, hill_h(gx, asp) - gh)), vec2<f32>(0.035, gh)) - 0.012;
        if i % 2 == 1 {    // a few crosses
            let q = lean * (p - vec2<f32>(gx, hill_h(gx, asp) - gh));
            g = min(sd_box(q, vec2<f32>(0.01, gh + 0.02)), sd_box(q + vec2<f32>(0.0, gh * 0.4), vec2<f32>(0.04, 0.01)));
        }
        sil = min(sil, g);
    }
    sil = min(sil, tree(p - vec2<f32>(-asp * 0.35, hill_h(-asp * 0.35, asp) - 0.08)));
    let house_x = asp * 0.58;
    let hpos = p - vec2<f32>(house_x, hill_h(house_x, asp) - 0.02);
    let hs = house(hpos);
    sil = min(sil, hs.x);
    col = mix(col, vec3<f32>(0.015, 0.0, 0.03), smoothstep(0.003, -0.003, sil));
    // Lightning rims the crest while it fires.
    col += vec3<f32>(0.3, 0.32, 0.5) * lflick * exp(-abs(sil) * 90.0) * 0.6;
    // The house window flickers with the bass — someone is home.
    let wflame = 0.4 + 0.6 * noise(vec2<f32>(t * 7.0, 3.0)) + 0.5 * u.bass;
    col += vec3<f32>(1.0, 0.6, 0.15) * hs.y * wflame * 0.9;
    col += vec3<f32>(1.0, 0.55, 0.1) * hs.y * wflame * 0.35 / (1.0 + pow(length(hpos + vec2<f32>(0.24, 0.08)) * 30.0, 2.0));

    // Ground fog: thicker and faster when the track drives.
    let fog = fbm(vec2<f32>(p.x * 1.5 - t * (0.08 + 0.1 * u.energy), p.y * 4.0 + t * 0.05));
    let fog_band = smoothstep(0.25, 0.75, p.y);
    col += vec3<f32>(0.3, 0.25, 0.4) * fog * fog * fog_band * (0.15 + 0.3 * u.energy);

    // Jack-o'-lanterns: each rides a different band (low / mid / high), so the
    // faces light up with different parts of the track. Smoothed, no strobe.
    let bands = vec3<f32>(spec(0.08), spec(0.5), spec(0.92));
    let pumpkins = array<vec3<f32>, 3>(vec3<f32>(-0.35, 0.78, 0.13), vec3<f32>(0.25, 0.83, 0.1), vec3<f32>(0.62, 0.8, 0.075));
    for (var i = 0; i < 3; i++) {
        let pk = pumpkins[i];
        let flicker = 0.7 + 0.3 * noise(vec2<f32>(t * 6.0, f32(i) + 1.0))
                    + 0.35 * u.intensity + bands[i] * 0.9;
        let c = vec2<f32>(pk.x * asp, pk.y);
        let r = pumpkin(p - c, pk.z);
        let lit = vec3<f32>(0.55, 0.22, 0.02) * (0.25 + 0.3 * flicker);
        let rim = exp(-abs(r.x) * 60.0) * 0.4;
        col = mix(col, lit * (0.6 + 0.4 * smoothstep(0.0, -pk.z, r.x)), smoothstep(0.002, -0.002, r.x));
        col += vec3<f32>(1.0, 0.6, 0.1) * r.y * flicker * 1.8;
        col += vec3<f32>(1.0, 0.45, 0.05) * (0.06 * flicker) / (1.0 + pow(length(p - c) / pk.z, 2.0)) + rim * vec3<f32>(0.8, 0.3, 0.0) * flicker * 0.3;
    }

    // Foreground wrought-iron fence: posts with spearheads and two rails.
    let fx = fract(p.x * asp / 0.11 + 0.5) - 0.5;
    var fence = abs(p.y - 0.93) - 0.0035;
    fence = min(fence, abs(p.y - 0.985) - 0.003);
    let post = max(abs(fx) * 0.11 - 0.005, 0.9 - p.y);
    let spear = sd_tri(vec2<f32>(fx * 0.11, p.y - 0.9), vec2<f32>(-0.006, 0.0), vec2<f32>(0.006, 0.0), vec2<f32>(0.0, -0.022));
    fence = min(fence, min(post, spear));
    col = mix(col, vec3<f32>(0.01, 0.0, 0.02), smoothstep(0.003, -0.002, fence));
    col += vec3<f32>(0.35, 0.3, 0.5) * lflick * exp(-abs(fence) * 120.0) * 0.5;

    col += prev(in.uv) * 0.08;
    return vec4<f32>(col, 1.0);
}
