// @bloom 0.7 @tonemap agx
// Synesthesia-style abstract: a psychedelic feedback tunnel. Each frame
// re-draws the last one slightly zoomed, rotated and hue-shifted, so a few
// crisp neon shapes drawn at the centre smear into an endless spiralling
// tunnel of echoes. 2D, cheap (not @heavy).
// Audio vocabulary:
//  - the zoom into the echo (the "travel") runs on the whole-mix energy
//    clock's *rate* — deep dive on drops, near-still in breakdowns;
//  - twist rate on the mid clock; the seed shape is a spectrum ring whose
//    radius follows each band; bass hits stamp a bright polygon;
//  - echo persistence rises with presence (long trails in big sections).

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // How fast the clocks are moving right now (0.3..1.5 x tempo) — derive
    // a per-frame zoom from it so the dive speed is smooth.
    let energy = clamp(u.lvl4.x * 0.45 + u.lvl4.y * 0.3 + u.lvl4.z * 0.15 + u.lvl4.w * 0.1, 0.0, 1.0);
    let zoom = 1.0 - (0.004 + 0.02 * energy * energy) * (u.bpm / 120.0);
    let twist = (0.002 + 0.006 * u.pres4.y) * sin(u.clock4.z * 0.07);
    // Sample the previous frame pulled toward the centre (a zoom in).
    let q = rot(twist) * p * zoom;
    var echo = prev(uncentred(q));
    // Hue-rotate the echo slightly each frame (cheap: rotate in a YIQ-ish plane).
    let l = dot(echo, vec3<f32>(0.3, 0.5, 0.2));
    let c = echo - l;
    let hs = 0.03 + 0.02 * u.lvl4.w;
    let axis = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let c2 = c * cos(hs) + cross(axis, c) * sin(hs) + axis * dot(axis, c) * (1.0 - cos(hs));
    echo = max(l + c2, vec3<f32>(0.0));
    let persist = 0.955 + 0.03 * max(u.pres4.x, u.pres4.y);
    var col = echo * persist;

    // Seed shapes at the centre.
    let r = length(p);
    let a = angle(p);
    let hue = u.clock4.w * 0.01 + u.hue;
    // Spectrum ring: radius bulges with each band (mirrored — no seam).
    let band = abs(fract(a / TAU + 0.5) - 0.5) * 2.0;
    let ring_r = 0.18 + 0.1 * spec(band * 0.85);
    let ring = smoothstep(0.012, 0.0, abs(r - ring_r));
    col += palette(band * 0.5 + hue) * ring * (0.5 + 0.8 * u.intensity);
    // Bass hits stamp a thin polygon that then flies down the tunnel.
    let n = 6.0;
    let sec = TAU / n;
    let pa = abs(fract((a + u.clock4.z * 0.05) / sec + 0.5) - 0.5) * sec;
    let poly = r * cos(pa) - 0.34;
    col += palette(hue + 0.5) * smoothstep(0.01, 0.0, abs(poly)) * u.hits4.x * 1.5;
    // High hits: a scatter of sparks around the ring.
    let sp = hash21(floor(p * 60.0) + floor(u.beat * 4.0));
    col += vec3<f32>(1.0) * step(0.997, sp) * u.hits4.w * smoothstep(0.5, 0.2, r);
    // Keep the very centre dark so the tunnel has a vanishing point.
    col *= smoothstep(0.0, 0.06, r);
    return vec4<f32>(min(col, vec3<f32>(8.0)), 1.0);
}
