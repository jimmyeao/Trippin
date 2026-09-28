// @heavy — 2026 tier. @bloom 0.6 @tonemap agx
// Drifting down a canyon through an endless brutalist megastructure at dusk
// (Blame!-style): stacked concrete slabs and recesses either side, bridges
// spanning the void, the floor lost in fog far below, a warm dusk glow at
// the far end. Thousands of tiny windows light the facades, and waves of
// them ripple through the spectrum as the music plays.
// Domain-repeated box SDF (one cell + its neighbour in z per wall), field
// AO and aerial perspective — no shadows, 90 steps.
// Forward drift on the tempo clock; audio lights windows, never moves walls.

const CANYON: f32 = 7.0;       // half-width of the void
const CELL: vec3<f32> = vec3<f32>(6.0, 7.0, 9.0);

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// One wall of the canyon (side = ±1): blocks protruding by a hashed depth.
// Each block stays inside its own y/z cell, so only that cell is evaluated;
// the march clamps its step so it can't skip into a neighbour's block.
fn wall(p: vec3<f32>, side: f32) -> f32 {
    let lx = side * p.x - CANYON;            // 0 at the canyon face, + into the wall
    let cy = floor(p.y / CELL.y);
    let cz = floor(p.z / CELL.z);
    let h = hash21(vec2<f32>(cy * 13.0 + side * 7.0, cz * 3.0));
    let h2 = hash21(vec2<f32>(cy * 5.0 + side, cz * 11.0 + 1.0));
    let proud = h * 3.0;                        // how far it juts out
    let c = vec3<f32>(-proud + 4.0, (cy + 0.5) * CELL.y, (cz + 0.5) * CELL.z);
    let b = vec3<f32>(4.0, CELL.y * (0.3 + 0.18 * h2), CELL.z * (0.32 + 0.16 * h));
    let d = sd_box(vec3<f32>(lx, p.y, p.z) - c, b);
    // Backing wall everything hangs off.
    return min(d, 4.2 - lx);
}

// (distance, material): 0 concrete, 1 bridge.
fn map(p: vec3<f32>) -> vec2<f32> {
    var d = min(wall(p, 1.0), wall(p, -1.0));
    var m = 0.0;
    // Bridges: every ~40 units, at a hashed height.
    let bz = floor(p.z / 40.0);
    let by = (hash21(vec2<f32>(bz, 2.0)) - 0.5) * 30.0;
    let br = sd_box(p - vec3<f32>(0.0, by, (bz + 0.5) * 40.0), vec3<f32>(CANYON + 1.0, 0.8, 1.6));
    if br < d {
        d = br;
        m = 1.0;
    }
    return vec2<f32>(d, m);
}

fn normal(p: vec3<f32>) -> vec3<f32> {
    let e = 0.01;
    let k = vec2<f32>(1.0, -1.0);
    return normalize(
        k.xyy * map(p + k.xyy * e).x + k.yyx * map(p + k.yyx * e).x +
        k.yxy * map(p + k.yxy * e).x + k.xxx * map(p + k.xxx * e).x
    );
}

fn sky(rd: vec3<f32>) -> vec3<f32> {
    let glow = pow(max(rd.z, 0.0), 40.0);
    var c = mix(vec3<f32>(0.03, 0.045, 0.08), vec3<f32>(1.0, 0.5, 0.22), glow * smoothstep(0.3, -0.1, rd.y));
    c = mix(c, vec3<f32>(0.006, 0.01, 0.025), smoothstep(0.1, 0.8, rd.y));
    return c;
}

// Windows on a facade point with normal n: tiny lit rectangles; some columns
// light up as a spectrum wave travels down the canyon.
fn windows(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    if abs(n.x) < 0.7 {
        return vec3<f32>(0.0);
    }
    let g = vec2<f32>(p.z * 6.0, p.y * 4.5);
    let cell = floor(g);
    let f = fract(g);
    let pane = smoothstep(0.35, 0.28, abs(f.x - 0.5)) * smoothstep(0.3, 0.22, abs(f.y - 0.5));
    let h = hash21(cell + sign(n.x) * 71.0);
    let base_lit = step(0.93, h);
    // Spectrum wave: band picked by column, travelling along z.
    let band = fract(cell.x * 0.037 + cell.y * 0.011);
    let wave = spec(band * 0.8) * smoothstep(0.6, 1.0, sin(p.z * 0.05 - u.flow * 0.4 + cell.y * 0.1) * 0.5 + 0.5);
    let lit = max(base_lit * 0.6, step(0.6, h) * wave * 1.8 * u.intensity);
    let warm = mix(vec3<f32>(1.0, 0.7, 0.4), palette(band), step(0.6, h) * wave);
    return warm * lit * pane * 0.8;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.flow * 1.1;
    let ro = vec3<f32>(sin(u.flow * 0.013) * 2.5, sin(u.flow * 0.009) * 6.0, z);
    let ta = ro + vec3<f32>(sin(u.flow * 0.011) * 0.25, -0.05 + sin(u.flow * 0.007) * 0.1, 1.0);
    let rd = cam_ray(p, ro, ta, sin(u.flow * 0.006) * 0.05, 1.45);
    let sun = normalize(vec3<f32>(0.25, 0.12, 1.0));

    var t = 0.1;
    var m = -1.0;
    for (var i = 0; i < 110; i++) {
        let h = map(ro + rd * t);
        if h.x < 0.002 * t {
            m = h.y;
            break;
        }
        t += min(h.x * 0.9, 2.5);
        if t > 160.0 {
            break;
        }
    }
    var col = sky(rd);
    if m >= 0.0 {
        let hp = ro + rd * t;
        let n = normal(hp);
        let ao = clamp(map(hp + n * 0.8).x / 0.8, 0.0, 1.0) * clamp(map(hp + n * 2.5).x / 2.5, 0.0, 1.0);
        let conc = vec3<f32>(0.16, 0.155, 0.15) * (0.5 + 0.9 * tnoise(hp * 0.06).b) * (0.8 + 0.4 * tnoise(hp * 0.5).a);
        // Low dusk key from down the canyon + cool sky fill from above.
        let key = max(dot(n, sun), 0.0) * vec3<f32>(1.0, 0.55, 0.3) * 0.6;
        let fill = (0.5 + 0.5 * n.y) * vec3<f32>(0.08, 0.1, 0.16);
        col = conc * (key + fill) * (0.3 + 0.7 * ao);
        col += windows(hp, n) * (0.5 + 0.5 * ao) * 0.9;
        if m > 0.5 {
            // Bridge underside lights.
            col += vec3<f32>(0.9, 0.8, 0.6) * step(n.y, -0.7) * smoothstep(0.3, 0.1, abs(fract(hp.x * 0.25) - 0.5)) * 0.15;
        }
    }
    // Aerial perspective + height fog pooling in the depths.
    let fog_c = sky(vec3<f32>(rd.x, min(rd.y, 0.05), rd.z)) * 0.8 + vec3<f32>(0.01, 0.015, 0.03);
    let depth_fog = 1.0 - exp(-t * 0.018);
    let height_fog = smoothstep(-5.0, -30.0, (ro + rd * min(t, 160.0)).y) * 0.7;
    col = mix(col, fog_c, clamp(max(depth_fog, height_fog), 0.0, 1.0));
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
