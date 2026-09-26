// Realistic ocean at sunset: layered choppy wave octaves ray-traced as a
// height field, with Fresnel sky reflections, sun glitter and green-blue
// light through the crests. The camera glides forward on the tempo clock; the
// music only lifts the swell a little.

const SEA_ITER: i32 = 4;
const SEA_ITER_FINE: i32 = 6;

fn snoise(p: vec2<f32>) -> f32 {
    return noise(p) * 2.0 - 1.0;
}

fn sea_octave(uv_in: vec2<f32>, choppy: f32) -> f32 {
    let uv = uv_in + snoise(uv_in);
    var wv = 1.0 - abs(sin(uv));
    let swv = abs(cos(uv));
    wv = mix(wv, swv, wv);
    return pow(1.0 - pow(wv.x * wv.y, 0.65), choppy);
}

fn sea_height(p: vec3<f32>, iters: i32) -> f32 {
    var freq = 0.16;
    var amp = 0.6 + 0.25 * u.intensity;
    var choppy = 4.0;
    var uv = p.xz * vec2<f32>(0.75, 1.0);
    let sea_time = 1.0 + u.time * 0.8;
    let m = mat2x2<f32>(1.6, 1.2, -1.2, 1.6);
    var h = 0.0;
    for (var i = 0; i < iters; i++) {
        var d = sea_octave((uv + sea_time) * freq, choppy);
        d += sea_octave((uv - sea_time) * freq, choppy);
        h += d * amp;
        uv = m * uv;
        freq *= 1.9;
        amp *= 0.22;
        choppy = mix(choppy, 1.0, 0.2);
    }
    return p.y - h;
}

fn sky(dir: vec3<f32>, sun: vec3<f32>) -> vec3<f32> {
    let y = max(dir.y, 0.0);
    var c = mix(vec3<f32>(1.0, 0.42, 0.16), vec3<f32>(0.04, 0.1, 0.32), pow(y, 0.32));
    c = mix(c, vec3<f32>(0.55, 0.2, 0.35), smoothstep(0.05, 0.25, y) * (1.0 - smoothstep(0.25, 0.6, y)) * 0.35);
    let s = max(dot(dir, sun), 0.0);
    c += vec3<f32>(1.0, 0.5, 0.2) * pow(s, 10.0) * 0.35 + vec3<f32>(1.0, 0.85, 0.6) * pow(s, 500.0) * 4.0;
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let sun = normalize(vec3<f32>(0.0, 0.12, 1.0));
    let ro = vec3<f32>(0.0, 3.2, u.flow * 1.5);
    // Yaw pans the view along the horizon without tilting it; a slow pitch
    // bob breathes. Rolling the camera made the sea line slant.
    let yaw = sin(u.time * 0.11) * 0.1;
    let dxz = rot(yaw) * vec2<f32>(p.x, 1.6);
    let rd = normalize(vec3<f32>(dxz.x, -p.y - 0.12 + sin(u.time * 0.07) * 0.02, dxz.y));

    var col: vec3<f32>;
    if rd.y > 0.0 {
        col = sky(rd, sun);
    } else {
        // Height-field tracing: bracket the surface, then bisect.
        var tm = 0.0;
        var tx = 1000.0;
        var hx = sea_height(ro + rd * tx, SEA_ITER);
        var hm = sea_height(ro, SEA_ITER);
        var tmid = 0.0;
        for (var i = 0; i < 8; i++) {
            tmid = mix(tm, tx, hm / (hm - hx));
            let hmid = sea_height(ro + rd * tmid, SEA_ITER);
            if hmid < 0.0 {
                tx = tmid;
                hx = hmid;
            } else {
                tm = tmid;
                hm = hmid;
            }
        }
        let hit = ro + rd * tmid;
        let dist = length(hit - ro);
        let eps = 0.001 * dist + 0.001;
        let h0 = sea_height(hit, SEA_ITER_FINE);
        let n = normalize(vec3<f32>(
            sea_height(hit + vec3<f32>(eps, 0.0, 0.0), SEA_ITER_FINE) - h0,
            eps,
            sea_height(hit + vec3<f32>(0.0, 0.0, eps), SEA_ITER_FINE) - h0));

        let fresnel = pow(clamp(1.0 - dot(n, -rd), 0.0, 1.0), 3.0) * 0.65;
        let reflected = sky(reflect(rd, n), sun);
        // Light through the water, stronger in the crests.
        let base = vec3<f32>(0.0, 0.09, 0.18);
        let water = vec3<f32>(0.8, 0.9, 0.6) * 0.6;
        let diffuse = pow(dot(n, sun) * 0.4 + 0.6, 80.0);
        let refracted = base + diffuse * water * 0.12 + water * (hit.y - 0.6) * 0.18 * max(1.0 - dist * 0.001, 0.0);
        col = mix(refracted, reflected, fresnel);
        // Sun glitter.
        let spec = pow(max(dot(reflect(rd, n), sun), 0.0), 60.0) * ((60.0 + 8.0) / (3.14159 * 8.0));
        col += vec3<f32>(1.0, 0.8, 0.55) * spec * 0.9;
        // Haze toward the horizon.
        col = mix(col, sky(vec3<f32>(rd.x, 0.0, rd.z), sun), smoothstep(40.0, 400.0, dist));
    }
    col = pow(col, vec3<f32>(1.4)) * 0.9;     // deepen for the ACES present pass
    return vec4<f32>(col, 1.0);
}
