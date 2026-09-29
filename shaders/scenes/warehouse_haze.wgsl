// @heavy — 2026 tier. @bloom 0.8 @tonemap agx
// Hangar-scale warehouse rave seen from the balcony: two trusses of
// moving-head beam fixtures plus a row of floor uplights cutting through
// thick haze, a huge LED wall behind the rig, beams landing as hot spots on
// a damp concrete floor between rows of pillars.
// Beams are *analytic*: each is a Gaussian tube, and its in-scattering along
// the view ray is the closed-form line integral at the closest approach
// (P / (√π·r·sinα) · e^(−d²/r²)) — 22 volumetric beams for the price of a few
// dot products each, with one noise fetch per beam for haze texture.
// Pan/tilt are poses of the tempo clock (sweeps, fans, crossings), so motion
// is smooth; the music drives each beam's brightness from its own band.

const N_A: i32 = 10;          // front truss (rear truss/uplights below)
const N_B: i32 = 6;           // rear truss
const N_UP: i32 = 6;          // floor uplights
const N_BEAMS: i32 = 22;
const ROOM_W: f32 = 22.0;
const ROOM_H: f32 = 17.0;
const WALL_Z: f32 = 34.0;

fn beam_pos(i: i32) -> vec3<f32> {
    if i < N_A {
        return vec3<f32>((f32(i) - 4.5) * 3.2, 11.0, 16.0);
    }
    if i < N_A + N_B {
        return vec3<f32>((f32(i - N_A) - 2.5) * 5.4, 14.0, 25.0);
    }
    return vec3<f32>((f32(i - N_A - N_B) - 2.5) * 5.0, 0.4, 28.0);
}

// Beam direction: a choreography of poses over bar/phrase time.
fn beam_dir(i: i32) -> vec3<f32> {
    let fi = f32(i);
    // Energy: the rig moves faster as the track drives.
    let ph = u.clock4.x * 0.125;
    let form = 0.5 + 0.5 * sin(u.clock4.x * 0.03125 * PI); // slow formation morph
    if i >= N_A + N_B {
        // Uplights: tall fans up into the roof haze, swaying.
        let k = f32(i - N_A - N_B) - 2.5;
        let pan = k * 0.08 + sin(ph * TAU * 0.5 + k * 0.4) * 0.3;
        let lean = 0.25 + 0.15 * sin(ph * TAU + k);
        return normalize(vec3<f32>(sin(pan), 1.0, -lean));
    }
    let n = select(f32(N_A), f32(N_B), i >= N_A);
    let k = select(fi, fi - f32(N_A), i >= N_A) - (n - 1.0) * 0.5;
    let rear = select(0.0, 1.0, i >= N_A);
    // Fan: spread symmetrically; sweep: all pan together; cross: mirrored.
    // Shape: the fan spreads wider as the mids build.
    let fan = k * (0.06 + 0.08 * u.pres4.y) * (0.6 + 0.4 * sin(ph * TAU));
    let sweep = sin(ph * TAU * 0.5 + fi * 0.15 + rear) * 0.5;
    let cross = sin(ph * TAU + rear * PI) * 0.45 * sign(k);
    let pan = mix(fan + sweep * 0.4, cross, form);
    let tilt = 0.72 + 0.16 * sin(ph * TAU * 0.5 + fi * 0.6) * (0.5 + 0.5 * form) - rear * 0.12;
    // Tilt measured down from pointing at the crowd (−z).
    return normalize(vec3<f32>(sin(pan), -sin(tilt), -cos(pan) * cos(tilt)));
}

fn beam_col(i: i32) -> vec3<f32> {
    if i >= N_A + N_B {
        return palette(0.75 + f32(i) * 0.01);
    }
    return palette(f32(i % N_A) / f32(N_A) * 0.5 + select(0.05, 0.4, i >= N_A));
}

fn beam_power(i: i32) -> f32 {
    let s = spec(f32(i % 10) / 10.0 * 0.8 + 0.05);
    var p = (0.4 + 1.3 * s) * (0.45 + 0.8 * u.intensity);
    if i >= N_A + N_B {
        // Uplights hit on the kick.
        p *= 0.5 + 1.2 * u.kick;
    }
    return p;
}

