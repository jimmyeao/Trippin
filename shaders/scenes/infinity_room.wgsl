// @heavy — 2026 tier. @bloom 0.9 @tonemap agx
// Kusama-style infinity mirror room: a small mirrored box hung with LED
// points, reflected into infinity in every direction.
// A mirror box is equivalent to an infinite lattice of mirrored copies of
// the room, so a *straight* ray walked through that lattice (3D DDA, one
// cell per step) sees every reflection: in cell c the room is flipped on
// each axis where c is odd, and each wall crossed costs ~12% of the light.
// The camera turns slowly on the tempo clock. Every point belongs to a
// spectrum band, and a wave of light ripples out from the centre each bar.

const L: f32 = 4.0;            // room size
const K: i32 = 9;              // LED points per room
const STEPS: i32 = 16;         // lattice cells walked

// Point positions in the base room (xyz) + spectrum band (w), baked.
const LEDS: array<vec4<f32>, 11> = array<vec4<f32>, 11>(
    vec4<f32>(1.436, 0.892, 2.483, 0.072),
    vec4<f32>(2.115, 1.451, 0.586, 0.507),
    vec4<f32>(0.520, 1.627, 0.624, 0.091),
    vec4<f32>(1.758, 2.650, 0.796, 0.223),
    vec4<f32>(2.408, 2.964, 2.247, 0.397),
    vec4<f32>(3.524, 0.621, 3.147, 0.290),
    vec4<f32>(0.862, 0.806, 1.387, 0.816),
    vec4<f32>(0.978, 2.012, 2.445, 0.372),
    vec4<f32>(2.153, 0.663, 0.591, 0.206),
    vec4<f32>(2.577, 1.612, 1.405, 0.586),
    vec4<f32>(1.850, 1.279, 2.942, 0.699)
);

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Stand near the middle of the room, turning slowly and looking around.
    // Direction: turns to look one way, then back; energy: pace.
    let yaw = u.seed + 1.5 * sin(u.clock4.x * 0.02) + u.clock4.x * 0.01;
    let pitch = sin(u.clock4.x * 0.013) * (0.15 + 0.3 * u.pres4.y);
    let ro = vec3<f32>(L * 0.5 + sin(u.flow * 0.01) * 0.3, 1.6, L * 0.5 + cos(u.flow * 0.012) * 0.3);
    let fwd = vec3<f32>(sin(yaw) * cos(pitch), sin(pitch), cos(yaw) * cos(pitch));
    let rd = cam_ray(p, ro, ro + fwd, 0.0, 1.1);

    // 3D DDA through the lattice of room cells.
    var cell = floor(ro / L);
    let stp = sign(rd);
    let inv = 1.0 / max(abs(rd), vec3<f32>(1e-5));
    var tmax = ((cell + max(stp, vec3<f32>(0.0))) * L - ro) / rd;
    let tdelta = L * inv;
    let start = cell;
    var t0 = 0.0;
    var col = vec3<f32>(0.0);
    let drive = 0.5 + 0.8 * u.intensity;
    for (var i = 0; i < STEPS; i++) {
        let t1 = min(tmax.x, min(tmax.y, tmax.z));
        let bounces = dot(abs(cell - start), vec3<f32>(1.0));
        let atten = pow(0.88, bounces);
        // Mirror flip for odd cells.
        let odd = abs(cell - 2.0 * floor(cell * 0.5));
        let base = cell * L;
        for (var k = 0; k < K; k++) {
            let lk = LEDS[k];
            let lp = lk.xyz;
            let wp = base + mix(lp, vec3<f32>(L) - lp, odd);
            let v = wp - ro;
            let tt = dot(v, rd);
            if tt < t0 - 0.5 || tt > t1 + 0.5 || tt <= 0.05 {
                continue;
            }
            let d = length(v - rd * tt);
            // Keep points at least ~1.5 px wide so the far ones don't alias.
            let r = max(0.035, tt * 0.0022);
            // Most points are nowhere near this ray — skip the colour work.
            if d > r * 10.0 {
                continue;
            }
            let band = lk.w;
            // Ripple: a ring of light travelling out from the room centre each bar.
            let dc = length(wp - vec3<f32>(L * 0.5, 1.6, L * 0.5));
            let ripple = exp(-abs(fract(dc * 0.04 - u.bar_phase) - 0.5) * 12.0) * 0.8;
            let lvl = (0.25 + 1.4 * spec(band * 0.85) + ripple * u.intensity) * drive;
            let c = palette(band * 0.8 + dc * 0.01);
            let core = exp(-d * d / (r * r));
            let halo = 0.06 * exp(-d / (r * 6.0));
            col += c * lvl * (core * 1.6 + halo) * atten * (0.035 / r) * 0.8;
        }
        t0 = t1;
        // Advance to the next cell.
        if tmax.x < tmax.y && tmax.x < tmax.z {
            cell.x += stp.x;
            tmax.x += tdelta.x;
        } else if tmax.y < tmax.z {
            cell.y += stp.y;
            tmax.y += tdelta.y;
        } else {
            cell.z += stp.z;
            tmax.z += tdelta.z;
        }
    }
    // Faint mirror seams (the room's edges repeat into the distance).
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
