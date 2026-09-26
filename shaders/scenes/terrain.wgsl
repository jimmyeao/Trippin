// Spectrum terrain: a voxel-space heightfield flyover at dusk. The ridge
// nearest the camera IS the spectrum — loud bands carve peaks that recede
// into noise-sculpted hills. Wireframe surface, kick quake, starfield.

fn theight(wx: f32, z: f32, wz: f32) -> f32 {
    var h = fbm(vec2<f32>(wx * 0.35, wz * 0.2)) * 2.4
          + fbm(vec2<f32>(wx * 0.9 + 7.3, wz * 0.55)) * 0.5;
    // A valley along the middle so the spectrum ridge reads clearly.
    h *= 0.5 + 0.5 * smoothstep(0.0, 12.0, abs(wx));
    // Spectrum ridge near the camera: x maps to a band, height rides it.
    let band = clamp(abs(wx) / 12.0, 0.0, 1.0);
    let near_z = exp(-z * 0.18);
    h += spec(band) * (2.0 + u.intensity * 1.8) * near_z;
    return h;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let horizon = 0.60;
    let travel = u.flow * 1.1;
    // Camera height bobs gently and jumps on the kick.
    let ch = 3.4 + sin(u.flow * 0.2) * 0.2 + u.kick * 0.25;

    // Sky: dusk gradient, low sun glow, stars twinkling on the highs.
    var sky = mix(vec3<f32>(0.015, 0.01, 0.035), palette(0.72) * 0.4,
                  pow(uv.y, 1.5) * (0.6 + u.energy * 0.5));
    let sun = exp(-length(vec2<f32>((uv.x - 0.5) * aspect(), (uv.y - horizon) * 1.6)) * 5.0);
    sky += palette(0.06) * sun * (0.3 + u.kick * 0.6);
    let st = step(0.9985, hash21(floor(uv * vec2<f32>(300.0, 160.0))));
    sky += vec3<f32>(0.8, 0.85, 1.0) * st * (0.3 + u.high) * smoothstep(horizon, 1.0, uv.y);
    var col = sky;

    // March each column's ray forward; the first depth where the terrain
    // surface projects above this pixel wins (near occludes far for free).
    let px = (uv.x - 0.5) * aspect() * 1.3;
    var hit_z = -1.0;
    var hit_wx = 0.0;
    var hit_wz = 0.0;
    var hit_h = 0.0;
    for (var i = 0; i < 110; i++) {
        let z = 0.9 + 45.0 * f32(i) / 109.0;
        let wx = px * z;
        let wz = z + travel;
        let h = theight(wx, z, wz);
        let ys = horizon - (ch - h) / (z * 0.85);
        if uv.y < ys {
            hit_z = z;
            hit_wx = wx;
            hit_wz = wz;
            hit_h = h;
            break;
        }
    }

    if hit_z > 0.0 {
        let fog = exp(-hit_z * 0.07);
        let band = clamp(abs(hit_wx) / 12.0, 0.0, 1.0);
        // Wireframe lines on the surface grid.
        let gx = abs(fract(hit_wx * 0.7) - 0.5);
        let gz = abs(fract(hit_wz * 0.7) - 0.5);
        let wire = smoothstep(0.10, 0.02, min(gx, gz));
        // Base tint by altitude + band hue.
        let alt = clamp(hit_h / 4.5, 0.0, 1.0);
        var tcol = palette(0.58 + band * 0.25) * (0.06 + 0.10 * alt);
        tcol += palette(0.58 + band * 0.25) * wire * (0.3 + u.energy * 0.5);
        // Glowing spectrum crest near the camera.
        tcol += palette(band) * (spec(band) * 1.5 + u.kick * 0.15) * exp(-hit_z * 0.30);
        col = mix(sky, tcol, fog);
    }

    return vec4<f32>(finite(col), 1.0);
}
