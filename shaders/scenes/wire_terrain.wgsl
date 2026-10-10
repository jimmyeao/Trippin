// @heavy — heightfield march. Neon wireframe terrain flyover: the Tron /
// synthwave grid-mountain look. Ridged fbm peaks glow against a dusk sky,
// the bass lifts the ground, and the camera swoops with the phrase.

fn ground(p: vec3<f32>, lift: f32) -> f32 {
    // Ridged noise reads as mountain crests.
    var h = fbm(p.xz * 0.16) - 0.45;
    h = 1.0 - abs(h) * 2.2;
    return p.y - h * (1.4 + lift * 1.2) + 1.1;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Energy: flight speed follows the mix; shape: the ground rises with
    // bass presence (smooth) and a kick.
    let z = u.clock4.x * 2.5;
    let lift = u.pres4.x + 0.4 * u.hits4.x;

    // Swoop: altitude and heading drift with the phrase, kick dips low.
    let ro = vec3<f32>(
        sin(z * 0.04) * 2.0,
        1.3 + sin(u.time * 0.3) * 0.25 - u.hits4.x * 0.25,
        z);
    var rd = normalize(vec3<f32>(
        p.x * 0.8 + cos(z * 0.04) * 0.22,
        -p.y * 0.8 - 0.22,
        1.0));
    // Direction: banking one way, then the other.
    rd = vec3<f32>(rot(0.3 * sin(u.clock4.x * 0.025)) * rd.xy, rd.z);

    var t = 0.0;
    var hit = false;
    var pos = ro;
    for (var i = 0; i < 64; i++) {
        pos = ro + rd * t;
        let d = ground(pos, lift) * 0.7;
        if d < 0.01 * (1.0 + t * 0.4) {
            hit = true;
            break;
        }
        t += clamp(d, 0.03, 1.2);
        if t > 60.0 {
            break;
        }
    }

    // Sky: dusk gradient with a low glow band at the horizon.
    let sky_t = clamp(-rd.y * 2.2 + 0.4, 0.0, 1.0);
    var col = mix(palette(0.05 + u.hue) * 0.55, vec3<f32>(0.01, 0.01, 0.03), sky_t);
    col += palette(0.1 + u.hue) * exp(-abs(rd.y + 0.1) * 14.0) * (0.3 + u.intensity * 0.5);

    if hit {
        // Wireframe: emissive grid lines over near-black ground.
        let gp = fract(pos.xz * 0.8);
        let lx = smoothstep(0.06, 0.01, min(gp.x, 1.0 - gp.x));
        let lz = smoothstep(0.06, 0.01, min(gp.y, 1.0 - gp.y));
        let line = max(lx, lz);
        let fade = exp(-t * 0.10);
        let ridge = clamp(1.0 - pos.y * 0.4, 0.0, 1.0);
        let lcol = palette(fract(pos.z * 0.02 + u.hue));
        col = mix(col, lcol * (line * (0.7 + u.energy * 1.4) + 0.04) * ridge, fade);
    }

    col += prev(uncentred(centred(in.uv) * 0.995)) * 0.15;
    return vec4<f32>(col, 1.0);
}
