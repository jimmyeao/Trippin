// @heavy — Synesthesia-style abstract. @bloom 0.8 @tonemap agx
// Looking up into volumetric aurora curtains: layered sheets of light that
// fold and ripple overhead, bright hems with rays streaming up, fading into
// violet above, over a starfield. Each sheet is a displaced vertical band
// ray-traced as a height slab and integrated over a few depth layers.
// Audio vocabulary:
//  - curtain folding drifts on the mid energy clock; ray shimmer on the
//    high clock;
//  - each layer's brightness follows its own band level (bass = lowest,
//    brightest hem); curtains surge on bass hits; ray sparkle on high hits;
//  - colour mixes with bass presence (green hems, magenta tops on drops).

const LAYERS: i32 = 20;

fn stars(rd: vec3<f32>) -> vec3<f32> {
    let g = rd * 220.0;
    let cell = floor(g);
    let r1 = hash22(cell.xy + cell.z * 17.3);
    let r2 = hash21(cell.yz + cell.x * 7.1);
    let sp = cell + vec3<f32>(r1.y, r2, fract(r2 * 13.1)) * 0.6 + 0.2;
    return vec3<f32>(0.8, 0.85, 1.0) * step(0.9, r1.x) * (pow(r2, 8.0) * 2.0 + 0.04) * smoothstep(0.22, 0.0, length(g - sp));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let ro = vec3<f32>(0.0, 0.0, 0.0);
    let look = vec3<f32>(sin(u.clock4.x * 0.006) * 0.4, 0.75, 1.0);
    let rd = cam_ray(p, ro, look, sin(u.clock4.x * 0.004) * 0.1, 1.1);
    var col = mix(vec3<f32>(0.004, 0.008, 0.02), vec3<f32>(0.0, 0.0, 0.006), clamp(rd.y, 0.0, 1.0)) + stars(rd);
    let drive = 0.5 + 0.8 * u.intensity;
    let jit = bluen(in.pos.xy);
    var acc = vec3<f32>(0.0);
    if rd.y > 0.02 {
        for (var i = 0; i < LAYERS; i++) {
            let fi = (f32(i) + jit) / f32(LAYERS);
            // Aurora shell between 8 and 16 units "above": sample by height.
            let h = 8.0 + fi * 8.0;
            let t = h / rd.y;
            let q = rd.xz * t;
            // Curtain: a folded line in the xz plane; distance to it.
            let fold = tnoise(vec3<f32>(q * 0.02, u.clock4.z * 0.004)).b;
            let fold2 = tnoise(vec3<f32>(q * 0.05 + 3.0, u.clock4.z * 0.006)).b;
            let line = q.y * 0.05 + (fold - 0.5) * 6.0 + (fold2 - 0.5) * 1.5;
            let band = abs(fract(line * 0.25) - 0.5) * 4.0;          // several curtains
            let sheet = exp(-band * band * 30.0);
            // Vertical rays shimmering along the curtain.
            let rays = pow(tnoise(vec3<f32>(q * 0.6, u.clock4.w * 0.02)).a * 1.6, 3.0);
            // Bright hem at the bottom, fading up.
            let hem = exp(-fi * 3.5);
            let bl = mix(u.lvl4.x, u.lvl4.w, fi);
            let lvl = (0.3 + 1.4 * bl + 1.5 * u.hits4.x * hem) * rays;
            let green = mix(vec3<f32>(0.1, 1.0, 0.45), palette(u.hue + 0.1), 0.35);
            let top = mix(vec3<f32>(0.55, 0.15, 0.9), palette(u.hue + 0.6), 0.4 + 0.3 * u.pres4.x);
            let c = mix(green, top, smoothstep(0.1, 0.8, fi));
            acc += c * sheet * lvl * (0.25 + hem) * 0.35 * smoothstep(0.02, 0.2, rd.y);
        }
        acc += vec3<f32>(1.0) * step(0.997, hash21(floor(in.pos.xy / 3.0) + floor(u.beat * 4.0))) * u.hits4.w * length(acc) * 2.0;
    }
    col += acc * drive;
    // Horizon silhouette of pines.
    let a = angle(rd.xz);
    let tree = 0.03 + 0.05 * abs(sin(a * 40.0) * sin(a * 13.0 + 1.0)) + 0.02 * sin(a * 3.0);
    col = select(col, vec3<f32>(0.002, 0.003, 0.004) + acc * 0.02, rd.y < tree);
    col += (bluen(in.pos.xy + 9.0) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
