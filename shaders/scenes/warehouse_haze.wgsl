// @heavy — 2026 tier. @bloom 0.8 @tonemap agx
// Concrete warehouse rave: a truss of moving-head beam fixtures cutting
// through thick haze, beams landing as hot spots on a damp concrete floor
// between the pillars. Beams are *analytic*: each is a Gaussian tube, and
// its in-scattering along the view ray is the closed-form line integral at
// the closest approach (P / (√π·r·sinα) · e^(−d²/r²)) — 8 volumetric beams for
// the price of a few dot products, with one noise fetch each for haze texture.
// Pan/tilt are poses of the tempo clock (sweeps, fans, crossings), so motion
// is smooth; the music drives each beam's brightness from its own band.

const N_BEAMS: i32 = 8;
const ROOM_W: f32 = 9.0;
const ROOM_H: f32 = 7.5;
const TRUSS_Z: f32 = 16.0;
const TRUSS_Y: f32 = 6.2;

fn beam_pos(i: i32) -> vec3<f32> {
    let x = (f32(i) - 3.5) * 1.9;
    return vec3<f32>(x, TRUSS_Y - 0.25, TRUSS_Z);
}

// Beam direction: a choreography of poses over bar/phrase time.
fn beam_dir(i: i32) -> vec3<f32> {
    let fi = f32(i);
    let side = fi - 3.5;
    let ph = u.flow * 0.125;                           // one cycle / 8 beats
    let form = 0.5 + 0.5 * sin(u.flow * 0.03125 * PI); // slow formation morph
    // Fan: spread symmetrically; sweep: all pan together; cross: mirrored.
    let fan = side * 0.13 * (0.6 + 0.4 * sin(ph * TAU));
    let sweep = sin(ph * TAU * 0.5 + fi * 0.15) * 0.55;
    let cross = sin(ph * TAU) * 0.5 * sign(side);
    let pan = mix(fan + sweep * 0.4, cross, form);
    let tilt = 0.44 + 0.16 * sin(ph * TAU * 0.5 + fi * 0.6) * (0.5 + 0.5 * form);
    // Tilt measured down from pointing at the crowd (−z).
    return normalize(vec3<f32>(sin(pan), -sin(tilt) * 0.9, -cos(pan) * cos(tilt)));
}

fn beam_col(i: i32) -> vec3<f32> {
    return palette(f32(i) / f32(N_BEAMS) * 0.6 + 0.05);
}

fn beam_power(i: i32) -> f32 {
    let s = spec(f32(i) / f32(N_BEAMS) * 0.8 + 0.05);
    return (0.35 + 1.3 * s) * (0.35 + 0.9 * u.intensity);
}

// Nearest opaque hit: floor, ceiling, side walls, back wall, pillars, truss.
// Returns (t, material): 0 floor, 1 wall/ceiling, 2 pillar, 3 truss.
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
    let tb = (TRUSS_Z + 6.0 - ro.z) / rd.z;
    if rd.z > 0.0 && tb < best.x {
        best = vec2<f32>(tb, 4.0);
    }
    // Pillars: square columns in two rows.
    for (var k = 0; k < 4; k++) {
        for (var sgn = -1; sgn <= 1; sgn += 2) {
            let c = vec3<f32>(f32(sgn) * 5.2, ROOM_H * 0.5, f32(k) * 6.0 + 2.0);
            let h = vec3<f32>(0.35, ROOM_H * 0.5, 0.35);
            let inv = 1.0 / rd;
            let t0 = (c - h - ro) * inv;
            let t1 = (c + h - ro) * inv;
            let tn = max(max(min(t0.x, t1.x), min(t0.y, t1.y)), min(t0.z, t1.z));
            let tf = min(min(max(t0.x, t1.x), max(t0.y, t1.y)), max(t0.z, t1.z));
            if tn < tf && tn > 0.0 && tn < best.x {
                best = vec2<f32>(tn, 2.0);
            }
        }
    }
    // Truss: a long box bar.
    {
        let c = vec3<f32>(0.0, TRUSS_Y, TRUSS_Z);
        let h = vec3<f32>(8.0, 0.18, 0.18);
        let inv = 1.0 / rd;
        let t0 = (c - h - ro) * inv;
        let t1 = (c + h - ro) * inv;
        let tn = max(max(min(t0.x, t1.x), min(t0.y, t1.y)), min(t0.z, t1.z));
        let tf = min(min(max(t0.x, t1.x), max(t0.y, t1.y)), max(t0.z, t1.z));
        if tn < tf && tn > 0.0 && tn < best.x {
            best = vec2<f32>(tn, 3.0);
        }
    }
    return best;
}

