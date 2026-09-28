// Rain on glass: streaks running down-screen (+y), each column's drop
// speed and length hashed. The drops carry the scene's hue; a soft city
// bokeh glows behind the pane.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);

    // Background: blurry lights (city through a wet window).
    var bg = vec3<f32>(0.01, 0.012, 0.03);
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let bp = vec2<f32>(hash21(vec2<f32>(fi, 1.0)) - 0.5, hash21(vec2<f32>(fi, 9.0)) - 0.5);
        let pos = bp * vec2<f32>(aspect(), 1.0) * 1.4;
        let d = length(p - pos);
        bg += palette(fi * 0.16 + u.hue) * exp(-d * d * (14.0 - u.bass * 6.0)) * 0.35;
    }

    var col = bg;
    let cols = 60.0;
    let cx = floor(p.x * cols);
    let hx = hash21(vec2<f32>(cx, 3.0));
    let speed = 0.4 + hx * 0.9 + u.energy * 0.3;
    let y = p.y + u.flow * speed; // screen-down scroll
    let dy = fract(y * 3.0 + hx * 7.0);
    let dx = fract(p.x * cols) - 0.5;
    let streak = exp(-dx * dx * 120.0) * smoothstep(0.0, 0.15, dy) * smoothstep(1.0, 0.85, dy);
    let head = exp(-dx * dx * 160.0) * exp(-(dy - 0.9) * (dy - 0.9) * 40.0);

    col += palette(hx + u.hue) * streak * 0.25;
    col += vec3<f32>(0.7, 0.8, 1.0) * head * (0.5 + u.high * 1.5);
    col += prev(in.uv) * 0.2;
    return vec4<f32>(col, 1.0);
}
