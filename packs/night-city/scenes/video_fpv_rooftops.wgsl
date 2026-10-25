// @bloom 0.5
// @title Rooftop Run
// Flying low over the rooftops toward the skyline (video clip). The drive speeds up with the track, the lights
// stretch into streaks as the bass presence builds, and the view drifts
// with the phrase. No beat flashes, the road never warps.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let phrase = sin(u.flow / 32.0 * TAU);
    let zoom = 1.06 + 0.025 * sin(u.flow / 64.0 * TAU);
    let pan = vec2<f32>(0.014 * phrase, 0.006 * sin(u.flow / 48.0 * TAU));
    let base = vid_cam(in.uv, zoom, pan);
    let len = 0.04 + (0.2 * u.pres4.x + 0.1 * u.intensity) * (1.0 - 0.5 * u.calm);
    let streak = vid_streak(in.uv, zoom, pan, vec2<f32>(0.5, 0.4), len);
    var c = untonemap(base);
    let glow = vid_grade(streak, u.hue + 0.15 * phrase, 0.45);
    c += untonemap(glow * 0.55) * (0.6 + 0.6 * u.pres4.x);
    return vec4<f32>(c, 1.0);
}
