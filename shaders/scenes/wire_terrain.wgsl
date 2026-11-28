// @heavy — heightfield march. Neon wireframe terrain flyover: the Tron /
// synthwave grid-mountain look. Ridged fbm peaks glow against a dusk sky,
// the bass lifts the ground, and the flight banks into a 90-degree turn
// left or right on every bar line (direction), swooping with the phrase.

const PATH_D: f32 = 10.0;  // a bar's flight
const PATH_START: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);
const TURN_W: f32 = 0.6;   // beats either side of the bar line to turn

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

fn ground(p: vec3<f32>, lift: f32) -> f32 {
    // Ridged noise reads as mountain crests.
    var h = fbm(p.xz * 0.16) - 0.45;
    h = 1.0 - abs(h) * 2.2;
    return p.y - h * (1.4 + lift * 1.2) + 1.1;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Shape: the ground rises with bass presence (smooth) and a kick.
    let lift = u.pres4.x + 0.4 * u.hits4.x;

    // The flight turns on each bar (flight()), banking into the corner;
    // altitude swoops with the phrase and the kick dips low.
    let cam = flight();
    let right = right_of(cam.fwd);
    let ro = cam.pos + vec3<f32>(0.0, 1.3 + sin(u.time * 0.3) * 0.25 - u.hits4.x * 0.25, 0.0);
    let bank = 0.35 * cam.turn + 0.08 * sin(TAU * u.flow / 32.0);
    let rl = rot(-bank) * vec2<f32>(p.x * 0.8, -p.y * 0.8 - 0.22);
    var rd = normalize(right * rl.x + vec3<f32>(0.0, rl.y, 0.0) + cam.fwd);

    var t = 0.0;
    var hit = false;
    var pos = ro;
    for (var i = 0; i < 64; i++) {
        pos = ro + rd * t;
        let d = ground(pos, lift) * 0.7;
        if d < 0.01 * (1.0 + t * 0.4) {
            hit = true;
            break;
        }
        t += clamp(d, 0.03, 1.2);
        if t > 60.0 {
            break;
        }
    }

    // Sky: dusk gradient with a low glow band at the horizon.
    let sky_t = clamp(-rd.y * 2.2 + 0.4, 0.0, 1.0);
    var col = mix(palette(0.05 + u.hue) * 0.55, vec3<f32>(0.01, 0.01, 0.03), sky_t);
    col += palette(0.1 + u.hue) * exp(-abs(rd.y + 0.1) * 14.0) * (0.3 + u.intensity * 0.5);

    if hit {
        // Wireframe: emissive grid lines over near-black ground.
        let gp = fract(pos.xz * 0.8);
        let lx = smoothstep(0.06, 0.01, min(gp.x, 1.0 - gp.x));
        let lz = smoothstep(0.06, 0.01, min(gp.y, 1.0 - gp.y));
        let line = max(lx, lz);
        let fade = exp(-t * 0.10);
        let ridge = clamp(1.0 - pos.y * 0.4, 0.0, 1.0);
        let lcol = palette(fract((pos.x + pos.z) * 0.02 + u.hue));
        col = mix(col, lcol * (line * (0.7 + u.energy * 1.4) + 0.04) * ridge, fade);
    }

    col += prev(uncentred(centred(in.uv) * 0.995)) * 0.15;
    return vec4<f32>(col, 1.0);
}
