// A double helix rotating upright through the frame — strands pump thick
// on the kick, rungs travel along it on the spectrum, and the whole helix
// leans into the phrase. World up is -p.y.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.006, 0.008, 0.02);

    let hy = -p.y; // up the helix is up-screen
    let tilt = rot(sin(u.time * 0.25) * 0.2 + u.bass * 0.1);
    let pp = tilt * p;
    let sway = sin(hy * 3.0 + u.flow * 0.8) * 0.08 * (0.4 + u.energy);

    let turns = 2.2;
    let phase = hy * turns * TAU + u.flow * 1.6;
    let thick = 0.012 + u.kick * 0.012 + u.energy * 0.008;

    for (var s = 0; s < 2; s++) {
        let fs = f32(s);
        let ph = phase + fs * PI;
        let sx = sin(ph) * 0.4 + sway;
        let depth = cos(ph); // -1 back … +1 front
        let dx = pp.x - sx;
        // Strand is a bright core + halo; front strands brighter.
        let strand = exp(-dx * dx / (thick * thick));
        let halo = exp(-abs(dx) * 14.0) * 0.2;
        let shade = mix(0.35, 1.0, depth * 0.5 + 0.5);
        // Band colour travels up the strand with height = spectrum.
        let band = spec(fract(hy * 0.4 + 0.5));
        col += palette(fs * 0.5 + hy * 0.2 + u.hue) * (strand + halo) * shade
               * (0.4 + u.energy * 1.2 + band * 0.6);
    }

    // Rungs: bright links where the strands cross the middle, pulsing
    // outward when the mids are hot.
    let rung_y = fract(hy * turns + u.flow * 0.28);
    let rung = smoothstep(0.08, 0.03, abs(rung_y - 0.5));
    let width = abs(cos(phase)) * 0.4;
    let rung_in = smoothstep(width, width * 0.4, abs(pp.x - sway));
    let band = spec(fract(hy * 0.5 + 0.5));
    col += palette(0.25 + u.hue) * rung * rung_in * (0.25 + band * 2.0 + u.mid);

    col += prev(uncentred(centred(in.uv) * 0.998)) * 0.2;
    return vec4<f32>(finite(col), 1.0);
}
