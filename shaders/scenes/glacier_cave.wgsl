// @heavy — 2026 tier. @bloom 0.6 @tonemap agx
// Inside a glacier: a winding ice tunnel with scalloped walls, lit through
// the ice itself — thin ceilings glow cyan-white, thick walls sink to deep
// blue — and by daylight pouring in from the mouth ahead. Wet ice catches
// sharp highlights.
// Fake subsurface: transmitted light = sky·e^(−k·thickness), with thickness
// from the baked noise (ceilings thinner than walls). Tetrahedral normals and
// 80 steps keep it inside an M2 budget.
// The flight follows the tempo clock along the tunnel's centreline; the
// music deepens the glow.

fn centre(z: f32) -> vec2<f32> {
    return vec2<f32>(sin(z * 0.11) * 2.2 + sin(z * 0.047) * 3.0, sin(z * 0.08) * 0.5);
}

fn radius(z: f32) -> f32 {
    // Shape: the ice tunnel swells open as the bass builds.
    return 2.4 + sin(z * 0.19) * 0.4 + sin(z * 0.07) * 0.5 + 0.45 * u.pres4.x;
}

// Air-positive field: inside the tunnel is positive.
fn map(p: vec3<f32>) -> f32 {
    let c = centre(p.z);
    let q = p.xy - c;
    // Slightly wider than tall.
    let r = length(q * vec2<f32>(0.85, 1.0));
    var d = radius(p.z) - r;
    // Large lumps + cellular scallops melted into the surface.
    let n = tnoise(p * 0.07);
    let sc = tnoise(p * 0.3);
    d += (n.b - 0.5) * 1.3 - (1.0 - sc.g) * 0.28;
    // Gravel floor.
    let fl = p.y - (c.y - 1.55) - (n.r - 0.22) * 0.4;
    return min(d, fl);
}

fn normal(p: vec3<f32>) -> vec3<f32> {
    let e = 0.01;
    let k = vec2<f32>(1.0, -1.0);
    return normalize(
        k.xyy * map(p + k.xyy * e) + k.yyx * map(p + k.yyx * e) +
        k.yxy * map(p + k.yxy * e) + k.xxx * map(p + k.xxx * e)
    );
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Energy: flight speed follows the mix.
    let z = u.clock4.x * 0.5;
    let c0 = centre(z);
    let ro = vec3<f32>(c0.x, c0.y - 0.35 + 0.08 * sin(u.flow * 0.05), z);
    let c1 = centre(z + 5.0);
    let ta = vec3<f32>(c1.x, c1.y + 0.1, z + 5.0);
    let bank = (centre(z + 3.0).x - c0.x) * 0.06;
    let rd = cam_ray(p, ro, ta, bank, 1.3);

    // Daylight from the mouth: always far ahead down the tunnel.
    let mouth = vec3<f32>(centre(z + 60.0), z + 60.0);
    let sky = vec3<f32>(0.8, 0.92, 1.0);
    let drive = 0.6 + 0.6 * u.intensity;

    var t = 0.05;
    var hit = false;
    for (var i = 0; i < 80; i++) {
        let d = map(ro + rd * t);
        if d < 0.002 * t {
            hit = true;
            break;
        }
        t += d * 0.8;
        if t > 60.0 {
            break;
        }
    }

    var col: vec3<f32>;
    if hit {
        let hp = ro + rd * t;
        let n = normal(hp);
        let c = centre(hp.z);
        let is_floor = step(hp.y, c.y - 1.2);
        // Thickness of ice above/around this point: ceilings are thin,
        // walls thicker, and it varies in patches.
        let th_n = tnoise(hp * 0.045 + 0.2);
        let ceiling = smoothstep(0.0, 0.8, -n.y);
        // Scallop centres are melted thinner — that's what makes the cells
        // glow in real ice caves.
        let scal = tnoise(hp * 0.3).g;
        let thick = mix(3.2, 1.0, ceiling) + (th_n.b - 0.5) * 3.0 + (0.5 - scal) * 2.2;
        let trans = exp(-max(thick, 0.2) * vec3<f32>(1.6, 0.55, 0.3));
        // Relief: cavities darken (cheap AO from the field).
        let ao = clamp(map(hp + n * 0.35) / 0.35, 0.0, 1.0);
        var ice = sky * trans * (1.3 + 0.9 * u.intensity) * (0.35 + 0.65 * ao);
        // Direct mouth light, falling off with distance to the mouth and with
        // how much the wall faces it.
        let lv = mouth - hp;
        let ld = length(lv);
        let l = lv / ld;
        let facing = max(dot(n, l), 0.0);
        let mouth_i = 120.0 / (ld * ld);
        ice += vec3<f32>(0.5, 0.8, 1.0) * facing * mouth_i * 0.6;
        // Wet specular highlight of the mouth.
        let hv = normalize(l - rd);
        ice += sky * ggx(max(dot(n, hv), 0.0), facing, 0.12) * mouth_i * 0.35;
        // A bass-driven luminous pulse deep in the ice.
        ice += vec3<f32>(0.1, 0.35, 0.9) * trans.b * u.bass * 0.25;
        // Floor: dark gravel with a meltwater sheen.
        let gravel = vec3<f32>(0.05, 0.08, 0.11) * (0.5 + tnoise(hp * 1.3).b) * (0.4 + facing * mouth_i * 3.0) + trans * sky * 0.15;
        col = mix(ice, gravel, is_floor) * drive;
        // Rim of reflected blue on the floor.
        col += is_floor * vec3<f32>(0.05, 0.15, 0.3) * pow(1.0 - abs(dot(rd, n)), 3.0) * 0.4;
    } else {
        t = 60.0;
        col = vec3<f32>(0.0);
    }
    // Cold blue air: distance haze brightening toward the mouth.
    let toward = max(dot(rd, normalize(mouth - ro)), 0.0);
    let haze = select(1.0 - exp(-t * 0.045), 1.0, t >= 60.0);
    col = mix(col, mix(vec3<f32>(0.02, 0.07, 0.14), vec3<f32>(0.5, 0.75, 0.95), pow(toward, 12.0)) * drive, haze);
    // Glare from the mouth.
    col += sky * pow(toward, 200.0) * 1.5 * drive;

    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
