// @heavy — canyon sprint: the camera threads a winding canyon, walls lit
// by a strip glow. Bass dips the camera and stokes the lights; onsets
// spark the strips. Strafes but never hits the wall — lanes are locked.

fn map(p: vec3<f32>) -> f32 {
    // Canyon: two walls at x = ±(2.0 + wav), floor below.
    let wav = sin(p.z * 0.15) * 0.9 + sin(p.z * 0.04) * 0.6;
    let half = 1.6 + sin(p.z * 0.3) * 0.25;
    let wall = abs(p.x + wav) - half;
    let floor_d = p.y + 1.2 + noise(p.xz * 2.0) * 0.3;
    return min(max(wall, -(abs(p.x + wav) - half - 0.0)), floor_d);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    // Constant speed — modulating travel by bass made the flight lurch.
    // Bass dips the camera and stokes the wall lights instead.
    let z = u.flow * 3.2;
    // Camera follows the canyon centre.
    let wav = sin(z * 0.15) * 0.9 + sin(z * 0.04) * 0.6;
    let ro = vec3<f32>(-wav + sin(u.time * 0.4) * 0.3, -0.2 + sin(u.time * 0.7) * 0.15 - u.kick * 0.12, z);
    var rd = normalize(vec3<f32>(p.x * 0.8, -p.y * 0.55 - 0.08, 1.0));
    rd.x += cos(z * 0.15) * 0.25; // lean into the curves

    var t = 0.0;
    var pos = ro;
    for (var i = 0; i < 64; i++) {
        pos = ro + rd * t;
        let d = map(pos) * 0.7;
        if d < 0.015 { break; }
        t += clamp(d, 0.04, 1.5);
        if t > 40.0 { break; }
    }

    // Sky: warm band at the canyon lip.
    var col = mix(palette(0.95 + u.hue) * 0.5, vec3<f32>(0.02, 0.01, 0.04),
                  clamp(1.0 - rd.y * 3.0, 0.0, 1.0));
    if t < 40.0 {
        let fade = exp(-t * 0.12);
        // Rock strata bands + a glow strip that sparks on onsets.
        let strata = 0.5 + 0.5 * sin(pos.y * 6.0 + noise(pos.xz) * 3.0);
        let strip = exp(-abs(fract(pos.z * 0.4) - 0.5) * 8.0) * step(pos.y, -0.4);
        var rock = palette(0.85 + u.hue) * strata * 0.22 + vec3<f32>(0.02, 0.01, 0.008);
        rock += palette(0.1 + u.hue) * strip * (0.3 + u.onset * 2.0 + u.energy * 0.6);
        col = mix(col, rock, fade);
    }
    col += prev(uncentred(centred(in.uv) * 0.99)) * 0.18;
    return vec4<f32>(col, 1.0);
}
