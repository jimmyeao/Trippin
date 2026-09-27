// @heavy — raymarched neon city canyon: a street through an endless block
// grid. Each tower's windows burn with its own slice of the spectrum; the
// kick drops the camera a touch and the sun glows down the road.

const CELL: f32 = 3.2; // block grid pitch

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// Height and glow of the tower in cell `id`; the street keeps x cells 0/-1 clear.
fn tower_h(id: vec2<f32>) -> f32 {
    return 0.5 + 3.0 * hash21(id * 3.17 + 11.0);
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
    h.ground = true;
    h.d = p.y + 0.02; // street plane
    // Only the blocks beside the street lane exist: columns |id.x| > 0.
    let id = floor(p.xz / CELL);
    if abs(id.x + 0.5) > 0.5 {
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
    let z = u.flow * 4.0;

    // Street-centre camera; dips on the kick, sways lazily.
    let ro = vec3<f32>(sin(u.time * 0.23) * 0.3, 1.05 - 0.10 * beat_pulse(8.0), z);
    // centred() has +y pointing down the screen — negate so up is up.
    var rd = normalize(vec3<f32>(p.x * 0.8, -p.y * 0.7 + 0.12, 1.0));
    rd = vec3<f32>(rot(sin(u.time * 0.09) * 0.04) * rd.xy, rd.z);

    var t = 0.0;
    var glow = vec3<f32>(0.0);
    var hit = false;
    var h: Hit;
    for (var i = 0; i < 80; i++) {
        h = map(ro + rd * t);
        if !h.ground {
            glow += palette(hash21(h.id + 31.0)) * exp(-max(h.d, 0.0) * 14.0) * 0.008;
        }
        if h.d < 0.0015 * (1.0 + t) {
            hit = true;
            break;
        }
        t += clamp(h.d * 0.9, 0.03, 0.8);
        if t > 60.0 {
            break;
        }
    }

    let pulse = 0.75 + 0.6 * u.intensity;
    // Sky: dusk gradient with a molten sun hung at the end of the road.
    let sun_dir = normalize(vec3<f32>(sin(u.time * 0.05) * 0.2, 0.16, 1.0));
    let sun = pow(max(dot(rd, sun_dir), 0.0), 14.0);
    var col = palette(0.52) * 0.10 * (1.0 - rd.y) + palette(0.62) * sun * (1.2 + u.energy);

    if hit {
        let pos = ro + rd * t;
        if h.ground {
            // Wet asphalt: mirror the glow, centre line dashed in the lane.
            col = vec3<f32>(0.010, 0.010, 0.018) + glow * 0.3;
            let lane = smoothstep(0.06, 0.0, abs(pos.x)) * step(fract(pos.z * 0.4), 0.55);
            col += palette(0.62) * lane * 0.5;
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