// Where beam i stops (floor), in beam-length units.
fn beam_end(i: i32) -> f32 {
    let d = beam_dir(i);
    return select(80.0, (0.0 - beam_pos(i).y) / d.y, d.y < -0.01);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let ro = vec3<f32>(sin(u.flow * 0.011) * 2.0, 2.6 + 0.15 * sin(u.flow * 0.017), -10.0 + sin(u.flow * 0.007) * 1.2);
    let ta = vec3<f32>(sin(u.flow * 0.009) * 1.5, 3.0, TRUSS_Z);
    let rd = cam_ray(p, ro, ta, sin(u.flow * 0.006) * 0.02, 1.45);

    let hit = scene(ro, rd);
    let t_hit = hit.x;
    let hp = ro + rd * t_hit;
    let drift = vec3<f32>(u.time * 0.012, u.time * 0.004, u.time * -0.006);

    // --- Surfaces ----------------------------------------------------------
    var col = vec3<f32>(0.0);
    let mat = i32(hit.y + 0.5);
    let conc = tnoise(hp * 0.21 + 0.5);
    var albedo = vec3<f32>(0.1, 0.1, 0.105) * (0.55 + 0.9 * conc.b);
    if mat == 3 {
        albedo = vec3<f32>(0.25);
    }
    // Irradiance from beams landing nearby (floor mostly), plus ambient
    // haze glow toward the stage.
    var irr = vec3<f32>(0.004, 0.004, 0.006);
    // Bounce: the beams' light scattered around the room, falling off away
    // from the rig — enough to read the concrete and the pillars.
    var bounce = vec3<f32>(0.0);
    for (var i = 0; i < N_BEAMS; i++) {
        bounce += beam_col(i) * beam_power(i);
    }
    irr += bounce * 0.035 * exp(-abs(hp.z - 6.0) * 0.08) * (0.5 + 0.5 * smoothstep(7.0, 0.0, hp.y));
    irr += vec3<f32>(0.05, 0.04, 0.06) * palette(0.5) * exp(-abs(hp.z - TRUSS_Z) * 0.15) * (0.3 + 0.5 * u.intensity);
    for (var i = 0; i < N_BEAMS; i++) {
        let bp = beam_pos(i);
        let bd = beam_dir(i);
        let v = hp - bp;
        let s = dot(v, bd);
        if s > 0.0 {
            let rr = 0.035 + s * 0.022;
            let d2 = dot(v, v) - s * s;
            let x = d2 / (rr * rr);
            irr += beam_col(i) * beam_power(i) * exp(-x * x) / (rr * rr) * 0.05;
        }
    }
    col = albedo * irr;
    if mat == 4 {
        // Back wall: a dim LED panel wall — slow spectrum gradient behind the
        // truss, so pillars and fixtures silhouette against it.
        let q = hp.xy;
        let cell = fract(q * vec2<f32>(3.0, 3.0));
        let px = smoothstep(0.5, 0.35, max(abs(cell.x - 0.5), abs(cell.y - 0.5)));
        let bar = spec(clamp((q.x + ROOM_W) / (2.0 * ROOM_W), 0.0, 1.0) * 0.8);
        let lvl = smoothstep(bar * ROOM_H * 0.9 + 0.3, bar * ROOM_H * 0.9, q.y);
        let led = palette(q.x * 0.03 + q.y * 0.04 + u.time * 0.02) * (0.08 + 0.35 * lvl * u.intensity);
        col = led * px * 0.6 + albedo * irr;
    }
    if mat == 0 {
        // Damp concrete: patches of rough reflection of the beam sources.
        let wet = smoothstep(0.45, 0.62, tnoise(vec3<f32>(hp.xz * 0.07, 0.3)).g);
        let rr = reflect(rd, vec3<f32>(0.0, 1.0, 0.0));
        var spec_c = vec3<f32>(0.0);
        for (var i = 0; i < N_BEAMS; i++) {
            let to = normalize(beam_pos(i) - hp);
            let al = max(dot(rr, to), 0.0);
            spec_c += beam_col(i) * beam_power(i) * pow(al, 300.0) * 3.0;
        }
        col += spec_c * wet * fresnel(0.04, -rd.y) * 6.0;
    }

    // --- Haze --------------------------------------------------------------
    // Uniform haze: dims the far room and glows faintly with the stage wash.
    let fog = 1.0 - exp(-t_hit * 0.045);
    col = mix(col, vec3<f32>(0.012, 0.01, 0.018) * (0.6 + 0.8 * u.intensity), fog);

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
        let s_end = beam_end(i);
        let s = clamp(sc, 0.0, s_end);
        let cp_ray = ro + rd * tc;
        let dist = length(cp_ray - (bp + bd * s));
        let rr = 0.035 + s * 0.022;
        let sin_a = sqrt(den);
        // Visible only in front of the camera and before the ray hits.
        let vis = smoothstep(0.0, 0.5, tc) * smoothstep(0.0, 0.8, t_hit - tc);
        let haze = 0.35 + 1.4 * tnoise((bp + bd * s) * 0.09 + drift).r;
        let lat = exp(-dist * dist / (rr * rr)) / (1.7725 * rr * max(sin_a, 0.12));
        // Beam brightness falls off along its length (spread + extinction).
        let along = exp(-s * 0.025) * smoothstep(0.0, 0.3, s);
        col += beam_col(i) * beam_power(i) * lat * haze * along * vis * 0.03;
        // The fixture's lens: a hot point looking into the beam.
        let lens_v = bp - ro;
        let lt = dot(lens_v, rd);
        let ld = length(lens_v - rd * lt);
        let facing = max(dot(-bd, normalize(lens_v * -1.0 + bd * 0.0)), 0.0);
        col += beam_col(i) * beam_power(i) * smoothstep(0.09, 0.0, ld) * (0.4 + 3.0 * pow(max(dot(bd, -normalize(lens_v)), 0.0), 8.0)) * step(0.0, lt) * (0.3 + facing * 0.0);
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
