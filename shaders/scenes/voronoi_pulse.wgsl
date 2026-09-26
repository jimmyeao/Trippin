// Voronoi pulse: a stained-glass mosaic where each cell's brightness is a
// spectrum band — edges glow like neon grout, cells throb on the beat.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv) * vec2<f32>(1.0, 1.0) * 4.0;

    // Voronoi: nearest and second-nearest cell points.
    let ip = floor(p);
    let fp = fract(p);
    var d1 = 8.0;
    var d2 = 8.0;
    var cell_id = vec2<f32>(0.0);
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let g = vec2<f32>(f32(i), f32(j));
            let o = hash22(ip + g + u.seed);
            // Cell points wander slightly with the beat.
            let wob = 0.12 * sin(u.beat_phase * TAU + hash21(ip + g) * TAU);
            let r = g + o + wob - fp;
            let d = dot(r, r);
            if d < d1 {
                d2 = d1;
                d1 = d;
                cell_id = ip + g;
            } else if d < d2 {
                d2 = d;
            }
        }
    }

    // Cell brightness from its hash slot in the spectrum.
    let slot = hash21(cell_id + u.seed);
    let v = spec(slot * 0.9);
    let edge = smoothstep(0.0, 0.10, d2 - d1);

    var col = palette(slot + u.hue * 0.5) * (0.08 + v * (0.5 + u.energy * 0.5)) * edge;
    col += palette(slot + 0.5) * (1.0 - edge) * (0.4 + u.kick * 0.5);
    col += palette(0.9) * u.flash * 0.3;

    return vec4<f32>(finite(col), 1.0);
}
