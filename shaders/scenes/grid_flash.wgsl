// The floor-is-lava grid: square cells that flash on onsets and throb to
// the spectrum, like a giant LED dancefloor seen flat on. Each cell has a
// hashed flash threshold so the floor ripples rather than strobing whole.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Direction: the floor tilts one way then the other; shape: the grid
    // coarsens/refines with mid presence.
    let p = rot(0.3 * sin(u.clock4.x * 0.025)) * centred(in.uv);
    let n = 10.0 + 8.0 * u.pres4.y;
    let cell = floor(p * n);
    let local = fract(p * n) - 0.5;
    let h = hash21(cell + floor(u.seed * 16.0));

    // Cells flash when the rolling energy passes their threshold.
    let e = u.energy * (0.6 + 0.4 * sin(u.clock4.w * 0.3 + h * TAU));
    let lit = step(1.0 - h, 0.5 + e * 0.9 + u.onset * 0.5);
    let border = smoothstep(0.5, 0.42, max(abs(local.x), abs(local.y)));
    let inner = smoothstep(0.30, 0.26, max(abs(local.x), abs(local.y)));

    var col = vec3<f32>(0.01, 0.01, 0.025);
    let cc = palette(h * 1.7 + u.hue);
    col += cc * lit * border * (0.3 + e * 1.8);
    col += cc * lit * inner * u.kick * 1.2;
    // Grid lines always faintly visible.
    let gline = smoothstep(0.03, 0.01, 0.5 - max(abs(local.x), abs(local.y)));
    col += palette(0.5 + u.hue) * gline * 0.08;
    return vec4<f32>(col, 1.0);
}
