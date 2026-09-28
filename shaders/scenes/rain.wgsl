// Rain on glass: streaks running DOWN-screen (+p.y = down, so the scroll
// must subtract the flow — adding it made the rain climb). Drop length and
// brightness ride the band under each column; storm intensity swells with
// energy.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);

    // Background: blurry lights (city through a wet window).
    var col = vec3<f32>(0.01, 0.012, 0.03);
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let bp = vec2<f32>(hash21(vec2<f32>(fi, 1.0)) - 0.5, hash21(vec2<f32>(fi, 9.0)) - 0.5);
        let pos = bp * vec2<f32>(aspect(), 1.0) * 1.4;
        let d = length(p - pos);
        col += palette(fi * 0.16 + u.hue) * exp(-d * d * (14.0 - u.bass * 6.0)) * 0.35;
    }

    let cols = 60.0;
    let cx = floor(p.x * cols);
    let hx = hash21(vec2<f32>(cx, 3.0));
    let band = spec((cx / cols) * 0.5 + 0.1);
    // Steady fall rate — audio changes brightness and length, not speed.
    let speed = 0.35 + hx * 0.7;
    let y = p.y - u.flow * speed;
    let dy = fract(y * 3.0 + hx * 7.0);
    let dx = fract(p.x * cols) - 0.5;
    // Loud bands leave longer streaks and brighter heads.
    let len = 0.85 - band * 0.35 - u.energy * 0.15;
    let streak = exp(-dx * dx * 120.0) * smoothstep(0.0, 0.15, dy) * smoothstep(1.0, len, dy);
    let head = exp(-dx * dx * 160.0) * exp(-(dy - 0.97) * (dy - 0.97) * 45.0);

    col += palette(hx + u.hue) * streak * (0.12 + band * 0.5 + u.energy * 0.3);
    col += vec3<f32>(0.7, 0.8, 1.0) * head * (0.3 + band * 0.8 + u.high * 1.5);
    // Sheet-lightning wash deep in the bokeh on drops.
    col += palette(0.6 + u.hue) * u.onset * 0.15;
    col += prev(in.uv) * 0.2;
    return vec4<f32>(finite(col), 1.0);
}
