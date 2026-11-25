// Embers (accent layer): glowing sparks drifting up through the frame in
// three depth layers. Rise rides the smooth energy clock (motion), the whole
// field sways on a four-bar sine (direction), and the sparks swell with the
// slow bass presence (shape). A fixed count per layer: nothing re-spaces on
// energy. (Many tiny ones read as noise; these are few, big and soft.)

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.0);
    for (var layer = 0; layer < 3; layer++) {
        let fl = f32(layer);
        // Near layers: bigger cells (bigger sparks), faster.
        let scale = 3.2 + fl * 1.6;
        let rise = u.clock4.x * (0.16 - 0.04 * fl);
        let sway = 0.1 * sin(TAU * u.flow / 16.0 + fl * 2.1);
        // +y is down: a spark that rises sits lower in the field over time.
        var q = vec2<f32>(p.x + sway, p.y + rise) * scale;
        q.x += 0.2 * sin(q.y * 0.8 + fl * 3.0);
        let cell = floor(q);
        let h = hash22(cell + vec2<f32>(fl * 31.0, 5.0));
        if h.x > 0.3 {
            continue;
        }
        let off = (hash22(cell + vec2<f32>(7.0, fl * 13.0)) - 0.5) * 0.5;
        // Slightly tall: a spark streaks a little as it rises.
        let dv = (fract(q) - 0.5 - off) * vec2<f32>(1.0, 0.6);
        let dd = length(dv);
        let r = (0.035 + 0.025 * u.pres4.x) * (0.7 + 0.6 * h.y);
        let core = smoothstep(r, r * 0.15, dd);
        let glow = exp(-dd * 7.0) * 0.3;
        // Each spark breathes on its own slow cycle (never on the beat).
        let breathe = 0.6 + 0.4 * sin(u.time * (0.7 + 1.0 * h.y) + h.x * 40.0);
        // Warm end of the palette: embers, not confetti.
        let c = palette(0.02 + h.y * 0.12);
        col += c * c * (core * 1.6 + glow) * breathe * (1.0 - 0.25 * fl);
    }
    return vec4<f32>(finite(col), 1.0);
}
