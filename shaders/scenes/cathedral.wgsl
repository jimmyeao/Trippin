// @heavy — 2026 tier. @bloom 0.8 @tonemap agx
// A gothic cathedral nave: clustered piers, a pointed vault, tall lancet
// windows of stained glass, and low sun pouring through them in coloured
// shafts through incense haze, laying the glass patterns on the floor. At
// the far end a rose window and an organ whose pipes are LED columns.
// Shafts are traced backwards: for each haze sample, project along the sun
// direction onto the left wall; if that lands in a window, the air there is
// lit with that pane's colour. The same test lights the floor.
// The camera glides up the nave on the tempo clock. Organ pipes show the
// spectrum, the rose window's rings follow the bands, and the shafts swell
// with the track's intensity.

const HW: f32 = 5.0;          // nave half-width
const BAY: f32 = 5.0;         // column spacing
const SPRING: f32 = 8.0;      // vault springline height

fn sun_dir() -> vec3<f32> {
    return normalize(vec3<f32>(1.0, -0.55, 0.3));   // travelling into the nave
}

// Real stained glass is mostly cobalt and ruby, with emerald, amethyst and
// gold accents (the global palette is kept for the LED organ / rose).
fn jewel(h: f32) -> vec3<f32> {
    if h < 0.38 {
        return vec3<f32>(0.08, 0.22, 0.85);
    }
    if h < 0.66 {
        return vec3<f32>(0.8, 0.07, 0.1);
    }
    if h < 0.78 {
        return vec3<f32>(0.1, 0.55, 0.25);
    }
    if h < 0.9 {
        return vec3<f32>(0.45, 0.15, 0.7);
    }
    return vec3<f32>(1.0, 0.7, 0.2);
}

// Stained glass at a wall point (lz across the bay, y up). Zero outside the
// lancet window. `detailed`: irregular Voronoi panes with lead lines (seen
// directly); otherwise a coarse, cheap version for shafts and projections.
fn window_glass(lz: f32, y: f32, seed: f32, detailed: bool) -> vec3<f32> {
    if abs(lz) > 0.95 || y < 2.8 {
        return vec3<f32>(0.0);
    }
    if y > SPRING && (length(vec2<f32>(lz + 0.95, y - SPRING)) > 1.9 || length(vec2<f32>(lz - 0.95, y - SPRING)) > 1.9) {
        return vec3<f32>(0.0);
    }
    if !detailed {
        let cell = floor(vec2<f32>(lz * 2.0, y * 1.4));
        return jewel(hash21(cell + seed * 17.0)) * 0.8;
    }
    let g = vec2<f32>(lz * 4.0, y * 3.0);
    let base = floor(g);
    var d1 = 9.0;
    var d2 = 9.0;
    var id = vec2<f32>(0.0);
    for (var k = 0; k < 9; k++) {
        let c = base + vec2<f32>(f32(k % 3) - 1.0, f32(k / 3) - 1.0);
        let pt = c + 0.15 + hash22(c + seed * 31.0) * 0.7;
        let d = length(g - pt);
        if d < d1 {
            d2 = d1;
            d1 = d;
            id = c;
        } else if d < d2 {
            d2 = d;
        }
    }
    let lead = smoothstep(0.06, 0.02, d2 - d1);
    let mullion = smoothstep(0.05, 0.025, abs(lz));
    // Coarse colour so the projected light matches what you see.
    let coarse = floor(vec2<f32>(lz * 2.0, y * 1.4));
    let c = mix(jewel(hash21(coarse + seed * 17.0)), jewel(hash21(id + seed)), 0.35);
    return c * (0.75 + 0.5 * hash21(id + 3.0)) * (1.0 - lead) * (1.0 - mullion);
}

// Window on the left wall seen from a point along the sun direction.
fn sunlit(s: vec3<f32>) -> vec3<f32> {
    let L = sun_dir();
    let t = (s.x + HW) / L.x;
    if t < 0.0 {
        return vec3<f32>(0.0);
    }
    let w = s - L * t;
    let zc = floor(w.z / BAY);
    let lz = w.z - (zc + 0.5) * BAY;
    // Sunlight through glass lands soft: no lead lines, lower saturation.
    let g = window_glass(lz, w.y, zc, false);
    let l = dot(g, vec3<f32>(0.3, 0.5, 0.2));
    return mix(vec3<f32>(l), g, 0.7);
}

