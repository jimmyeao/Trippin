// Honeycomb of hexagonal cells. Each cell charges with the mids and dumps
// on the kick — a honeycomb strobe wall. Uses iq's hexagon() tiling —
// the two-lattice nearest-centre method left a visible vertical glitch
// column at x = 0.

// iq's hexagonal tiling: returns (local coords, cell id).
// local is measured in a hex of side 1; id is integer lattice coords.
fn hex_tile(p: vec2<f32>) -> vec4<f32> {
    let q2 = vec2<f32>(p.x * 1.1547005, p.y + p.x * 0.5773503);
    let pi = floor(q2);
    let pf = fract(q2);
    // i32 % can be negative — normalise into 0..3.
    let vv = f32((i32(pi.x + pi.y) % 3 + 3) % 3);
    let ca = step(1.0, vv);
    let cb = step(2.0, vv);
    let ma = step(pf.xy, pf.yx);
    // barycentric coordinate inside the hex
    let e = dot(ma, vec2<f32>(1.0) - pf.yx)
          + ca * (pf.x + pf.y - 1.0)
          + cb * (pf.yx - 2.0 * pf.xy);
    return vec4<f32>(e, pi); // e = distance-to-edge measure, pi = cell id
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let s = 4.0;
    let tile = hex_tile(p * s + vec2<f32>(0.0, u.flow * 0.25));
    let edge_d = tile.x;
    let id = tile.zw;

    let h = hash21(id);
    // Per-cell charge: hashed phase, flashed by onsets rolling through.
    let phase = fract(h + u.beat * 0.125 + h * 0.3);
    let charge = u.mid * (0.4 + 0.6 * h) + u.onset * smoothstep(0.5, 0.0, phase);

    // e<0.5 is the interior; edges light the honeycomb lines.
    let fill = smoothstep(0.48, 0.25, edge_d) * charge;
    let edge = smoothstep(0.04, 0.015, abs(edge_d - 0.5));

    let cell_col = palette(h * 0.7 + u.hue);
    var col = vec3<f32>(0.008, 0.008, 0.02);
    col += cell_col * fill * 1.6;
    col += cell_col * edge * (0.15 + u.energy * 0.5);
    col += prev(in.uv) * 0.2;
    return vec4<f32>(col, 1.0);
}
