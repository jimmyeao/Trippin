// Comets (accent layer): a few comets crossing the frame with long tapering
// tails. Each crosses on the smooth energy clock (motion), their heading
// swings over eight bars so they cross one way, then the other (direction),
// and their tails stretch with the slow high presence (shape). Four comets,
// always four, on staggered phases, each on a new line every crossing.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.0);
    let swing = 0.45 * sin(TAU * u.flow / 32.0);
    let span = aspect() + 0.6;
    for (var i = 0; i < 4; i++) {
        let fi = f32(i);
        let ph = u.clock4.x * 0.045 + fi / 4.0;
        let life = fract(ph);
        let h = hash22(vec2<f32>(fi * 7.0, floor(ph) * 3.0 + 1.0));
        let ang = 0.35 + swing + (h.x - 0.5) * 0.3;
        let dir = vec2<f32>(cos(ang), sin(ang));
        let n = vec2<f32>(-dir.y, dir.x);
        // The line passes through a point near the centre; the head runs
        // along it from off one edge to off the other.
        let mid = n * (h.y - 0.5) * 1.4;
        let head = mid + dir * (life * 2.0 - 1.0) * span;
        let rel = p - head;
        let back = -dot(rel, dir);
        let across = dot(rel, n);
        let tail = 0.55 + 0.5 * u.pres4.w;
        var c = 0.0;
        if back > 0.0 && back < tail {
            let k = 1.0 - back / tail;
            let w = 0.004 + 0.012 * (1.0 - k);
            c = exp(-(across * across) / (w * w)) * k * k * 0.7;
        }
        let d = length(rel);
        c += exp(-d * 90.0) * 1.4 + exp(-d * 22.0) * 0.18;
        let tone = palette(0.6 + h.x * 0.3);
        col += mix(tone * tone, vec3<f32>(1.0), 0.25) * c;
    }
    return vec4<f32>(finite(col), 1.0);
}
