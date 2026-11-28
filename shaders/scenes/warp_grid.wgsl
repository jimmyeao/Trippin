// A tron-style ground plane rushing under the camera — bright neon grid,
// glowing horizon line, spectrum-lit line colour. The run turns 90 degrees
// left or right on every bar line, the grid swinging round (direction).

const PATH_D: f32 = 3.0;   // a bar's run: three grid lines a beat
const PATH_START: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
const TURN_W: f32 = 0.5;   // beats either side of the bar line to turn

// ---- The flight: a 90-degree turn on every bar line -----------------------
// The camera turns left or right on each bar line, rounding the corner on a
// curve over part of a beat either side, the view following the path. The
// turns follow a fixed eight-bar pattern (forward, aside, forward, aside...)
// whose side turns cancel, so the position is a closed-form sum and needs
// no state; each eight bars mirrors the pattern its own way. A bar covers
// PATH_D, so travel is tempo-locked and each bar line falls where the
// scene's crossings are.
fn fsgn(b: bool) -> f32 {
    return select(-1.0, 1.0, b);
}

// Bar n's heading: +z, or across along x.
fn head(n: u32) -> vec3<f32> {
    let k = n % 8u;
    if k % 2u == 0u {
        return vec3<f32>(0.0, 0.0, 1.0);
    }
    let v = pcg((n / 8u) * 7919u + 13u);
    let first = vec3<f32>(fsgn((v & 1u) != 0u), 0.0, 0.0);
    let second = vec3<f32>(fsgn((v & 2u) != 0u), 0.0, 0.0);
    switch k {
        case 1u: { return first; }
        case 3u: { return second; }
        case 5u: { return -first; }
        default: { return -second; }
    }
}

// Where the camera is at the start of bar n.
fn bar_pos(n: u32) -> vec3<f32> {
    var p = PATH_START + vec3<f32>(0.0, 0.0, f32(n / 8u) * 4.0 * PATH_D);
    let k = n % 8u;
    for (var j = 0u; j < k; j++) {
        p += head(n - k + j) * PATH_D;
    }
    return p;
}

// pos, the heading (fwd, level), and turn: how far into a corner (0 on the
// straight, +-1 at its middle; positive turning right), for banking.
struct Cam {
    pos: vec3<f32>,
    fwd: vec3<f32>,
    turn: f32,
};

fn flight() -> Cam {
    let n = u32(max(floor((u.beat - u.bar_phase * 4.0) / 4.0 + 0.5), 0.0)) % 1024u;
    let fb = u.bar_phase * 4.0;
    let v = PATH_D / 4.0;
    let w = TURN_W;
    let h = head(n);
    var o: Cam;
    o.pos = bar_pos(n) + h * v * fb;
    o.fwd = h;
    o.turn = 0.0;
    var a = h;
    var b = h;
    var c = vec3<f32>(0.0);
    var s = -1.0;
    if fb < w {
        a = head((n + 1023u) % 1024u);
        c = bar_pos(n);
        s = (fb + w) / (2.0 * w);
    } else if fb > 4.0 - w {
        b = head((n + 1u) % 1024u);
        c = bar_pos(n) + h * PATH_D;
        s = (fb - (4.0 - w)) / (2.0 * w);
    }
    if s >= 0.0 {
        let l = v * w;
        let p0 = c - a * l;
        let p2 = c + b * l;
        o.pos = (1.0 - s) * (1.0 - s) * p0 + 2.0 * (1.0 - s) * s * c + s * s * p2;
        o.fwd = normalize((1.0 - s) * a + s * b);
        o.turn = sin(PI * s) * fsgn(a.z * b.x - a.x * b.z > 0.0);
    }
    return o;
}

// The level right-hand side of a heading.
fn right_of(f: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(f.z, 0.0, -f.x);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);

    // Sky: gradient with a bright horizon glow.
    var col = mix(vec3<f32>(0.04, 0.01, 0.08), vec3<f32>(0.0, 0.0, 0.015),
                  clamp(-p.y * 1.5 + 0.5, 0.0, 1.0));
    col += palette(0.9 + u.hue) * exp(-abs(p.y) * 14.0) * (0.6 + u.kick * 1.2);

    if (p.y > 0.005) {
        // Perspective depth: 1/y, clamped so the horizon isn't infinite.
        let z = clamp(0.18 / p.y, 0.0, 12.0);
        // The ground point in the run's frame, turned to the heading
        // (flight()); square cells so a turn shows the same grid.
        let cam = flight();
        let right = right_of(cam.fwd);
        let wp = cam.pos.xz + (right.xz * p.x * z + cam.fwd.xz * z) * 0.75;
        let g = wp * 4.0;
        // Line thickness grows with depth so screen width stays visible.
        let th = 0.03 + z * 0.045;
        let lx = smoothstep(0.5 - th, 0.5, abs(fract(g.x) - 0.5));
        let ly = smoothstep(0.5 - th, 0.5, abs(fract(g.y) - 0.5));
        let band = spec(fract(p.x / aspect() * 0.5 + 0.5));
        let line_col = mix(palette(0.5 + u.hue), palette(0.85 + u.hue), band);
        // Bright near horizon (converge), taper at the very bottom.
        let gain = (0.6 + band * 1.4 + u.hits4.x * 0.6) * (0.5 + z * 0.12);
        col += line_col * (lx + ly) * gain;
        // Dark ground fill between lines.
        col = mix(col, vec3<f32>(0.008, 0.004, 0.03),
                  (1.0 - max(lx, ly)) * clamp(p.y * 2.0, 0.0, 0.9));
        // Haze pooling at the horizon.
        col += palette(0.7 + u.hue) * exp(-p.y * 9.0) * 0.25;
    }
    return vec4<f32>(finite(col), 1.0);
}
