// Embers rising off a fire pit at the bottom of the frame: particles drift
// UP-screen (−p.y is up), wavering with noise, glowing hot then cooling
// out. Bass feeds the fire; onsets throw up sparks.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.012, 0.006, 0.01);

    // Fire glow at the bottom of the frame (+p.y is down).
    col += mix(vec3<f32>(1.0, 0.4, 0.1), palette(0.95 + u.hue), 0.4)
           * exp(-(p.y + 0.8) * 3.0) * (0.4 + u.bass * 0.9 + u.kick * 0.4);

    for (var layer = 0; layer < 3; layer++) {
        let fl = f32(layer);
        let scale = 14.0 + fl * 8.0;
        let speed = 0.12 + fl * 0.10; // steady rise — audio feeds the glow
        var q = p * scale;
        q.y += u.flow * speed * scale * 0.4; // rise = -screen-y → scroll +y
        let cell = floor(q);
        let h = hash21(cell + fl * 61.0);
        let local = fract(q) - 0.5;
        if h < 0.30 + u.energy * 0.2 {
            // Ember drifts sideways as it climbs.
            let sway = (hash21(cell + 3.0) - 0.5) * 0.5;
            let sp = local + vec2<f32>(sway * sin(u.time + h * 20.0), 0.0);
            let d = length(sp);
            // Bright core, warm tail; hotter when born (bottom of cell).
            let cool = fract(h * 7.0 + u.flow * speed * 0.4);
            let heat = mix(vec3<f32>(1.0, 0.85, 0.4), vec3<f32>(1.0, 0.2, 0.05), cool);
            col += heat * exp(-d * d * 90.0) * (0.5 + u.onset * 1.5);
        }
    }
    return vec4<f32>(finite(col), 1.0);
}
