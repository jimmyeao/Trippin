// Notes (accent layer): glowing musical notes, single quavers and beamed
// pairs, floating up through the frame and fading. They rise on the smooth
// energy clock (motion), rock on a four-bar sine (direction) and swell with
// the slow mid presence (shape). Twelve notes in flight, always twelve, each
// born in a new lane when its last one has faded out.

fn seg(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// A note head: a tilted ellipse (approximate distance).
fn head(p: vec2<f32>, c: vec2<f32>) -> f32 {
    let q = rot(-0.4) * (p - c);
    let r = vec2<f32>(0.27, 0.19);
    return (length(q / r) - 1.0) * 0.19;
}

// Note-local units: the head sits at the origin, +y down, the stem goes up
// to y = -1.
fn quaver(p: vec2<f32>) -> f32 {
    var d = head(p, vec2<f32>(0.0));
    d = min(d, seg(p, vec2<f32>(0.24, -0.05), vec2<f32>(0.24, -1.0)) - 0.035);
    // The flag: two segments curling down off the top of the stem.
    d = min(d, seg(p, vec2<f32>(0.24, -1.0), vec2<f32>(0.5, -0.72)) - 0.04);
    d = min(d, seg(p, vec2<f32>(0.5, -0.72), vec2<f32>(0.48, -0.45)) - 0.03);
    return d;
}

fn beamed(p: vec2<f32>) -> f32 {
    var d = min(head(p, vec2<f32>(0.0)), head(p, vec2<f32>(0.75, 0.08)));
    d = min(d, seg(p, vec2<f32>(0.24, -0.05), vec2<f32>(0.24, -1.0)) - 0.035);
    d = min(d, seg(p, vec2<f32>(0.99, 0.03), vec2<f32>(0.99, -0.92)) - 0.035);
    d = min(d, seg(p, vec2<f32>(0.24, -1.0), vec2<f32>(0.99, -0.92)) - 0.075);
    return d;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.0);
    for (var i = 0; i < 12; i++) {
        let fi = f32(i);
        let hs = hash22(vec2<f32>(fi, 41.0));
        let ph = u.clock4.y * 0.03 * (0.75 + 0.5 * hs.x) + fi / 12.0;
        let life = fract(ph);
        let h = hash22(vec2<f32>(fi * 3.0 + 1.0, floor(ph) * 5.0));
        let s = (0.085 + 0.04 * h.y) * (1.0 + 0.2 * u.pres4.y);
        var pos = vec2<f32>((h.x - 0.5) * 2.0 * aspect() * 0.85, 1.15 - life * 2.3);
        pos.x += 0.07 * sin(life * TAU * 1.5 + h.y * 9.0);
        // The note's centre is a little above its head.
        if length(p - pos - vec2<f32>(0.0, -0.5 * s)) > s * 2.2 {
            continue;
        }
        let tilt = 0.28 * sin(TAU * u.flow / 16.0 + h.y * 6.0);
        let q = rot(tilt) * (p - pos) / s;
        var d = quaver(q);
        if h.y > 0.55 {
            d = beamed(q + vec2<f32>(0.35, 0.0));
        }
        d *= s;
        let fill = smoothstep(0.003, 0.0, d);
        let glow = exp(-max(d, 0.0) * 55.0);
        let fade = smoothstep(0.0, 0.15, life) * smoothstep(1.0, 0.7, life);
        let tone = palette(0.55 + h.x * 0.35);
        col += tone * tone * (fill * 1.1 + glow * 0.35) * fade;
    }
    return vec4<f32>(finite(col), 1.0);
}
