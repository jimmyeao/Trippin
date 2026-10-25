// @bloom 0.5
// @title Highway Lights
// Car lights streaming along a city highway at night (video clip). The moving lights leave trails that grow with the
// energy, a light wave rolls out from the centre on each kick and colours
// them, and the camera drifts with the phrase. The buildings stay put.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let phrase = sin(u.flow / 32.0 * TAU);
    let zoom = 1.07 + 0.02 * sin(u.flow / 64.0 * TAU);
    let pan = vec2<f32>(0.012 * phrase, 0.0);
    let base = vid_cam(in.uv, zoom, pan);
    let lit = vid_hl(base);
    // A ring of colour expanding from the centre each beat, only on lights.
    let p = centred(in.uv);
    let r = length(p);
    let dr = r - u.beat_phase * 1.8;
    let ring = exp(-dr * dr * 40.0) * (0.3 + 0.7 * u.lvl4.x) * (1.0 - u.calm);
    let tint = palette(u.hue + r * 0.3 + u.flow * 0.01);
    var c = untonemap(base) + untonemap(lit * tint * tint * ring * 0.9);
    // Lighten feedback: trails persist longer as the energy rises.
    let keep = 0.78 + 0.16 * u.pres4.y * (1.0 - 0.4 * u.calm);
    c = max(c, prev(in.uv) * keep);
    return vec4<f32>(c, 1.0);
}
