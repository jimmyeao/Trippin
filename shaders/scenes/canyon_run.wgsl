// @heavy — canyon sprint: the camera threads a winding slot canyon at dusk,
// sheer layered walls rising to a jagged rim with the sky and first stars
// between them, glow strips running low along both walls. Bass dips the
// camera and stokes the strips; onsets spark them. Strafes but never hits
// the wall — the lane is locked to the canyon centreline.

fn wav(z: f32) -> f32 {
    return sin(z * 0.15) * 0.9 + sin(z * 0.04) * 0.6;
}

fn half_w(z: f32) -> f32 {
    return 1.7 + sin(z * 0.3) * 0.25;
}

fn rim(p: vec3<f32>) -> f32 {
    return 4.5 + (tnoise(vec3<f32>(p.z * 0.02, sign(p.x + wav(p.z)) * 0.3, 0.1)).b - 0.5) * 6.0;
}

// Air-positive field: the rock is |x'| > half and below the rim.
fn map(p: vec3<f32>) -> f32 {
    let xl = p.x + wav(p.z);
    let n = tnoise(vec3<f32>(p.y * 0.15, p.z * 0.06, 0.4)).b;
    let n2 = tnoise(p * 0.4 + 0.7).r;
    let side = half_w(p.z) - abs(xl) + (n - 0.5) * 0.9 + (n2 - 0.22) * 0.25;
    let walls = max(side, p.y - rim(p));
    let floor_d = p.y + 1.2 + (tnoise(vec3<f32>(p.xz * 0.5, 0.2)).r - 0.22) * 0.3;
    return min(walls, floor_d);
}

fn sky(rd: vec3<f32>) -> vec3<f32> {
    let h = clamp(rd.y, 0.0, 1.0);
    var c = mix(palette(0.95 + u.hue) * 0.35 + vec3<f32>(0.25, 0.1, 0.05), vec3<f32>(0.015, 0.012, 0.04), pow(h, 0.5));
    let g = rd * 200.0;
    let cell = floor(g);
    let r1 = hash22(cell.xy + cell.z * 17.3);
    let r2 = hash21(cell.yz + cell.x * 7.1);
    let sp = cell + vec3<f32>(r1.y, r2, fract(r2 * 13.1)) * 0.6 + 0.2;
    c += vec3<f32>(0.8, 0.85, 1.0) * step(0.9, r1.x) * (pow(r2, 8.0) * 1.5 + 0.03) * smoothstep(0.22, 0.0, length(g - sp)) * smoothstep(0.2, 0.5, rd.y);
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Constant speed — modulating travel by bass made the flight lurch.
    // Bass dips the camera and stokes the wall lights instead.
    let z = u.flow * 3.2;
    let ro = vec3<f32>(-wav(z) + sin(u.time * 0.4) * 0.25, -0.1 + sin(u.time * 0.7) * 0.12 - u.kick * 0.12, z);
    // Look down the canyon, leaning into the curves, tilted up a touch.
    let ahead = vec3<f32>(-wav(z + 4.0), 0.35, z + 4.0);
    let rd = cam_ray(p, ro, ahead, (wav(z + 3.0) - wav(z)) * 0.08, 1.2);

    var t = 0.05;
    var hit = false;
    for (var i = 0; i < 80; i++) {
        let d = map(ro + rd * t);
        if d < 0.003 * t {
            hit = true;
            break;
        }
        t += clamp(d * 0.7, 0.02, 1.5);
        if t > 45.0 {
            break;
        }
    }

    var col = sky(rd);
    if hit {
        let pos = ro + rd * t;
        let e = 0.02;
        let k = vec2<f32>(1.0, -1.0);
        let n = normalize(k.xyy * map(pos + k.xyy * e) + k.yyx * map(pos + k.yyx * e) + k.yxy * map(pos + k.yxy * e) + k.xxx * map(pos + k.xxx * e));
        // Sandstone strata.
        let strata = 0.5 + 0.5 * sin(pos.y * 5.0 + tnoise(vec3<f32>(pos.z * 0.05, pos.y * 0.2, 0.9)).b * 6.0);
        var albedo = mix(vec3<f32>(0.25, 0.1, 0.06), vec3<f32>(0.45, 0.22, 0.12), strata) * (0.7 + 0.5 * tnoise(pos * 0.8).a);
        if pos.y < -0.9 {
            albedo = vec3<f32>(0.12, 0.07, 0.05);
        }
        // Skylight from the slot above: brighter higher up the walls.
        let sky_l = sky(vec3<f32>(0.0, 0.3, 1.0)) * (0.25 + 0.75 * max(n.y, 0.0)) * smoothstep(-1.5, 4.0, pos.y) * 2.2 + vec3<f32>(0.02, 0.015, 0.02);
        // Glow strips low on each wall, dashes running along z.
        let xl = pos.x + wav(pos.z);
        let dash = exp(-abs(fract(pos.z * 0.4) - 0.5) * 8.0);
        let strip_y = smoothstep(0.25, 0.0, abs(pos.y + 0.55));
        let strip = dash * strip_y * step(0.8, abs(xl));
        let sc = palette(0.1 + u.hue + step(0.0, xl) * 0.3);
        let stoke = 0.4 + u.bass * 1.2 + u.onset * 2.0 + u.energy * 0.5;
        // Strip light spilling onto nearby rock and floor.
        let spill = sc * stoke * (exp(-abs(pos.y + 0.55) * 1.5) * 0.25 * (0.5 + 0.5 * dash));
        col = albedo * (sky_l + spill) + sc * strip * stoke * 1.6;
        // Dusk haze down the canyon.
        col = mix(col, sky(vec3<f32>(rd.x, 0.05, rd.z)) * 0.6, 1.0 - exp(-t * 0.05));
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
