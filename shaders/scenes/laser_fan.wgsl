// @bloom 0.9 @tonemap agx
// Laser fans cutting through fog-machine smoke — the club look from real
// photos: razor-thin saturated beams fanning from a hot projector point,
// each beam brightening and fading along its length as it passes through
// thicker and thinner smoke, and the billowing smoke itself lit in the
// beams' colours, with dark air between the clouds.
// Two projectors: a main one low-centre-left firing up and across, and a
// second from the right edge. Audio: the fans open with mid presence and
// sweep on the energy clock, beam count grows with energy, each beam's
// brightness follows its band, the smoke churns faster as the track drives,
// and the projectors flare on kicks.

fn fan(p: vec2<f32>, o: vec2<f32>, centre: f32, span: f32, n: i32, hue0: f32, tsm: f32, smoke: f32) -> vec3<f32> {
    var col = vec3<f32>(0.0);
    var lit = vec3<f32>(0.0);
    for (var i = 0; i < 16; i++) {
        if i >= n {
            break;
        }
        let fi = f32(i) / f32(max(n - 1, 1));
        // Per-beam drift so the fan never moves as one rigid block.
        let wob = 0.04 * sin(tsm * 0.7 + f32(i) * 1.9);
        let ang = centre + (fi - 0.5) * span + wob;
        let l = laser_line(p, o, ang);
        let band = spec(fi * 0.8 + 0.05);
        let bc = palette(hue0 + fi * 0.35);
        let c = bc * bc * 1.2 + bc * 0.2;          // saturated laser colour
        let lvl = (0.35 + 1.4 * band) * (0.6 + 0.6 * u.intensity);
        // Visible where the smoke is: patchy along the beam's length.
        col += c * l.x * lvl * (0.15 + 2.6 * smoke);
        lit += c * l.y * lvl;
    }
    // Smoke around the beams glows in their colour.
    return col + lit * smoke * 0.12;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let asp = aspect();
    // Smoke churns on the mid energy clock.
    let tsm = u.clock4.z;
    let smoke = smoke2d(p * vec2<f32>(1.0, 1.2), tsm * 2.0);

    // Near-black room; unlit smoke barely visible.
    var col = vec3<f32>(0.002, 0.002, 0.005) + vec3<f32>(0.012, 0.014, 0.025) * smoke;

    let n = 9 + i32(u.energy * 6.0);
    let span = 0.7 + 0.8 * u.pres4.y;
    // Main projector low-left, sweeping up-right; direction swings each phrase.
    let o1 = vec2<f32>(-0.25 * asp, 0.55);
    let c1 = -PI * 0.5 + 0.35 + 0.45 * sin(u.clock4.x * 0.03);
    col += fan(p, o1, c1, span, n, 0.3 + u.hue, tsm, smoke);
    // Second projector on the right edge, firing left across the first.
    let o2 = vec2<f32>(0.98 * asp, 0.1);
    let c2 = PI + 0.2 * sin(u.clock4.x * 0.025 + 1.5) - 0.15;
    col += fan(p, o2, c2, span * 0.7, n - 3, 0.75 + u.hue, tsm + 5.0, smoke);

    // Projector hot spots (lens flare), flaring on kicks.
    for (var k = 0; k < 2; k++) {
        let o = select(o1, o2, k == 1);
        let d = length(p - o);
        let hc = palette(select(0.3, 0.75, k == 1) + u.hue);
        col += (vec3<f32>(1.0) * exp(-d * 90.0) * 2.0 + hc * exp(-d * 12.0) * 0.4) * (0.7 + 1.2 * u.hits4.x);
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.004;
    return vec4<f32>(col, 1.0);
}