// Air-positive field. Returns (distance, material): 0 floor, 1 wall/vault,
// 2 pier, 3 end wall.
fn map(p: vec3<f32>, end_z: f32) -> vec2<f32> {
    let side = HW - abs(p.x);
    let vault = 7.6 - length(vec2<f32>(abs(p.x) + 2.6, max(p.y - SPRING, 0.0)));
    var d = min(side, vault);
    var m = 1.0;
    if p.y < d {
        d = p.y;
        m = 0.0;
    }
    let endw = end_z - p.z;
    if endw < d {
        d = endw;
        m = 3.0;
    }
    // Clustered piers: a core plus four shafts.
    let cz = p.z - (floor(p.z / BAY) + 0.5) * BAY + BAY * 0.5;
    let q = vec2<f32>(abs(p.x) - (HW - 0.9), cz - BAY * 0.5 + BAY * 0.5);
    let qz = p.z - round(p.z / BAY) * BAY;
    let pq = vec2<f32>(abs(p.x) - (HW - 0.9), qz);
    let core = length(pq) - 0.5;
    let shafts = min(min(length(pq - vec2<f32>(0.5, 0.0)), length(pq + vec2<f32>(0.5, 0.0))),
                     min(length(pq - vec2<f32>(0.0, 0.5)), length(pq + vec2<f32>(0.0, 0.5)))) - 0.18;
    let pier = min(core, shafts);
    if pier < d {
        d = pier;
        m = 2.0;
    }
    // Transverse vault ribs.
    let rib = max(-vault + 0.25, abs(qz) - 0.15);
    if -rib > 0.0 && p.y > SPRING - 0.5 {
        // (ribs are a thin shading detail only)
    }
    return vec2<f32>(d, m);
}

fn normal(p: vec3<f32>, end_z: f32) -> vec3<f32> {
    let e = 0.01;
    let k = vec2<f32>(1.0, -1.0);
    return normalize(
        k.xyy * map(p + k.xyy * e, end_z).x + k.yyx * map(p + k.yyx * e, end_z).x +
        k.yxy * map(p + k.yxy * e, end_z).x + k.xxx * map(p + k.xxx * e, end_z).x
    );
}

