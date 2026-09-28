// Neon city drive-by: three parallax layers of skyline sliding past like
// you're cruising the strip. Buildings rise from the BOTTOM edge (uv.y=0
// is screen top — up = -p.y / 1-uv.y). Windows flicker, rooftops glow on
// the beat, a giant moon hangs over it all.

fn skyline(x: f32, seed: f32) -> f32 {
    // Building tops: blocky skyline, heights hashed + driven by spectrum.
    let i = floor(x);
    let h = hash21(vec2<f32>(i, seed));
    let v = spec(fract(x * 0.05));
    return 0.15 + h * 0.45 + v * 0.3;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let up = 1.0 - in.uv.y;
    let x = in.uv.x;

    // Night sky gradient + a huge low moon.
    var col = mix(vec3<f32>(0.008, 0.005, 0.025), palette(u.hue) * 0.35,
                  pow(1.0 - up, 1.8));
    let moon_d = length(vec2<f32>((x - 0.72) * aspect(), up - 0.55));
    let moon = smoothstep(0.24, 0.22, moon_d);
    col = mix(col, palette(0.15 + u.hue) * (0.7 + u.bass * 0.5), moon * 0.9);
    col += palette(0.15 + u.hue) * exp(-moon_d * 4.0) * 0.3;

    // Three parallax layers: far first, scrolling slowest.
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let depth = 1.0 + fi;           // 1=far, 3=near
        let parallax = u.flow * 0.35 / depth;
        let sx = x * aspect() * 14.0 * depth + parallax * 14.0;
        let h = skyline(sx * 0.25, fi * 7.0) * (0.55 + fi * 0.25);
        if up < h {
            let shade = 0.10 + fi * 0.08; // near = darker silhouette
            let wall = vec3<f32>(0.015, 0.012, 0.03) + palette(0.55 + fi * 0.15 + u.hue) * shade;
            col = wall;
            // Windows aligned to the building: local-x within the tower,
            // floor rows by height — sparse warm panes, not noise.
            let bx = fract(sx * 0.25);
            let bix = floor(sx * 0.25);
            let wcol = floor(bx * 5.0);
            let wrow = floor(up * 26.0 * depth);
            let lit = step(0.82 - fi * 0.03,
                           hash21(vec2<f32>(bix * 17.0 + wcol, wrow) + fi));
            let pane = step(0.1, fract(bx * 5.0)) * step(fract(bx * 5.0), 0.55)
                     * step(0.25, fract(up * 26.0 * depth)) * step(fract(up * 26.0 * depth), 0.7);
            col += mix(vec3<f32>(0.95, 0.8, 0.45), palette(fi * 0.2 + u.hue), 0.3)
                   * lit * pane * step(0.02, up) * (0.5 + u.energy * 0.9 + u.high * 0.4);
            // Roof edge glow.
            let roof = smoothstep(0.01 + fi * 0.004, 0.0, h - up);
            col += palette(fi * 0.3 + u.hue) * roof * (0.5 + u.kick * 1.2);
        }
    }
    // Street glow strip.
    col += palette(0.1 + u.hue) * exp(-up * 25.0) * (0.2 + u.bass * 0.4);
    return vec4<f32>(finite(col), 1.0);
}
