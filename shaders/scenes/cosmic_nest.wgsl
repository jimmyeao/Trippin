// @bloom 0.9 @tonemap agx
// Synesthesia-style abstract: drifting through a cosmic fractal volume — the
// Kali "Star Nest" formula (p = |p|/dot(p,p) − k, iterated) accumulated
// along the view ray, so filaments, nebulae and star clusters appear out of
// pure maths. Palette-coloured by depth.
// Cost is fixed arithmetic (18 volume steps × 14 iterations, no textures),
// so it runs anywhere — not @heavy.
// Audio vocabulary:
//  - flight speed on the whole-mix energy clock; slow roll on the mid clock;
//  - the fractal parameter morphs with mid presence (structures unfold as
//    the track builds);
//  - brightness breathes with the bass level, star cores flash on high hits.

const VOLSTEPS: i32 = 18;
const ITERS: i32 = 14;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var dir = normalize(vec3<f32>(p * 0.7, 1.0));
    // Slow roll and yaw.
    let r1 = rot(u.clock4.z * 0.03) * dir.xy;
    dir = vec3<f32>(r1, dir.z);
    let r2 = rot(0.4 + sin(u.clock4.x * 0.01) * 0.3) * dir.xz;
    dir = vec3<f32>(r2.x, dir.y, r2.y);

    let travel = u.clock4.x * 0.012;
    let origin = vec3<f32>(1.0 + u.seed * 0.01, 0.5, 0.5) + vec3<f32>(travel * 2.0, travel, -2.0);
    let form = 0.53 + 0.03 * sin(u.clock4.z * 0.02) + 0.025 * u.pres4.y;
    let tile = 0.85;

    var s = 0.1 + 0.02 * bluen(in.pos.xy);
    var fade = 1.0;
    var v = vec3<f32>(0.0);
    let hue = u.clock4.w * 0.008 + u.hue;
    for (var r = 0; r < VOLSTEPS; r++) {
        var q = origin + s * dir * 0.5;
        // Tiling fold: space repeats seamlessly.
        q = abs(vec3<f32>(tile) - (q - floor(q / (tile * 2.0)) * tile * 2.0));
        var pa = 0.0;
        var a = 0.0;
        for (var i = 0; i < ITERS; i++) {
            q = abs(q) / dot(q, q) - form;
            let lq = length(q);
            a += abs(lq - pa);
            pa = lq;
        }
        // Dark matter: dims the near steps so the volume has depth.
        let dm = max(0.0, 0.3 - a * a * 0.001);
        a *= a * a;
        if r > 6 {
            fade *= 1.0 - dm;
        }
        let depth = f32(r) / f32(VOLSTEPS);
        v += fade * 0.003;
        v += palette(depth * 1.1 + hue) * a * 0.000018 * fade * (s * s);
        fade *= 0.73;
        s += 0.1;
    }
    let drive = 0.5 + 0.7 * u.intensity;
    var col = v * (0.7 + 0.8 * u.lvl4.x) * drive;
    // Contrast curve: deep black space, bright filaments.
    col = max(col - 0.02, vec3<f32>(0.0)) * 1.4;
    col += col * col * u.hits4.w * 2.0;
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
