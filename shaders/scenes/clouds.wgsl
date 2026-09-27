// @heavy — raymarched; only in rotation when the GPU tier allows.
// Realistic cloud flight: skimming over a sea of volumetric clouds at sunset.
// Density is 3D noise shaped into a layer; each sample is lit by marching a
// short way toward the sun, so tops glow warm and undersides fall into shade.
// Forward motion follows the tempo clock; the music warms the light a little.

fn hash3(p: vec3<f32>) -> f32 {
    let i = vec3<i32>(floor(p));
    return f32(pcg(u32(i.x) + pcg(u32(i.y) + pcg(u32(i.z))))) / 4294967295.0;
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), w.x),
                mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), w.x), w.y);
    let b = mix(mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), w.x),
                mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), w.x), w.y);
    return mix(a, b, w.z);
}

fn fbm3(p_in: vec3<f32>, octaves: i32) -> f32 {
    var p = p_in;
    var v = 0.0;
    var a = 0.5;
    for (var i = 0; i < octaves; i++) {
        v += a * noise3(p);
        p = p * 2.03 + vec3<f32>(1.7, 9.2, 3.1);
        a *= 0.5;
    }
    return v;
}

const BOTTOM: f32 = -1.6;
const TOP: f32 = -0.2;

fn density(p: vec3<f32>, octaves: i32) -> f32 {
    let h = (p.y - BOTTOM) / (TOP - BOTTOM);
    let shape = smoothstep(0.0, 0.25, h) * smoothstep(1.0, 0.55, h);
    let n = fbm3(p * vec3<f32>(0.45, 0.75, 0.45) + vec3<f32>(u.time * 0.02, 0.0, 0.0), octaves);
    // Firm edges with gaps between the clouds; tops rounder than bases.
    return clamp((n - 0.5 + 0.12 * (1.0 - h)) * 7.0 * shape, 0.0, 1.0);
}

fn sky(dir: vec3<f32>, sun: vec3<f32>) -> vec3<f32> {
    let y = max(dir.y, 0.0);
    var c = mix(vec3<f32>(1.0, 0.45, 0.2), vec3<f32>(0.08, 0.14, 0.38), pow(y, 0.35));
    let s = max(dot(dir, sun), 0.0);
    c += vec3<f32>(1.0, 0.55, 0.25) * pow(s, 8.0) * 0.4 + vec3<f32>(1.0, 0.9, 0.7) * pow(s, 600.0) * 5.0;
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let sun = normalize(vec3<f32>(0.25, 0.1, 1.0));
    let ro = vec3<f32>(sin(u.time * 0.05) * 2.0, 0.8, u.flow * 1.2);
    // Yaw, not roll: banking tilted the cloud deck's horizon line.
    let yaw = sin(u.time * 0.08) * 0.09;
    let dxz = rot(yaw) * vec2<f32>(p.x, 1.5);
    let rd = normalize(vec3<f32>(dxz.x, -p.y - 0.18 + sin(u.time * 0.06) * 0.015, dxz.y));

    var col = sky(rd, sun);
    // Only rays heading down into the cloud layer need marching.
    if rd.y < -0.01 {
        let t0 = (TOP - ro.y) / rd.y;
        let t1 = min((BOTTOM - ro.y) / rd.y, t0 + 30.0);
        let steps = 40;
        let dt = (t1 - t0) / f32(steps);
        var trans = 1.0;
        var light = vec3<f32>(0.0);
        let warm = vec3<f32>(1.25, 0.72, 0.4) * (1.0 + 0.25 * u.intensity);
        let shade = vec3<f32>(0.1, 0.09, 0.18);
        // Jitter the start to hide banding.
        var t = t0 + dt * hash21(in.uv * vec2<f32>(u.res_x, u.res_y));
        for (var i = 0; i < steps; i++) {
            let pos = ro + rd * t;
            let d = density(pos, 5);
            if d > 0.01 {
                // How much cloud lies between here and the sun.
                let towards = density(pos + sun * 0.35, 3) + density(pos + sun * 0.8, 2);
                let lit = exp(-towards * 2.6);
                let height = clamp((pos.y - BOTTOM) / (TOP - BOTTOM), 0.0, 1.0);
                // Sun on the lit sides, blue sky light on the tops, dark bases.
                let c = mix(shade, warm, lit) * (0.45 + 0.55 * height) + vec3<f32>(0.08, 0.1, 0.18) * height;
                let a = 1.0 - exp(-d * dt * 2.2);
                light += trans * a * c;
                trans *= 1.0 - a;
                if trans < 0.02 {
                    break;
                }
            }
            t += dt;
        }
        // Distant cloud fades into the horizon haze.
        let haze = smoothstep(14.0, 40.0, t0);
        // Through the gaps: the dim world far below, not more sky.
        let below = vec3<f32>(0.06, 0.06, 0.12);
        col = mix(light + below * trans, sky(vec3<f32>(rd.x, 0.0, rd.z), sun), haze);
    }
    col = pow(col, vec3<f32>(1.3)) * 0.95;
    return vec4<f32>(col, 1.0);
}
