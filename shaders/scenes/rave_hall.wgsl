// @heavy — raymarched. Infinite hall of light pillars: the camera flies the
// aisles, turning 90 degrees left or right into a crossing aisle on every
// bar line (direction), while each column glows with its own slice of the
// spectrum, like an LED cathedral. Floors and ceiling are dark mirrors of
// the glow.

const CELL: f32 = 2.4;    // pillar grid spacing
const RADIUS: f32 = 0.26; // pillar radius
const FLOOR_Y: f32 = -1.5;
const CEIL_Y: f32 = 1.9;

const PATH_D: f32 = 12.0;  // a bar's travel: five cells
const PATH_START: vec3<f32> = vec3<f32>(0.0, 0.15, 0.0); // an aisle crossing
const TURN_W: f32 = 0.5;   // beats either side of the bar line to round a corner
const HEAD_A: vec3<f32> = vec3<f32>(1.0, 0.0, 0.0);
const HEAD_B: vec3<f32> = vec3<f32>(1.0, 0.0, 0.0);

fn up_of(h: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(0.0, 1.0, 0.0);
}

// ---- The flight: a 90-degree turn on every bar line -----------------------
// The camera runs the hall's aisles and turns into a crossing aisle on
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

struct Hit {
    d: f32,
    id: vec2<f32>,
    local: vec2<f32>,  // pillar-local xz
    kind: f32,         // 0 pillar, 1 floor, 2 ceiling
};

fn map(p: vec3<f32>) -> Hit {
    let id = floor(p.xz / CELL);
    let local = (fract(p.xz / CELL) - 0.5) * CELL;
    let pil = length(local) - RADIUS;
    var h: Hit;
    h.id = id;
    h.local = local;
    h.d = pil;
    h.kind = 0.0;
    let fl = p.y - FLOOR_Y;
    if fl < h.d {
        h.d = fl;
        h.kind = 1.0;
    }
    let cl = CEIL_Y - p.y;
    if cl < h.d {
        h.d = cl;
        h.kind = 2.0;
    }
    return h;
}

// Column light colour: a steady hue per pillar, brightness from the spectrum.
fn pillar_light(id: vec2<f32>) -> vec3<f32> {
    let band = hash21(id * 7.31 + 3.7);
    return palette(band) * (0.25 + 1.6 * spec(band));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Camera: down the aisles between the pillars (cell boundaries; the
    // pillars sit on cell centres), turning on the bar (flight()). Sway is
    // kept well under half a cell so the view never meets a column head
    // on; the kick nod stays subtle.
    let cam = flight();
    let right = cross(cam.up, cam.fwd);
    let ro = cam.pos + right * (sin(u.flow * 0.3) * 0.25 + sin(u.time * 0.7) * 0.08 * u.hits4.x) + vec3<f32>(0.0, sin(u.time * 0.4) * 0.1, 0.0);
    // centred() has +y pointing down the screen — negate so up is up. A
    // roll swings one way, then the other, on the phrase.
    let rl = rot(0.15 * sin(TAU * u.flow / 32.0)) * vec2<f32>(p.x * 0.75, -p.y * 0.75 - 0.05 + 0.04 * beat_pulse(6.0));
    var rd = normalize(right * rl.x + cam.up * rl.y + cam.fwd);

    var t = 0.0;
    var glow = vec3<f32>(0.0);
    var hit = false;
    var h: Hit;
    for (var i = 0; i < 72; i++) {
        h = map(ro + rd * t);
        // Volumetric sheen: near-misses pick up the pillar's colour.
        if h.kind == 0.0 {
            glow += pillar_light(h.id) * exp(-max(h.d, 0.0) * 22.0) * 0.012;
        }
        if h.d < 0.001 * (1.0 + t * 0.5) {
            hit = true;
            break;
        }
        // Caps don't need tiny steps; clamp keeps the march brisk.
        t += clamp(h.d * 0.9, 0.02, 0.6);
        if t > 46.0 {
            break;
        }
    }

    let pulse = 0.8 + 0.5 * u.intensity;
    var col = glow * pulse;

    if hit {
        let pos = ro + rd * t;
        if h.kind == 0.0 {
            // Pillar: dark body, vertical light ribs pulsing down the shaft.
            let rib = pow(0.5 + 0.5 * sin(pos.y * 9.0 - u.clock4.z * 8.0 + hash21(h.id) * TAU), 6.0);
            let lit = pillar_light(h.id);
            col += vec3<f32>(0.02, 0.02, 0.035) + lit * rib * 0.55;
        } else {
            // Floor / ceiling: near-black with a smear of the column glow.
            col += vec3<f32>(0.015, 0.015, 0.028) + glow * select(0.35, 0.2, h.kind == 2.0);
        }
    }

    // Distance haze pulls everything toward the hall's vanishing light.
    let fog = 1.0 - exp(-t * 0.045);
    col = mix(col, palette(0.6) * pulse * 0.35, fog);
    col += prev(uncentred(centred(in.uv) * 0.985)) * 0.18;
    return vec4<f32>(col, 1.0);
}
