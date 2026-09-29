// @heavy — infinite cube lattice flythrough: the camera rides through a
// repeating lattice of glowing cube frames. Bass feeds the frame glow,
// onsets flash whole cells at once.

// Returns (distance, cell hash, edge proximity 0..1).
fn map3(p: vec3<f32>) -> vec3<f32> {
    // Fixed cell size — putting bass in the world scale made the geometry
    // visibly lurch. Bass lives in the glow instead.
    let s = 1.6;
    let id = floor(p / s + 0.5);
    let h = hash21(id.xz + id.y * 3.0);
    let c = p - id * s;
    // Cube frame as 12 edge struts: distance to the nearest edge line of a
    // cube of half-size k (the old box-minus-box left the faces solid, so
    // cells read as flat colour tiles).
    // IQ's box-frame SDF: a cube of half-size k made of struts of radius r.
    let k = 0.42;
    // Shape: struts thicken with bass presence.
    let r = 0.03 + 0.035 * u.pres4.x;
    let pp = abs(c) - vec3<f32>(k);
    let qq = abs(pp + vec3<f32>(r)) - vec3<f32>(r);
    let f1 = length(max(vec3<f32>(pp.x, qq.y, qq.z), vec3<f32>(0.0))) + min(max(pp.x, max(qq.y, qq.z)), 0.0);
    let f2 = length(max(vec3<f32>(qq.x, pp.y, qq.z), vec3<f32>(0.0))) + min(max(qq.x, max(pp.y, qq.z)), 0.0);
    let f3 = length(max(vec3<f32>(qq.x, qq.y, pp.z), vec3<f32>(0.0))) + min(max(qq.x, max(qq.y, pp.z)), 0.0);
    let frame = min(min(f1, f2), f3);
    return vec3<f32>(frame, h, 0.0);
}

fn map(p: vec3<f32>) -> vec2<f32> {
    return map3(p).xy;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.clock4.x * 3.0;
    // Fly the gap between cube columns (x, y at cell edges +-0.8).
    let ro = vec3<f32>(0.8 + sin(z * 0.06) * 0.12, 0.8 + sin(z * 0.05) * 0.12, z);
    var rd = normalize(vec3<f32>(p.x * 0.8, -p.y * 0.7, 1.0));
    // Direction: a roll that swings one way, then the other.
    rd = vec3<f32>(rot(0.5 * sin(u.clock4.x * 0.02)) * rd.xy, rd.z);
    rd = vec3<f32>(rot(sin(z * 0.04) * 0.15) * rd.xy, rd.z);

    var t = 0.0;
    var hit = 0.0;
    var pos = ro;
    var glow = vec3<f32>(0.0);
    var found = false;
    for (var i = 0; i < 90; i++) {
        pos = ro + rd * t;
        let m = map(pos);
        // Neon: struts glow through the haze as the ray passes near them.
        glow += palette(m.y * 1.3 + u.hue) * exp(-m.x * 30.0) * 0.012;
        if m.x < 0.002 * (1.0 + t) { hit = m.y; found = true; break; }
        t += m.x * 0.8;
        if t > 30.0 { break; }
    }

    var col = vec3<f32>(0.004, 0.005, 0.014);
    let drive = 0.5 + 0.8 * u.intensity;
    if found {
        let e = 0.003;
        let kk = vec2<f32>(1.0, -1.0);
        let n = normalize(kk.xyy * map(pos + kk.xyy * e).x + kk.yyx * map(pos + kk.yyx * e).x + kk.yxy * map(pos + kk.yxy * e).x + kk.xxx * map(pos + kk.xxx * e).x);
        let fade = exp(-t * 0.13);
        // Onsets flash whole cells; per-cell band level lights the struts.
        let flash = step(0.8, fract(hit + u.beat * 0.125)) * u.onset * 2.0;
        let lvl = 0.4 + 1.4 * spec(hit) + flash;
        let fres = pow(1.0 - max(dot(n, -rd), 0.0), 2.0);
        let c = palette(hit * 1.3 + u.hue);
        col = mix(col, c * (0.25 + 0.75 * fres) * lvl * drive + vec3<f32>(1.0) * pow(fres, 6.0) * 0.3, fade);
    }
    col += glow * drive;
    // Distance fog toward the vanishing point.
    col += palette(0.7 + u.hue) * (1.0 - exp(-t * 0.06)) * 0.05;
    return vec4<f32>(col, 1.0);
}