// The far end: an organ of LED pipes under a rose window.
fn end_wall(p: vec3<f32>) -> vec3<f32> {
    var c = vec3<f32>(0.0);
    // Organ pipes: 24 columns, heights symmetric, lit up to the band level.
    let px = p.x / 3.6;
    if abs(px) < 1.0 && p.y < 6.5 && p.y > 0.8 {
        let id = floor((px * 0.5 + 0.5) * 24.0);
        let f = fract((px * 0.5 + 0.5) * 24.0);
        let pipe_h = 2.5 + 3.5 * (1.0 - abs(id - 11.5) / 12.0);
        let tube = smoothstep(0.45, 0.35, abs(f - 0.5));
        let band = abs(id - 11.5) / 12.0;
        let lvl = spec(band * 0.85) * (0.6 + 0.6 * u.intensity);
        let lit = smoothstep(0.8 + lvl * pipe_h + 0.05, 0.8 + lvl * pipe_h, p.y);
        let inpipe = tube * step(p.y, pipe_h + 0.8);
        c += (vec3<f32>(0.05, 0.045, 0.04) + palette(band * 0.6 + 0.1) * lit * 2.0) * inpipe;
    }
    // Rose window: concentric rings of glass, each ring a band.
    let rc = vec2<f32>(p.x, p.y - 10.5);
    let r = length(rc);
    if r < 2.6 {
        let a = angle(rc);
        let ring = floor(r / 2.6 * 5.0);
        let petal = abs(fract(a / TAU * 12.0 + ring * 0.5) - 0.5);
        let lead = smoothstep(0.42, 0.47, abs(fract(r / 2.6 * 5.0) - 0.5)) + smoothstep(0.44, 0.49, petal);
        let lvl = 0.3 + 1.6 * spec(ring / 5.0 * 0.8);
        c += palette(ring * 0.18 + petal) * lvl * (1.0 - clamp(lead, 0.0, 1.0)) * (0.5 + 0.7 * u.intensity);
    }
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.flow * 0.3;
    let ro = vec3<f32>(sin(u.flow * 0.01) * 1.2, 1.7 + 0.1 * sin(u.flow * 0.02), z);
    let end_z = z + 38.0;
    let ta = vec3<f32>(sin(u.flow * 0.008) * 1.0, 4.0 + sin(u.flow * 0.011) * 1.0, z + 10.0);
    let rd = cam_ray(p, ro, ta, 0.0, 1.2);
    let L = sun_dir();
    let drive = 0.5 + 0.8 * u.intensity;

    var t = 0.05;
    var m = -1.0;
    for (var i = 0; i < 90; i++) {
        let h = map(ro + rd * t, end_z);
        if h.x < 0.002 * t {
            m = h.y;
            break;
        }
        t += h.x * 0.9;
        if t > 60.0 {
            break;
        }
    }
    var col = vec3<f32>(0.0);
    if m >= 0.0 {
        let hp = ro + rd * t;
        let n = normal(hp, end_z);
        let stone = vec3<f32>(0.18, 0.16, 0.14) * (0.6 + 0.8 * tnoise(hp * 0.15).b) * (0.85 + 0.3 * tnoise(hp * 1.2).a);
        // Ambient: cool skylight from the clerestory, warmer low down.
        var light = vec3<f32>(0.06, 0.065, 0.08) * (0.5 + 0.5 * n.y) + vec3<f32>(0.035, 0.028, 0.02);
        // Direct sun through the glass (stained-glass patterns on surfaces).
        let ndl = max(dot(n, -L), 0.0);
        light += sunlit(hp) * ndl * 0.55 * drive;
        col = stone * light;
        if m < 0.5 {
            // Polished floor: a faint reflection of the lit windows.
            let rr = reflect(rd, n);
            col += sunlit(hp + rr * 3.0) * 0.02 * fresnel(0.04, -rd.y) * 4.0;
        }
        // Windows glow when seen directly (left wall backlit by the sun,
        // right wall by the sky).
        if m > 0.5 && m < 1.5 && abs(n.x) > 0.8 && hp.y < 12.0 {
            let zc = floor(hp.z / BAY);
            let lz = hp.z - (zc + 0.5) * BAY;
            let g = window_glass(lz, hp.y, zc + select(0.0, 50.0, hp.x > 0.0), true);
            col += g * select(0.2, 0.95, hp.x < 0.0) * drive;
        }
        if m > 2.5 {
            col += end_wall(hp);
        }
    }

    // Incense haze + coloured shafts: march the view ray, test each sample
    // against the windows along the sun direction.
    let tmax = min(select(60.0, t, m >= 0.0), 40.0);
    let jit = bluen(in.pos.xy);
    var shaft = vec3<f32>(0.0);
    let steps = 24;
    for (var i = 0; i < steps; i++) {
        let s = (f32(i) + jit) / f32(steps) * tmax;
        let q = ro + rd * s;
        let dens = 0.4 + 1.2 * tnoise(q * 0.07 + vec3<f32>(0.0, u.time * 0.01, u.time * 0.004)).r;
        shaft += sunlit(q) * dens;
    }
    // Forward scattering: brighter looking toward the sun.
    let phase = 0.4 + 1.2 * pow(max(dot(rd, -L), 0.0), 3.0);
    col += shaft / f32(steps) * tmax * 0.03 * phase * drive * (0.6 + 0.8 * u.intensity);
    // General haze.
    col = mix(col, vec3<f32>(0.02, 0.018, 0.02) * drive, 1.0 - exp(-t * 0.02));
    col += (bluen(in.pos.xy + 11.0) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