fn box_t(ro: vec3<f32>, rd: vec3<f32>, c: vec3<f32>, h: vec3<f32>) -> f32 {
    let inv = 1.0 / rd;
    let t0 = (c - h - ro) * inv;
    let t1 = (c + h - ro) * inv;
    let tn = max(max(min(t0.x, t1.x), min(t0.y, t1.y)), min(t0.z, t1.z));
    let tf = min(min(max(t0.x, t1.x), max(t0.y, t1.y)), max(t0.z, t1.z));
    return select(-1.0, tn, tn < tf && tn > 0.0);
}

// Nearest opaque hit. Returns (t, material):
// 0 floor, 1 wall/ceiling, 2 pillar, 3 truss, 4 LED back wall.
fn scene(ro: vec3<f32>, rd: vec3<f32>) -> vec2<f32> {
    var best = vec2<f32>(1e5, 1.0);
    if rd.y < 0.0 {
        best = vec2<f32>(-ro.y / rd.y, 0.0);
    } else if rd.y > 0.0 {
        best = vec2<f32>((ROOM_H - ro.y) / rd.y, 1.0);
    }
    let tw = (sign(rd.x) * ROOM_W - ro.x) / rd.x;
    if tw > 0.0 && tw < best.x {
        best = vec2<f32>(tw, 1.0);
    }
    let tb = (WALL_Z - ro.z) / rd.z;
    if rd.z > 0.0 && tb < best.x {
        best = vec2<f32>(tb, 4.0);
    }
    // Pillars: square columns in two rows.
    for (var k = 0; k < 5; k++) {
        for (var sgn = -1; sgn <= 1; sgn += 2) {
            let tp = box_t(ro, rd, vec3<f32>(f32(sgn) * 13.0, ROOM_H * 0.5, f32(k) * 8.0 - 2.0), vec3<f32>(0.6, ROOM_H * 0.5, 0.6));
            if tp > 0.0 && tp < best.x {
                best = vec2<f32>(tp, 2.0);
            }
        }
    }
    // Trusses.
    let ta = box_t(ro, rd, vec3<f32>(0.0, 11.3, 16.0), vec3<f32>(17.0, 0.3, 0.3));
    if ta > 0.0 && ta < best.x {
        best = vec2<f32>(ta, 3.0);
    }
    let tt = box_t(ro, rd, vec3<f32>(0.0, 14.3, 25.0), vec3<f32>(19.0, 0.3, 0.3));
    if tt > 0.0 && tt < best.x {
        best = vec2<f32>(tt, 3.0);
    }
    return best;
}

