// Twin laser fans sweeping the room — the festival rig look. Beams are
// razor-thin cores with hot halos, each beam's angle walks independently.
// The fans open wider with energy; onsets snap them to new spreads.
// +p.y is down-screen, so the rig sits just below the frame bottom.

fn fan(d: vec2<f32>, count: f32, spread: f32, phase: f32) -> vec2<f32> {
    // Returns (beam intensity, per-beam hash index).
    let a = atan2(d.y, d.x); // up-screen is -PI/2
    let mid = -PI * 0.5;
    let fan_span = spread * (0.7 + u.energy * 0.6);
    let centred_a = a - mid + phase;
    let half = fan_span * 0.5;
    if abs(centred_a) > half {
        return vec2<f32>(0.0, 0.0);
    }
    let n = count + floor(u.energy * 4.0);
    let pos = (centred_a + half) / fan_span; // 0..1 across the fan
    let idx = floor(pos * n);
    let loc = fract(pos * n) - 0.5;
    // Per-beam wobble so they don't move as one rigid fan.
    let wob = (hash21(vec2<f32>(idx, floor(u.beat))) - 0.5) * u.mid * 0.10;
    let core = exp(-abs(loc + wob) * 160.0);
    let halo = exp(-abs(loc + wob) * 18.0) * 0.3;
    return vec2<f32>(core + halo, idx / max(n, 1.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.006, 0.008, 0.02);

    // Atmospheric haze.
    col += palette(0.55 + u.hue) * fbm(p * 1.5 + u.flow * 0.03) * 0.10 * (0.4 + u.mid);

    // Two rigs: one below the frame, one above — beams cross mid-screen.
    let o1 = vec2<f32>(-0.5 * aspect(), 1.3);
    let o2 = vec2<f32>(0.5 * aspect(), -1.3);
    let swing1 = sin(u.flow * 0.35) * 0.25;
    let swing2 = -sin(u.flow * 0.28 + 1.3) * 0.25;

    let f1 = fan(p - o1, 9.0, 0.9, swing1);
    let f2 = fan(vec2<f32>(-(p - o2).x, -(p - o2).y), 9.0, 0.9, swing2);

    col += palette(f1.y * 0.8 + u.hue) * f1.x * (0.5 + u.energy * 1.6);
    col += palette(f2.y * 0.8 + 0.5 + u.hue) * f2.x * (0.5 + u.energy * 1.6);

    // Hot sources at each rig.
    col += palette(u.hue) * exp(-length(p - o1) * 8.0) * (0.4 + u.kick);
    col += palette(0.5 + u.hue) * exp(-length(p - o2) * 8.0) * (0.4 + u.kick);
    col += prev(in.uv) * 0.22;
    return vec4<f32>(col, 1.0);
}
