// @heavy — raymarched. Flying through a tunnel of glowing gate rings —
// the warp-tunnel festival staple. Each ring pulses with a spectrum band,
// the bore squeezes on drops, and the camera sways inside the safe middle.

const SPACING: f32 = 3.0;   // ring spacing along z
const RING_R: f32 = 1.15;   // ring radius (base)
const TUBE: f32 = 0.11;     // ring tube thickness

fn map(p: vec3<f32>, squeeze: f32) -> f32 {
    let zz = (fract(p.z / SPACING + 0.5) - 0.5) * SPACING;
    let rr = RING_R * (1.0 - squeeze * 0.25) + sin(p.z * 0.35) * 0.08;
    // Torus around the z axis.
    return length(vec2<f32>(length(p.xy) - rr, zz)) - TUBE;
}

fn ring_hue(z: f32) -> f32 {
    return fract(floor(z / SPACING + 0.5) * 0.13 + u.hue);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.clock4.x * 4.0;
    let squeeze = clamp(u.build, 0.0, 1.0);

    // Camera: gentle sway, always well inside the ring bore.
    let ro = vec3<f32>(
        sin(z * 0.07) * 0.22,
        cos(z * 0.05) * 0.16,
        z);
    // centred() +y is down-screen — negate. Aim slightly into the sway so
    // rings approach off-centre, like a real gate course.
    var rd = normalize(vec3<f32>(
        p.x * 0.72 + cos(z * 0.07) * 0.09,
        -p.y * 0.72 - sin(z * 0.05) * 0.07,
        1.0));

    var t = 0.0;
    var glow = vec3<f32>(0.0);
    var hit = false;
    var hue: f32 = 0.0;
    var last: f32 = 0.02;
    for (var i = 0; i < 90; i++) {
        let pos = ro + rd * t;
        let d = map(pos, squeeze);
        // Tight halo around each ring surface — volumetric, weighted by how
        // much the ray actually spends near it (closer rings glow harder).
        let id = floor(pos.z / SPACING + 0.5);
        let band = hash21(vec2<f32>(id, 3.7));
        glow += palette(ring_hue(pos.z)) * exp(-max(d, 0.0) * 48.0)
              * last * (0.06 + spec(band) * 0.35 + u.kick * 0.10);
        if d < 0.0015 * (1.0 + t * 0.4) {
            hit = true;
            hue = ring_hue(pos.z);
            break;
        }
        last = clamp(d * 0.8, 0.02, 0.7);
        t += last;
        if t > 42.0 {
            break;
        }
    }

    // Dark tunnel haze — fog converges to near-black with a faint tint so
    // the rings stay crisp against the void between them.
    let fog = 1.0 - exp(-t * 0.10);
    var col = mix(vec3<f32>(0.004, 0.004, 0.012), palette(0.6 + u.hue) * 0.06, 0.4);
    col += glow;
    if hit {
        col = mix(col, palette(hue) * (0.6 + spec(fract(hue * 3.0)) * 1.8
                  + beat_pulse(5.0) * 0.7), 1.0 - fog);
    }
    col *= 0.85 + u.intensity * 0.6;

    // Vanishing-point bloom pulls the eye down the bore.
    col += palette(0.55 + u.hue) * exp(-length(p) * 3.5) * (0.05 + u.intensity * 0.15);
    col += prev(uncentred(centred(in.uv) * 0.99)) * 0.18;
    return vec4<f32>(col, 1.0);
}
