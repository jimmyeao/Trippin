// Retro horizon: a grid that moves one line per beat, a striped sun that swells
// with the bass, and a skyline made of the live spectrum.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let y = -p.y;
    let horizon = -0.05;

    // Floor grid (computed everywhere; masked below the horizon).
    let depth = 0.5 / max(horizon - y, 0.002);
    // Direction: the road drifts left and right; energy: drive speed.
    let gx = p.x * depth + 3.0 * sin(u.clock4.x * 0.02);
    let gz = depth + u.clock4.x * 2.0;
    let wx = fwidth(gx) * 1.5 + 0.01;
    let wz = fwidth(gz) * 1.5 + 0.01;
    let lines = max(
        smoothstep(wx, 0.0, abs(fract(gx * 0.5) - 0.5) - 0.02),
        smoothstep(wz, 0.0, abs(fract(gz * 0.5) - 0.5) - 0.02)
    );
    let fog = exp(-depth * 0.08);
    let ground = palette(0.8) * lines * fog * (0.7 + 0.6 * u.intensity);
    let below = step(y, horizon);

    // Sky: gradient, sun, spectrum skyline, stars.
    let sky_t = clamp((y - horizon) / 1.0, 0.0, 1.0);
    var sky = mix(palette(0.95) * 0.35, vec3<f32>(0.02, 0.0, 0.06), sqrt(sky_t));
    let sun_c = vec2<f32>(0.0, horizon + 0.35);
    // Shape: the sun swells with bass presence.
    let sun_r = 0.28 + 0.1 * u.pres4.x;
    let sd = length(vec2<f32>(p.x, y) - sun_c);
    let stripes = step(0.5, fract((y - horizon) * 14.0 + u.clock4.w * 0.25)) + step(sun_c.y, y);
    let sun = smoothstep(sun_r, sun_r - 0.01, sd) * min(stripes, 1.0);
    sky = mix(sky, mix(palette(0.05), palette(0.25), (y - horizon) / 0.7) * 1.5, sun);
    sky += palette(0.1) * 0.08 / (sd * sd * 4.0 + 0.1) * (0.4 + u.bass);

    let fx = abs(p.x) / aspect();
    let sky_h = horizon + 0.03 + 0.35 * spec(fx) * (0.5 + u.intensity);
    let skyline = smoothstep(sky_h + 0.004, sky_h, y);
    sky = mix(sky, vec3<f32>(0.01, 0.0, 0.03) + palette(0.7) * 0.25 * smoothstep(sky_h - 0.01, sky_h, y), skyline);

    let star = step(0.997, hash21(floor(in.uv * vec2<f32>(u.res_x, u.res_y) / 3.0)));
    sky += star * (0.3 + u.high) * step(sky_h, y) * (1.0 - sun);

    var c = mix(sky, ground, below);
    c += prev(in.uv) * 0.35;
    return vec4<f32>(c, 1.0);
}
