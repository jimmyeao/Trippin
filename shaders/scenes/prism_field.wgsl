// @heavy — raymarched. A field of glowing prisms seen from above: every
// column's height tracks a spectrum band, so the whole field is a 3D
// equaliser. The camera glides over it; kicks make the near columns punch up.

const CELL: f32 = 0.55;
const LANE: f32 = 14.0; // spectrum columns per grid row-wrap

fn col_spec(id: vec2<f32>) -> f32 {
    // Walk the spectrum across the grid: band index from cell coords.
    let k = (id.x + id.y * LANE) / (LANE * LANE);
    return spec(fract(k) * 0.95);
}

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

struct Hit {
    d: f32,
    id: vec2<f32>,
    top: bool,
};

fn map(p: vec3<f32>) -> Hit {
    let id = floor(p.xz / CELL);
    // Always a real column; the EQ adds height on top and the kick lifts all.
    let hgt = 0.45 + 1.3 * pow(col_spec(id), 0.7) + 0.3 * u.hits4.x;
    let c = (id + 0.5) * CELL;
    // Prism from the floor up to its EQ height; a narrow gap between columns.
    let local = p - vec3<f32>(c.x, hgt * 0.5, c.y);
    let d = sd_box(local, vec3<f32>(CELL * 0.42, hgt * 0.5, CELL * 0.42));
    var h: Hit;
    h.id = id;
    h.top = d < p.y + 0.03;
    h.d = min(d, p.y + 0.03); // dark floor between columns
    return h;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.clock4.x * 1.4;
    // High oblique view, drifting forward; tilts a touch with the kick.
    let ro = vec3<f32>(sin(z * 0.1) * 1.4, 1.9 - 0.12 * beat_pulse(8.0), z);
    // centred() has +y pointing down the screen — negate so up is up.
    var rd = normalize(vec3<f32>(p.x * 0.9, -p.y * 0.65 - 0.42, 1.0));
    // Direction: banking one way over the field, then the other.
    rd = vec3<f32>(rot(sin(u.clock4.x * 0.025) * 0.2) * rd.xy, rd.z);

    var t = 0.0;
    var glow = vec3<f32>(0.0);
    var hit = false;
    var h: Hit;
    for (var i = 0; i < 72; i++) {
        h = map(ro + rd * t);
        if h.top {
            let band = (h.id.x + h.id.y * LANE) / (LANE * LANE);
            glow += palette(fract(band)) * exp(-max(h.d, 0.0) * 18.0) * 0.012;
        }
        if h.d < 0.0015 * (1.0 + t) {
            hit = true;
            break;
        }
        t += clamp(h.d * 0.85, 0.02, 0.5);
        if t > 30.0 {
            break;
        }
    }

    var col = palette(0.55) * 0.05 + glow * (0.6 + 0.8 * u.intensity);

    if hit {
        let pos = ro + rd * t;
        let band = (h.id.x + h.id.y * LANE) / (LANE * LANE);
        let e = col_spec(h.id);
        let face = palette(fract(band));
        if !h.top {
            // The floor plane between columns.
            col = vec3<f32>(0.012, 0.012, 0.02) + glow * 0.25;
        } else {
            let hgt = 0.45 + 1.3 * pow(e, 0.7) + 0.3 * u.hits4.x;
            if pos.y > hgt - 0.09 {
                // Lit cap: bright face, hotter with the band's energy.
                col = face * (0.6 + 1.5 * e) * (0.7 + 0.6 * u.intensity);
            } else {
                // Column sides: dim sheen of their own hue.
                col = face * (0.10 + 0.25 * e);
            }
        }
    }

    col *= exp(-t * 0.06);
    col += prev(uncentred(centred(in.uv) * 0.985)) * 0.15;
    return vec4<f32>(col, 1.0);
}
