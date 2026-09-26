// Giant VU meters: three chunky LED columns for bass, mid and high with
// segmented rungs, peak markers and a warm tube-amp backdrop.

fn column(uv: vec2<f32>, x0: f32, w: f32, level: f32, tint: f32, peak: f32) -> vec3<f32> {
    if uv.x < x0 || uv.x > x0 + w {
        return vec3<f32>(0.0);
    }
    let segs = 24.0;
    let s = floor(uv.y * segs);
    let lit = step(s / segs, level);
    let cell = step(0.12, fract(uv.y * segs)) * step(fract(uv.y * segs), 0.88);
    // Green -> amber -> red as the column climbs.
    let c_off = vec3<f32>(0.03, 0.03, 0.035);
    var c_on = mix(vec3<f32>(0.15, 0.9, 0.3), vec3<f32>(1.0, 0.6, 0.1), step(0.6, s / segs));
    c_on = mix(c_on, vec3<f32>(1.0, 0.15, 0.15), step(0.85, s / segs));
    c_on = mix(c_on, palette(tint), 0.25);
    var col = mix(c_off, c_on, lit * (0.55 + level * 0.8)) * cell;
    // Peak-hold-ish bright rung near the top of the current level.
    let pl = abs(uv.y - peak);
    col += c_on * smoothstep(1.5 / segs, 0.0, pl) * 0.9;
    // Column side shading.
    col *= 0.75 + 0.25 * smoothstep(0.0, 0.03, uv.x - x0) * smoothstep(0.0, 0.03, x0 + w - uv.x);
    return col;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Backdrop: dark lacquer with a breathing warmth.
    var col = vec3<f32>(0.02, 0.015, 0.02);
    col += palette(0.1) * 0.06 * exp(-length(centred(uv)) * 0.8) * (0.4 + u.energy);

    // Fake peak-hold: keep a decaying memory in the feedback buffer.
    let pm = prev(vec2<f32>(0.5, 0.99)).rgb; // stored peaks: r=bass g=mid b=high
    let peaks = max(vec3<f32>(u.bass, u.mid, u.high), pm - vec3<f32>(u.dt * 0.4));

    col += column(uv, 0.18, 0.16, u.bass, 0.0, peaks.r);
    col += column(uv, 0.42, 0.16, u.mid, 0.33, peaks.g);
    col += column(uv, 0.66, 0.16, u.high, 0.66, peaks.b);

    // Kick flash along the top edge.
    col += palette(0.9) * smoothstep(0.94, 1.0, uv.y) * u.kick * 0.4;

    // Store peaks in the top row for next frame (overwrites meter pixels there).
    if uv.y > 0.985 && abs(uv.x - 0.5) < 0.02 {
        col = peaks;
    }

    return vec4<f32>(finite(col), 1.0);
}
