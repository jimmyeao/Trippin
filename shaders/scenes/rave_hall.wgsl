// @heavy — raymarched. Infinite hall of light pillars: the camera flies a
// straight course while each column glows with its own slice of the spectrum,
// like an LED cathedral. Floors and ceiling are dark mirrors of the glow.

const CELL: f32 = 2.4;    // pillar grid spacing
const RADIUS: f32 = 0.26; // pillar radius
const FLOOR_Y: f32 = -1.5;
const CEIL_Y: f32 = 1.9;

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
    let z = u.clock4.x * 3.0;

    // Camera: stay inside a corridor lane (cell boundary at x=0 — pillars
    // sit on cell centres at ±CELL/2). Sway is capped well under half a
    // cell so the view never points head-on into a column; the kick nod
    // stays subtle.
    let ro = vec3<f32>(
        sin(z * 0.05) * 0.45 + sin(u.time * 0.7) * 0.08 * u.hits4.x,
        0.15 + sin(u.time * 0.4) * 0.1,
        z);
    // centred() has +y pointing down the screen — negate so up is up.
    var rd = normalize(vec3<f32>(
        p.x * 0.75 + sin(z * 0.04) * 0.1,
        -p.y * 0.75 - 0.05 + 0.04 * beat_pulse(6.0),
        1.0));
    // Direction: rolls one way down the hall, then the other.
    rd = vec3<f32>(rot(sin(u.clock4.x * 0.025) * 0.3) * rd.xy, rd.z);

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
