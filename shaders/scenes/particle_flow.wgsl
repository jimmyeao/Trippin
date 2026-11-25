// @particles 16000 @bloom 0.4
// @title Combed Flow
// Combed Flow: 16,000 short streaks of light combed along flowing
// streamlines, like light drawn through hair. The streamlines' waves deepen
// with the mids and bass (shape), the streaks travel along them on the
// smooth energy clock (motion), and the waves' phase swings back and forth
// over eight bars (direction). After VS's Combed Flow particle system.

// The stream's height at x for a lane at y0: three waves whose depth follows
// the music.
fn stream_y(x: f32, y0: f32) -> f32 {
    let swing = sin(TAU * u.flow / 32.0);
    let a1 = 0.12 + 0.14 * (u.pres4.x * 0.7 + u.lvl4.x * 0.3);
    let a2 = 0.06 + 0.08 * (u.pres4.y * 0.7 + u.lvl4.y * 0.3);
    let a3 = 0.03;
    return y0
        + a1 * sin(x * 1.3 + y0 * 2.0 + swing * 1.5)
        + a2 * sin(x * 3.1 - y0 * 3.0 - swing * 2.2 + 1.7)
        + a3 * sin(x * 7.0 + y0 * 9.0 + u.flow * 0.2);
}

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let cell = vec2<f32>(f32(ii % 1024u), f32(ii / 1024u));
    let h = hash22(cell + vec2<f32>(2.3, 5.9));
    let h2 = hash22(cell + vec2<f32>(9.1, 3.3));
    let asp = aspect();
    let span = 2.0 * asp + 0.6;
    let y0 = (h.x - 0.5) * 2.3;
    // Travel along the lane; lanes run at slightly different speeds.
    let speed = 0.2 + 0.15 * h2.x;
    let x = (fract(h.y + u.clock4.x * speed / span) - 0.5) * span;
    let y = stream_y(x, y0);
    let e = 0.01;
    let tang = normalize(vec2<f32>(2.0 * e, stream_y(x + e, y0) - stream_y(x - e, y0)));
    let nrm = vec2<f32>(-tang.y, tang.x);
    let corners = array<vec2<f32>, 3>(vec2<f32>(-1.732, -1.0), vec2<f32>(1.732, -1.0), vec2<f32>(0.0, 2.0));
    let q = corners[vi % 3u];
    // A streak: long along the stream, thin across it.
    let len = 0.028 + 0.02 * h2.y;
    let wid = max(0.0022, 1.5 / u.res_y);
    let p = vec2<f32>(x, y) + tang * q.x * len + nrm * q.y * wid;
    o.pos = vec4<f32>(p.x / asp, -p.y, 0.0, 1.0);
    o.q = q;
    // Fade in from the left edge and out at the right, so wrapping is unseen.
    let edge = smoothstep(-0.5 * span, -0.5 * span + 0.3, x) * (1.0 - smoothstep(0.5 * span - 0.3, 0.5 * span, x));
    let c = palette(h.x * 0.6 + 0.1);
    o.col = c * c * 0.3 * edge;
    return o;
}

@fragment
fn fs_particles(in: POut) -> @location(0) vec4<f32> {
    let r2 = dot(in.q, in.q);
    if r2 > 1.0 {
        discard;
    }
    // Bright along the streak's axis, soft at its ends.
    let a = (1.0 - in.q.x * in.q.x) * exp(-in.q.y * in.q.y * 3.0);
    return vec4<f32>(in.col * a, 0.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Silky trails: the streaks smear into continuous strands.
    var col = prev(in.uv) * 0.62;
    col += vec3<f32>(0.002, 0.002, 0.006);
    return vec4<f32>(finite(col), 1.0);
}
