// Honeycomb of hexagonal cells. Each cell charges with the mids and dumps
// on the kick — a honeycomb strobe wall.

fn hex_dist(p: vec2<f32>) -> f32 {
    let q = abs(p);
    return max(dot(q, normalize(vec2<f32>(1.0, 1.7320508))), q.x);
}

fn hex_cell(p: vec2<f32>) -> vec4<f32> {
    // Returns cell centre (xy) and local offset (zw). Radius = 0.5.
    let r = vec2<f32>(1.0, 1.7320508);
    let h = r * 0.5;
    let a = (p - floor(p / r) * r) - h;
    let b = (p - floor((p - h) / r) * r - h) - h;
    let g = select(b, a, dot(a, a) < dot(b, b));
    return vec4<f32>(p - g, g);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let s = 6.0;
    let hc = hex_cell(p * s + vec2<f32>(0.0, u.flow * 0.35));
    let id = hc.xy;
    let local = hc.zw;

    // Per-cell energy: hashed phase + global mids, flashed by onsets.
    let h = hash21(id);
    let phase = fract(h + u.beat * 0.125 + h * 0.3);
    let charge = u.mid * (0.4 + 0.6 * h) + u.onset * smoothstep(0.5, 0.0, phase);
    let d = hex_dist(local);
    let fill = smoothstep(0.45, 0.10, d) * charge;
    let edge = smoothstep(0.05, 0.02, abs(d - 0.42));

    let cell_col = palette(h * 0.7 + u.hue);
    var col = vec3<f32>(0.008, 0.008, 0.02);
    col += cell_col * fill * 1.6;
    col += cell_col * edge * (0.15 + u.energy * 0.5);
    col += prev(in.uv) * 0.2;
    return vec4<f32>(col, 1.0);
}
