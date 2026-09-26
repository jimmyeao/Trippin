// Desert dunes at dusk: a slow drift over ridged sand seas under a sinking
// sun, heat shimmer near the ground, grain sparkle on the highs and a warm
// haze that thickens as the track drives. Smooth motion on the tempo clock.

// Dune height at world (x, z): big smooth ridges plus fine wind ripples.
fn dune_h(x: f32, z: f32) -> f32 {
    // Crests run roughly across x, warped so they wind and merge.
    let warp = fbm(vec2<f32>(x * 0.05, z * 0.05)) * 2.0;
    let crest = 1.0 - abs(sin(x * 0.35 + warp + z * 0.06));
    var h = crest * crest * 1.6;
    h += fbm(vec2<f32>(x * 0.12, z * 0.12)) * 2.4;
    // Fine ripples running down the slip faces.
    h += noise(vec2<f32>(x * 6.0, z * 1.2)) * 0.05 * crest;
    return h;
}

fn field(p: vec3<f32>) -> f32 {
    return p.y - dune_h(p.x, p.z);
}

fn sky(dir: vec3<f32>, sun: vec3<f32>) -> vec3<f32> {
    let y = max(dir.y, 0.0);
    // Dusk: embers at the horizon into violet overhead.
    var c = mix(vec3<f32>(0.9, 0.35, 0.12), vec3<f32>(0.06, 0.05, 0.22), pow(y, 0.38));
    c = mix(c, vec3<f32>(0.5, 0.15, 0.3), smoothstep(0.02, 0.2, y) * (1.0 - smoothstep(0.2, 0.5, y)) * 0.4);
    let s = max(dot(dir, sun), 0.0);
    c += vec3<f32>(1.0, 0.5, 0.2) * pow(s, 8.0) * 0.5 + vec3<f32>(1.0, 0.8, 0.5) * pow(s, 300.0) * 5.0;
    // First stars up top.
    let star = step(0.997, hash21(floor(dir.xy / max(dir.z, 0.1) * 300.0)));
    c += vec3<f32>(0.8, 0.85, 1.0) * star * smoothstep(0.25, 0.7, y) * 0.5;
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var p = centred(in.uv);
    // Heat shimmer: a tiny wobble that grows toward the ground and with energy.
    p.x += (noise(vec2<f32>(p.y * 20.0, u.time * 3.0)) - 0.5) * 0.006
         * smoothstep(-0.1, 0.6, p.y) * (0.6 + 0.8 * u.energy);

    let sun = normalize(vec3<f32>(0.0, 0.1, 1.0));
    let cz = u.flow * 1.4;
    let cx = sin(cz * 0.04) * 3.0;
    let ro = vec3<f32>(cx, dune_h(cx, cz) + 1.1 + sin(u.time * 0.09) * 0.1, cz);
    let yaw = sin(u.time * 0.05) * 0.1;
    let dxz = rot(yaw) * vec2<f32>(p.x, 1.7);
    let rd = normalize(vec3<f32>(dxz.x, -p.y - 0.14, dxz.y));

    var col = sky(rd, sun);

    // March rays pointing slightly UP too: a crest can rise above the
    // ray-horizon when it's taller than the camera's eye line, and clipping
    // them at rd.y == 0 slices dune tops off flat.
    if rd.y < 0.35 {
        var t = 0.0;
        var hit_t = -1.0;
        for (var i = 0; i < 48; i++) {
            t += 0.1 + t * 0.07;
            if field(ro + rd * t) < 0.0 {
                hit_t = t;
                break;
            }
            if t > 140.0 {
                break;
            }
        }
        if hit_t > 0.0 {
            var lo = hit_t - (0.1 + hit_t * 0.07);
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
            let e = 0.015 * hi + 0.015;
            let n = normalize(vec3<f32>(
                field(hp + vec3<f32>(e, 0.0, 0.0)) - field(hp - vec3<f32>(e, 0.0, 0.0)),
                2.0 * e,
                field(hp + vec3<f32>(0.0, 0.0, e)) - field(hp - vec3<f32>(0.0, 0.0, e))));

            // Sand albedo shifts with height and slope; crests catch the light.
            var alb = mix(vec3<f32>(0.45, 0.22, 0.1), vec3<f32>(0.7, 0.42, 0.2),
                          fbm(hp.xz * 0.6) * 0.7 + 0.3 * n.y);
            // Grain sparkle rides the high band.
            let glint = step(0.992, hash21(floor(hp.xz * 40.0))) * spec(0.9);
            let dif = max(dot(n, sun), 0.0);
            // Fake the long shadow side: steep faces away from the sun go cool.
            let shade = smoothstep(-0.2, 0.5, dot(n, sun));
            col = alb * (vec3<f32>(1.0, 0.55, 0.3) * dif * (1.0 + 0.5 * u.intensity)
                       + vec3<f32>(0.1, 0.1, 0.2) * (1.0 - shade) * 0.8);
            col += vec3<f32>(1.0, 0.8, 0.5) * glint * (0.3 + 0.7 * dif);
            // Wind-blown sand haze near the ground, thicker with energy.
            let haze = exp(-hi * 0.05) * (0.4 + 0.4 * u.energy);
            let haze_c = mix(vec3<f32>(0.5, 0.3, 0.2), vec3<f32>(0.95, 0.5, 0.25),
                             pow(max(dot(rd, sun), 0.0), 2.0));
            col = mix(haze_c * (0.5 + 0.3 * u.intensity), col, haze);
        }
    }
    col = pow(col, vec3<f32>(1.3)) * 0.9;
    return vec4<f32>(col, 1.0);
}