fn led_wall(q: vec2<f32>) -> vec3<f32> {
    let cell = fract(q * 2.0);
    let px = smoothstep(0.5, 0.36, max(abs(cell.x - 0.5), abs(cell.y - 0.5)));
    let xn = clamp((q.x + ROOM_W) / (2.0 * ROOM_W), 0.0, 1.0);
    let bar = spec(abs(xn - 0.5) * 1.6);                 // mirrored analyser
    let lvl = smoothstep(bar * ROOM_H * 0.8 + 0.4, bar * ROOM_H * 0.8, q.y);
    let base = palette(q.x * 0.02 + q.y * 0.03 + u.flow * 0.02);
    return base * px * (0.12 + 1.1 * lvl * (0.4 + 0.8 * u.intensity));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let ro = vec3<f32>(sin(u.flow * 0.011) * 3.0, 5.0 + 0.3 * sin(u.flow * 0.017), -6.0 + sin(u.flow * 0.007) * 1.5);
    let ta = vec3<f32>(sin(u.flow * 0.009) * 3.0, 7.5, 20.0);
    let rd = cam_ray(p, ro, ta, sin(u.flow * 0.006) * 0.02, 1.0);

    let hit = scene(ro, rd);
    let t_hit = hit.x;
    let hp = ro + rd * t_hit;
    let drift = vec3<f32>(u.time * 0.012, u.time * 0.004, u.time * -0.006);

    // --- Surfaces ----------------------------------------------------------
    let mat = i32(hit.y + 0.5);
    let conc = tnoise(hp * 0.12 + 0.5);
    var albedo = vec3<f32>(0.14, 0.14, 0.145) * (0.55 + 0.9 * conc.b);
    if mat == 3 {
        albedo = vec3<f32>(0.3);
    }
    // Bounce light: all beams scattered around the room + the LED wall.
    var bounce = vec3<f32>(0.0);
    for (var i = 0; i < N_BEAMS; i++) {
        bounce += beam_col(i) * beam_power(i);
    }
    let wall_glow = mix(palette(0.3), vec3<f32>(0.35, 0.35, 0.5), 0.6) * (0.3 + 0.7 * u.intensity);
    var irr = vec3<f32>(0.01, 0.01, 0.016);
    irr += bounce * 0.012 * (0.5 + 0.5 * smoothstep(ROOM_H, 0.0, hp.y));
    irr += wall_glow * 0.25 * exp(-(WALL_Z - hp.z) * 0.06);
    for (var i = 0; i < N_BEAMS; i++) {
        let bp = beam_pos(i);
        let bd = beam_dir(i);
        let v = hp - bp;
        let s = dot(v, bd);
        if s > 0.0 {
            let rr = 0.05 + s * 0.022;
            let d2 = dot(v, v) - s * s;
            let x = d2 / (rr * rr);
            irr += beam_col(i) * beam_power(i) * exp(-x * x) / (rr * rr) * 0.05;
        }
    }
    var col = albedo * irr;
    if mat == 4 {
        col = led_wall(hp.xy) + albedo * irr * 0.3;
    }
    if mat == 0 {
        // Damp concrete: wet patches mirror the LED wall and the lenses.
        let wet = smoothstep(0.6, 0.64, tnoise(vec3<f32>(hp.xz * 0.11, 0.3)).g);
        let rr = reflect(rd, vec3<f32>(0.0, 1.0, 0.0));
        var spec_c = vec3<f32>(0.0);
        for (var i = 0; i < N_A + N_B; i++) {
            let to = normalize(beam_pos(i) - hp);
            spec_c += beam_col(i) * beam_power(i) * pow(max(dot(rr, to), 0.0), 400.0) * 4.0;
        }
        if rr.z > 0.0 {
            let tw = (WALL_Z - hp.z) / rr.z;
            let wq = hp + rr * tw;
            if wq.y < ROOM_H {
                spec_c += led_wall(wq.xy) * 0.3;
            }
        }
        col += spec_c * wet * fresnel(0.04, -rd.y) * 2.5;
    }

    // --- Haze --------------------------------------------------------------
    // Uniform haze lit by the LED wall and the rig — the room glows.
    let fog = 1.0 - exp(-t_hit * 0.02);
    let haze_c = (vec3<f32>(0.012, 0.011, 0.02) + wall_glow * 0.03 + bounce * 0.0025) * (0.6 + 0.6 * u.intensity);
    col = mix(col, haze_c, fog);

    // Analytic volumetric beams.
    for (var i = 0; i < N_BEAMS; i++) {
        let bp = beam_pos(i);
        let bd = beam_dir(i);
        let w0 = ro - bp;
        let b = dot(rd, bd);
        let dd = dot(rd, w0);
        let e = dot(bd, w0);
        let den = max(1.0 - b * b, 1e-4);
        let tc = (b * e - dd) / den;
        let sc = (e - b * dd) / den;
        // Stop at the floor (or the roof for uplights).
        let s_end = select(select(120.0, -bp.y / bd.y, bd.y < -0.01), (ROOM_H - bp.y) / bd.y, bd.y > 0.01);
        let s = clamp(sc, 0.0, s_end);
        let dist = length(ro + rd * tc - (bp + bd * s));
        let rr = 0.05 + s * 0.022;
        let vis = smoothstep(0.0, 0.5, tc) * smoothstep(0.0, 0.8, t_hit - tc);
        let haze = 0.4 + 1.3 * tnoise((bp + bd * s) * 0.06 + drift).r;
        let lat = exp(-dist * dist / (rr * rr)) / (1.7725 * rr * max(sqrt(den), 0.12));
        let along = exp(-s * 0.02) * smoothstep(0.0, 0.3, s);
        col += beam_col(i) * beam_power(i) * lat * haze * along * vis * 0.06;
        // The fixture's lens: a hot point looking into the beam.
        let lv = bp - ro;
        let lt = dot(lv, rd);
        let ld = length(lv - rd * lt) / max(lt, 1.0);
        col += beam_col(i) * beam_power(i) * smoothstep(0.006, 0.0, ld) * (0.5 + 3.0 * pow(max(dot(bd, -normalize(lv)), 0.0), 8.0)) * step(0.0, lt);
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
