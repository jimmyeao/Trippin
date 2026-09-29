// @heavy — Synesthesia-style abstract. @bloom 0.8 @tonemap agx
// Drifting through a kaleidoscopic IFS fractal: an endless folded
// cathedral of struts and chambers, its edges traced in thin glowing lines
// (orbit-trap colouring), deep space fading to haze.
// Audio vocabulary (see common.wgsl):
//  - camera travel on the whole-mix energy clock (glide / surge);
//  - the fold angles morph on the mid clock, the structure "breathes"
//    with bass presence (slow — never a twitch);
//  - edge lines flare on bass hits, sparks on high hits;
//  - colour drifts on the high clock.

// The winding flight corridor carved through the lattice.
fn path(z: f32) -> vec2<f32> {
    return vec2<f32>(0.6 * sin(z * 0.23), 0.45 * sin(z * 0.17 + 1.3));
}

const CELL: f32 = 2.6;

fn kifs(p_in: vec3<f32>) -> vec2<f32> {
    // A lattice of fractal sculptures: repeat the cell in all three axes,
    // each cell a KIFS scaled to reach its neighbours.
    var p = (p_in - round(p_in / CELL) * CELL) * 0.95;
    let a1 = 0.6 + 0.25 * sin(u.clock4.z * 0.05);
    let a2 = 0.4 + 0.2 * cos(u.clock4.z * 0.037);
    let scale = 1.9 + 0.12 * u.pres4.x;
    let off = vec3<f32>(1.0, 0.75 + 0.1 * sin(u.clock4.z * 0.03), 0.55);
    var s = 1.0;
    var trap = 1e3;
    for (var i = 0; i < 4; i++) {
        p = abs(p);
        if p.x < p.y {
            p = p.yxz;
        }
        if p.x < p.z {
            p = p.zyx;
        }
        if p.y < p.z {
            p = p.xzy;
        }
        let r1 = rot(a1) * p.xy;
        p = vec3<f32>(r1.x, r1.y, p.z);
        let r2 = rot(a2) * p.yz;
        p = vec3<f32>(p.x, r2.x, r2.y);
        p = p * scale - off * (scale - 1.0);
        s *= scale;
        trap = min(trap, length(p.xy) - 0.2 * f32(i));
    }
    // Struts: thin boxes in the folded space.
    let q = abs(p) - vec3<f32>(1.3, 0.32, 0.32);
    var d = (length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0)) / s / 0.95;
    // Carve the corridor (and keep its walls from reaching the camera).
    let corridor = 0.45 - length(p_in.xy - path(p_in.z));
    d = max(d, corridor);
    return vec2<f32>(d, trap);
}

fn normal(p: vec3<f32>, t: f32) -> vec3<f32> {
    // Epsilon grows with distance: no sub-pixel shading noise far away.
    let e = 0.0008 + 0.0015 * t;
    let k = vec2<f32>(1.0, -1.0);
    return normalize(
        k.xyy * kifs(p + k.xyy * e).x + k.yyx * kifs(p + k.yyx * e).x +
        k.yxy * kifs(p + k.yxy * e).x + k.xxx * kifs(p + k.xxx * e).x
    );
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.clock4.x * 0.9;
    let ro = vec3<f32>(path(z), z);
    let ta = vec3<f32>(path(z + 1.5), z + 1.5);
    let rd = cam_ray(p, ro, ta, 0.25 * sin(u.clock4.z * 0.02), 1.3);

    var t = 0.02 + 0.03 * bluen(in.pos.xy);
    var glow = vec3<f32>(0.0);
    var hit = false;
    var trap = 0.0;
    var steps = 0.0;
    let hue = u.clock4.w * 0.01 + u.hue;
    for (var i = 0; i < 110; i++) {
        let pos = ro + rd * t;
        let h = kifs(pos);
        // Edge glow: accumulate near-misses, coloured by the orbit trap.
        let near = exp(-h.x * 140.0);
        glow += palette(h.y * 0.35 + pos.z * 0.02 + hue) * near * 0.006;
        steps = f32(i);
        if h.x < 0.0006 * t {
            hit = true;
            trap = h.y;
            break;
        }
        t += max(h.x * 0.85, 0.002 * t);
        if t > 18.0 {
            break;
        }
    }
    let drive = 0.5 + 0.8 * u.intensity;
    var col = vec3<f32>(0.0);
    if hit {
        let pos = ro + rd * t;
        let n = normal(pos, t);
        // Step-count occlusion: rays that crept through crevices took many
        // steps — dark there, bright on open faces.
        let ao = clamp(1.0 - steps / 70.0, 0.0, 1.0);
        let lit = 0.3 + 0.7 * max(dot(n, normalize(vec3<f32>(0.3, 0.8, -0.4))), 0.0);
        let rim = pow(1.0 - max(dot(n, -rd), 0.0), 3.0);
        // Smooth colouring: along the flight and by facing, not the chaotic
        // orbit trap (that reads as per-pixel noise).
        let base = palette(pos.z * 0.035 + n.y * 0.12 + hue);
        let key = normalize(vec3<f32>(0.3, 0.8, -0.4));
        let spec_l = pow(max(dot(reflect(rd, n), key), 0.0), 24.0);
        col = base * lit * ao * 0.55 + base * rim * ao * 0.9 + vec3<f32>(1.0, 0.95, 0.9) * spec_l * ao * 0.5;
        // Crevices glow from within and flare on the kick.
        let crev = pow(1.0 - ao, 2.0);
        col += palette(hue + 0.45) * crev * (0.6 + 3.0 * u.hits4.x) * drive;
        col *= exp(-t * 0.08);
    }
    col += glow * (0.6 + 1.2 * u.lvl4.y) * drive;
    // Sparks on high hits: a few bright specks in the haze.
    let sg = floor(in.pos.xy / 6.0);
    let sh = hash21(sg + floor(u.beat * 4.0) * 17.0);
    col += palette(hue + 0.5) * step(0.9985, sh) * u.hits4.w * 3.0;
    // Deep haze.
    col = mix(col, palette(hue + 0.3) * 0.02, 1.0 - exp(-t * 0.08));
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
