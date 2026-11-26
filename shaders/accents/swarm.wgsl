// Swarm (accent layer): a murmuration. A flock of motes in a soft, shifting
// cloud whose centre orbits over eight bars and which slowly turns
// (direction); the cloud tightens as the slow bass presence swells (shape),
// and each mote wanders on smooth sines of the energy clock (motion). The
// motes live in the flock's own frame, so they travel with it. (A grid of
// motes over the whole frame read as blue noise, not a flock.)

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let c = vec2<f32>(0.6 * sin(TAU * u.flow / 32.0), 0.28 * sin(TAU * u.flow / 24.0 + 1.0));
    let rel = rot(TAU * u.flow / 64.0) * (p - c);
    // The cloud's outline: an ellipse roughed up by slow noise.
    let t = u.clock4.x * 0.35;
    let rough = noise(rel * 2.2 + vec2<f32>(t * 0.15, -t * 0.1)) - 0.5;
    let radius = 0.62 - 0.22 * u.pres4.x;
    let r = length(rel * vec2<f32>(0.62, 1.0)) + rough * 0.28;
    let env = smoothstep(radius, radius * 0.35, r);
    if env <= 0.001 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let scale = 16.0;
    let q = rel * scale;
    let base = floor(q);
    var col = vec3<f32>(0.0);
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let cell = base + vec2<f32>(f32(i), f32(j));
            let h = hash22(cell + vec2<f32>(19.0, 3.0));
            let w = 0.6 + 0.8 * h;
            let m = cell + 0.5 + 0.38 * vec2<f32>(sin(t * w.x + h.y * TAU), cos(t * w.y + h.x * TAU));
            let d = length(q - m) / scale;
            let core = smoothstep(0.0075, 0.002, d);
            let glow = exp(-d * 90.0) * 0.3;
            let tone = palette(0.5 + h.y * 0.25 + h.x * 0.1);
            col += tone * tone * (core * 1.4 + glow);
        }
    }
    return vec4<f32>(finite(col * env), 1.0);
}
