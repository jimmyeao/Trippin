// Embers rising off a fire pit at the bottom of the frame: particles drift
// UP-screen (−p.y is up), wavering with noise, glowing hot then cooling
// out. Bass feeds the fire; onsets throw up sparks.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.012, 0.006, 0.01);

    // Fire glow at the bottom of the frame (+p.y is down).
    col += mix(vec3<f32>(1.0, 0.4, 0.1), palette(0.95 + u.hue), 0.4)
           * exp(-(1.0 - p.y) * 2.5) * (0.4 + u.bass * 0.9 + u.kick * 0.4);

    for (var layer = 0; layer < 3; layer++) {
        let fl = f32(layer);
        let scale = 14.0 + fl * 8.0;
        let speed = 0.12 + fl * 0.10; // steady rise — audio feeds the glow
        var q = p * scale;
        // Energy: the rise speed follows the smooth energy clock;
        // direction: a wind that swings the embers left and right.
        q.y += u.clock4.x * speed * scale * 0.4; // rise = -screen-y → scroll +y
        q.x += sin(u.clock4.x * 0.03 + fl) * (1.0 - p.y) * scale * 0.08;
        let cell = floor(q);
        let h = hash21(cell + fl * 61.0);
        let local = fract(q) - 0.5;
        if h < 0.1 + u.energy * 0.12 {
            // Ember drifts sideways as it climbs.
            let sway = (hash21(cell + 3.0) - 0.5) * 0.5;
            // Jitter each ember inside its cell so they don't sit on a grid.
            let jit = (hash22(cell + 9.0) - 0.5) * 0.7;
            let sp = local - jit + vec2<f32>(sway * sin(u.clock4.w * 0.5 + h * 20.0), 0.0);
            let d = length(sp);
            // Bright core, warm tail; hotter when born (bottom of cell).
            let cool = fract(h * 7.0 + u.clock4.x * speed * 0.4);
            let heat = mix(vec3<f32>(1.0, 0.85, 0.4), vec3<f32>(1.0, 0.2, 0.05), cool);
            col += heat * exp(-d * d * 90.0) * (0.5 + u.onset * 1.5);
        }
    }
    return vec4<f32>(finite(col), 1.0);
}
