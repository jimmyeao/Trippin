// @heavy — raymarched neon city canyon: streets through an endless block
// grid, the camera turning left or right at a crossroads on every bar line
// (direction) with the sun swinging round as it goes. Each tower's windows
// burn with its own slice of the spectrum; the kick drops the camera a
// touch.

const CELL: f32 = 3.2;      // block grid pitch
const STREETS: f32 = 4.0;   // a street every four cells, both ways
const PATH_D: f32 = 12.8;   // a bar's drive: one block, crossroads to crossroads
const PATH_START: vec3<f32> = vec3<f32>(1.6, 0.0, 1.6); // a crossroads
const TURN_W: f32 = 0.5;    // beats either side of the bar line to take a corner

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

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// Height and glow of the tower in cell `id`; street cells stay clear.
// Shape: each tower rises and sinks with its own spectrum band (a street
// of equaliser bars).
fn tower_h(id: vec2<f32>) -> f32 {
    let band = hash21(id + 31.0);
    return 0.5 + 3.0 * hash21(id * 3.17 + 11.0) * (0.55 + 0.7 * spec(band));
}

struct Hit {
    d: f32,
    id: vec2<f32>,
    local: vec3<f32>,
    half: vec3<f32>,
    ground: bool,
};

fn map(p: vec3<f32>) -> Hit {
    var h: Hit;
    // The street plane is intersected analytically in fs_main — marching a
    // flat plane at grazing angles crawls and gave up long before the towers.
    h.ground = true;
    h.d = 1e3;
    // Every fourth row and column of cells is a street (centre lines at
    // 1.6 + 12.8k each way); towers fill the rest.
    let id = floor(p.xz / CELL);
    let m = id - STREETS * floor(id / STREETS);
    if m.x > 0.5 && m.y > 0.5 {
        let th = tower_h(id);
        let c = (id + 0.5) * CELL;
        let half = vec3<f32>(0.95, th, 0.95);
        let local = p - vec3<f32>(c.x, th, c.y);
        let d = sd_box(local, half);
        if d < h.d {
            h.d = d;
            h.id = id;
            h.local = local;
            h.half = half;
            h.ground = false;
        }
    }
    return h;
}

// Window emission for a tower face: a lit grid whose fill follows a spectrum
// band picked per block.
fn windows(pos: vec3<f32>, h: Hit) -> vec3<f32> {
    let face_x = abs(h.local.x) > abs(h.local.z);
    let along = select(h.local.z, h.local.x, face_x);
    let win = vec2<f32>(fract(along * 2.4), fract(pos.y * 1.5));
    let lit_cell = hash21(h.id * 9.7 + floor(vec2<f32>(along * 2.4, pos.y * 1.5)));
    let on = step(0.55, lit_cell);
    let pane = smoothstep(0.9, 0.6, win.x) * smoothstep(0.85, 0.55, win.y);
    let band = hash21(h.id + 31.0);
    return palette(band) * spec(band) * on * pane * 2.2;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Street-centre camera, turning at a crossroads each bar (flight());
    // dips on the kick, weaves lazily across the lane.
    let cam = flight();
    let right = right_of(cam.fwd);
    let ro = cam.pos + right * sin(u.flow * 0.2) * 0.4 + vec3<f32>(0.0, 1.05 - 0.10 * beat_pulse(8.0), 0.0);
    // centred() has +y pointing down the screen — negate so up is up.
    let rl = rot(sin(u.flow * 0.2) * 0.06 + 0.1 * cam.turn) * vec2<f32>(p.x * 0.8, -p.y * 0.7 + 0.12);
    var rd = normalize(right * rl.x + vec3<f32>(0.0, rl.y, 0.0) + cam.fwd);

    let t_ground = select(1e5, (-0.02 - ro.y) / rd.y, rd.y < 0.0);
    let t_max = min(t_ground, 90.0);
    var t = 0.0;
    var glow = vec3<f32>(0.0);
    var hit = false;
    var h: Hit;
    for (var i = 0; i < 90; i++) {
        h = map(ro + rd * t);
        if !h.ground {
            glow += palette(hash21(h.id + 31.0)) * exp(-max(h.d, 0.0) * 14.0) * 0.008;
        }
        if h.d < 0.0015 * (1.0 + t) {
            hit = true;
            break;
        }
        t += clamp(h.d * 0.9, 0.03, 0.8);
        if t > t_max {
            break;
        }
    }
    if !hit && t_ground < 90.0 {
        // Reached the street without meeting a tower.
        hit = true;
        t = t_ground;
        h.ground = true;
    }

    let pulse = 0.75 + 0.6 * u.intensity;
    // Sky: dusk gradient with a molten sun hung at the end of the road.
    let sun_dir = normalize(vec3<f32>(sin(u.time * 0.05) * 0.2, 0.16, 1.0));
    let sun = pow(max(dot(rd, sun_dir), 0.0), 14.0);
    var col = palette(0.52) * 0.10 * (1.0 - rd.y) + palette(0.62) * sun * (1.2 + u.energy);

    if hit {
        let pos = ro + rd * t;
        if h.ground {
            // Wet asphalt: mirror the glow, centre lines dashed down each
            // street (not across the crossroads).
            col = vec3<f32>(0.010, 0.010, 0.018) + glow * 0.3;
            let pitch = CELL * STREETS;
            let cx = abs(pos.x - 1.6 - pitch * floor((pos.x - 1.6) / pitch + 0.5));
            let cz = abs(pos.z - 1.6 - pitch * floor((pos.z - 1.6) / pitch + 0.5));
            let lane_z = smoothstep(0.06, 0.0, cx) * step(fract(pos.z * 0.4), 0.55) * step(1.6, cz);
            let lane_x = smoothstep(0.06, 0.0, cz) * step(fract(pos.x * 0.4), 0.55) * step(1.6, cx);
            col += palette(0.62) * max(lane_z, lane_x) * 0.5;
        } else {
            let shade = 0.10 + 0.9 * smoothstep(0.0, 4.0, pos.y);
            col = vec3<f32>(0.018, 0.016, 0.03) * shade + windows(pos, h) * pulse;
        }
    }

    let fog = 1.0 - exp(-t * 0.03);
    col = mix(col, palette(0.55) * (0.12 + 0.3 * u.energy), fog);
    col += prev(uncentred(centred(in.uv) * 0.985)) * 0.15;
    return vec4<f32>(col, 1.0);
}
