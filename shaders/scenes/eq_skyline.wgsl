// A smooth spectrum skyline — continuous towers of light rising out of a
// dark horizon, drawn soft rather than blocky: wide columns with glowing
// crowns, a floor reflection, and atmospheric haze instead of a starfield.
// up = 1 - uv.y.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let up = 1.0 - in.uv.y;

    // Night gradient with city glow pooling on the horizon.
    var col = mix(vec3<f32>(0.004, 0.006, 0.02), vec3<f32>(0.02, 0.008, 0.04),
                  up);
    col += vec3<f32>(0.4, 0.12, 0.05) * exp(-up * 9.0) * 0.5;

    // Towers: smooth wide columns, height = spectrum at their x.
    let n = 22.0;
    // Direction: the skyline pans left, then right, across a phrase.
    let sx = in.uv.x + 0.25 * sin(u.clock4.x * 0.02);
    let cx = floor(sx * n);
    let fx = fract(sx * n);
    let band = spec((cx + 0.5) / n);
    let wob = hash21(vec2<f32>(cx, 5.0));
    // Shape: towers lean their crowns into a wave with mid presence.
    let h = 0.12 + band * (0.4 + u.energy * 0.3) + wob * 0.08 + 0.06 * u.pres4.y * sin(cx * 0.6 + u.clock4.z * 0.3);

    // Tower body: soft-edged silhouette slightly darker than the sky.
    let body = smoothstep(0.0, 0.1, fx) * smoothstep(1.0, 0.9, fx)
             * step(up, h);
    col = mix(col, vec3<f32>(0.008, 0.01, 0.028), body * 0.9);

    // Lit crown: the top of each tower glows with its band.
    let crown = exp(-abs(up - h) * 40.0) * smoothstep(0.0, 0.1, fx)
              * smoothstep(1.0, 0.9, fx);
    col += palette(cx / n + u.hue) * crown * (0.7 + band * 1.5 + u.kick);

    // Vertical light wash inside each tower — smooth, not panes.
    let wash = exp(-abs(fx - 0.5) * 6.0) * step(up, h)
             * (0.2 + band * 0.8) * up / max(h, 0.01);
    col += palette(cx / n + u.hue) * wash * 0.4;

    // Flare spiking above the crown on loud bands.
    let spike = exp(-max(up - h, 0.0) * 18.0) * step(h, up)
              * exp(-abs(fx - 0.5) * 5.0);
    col += palette(cx / n + u.hue) * spike * band * 0.6;

    // Reflection: mirrored smear below the horizon line.
    let refl = exp(-up * 3.0) * 0.08 * band;
    col += palette(cx / n + u.hue) * refl;

    return vec4<f32>(finite(col), 1.0);
}
