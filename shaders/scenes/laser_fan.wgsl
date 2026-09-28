// A fan of laser beams sweeping from a point at the bottom of the frame —
// the festival laser look. Beams swing with the phrase and split with
// energy; the haze follows the mids.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Origin below the frame bottom; +p.y points down-screen.
    let origin = vec2<f32>(0.0, 1.25);
    let d = p - origin;
    let a = atan2(d.y, d.x);
    let r = length(d);

    let beams = 5.0 + floor(u.energy * 5.0);
    let swing = sin(u.flow * 0.4) * 0.35;
    // Beams occupy a fan centred on straight-up (angle ~ -PI/2 in this
    // y-down space; up-screen = -y → negative angle on atan2(d.y,d.x)).
    let fan = fract((a + swing + PI * 0.5) * beams / PI * 0.5) - 0.5;
    let beam = exp(-abs(fan) * (24.0 - u.high * 8.0)) * smoothstep(0.12, 0.0, abs(a + PI * 0.5));

    var col = vec3<f32>(0.008, 0.01, 0.02);
    let lc = palette(a * 0.8 + u.hue);
    col += lc * beam * (0.6 + u.energy * 1.8);
    // Haze pool at the origin.
    col += palette(0.5 + u.hue) * exp(-r * 2.0) * (0.2 + u.mid * 0.5);
    // Haze catch on the beams near the origin.
    col += lc * exp(-r * 3.0) * beam * 1.2;
    col += prev(in.uv) * 0.3;
    return vec4<f32>(col, 1.0);
}
