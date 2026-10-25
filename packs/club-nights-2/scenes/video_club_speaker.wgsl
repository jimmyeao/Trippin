// @bloom 0.6
// @title Speaker Splash
// @video-lock
// Water jumping on a pumping speaker cone (video clip, locked to the bar). The lights flare into glints on
// each kick, fills stutter the footage and drops scratch it back. The
// camera never punches in on the beat: that read badly on footage.

fn glints(uv: vec2<f32>, zoom: f32, pan: vec2<f32>, len: f32) -> vec3<f32> {
    var acc = vec3<f32>(0.0);
    for (var i = 1; i <= 6; i++) {
        let o = f32(i) / 6.0 * len;
        let w = 1.0 - f32(i) / 7.0;
        acc += (vid_hl(vid_cam(uv + vec2<f32>(o, 0.0), zoom, pan)) + vid_hl(vid_cam(uv - vec2<f32>(o, 0.0), zoom, pan))
            + vid_hl(vid_cam(uv + vec2<f32>(0.0, o * 1.6), zoom, pan)) + vid_hl(vid_cam(uv - vec2<f32>(0.0, o * 1.6), zoom, pan))) * w * w;
    }
    return acc / 8.0;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let phrase = sin(u.flow / 32.0 * TAU);
    // The kick: on the beat grid (not the detector's lag), sharp attack,
    // gone by the half beat; none in breakdowns.
    let kick = exp(-u.beat_phase * 7.0) * (1.0 - u.calm) * (0.4 + 0.6 * min(u.hits4.x, 1.0));
    let zoom = 1.08 + 0.03 * u.pres4.x;
    let pan = vec2<f32>(0.016 * phrase, 0.008 * sin(u.flow / 16.0 * TAU));
    let base = vid_cam(in.uv, zoom, pan);
    let len = (0.01 + 0.045 * kick + 0.02 * u.hits4.x + 0.02 * u.pres4.z) * (1.0 - 0.6 * u.calm);
    let g = glints(in.uv, zoom, pan, len);
    var c = untonemap(vid_grade(base, u.hue + u.flow * 0.02, 0.25));
    c += untonemap(vid_grade(g, u.hue + 0.5, 0.6) * 0.7);
    return vec4<f32>(c, 1.0);
}
