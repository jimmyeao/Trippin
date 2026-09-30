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

// Beam angle + level for formation f, beam i of n from origin o.
//   0 fan sweep   1 scissor   2 wave   3 tight bundle   4 crossfire (every
//   beam aims near a target point that both projectors share, so the rigs
//   meet in a knot of light)   5 soft chase
fn formation(f: i32, i: i32, n: i32, o: vec2<f32>, centre: f32, span: f32, tsm: f32) -> vec2<f32> {
    let fi = f32(i) / f32(max(n - 1, 1));
    let k = fi - 0.5;
    let t = u.clock4.x;
    var a = centre;
    var l = 1.0;
    if f == 0 {
        a += k * span + 0.04 * sin(tsm * 0.7 + f32(i) * 1.9);
    } else if f == 1 {
        let s = select(-1.0, 1.0, i % 2 == 0);
        a += s * (0.1 + 0.4 * (0.5 + 0.5 * sin(t * 0.5))) + k * 0.2;
    } else if f == 2 {
        a += k * span * 1.1 + 0.18 * sin(t * 0.8 - f32(i) * 0.6);
    } else if f == 3 {
        a += k * (0.05 + 0.4 * u.hits4.x) + 0.5 * sin(t * 0.25);
        l = 0.4;
    } else if f == 4 {
        let tgt = vec2<f32>(0.5 * aspect() * sin(t * 0.13), -0.2 + 0.25 * sin(t * 0.21));
        let d = tgt - o;
        a = angle(d) + k * (0.08 + 0.12 * u.pres4.y);
    } else {
        a += k * span * 1.1;
        let seq = fract(t * 0.5 - fi);
        l = 0.15 + 0.85 * exp(-seq * 6.0);
    }
    return vec2<f32>(a, l);
}

fn fan(p: vec2<f32>, o: vec2<f32>, centre: f32, span: f32, n: i32, hue0: f32, tsm: f32, smoke: f32, salt: f32) -> vec3<f32> {
    // A new formation every 4 bars (each projector picks its own), morphing
    // in over one beat along the shortest angle.
    let blk = floor(u.beat / 16.0);
    let fnow = i32(hash21(vec2<f32>(blk, salt)) * 6.0);
    let fprev = i32(hash21(vec2<f32>(blk - 1.0, salt)) * 6.0);
    let m = smoothstep(0.0, 1.0 / 16.0, fract(u.beat / 16.0));
    var col = vec3<f32>(0.0);
    var lit = vec3<f32>(0.0);
    for (var i = 0; i < 16; i++) {
        if i >= n {
            break;
        }
        let fi = f32(i) / f32(max(n - 1, 1));
        let a0 = formation(fprev, i, n, o, centre, span, tsm);
        let a1 = formation(fnow, i, n, o, centre, span, tsm);
        let da = atan2(sin(a1.x - a0.x), cos(a1.x - a0.x));
        let ang = a0.x + da * m;
        let fl = mix(a0.y, a1.y, m);
        let l = laser_line(p, o, ang);
        let band = spec(fi * 0.8 + 0.05);
        let bc = palette(hue0 + fi * 0.35);
        let c = bc * bc * 1.2 + bc * 0.2;          // saturated laser colour
        // Fixed beam count (changing it re-spaced every beam = jumps);
        // outer beams fade in smoothly as the mids build instead.
        let edge = abs(fi - 0.5) * 2.0;
        let fade_in = smoothstep(edge - 0.2, edge + 0.05, 0.35 + 0.75 * u.pres4.y);
        let lvl = (0.35 + 1.4 * band) * (0.6 + 0.6 * u.intensity) * fade_in * fl;
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

    let n = 13;
    let span = 0.7 + 0.8 * u.pres4.y;
    // Main projector low-left, sweeping up-right; direction swings each phrase.
    let o1 = vec2<f32>(-0.25 * asp, 0.55);
    let c1 = -PI * 0.5 + 0.35 + 0.45 * sin(u.clock4.x * 0.03);
    col += fan(p, o1, c1, span, n, 0.3 + u.hue, tsm, smoke, 3.1);
    // Second projector on the right edge, firing left across the first.
    let o2 = vec2<f32>(0.98 * asp, 0.1);
    let c2 = PI + 0.2 * sin(u.clock4.x * 0.025 + 1.5) - 0.15;
    col += fan(p, o2, c2, span * 0.7, 10, 0.75 + u.hue, tsm + 5.0, smoke, 8.7);

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
