// Confetti cannon: hashed particles falling down-screen (+y), tumbling
// and twinkling. Density rides energy; each onset launches a fresh burst
// from the top.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let n = 30.0;
    var col = vec3<f32>(0.008, 0.006, 0.018);

    for (var layer = 0; layer < 3; layer++) {
        let fl = f32(layer);
        let scale = n * (0.6 + fl * 0.35);
        let speed = 0.25 + fl * 0.12; // fixed — energy lights the pieces, not their fall rate
        var q = p * scale;
        q.y -= u.flow * speed * scale * 0.22; // fall = +y screen direction
        let cell = floor(q);
        let h = hash21(cell + fl * 137.0);
        let local = fract(q) - 0.5;

        // Sparse occupancy — fixed so pieces don't pop in and out.
        if h < 0.24 {
            let jx = (hash21(cell + 7.0) - 0.5) * 0.6;
            let tumble = u.time * (2.0 + h * 6.0) + h * 40.0;
            let sq = abs(rot(tumble) * local);
            let piece = smoothstep(0.30, 0.22, max(sq.x, sq.y * (2.0 + sin(tumble) * 1.5)));
            let twinkle = 0.6 + 0.4 * sin(tumble * 3.0);
            col += palette(h * 9.0 + u.hue) * piece * twinkle * (0.4 + u.energy * (0.7 + u.onset));
        }
    }
    col += prev(in.uv) * 0.15;
    return vec4<f32>(col, 1.0);
}
