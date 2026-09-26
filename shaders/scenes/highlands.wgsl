// Misty mountain valley at dawn: the camera glides down a river-valley
// corridor between ridged fbm peaks, low sun ahead, fog pooling on the floor.
// The tempo clock drives the flight; the music warms the light and stirs the
// mist. No flashes.

// Terrain height at world (x, z). A carved corridor along x~0 is the valley.
fn terrain_h(x: f32, z: f32) -> f32 {
    let broad = fbm(vec2<f32>(x * 0.09, z * 0.09));
    let ridged = 1.0 - abs(2.0 * noise(vec2<f32>(x * 0.22, z * 0.22)) - 1.0);
    var h = broad * 5.5 + ridged * ridged * 2.2 + fbm(vec2<f32>(x * 0.5, z * 0.5)) * 0.6;
    // Valley floor: low and flat near the flight line, climbing with distance.
    let wall = smoothstep(0.8, 6.0, abs(x) + 1.2 * sin(z * 0.05));
    return h * wall + 0.35 * fbm(vec2<f32>(x * 0.4, z * 0.18)) - 0.2;
}

fn field(p: vec3<f32>) -> f32 {
    return p.y - terrain_h(p.x, p.z);
}

fn sky(dir: vec3<f32>, sun: vec3<f32>) -> vec3<f32> {
    let y = max(dir.y, 0.0);
    var c = mix(vec3<f32>(0.95, 0.5, 0.28), vec3<f32>(0.12, 0.18, 0.38), pow(y, 0.4));
    let s = max(dot(dir, sun), 0.0);
    c += vec3<f32>(1.0, 0.55, 0.3) * pow(s, 6.0) * 0.5 + vec3<f32>(1.0, 0.9, 0.75) * pow(s, 400.0) * 4.0;
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let sun = normalize(vec3<f32>(0.15, 0.14, 1.0));

    // Fly down the valley on the tempo clock, riding a little above the floor.
    let cz = u.flow * 2.0;
    let cx = sin(cz * 0.06) * 0.8;
    let floor_h = terrain_h(cx, cz);
    let ro = vec3<f32>(cx, floor_h + 1.5 + sin(u.time * 0.1) * 0.15, cz);
    let yaw = sin(u.time * 0.06) * 0.12 + cos(cz * 0.06) * 0.1;
    let dxz = rot(yaw) * vec2<f32>(p.x, 1.7);
    let rd = normalize(vec3<f32>(dxz.x, -p.y - 0.1 + sin(u.time * 0.08) * 0.015, dxz.y));

    var col = sky(rd, sun);

    if rd.y < 0.0 {
        // March the heightfield: growing steps out, then bisect the hit.
        var t = 0.0;
        var prev_h = field(ro);
        var hit_t = -1.0;
        for (var i = 0; i < 56; i++) {
            t += 0.12 + t * 0.06;
            let h = field(ro + rd * t);
            if h < 0.0 {
                hit_t = t;
                break;
            }
            prev_h = h;
            if t > 160.0 {
                break;
            }
        }
        if hit_t > 0.0 {
            // Refine the crossing.
            var lo = hit_t - (0.12 + hit_t * 0.06);
            var hi = hit_t;
            for (var i = 0; i < 5; i++) {
                let mid = (lo + hi) * 0.5;
                if field(ro + rd * mid) < 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let hp = ro + rd * hi;
            let e = 0.02 * hi + 0.02;
            let n = normalize(vec3<f32>(
                field(hp + vec3<f32>(e, 0.0, 0.0)) - field(hp - vec3<f32>(e, 0.0, 0.0)),
                2.0 * e,
                field(hp + vec3<f32>(0.0, 0.0, e)) - field(hp - vec3<f32>(0.0, 0.0, e))));

            // Rock and scree, greener low down, snow on the high steep tops.
            let rock = mix(vec3<f32>(0.05, 0.045, 0.05), vec3<f32>(0.14, 0.12, 0.11),
                           fbm(hp.xz * 0.8));
            let snow = smoothstep(2.6, 4.2, hp.y) * smoothstep(0.55, 0.8, n.y);
            var alb = mix(rock, vec3<f32>(0.5, 0.55, 0.6), snow);
            alb = mix(alb, vec3<f32>(0.06, 0.09, 0.05), smoothstep(1.2, 0.2, hp.y) * smoothstep(0.5, 0.85, n.y) * 0.7);
            let warm = 1.0 + 0.5 * u.intensity;
            let dif = max(dot(n, sun), 0.0);
            col = alb * (vec3<f32>(1.0, 0.6, 0.4) * dif * warm + vec3<f32>(0.1, 0.14, 0.22) * (0.5 + 0.5 * n.y));

            // Valley mist: thick on the floor, thinning with height and range.
            let mist = exp(-hi * 0.045) * (0.5 + 0.5 * exp(-hp.y * 0.5));
            let mist_c = mix(vec3<f32>(0.5, 0.5, 0.62), vec3<f32>(0.95, 0.6, 0.4),
                             pow(max(dot(rd, sun), 0.0), 3.0));
            col = mix(mist_c * (0.55 + 0.3 * u.energy), col, mist);
            // Sun glow bleeding through the mist ahead.
            col += vec3<f32>(1.0, 0.5, 0.25) * pow(max(dot(rd, sun), 0.0), 5.0) * (1.0 - mist) * 0.5;
        }
    }
    col = pow(col, vec3<f32>(1.3)) * 0.95;
    return vec4<f32>(col, 1.0);
}
