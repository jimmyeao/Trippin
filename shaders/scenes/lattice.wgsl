// @heavy — infinite cube lattice flythrough: the camera rides the corridors
// of a repeating lattice of glowing cube frames, turning 90 degrees on every
// bar line (left, right, up or down) into the crossing corridor (direction).
// Bass feeds the frame glow, onsets flash whole cells at once.

const PATH_D: f32 = 11.2;  // a bar's travel: seven cells
const PATH_START: vec3<f32> = vec3<f32>(0.8, 0.8, 0.8); // a corridor crossing
const TURN_W: f32 = 0.25;  // beats either side of the bar line to round a corner
const HEAD_A: vec3<f32> = vec3<f32>(1.0, 0.0, 0.0);
const HEAD_B: vec3<f32> = vec3<f32>(0.0, 1.0, 0.0);

// The camera's up for a heading: +y, except straight up or down the
// lattice, where it is the way it came from (as a pitch would leave it).
fn up_of(h: vec3<f32>) -> vec3<f32> {
    if abs(h.y) > 0.5 {
        return vec3<f32>(0.0, 0.0, -h.y);
    }
    return vec3<f32>(0.0, 1.0, 0.0);
}

// ---- The flight: a 90-degree turn on every bar line -----------------------
// The camera runs the lattice's corridors and turns into a crossing one on
// each bar line (eased round the corner over the last and first part of a
// beat, the view swinging with the path). A bar covers a whole number of
// cells, so every bar line falls on a crossing. The turns follow a fixed
// eight-bar pattern (forward, aside, forward, aside...) whose side turns
// cancel, so the position is a closed-form sum and needs no state; each
// eight bars mirrors the pattern its own way so it doesn't repeat.
fn fsgn(b: bool) -> f32 {
    return select(-1.0, 1.0, b);
}

// Bar n's heading.
fn head(n: u32) -> vec3<f32> {
    let k = n % 8u;
    if k % 2u == 0u {
        return vec3<f32>(0.0, 0.0, 1.0);
    }
    let v = pcg((n / 8u) * 7919u + 13u);
    let a = HEAD_A * fsgn((v & 1u) != 0u);
    let b = HEAD_B * fsgn((v & 2u) != 0u);
    let swap = (v & 4u) != 0u;
    let first = select(a, b, swap);
    let second = select(b, a, swap);
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

struct Cam {
    pos: vec3<f32>,
    fwd: vec3<f32>,
    up: vec3<f32>,
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
    o.up = up_of(h);
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
        // Round the corner on a quadratic curve; the view follows it.
        let l = v * w;
        let p0 = c - a * l;
        let p2 = c + b * l;
        o.pos = (1.0 - s) * (1.0 - s) * p0 + 2.0 * (1.0 - s) * s * c + s * s * p2;
        o.fwd = normalize((1.0 - s) * a + s * b);
        o.up = mix(up_of(a), up_of(b), s);
    }
    o.up = normalize(o.up - o.fwd * dot(o.up, o.fwd));
    return o;
}


// Returns (distance, cell hash, edge proximity 0..1).
fn map3(p: vec3<f32>) -> vec3<f32> {
    // Fixed cell size — putting bass in the world scale made the geometry
    // visibly lurch. Bass lives in the glow instead.
    let s = 1.6;
    let id = floor(p / s + 0.5);
    let h = hash21(id.xz + id.y * 3.0);
    let c = p - id * s;
    // Cube frame as 12 edge struts: distance to the nearest edge line of a
    // cube of half-size k (the old box-minus-box left the faces solid, so
    // cells read as flat colour tiles).
    // IQ's box-frame SDF: a cube of half-size k made of struts of radius r.
    let k = 0.42;
    // Shape: struts thicken with bass presence.
    let r = 0.03 + 0.035 * u.pres4.x;
    let pp = abs(c) - vec3<f32>(k);
    let qq = abs(pp + vec3<f32>(r)) - vec3<f32>(r);
    let f1 = length(max(vec3<f32>(pp.x, qq.y, qq.z), vec3<f32>(0.0))) + min(max(pp.x, max(qq.y, qq.z)), 0.0);
    let f2 = length(max(vec3<f32>(qq.x, pp.y, qq.z), vec3<f32>(0.0))) + min(max(qq.x, max(pp.y, qq.z)), 0.0);
    let f3 = length(max(vec3<f32>(qq.x, qq.y, pp.z), vec3<f32>(0.0))) + min(max(qq.x, max(qq.y, pp.z)), 0.0);
    let frame = min(min(f1, f2), f3);
    return vec3<f32>(frame, h, 0.0);
}

fn map(p: vec3<f32>) -> vec2<f32> {
    return map3(p).xy;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Fly the gaps between cube columns, turning on the bar (flight()).
    let cam = flight();
    let right = cross(cam.up, cam.fwd);
    let z = u.flow;
    let ro = cam.pos + (right * sin(z * 0.37) + cam.up * sin(z * 0.29)) * 0.1;
    // A gentle roll that swings one way, then the other, on the phrase.
    let rl = rot(0.12 * sin(TAU * u.flow / 32.0)) * vec2<f32>(p.x * 0.8, -p.y * 0.7);
    var rd = normalize(right * rl.x + cam.up * rl.y + cam.fwd);

    var t = 0.0;
    var hit = 0.0;
    var pos = ro;
    var glow = vec3<f32>(0.0);
    var found = false;
    for (var i = 0; i < 90; i++) {
        pos = ro + rd * t;
        let m = map(pos);
        // Neon: struts glow through the haze as the ray passes near them.
        glow += palette(m.y * 1.3 + u.hue) * exp(-m.x * 30.0) * 0.012;
        if m.x < 0.002 * (1.0 + t) { hit = m.y; found = true; break; }
        t += m.x * 0.8;
        if t > 30.0 { break; }
    }

    var col = vec3<f32>(0.004, 0.005, 0.014);
    let drive = 0.5 + 0.8 * u.intensity;
    if found {
        let e = 0.003;
        let kk = vec2<f32>(1.0, -1.0);
        let n = normalize(kk.xyy * map(pos + kk.xyy * e).x + kk.yyx * map(pos + kk.yyx * e).x + kk.yxy * map(pos + kk.yxy * e).x + kk.xxx * map(pos + kk.xxx * e).x);
        let fade = exp(-t * 0.13);
        // Onsets flash whole cells; per-cell band level lights the struts.
        let flash = step(0.8, fract(hit + u.beat * 0.125)) * u.onset * 2.0;
        let lvl = 0.4 + 1.4 * spec(hit) + flash;
        let fres = pow(1.0 - max(dot(n, -rd), 0.0), 2.0);
        let c = palette(hit * 1.3 + u.hue);
        col = mix(col, c * (0.25 + 0.75 * fres) * lvl * drive + vec3<f32>(1.0) * pow(fres, 6.0) * 0.3, fade);
    }
    col += glow * drive;
    // Distance fog toward the vanishing point.
    col += palette(0.7 + u.hue) * (1.0 - exp(-t * 0.06)) * 0.05;
    return vec4<f32>(col, 1.0);
}
