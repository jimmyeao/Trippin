// Fire-and-ice mandala: a kaleidoscopic ring of flame filaments around a black
// core, wrapped in wispy blue smoke petals, with drifting sparks. Motion is
// continuous (flowing outward, slowly turning); the music feeds its heat.

const SEG: f32 = 12.0;

// Ridged noise: thin bright creases, the look of flame and smoke filaments.
fn ridged(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var v = 0.0;
    var a = 0.55;
    for (var i = 0; i < 5; i++) {
        let n = 1.0 - abs(noise(p) * 2.0 - 1.0);
        v += a * n * n;
        p = rot(0.7) * p * 2.1 + 3.1;
        a *= 0.5;
    }
    return v;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p0 = centred(in.uv) * 1.05;
    // Direction: the mandala turns one way, then the other.
    let p = rot(1.5 * sin(u.clock4.x * 0.02)) * p0;
    let r = length(p);
    // Mirror into one wedge so the pattern is symmetrical like a mandala.
    let wedge = TAU / SEG;
    let a = abs(((angle(p) % wedge) + wedge) % wedge - wedge * 0.5);
    // Energy: flames churn faster as the mids drive.
    let t = u.clock4.z * 0.5;

    // --- Fire ring -------------------------------------------------------
    // Shape: the fire ring swells with bass presence, jolts on kicks.
    let fire_r = 0.34 + 0.12 * u.pres4.x + 0.04 * u.hits4.x;
    let fq = vec2<f32>(a * 5.0, (r - t * 0.06) * 10.0);
    let warp = vec2<f32>(fbm(fq * 0.7 + t * 0.15), fbm(fq * 0.7 - t * 0.12 + 4.0));
    let flame = ridged(fq + warp * 1.6);
    let fire_band = exp(-pow((r - fire_r) / (0.085 + 0.02 * u.intensity), 2.0));
    let heat = pow(flame, 4.5) * fire_band * (1.6 + 1.2 * u.intensity);
    var col = vec3<f32>(1.4, 0.42, 0.08) * heat + vec3<f32>(1.0, 0.85, 0.45) * pow(heat, 3.0) * 0.8;
    // A touch of magenta where fire meets smoke.
    col += vec3<f32>(0.9, 0.2, 0.7) * exp(-pow((r - fire_r - 0.1) / 0.03, 2.0)) * flame * 0.25;

    // --- Smoke petals ----------------------------------------------------
    // Irregular, slowly changing outline (mirrored per wedge), not a gear.
    let edge = 0.8 + 0.16 * fbm(vec2<f32>(a * 5.0, t * 0.12)) + 0.03 * sin(t * 0.3);
    let petal = smoothstep(0.47, 0.58, r) * smoothstep(edge, edge - 0.14, r);
    let sq = vec2<f32>(a * 2.4, r * 3.2 - t * 0.08);
    let swirl = fbm(sq + vec2<f32>(fbm(sq * 1.3 + t * 0.05), fbm(sq * 1.3 - t * 0.04 + 7.0)) * 1.4);
    // Thin translucent ribbons: mostly the crease lines, very little fill.
    let wisp = pow(1.0 - abs(swirl * 2.0 - 1.0), 12.0);
    let wisp2 = pow(1.0 - abs(fbm(sq * 2.3 + 11.0 + t * 0.03) * 2.0 - 1.0), 16.0);
    let smoke = (wisp * 1.2 + wisp2 * 0.7 + 0.04 * swirl) * petal;
    col += vec3<f32>(0.12, 0.45, 1.3) * smoke + vec3<f32>(0.7, 0.9, 1.0) * pow(wisp, 3.0) * petal * 0.6;
    // Curling tips just beyond the petal edge.
    col += vec3<f32>(0.2, 0.55, 1.2) * pow(wisp, 2.0) * smoothstep(edge + 0.06, edge, r) * smoothstep(edge - 0.1, edge, r) * 0.6;

    // --- Black core and sparks --------------------------------------------
    col *= smoothstep(0.24, 0.34, r);
    let cell = floor(p0 * 45.0);
    let h = hash21(cell);
    let spark = step(0.985, h) * smoothstep(1.4, 0.5, r) * smoothstep(0.35, 0.55, r);
    let twinkle = 0.5 + 0.5 * sin(t * 3.0 + h * 50.0);
    let dot_d = length(fract(p0 * 45.0) - 0.5);
    col += vec3<f32>(0.35, 0.6, 1.0) * spark * twinkle * smoothstep(0.35, 0.0, dot_d) * 0.9;

    col += prev(uncentred(centred(in.uv) * 0.995)) * 0.1;
    return vec4<f32>(col, 1.0);
}
