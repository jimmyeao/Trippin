// Rainy neon city street at night: a corridor of buildings with glowing signs
// and lit windows, mirrored in the wet asphalt, rain streaking through the
// haze. The camera drifts down the street on the tempo clock; the neon
// breathes with the music and a few signs flicker on the beat.

// Wall facade at a point on a wall plane (z down the street, y up from the
// road, side = -1 left / +1 right). Returns the emissive + lit colour.
fn wall_light(z: f32, y: f32, side: f32) -> vec3<f32> {
    var c = vec3<f32>(0.008, 0.008, 0.014);            // dark brick

    // Windows: a sparse grid, a few warm-lit.
    let wz = floor(z * 1.4);
    let wy = floor(y * 1.6);
    let in_win = step(0.25, fract(z * 1.4)) * step(0.3, fract(z * 1.4) - 0.55)
               * step(0.35, fract(y * 1.6)) * step(0.0, -fract(y * 1.6) + 0.8)
               * step(0.25, y) * step(y, 6.0);
    let lit = step(0.78, hash21(vec2<f32>(wz * 7.0 + side * 31.0, wy)));
    c += in_win * lit * vec3<f32>(1.0, 0.75, 0.45) * 0.35 * (0.8 + 0.4 * spec(0.45));

    // Neon signs: a band of storefronts along z, each a glowing slab whose
    // colour walks the palette. They sit low on the walls.
    let seg = floor(z / 2.6);
    let sz = fract(z / 2.6);                            // 0..1 within a segment
    let sign_h = 0.5 + 0.5 * hash21(vec2<f32>(seg, side * 7.0));
    let y0 = 0.55 + 0.35 * hash21(vec2<f32>(seg * 3.0, side));
    let in_sign = step(0.12, sz) * step(sz, 0.88) * step(abs(y - y0), sign_h * 0.22);
    // Not every segment has a sign; some flicker with the kick.
    let has = step(0.18, hash21(vec2<f32>(seg, side * 11.0)));
    let flicker = f32(hash21(vec2<f32>(seg, side * 17.0)) < 0.25);
    let flick = mix(1.0, 0.55 + 0.45 * sin(u.time * 17.0 + seg * 9.0) * step(0.4, u.kick), flicker);
    let srgb = palette(hash21(vec2<f32>(seg, side * 5.0)) * 0.9);
    let drive = 0.7 + 0.6 * u.intensity + 0.4 * u.energy;
    c += in_sign * has * flick * srgb * (1.3 * drive);
    // Faint spill around the sign.
    let spill = exp(-abs(y - y0) * 3.0) * step(0.05, sz) * step(sz, 0.95);
    c += spill * has * srgb * 0.12 * drive;
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let road = 1.6;                                   // half-width of the canyon

    // Camera: chest height, drifting down the street on the tempo clock.
    let ro = vec3<f32>(sin(u.time * 0.07) * 0.4, 1.15 + sin(u.time * 0.11) * 0.05, u.flow * 0.8);
    let yaw = sin(u.time * 0.05) * 0.06;
    let dxz = rot(yaw) * vec2<f32>(p.x, 1.9);
    let rd = normalize(vec3<f32>(dxz.x, -p.y * 0.75 - 0.04, dxz.y));

    // Nearest of the three planes: ground (y=0), then the wall we face.
    let t_ground = select(1e5, -ro.y / rd.y, rd.y < -0.0001);
    let wall_x = sign(rd.x) * road;
    let t_wall2 = select(1e5, (wall_x - ro.x) / rd.x, abs(rd.x) > 0.0001);

    var col: vec3<f32>;
    var dist = 1e5;
    if t_ground < t_wall2 && t_ground < 1e4 {
        // Wet asphalt: mirror the walls and the end-glow.
        dist = t_ground;
        let hit = ro + rd * t_ground;
        let rro = vec3<f32>(hit.x, 0.0, hit.z);
        let rrd = vec3<f32>(rd.x, -rd.y, rd.z);
        let rwx = sign(rrd.x) * road;
        let rt = select(1e5, (rwx - rro.x) / rrd.x, abs(rrd.x) > 0.0001);
        let rp = rro + rrd * min(rt, 30.0);
        var refl = select(vec3<f32>(0.02, 0.02, 0.05), wall_light(rp.z, rp.y, sign(rrd.x)), rt < 1e4);
        // Reflection smears and darkens; puddle ripples wobble it.
        let rip = noise(vec2<f32>(hit.x * 6.0, hit.z * 6.0 - u.time * 2.0));
        refl *= 0.5 + 0.2 * rip;
        let wet = 0.55 + 0.45 * smoothstep(0.0, 8.0, hit.z - ro.z);
        col = vec3<f32>(0.012, 0.013, 0.02) + refl * wet * 0.55;
        // Raindrop splashes: sparse specks twinkling on the surface.
        let sp = hash21(floor(vec2<f32>(hit.x * 14.0, hit.z * 14.0 - u.time * 14.0)));
        col += vec3<f32>(0.5, 0.55, 0.6) * step(0.985, sp) * 0.25 * wet;
    } else if t_wall2 < 1e4 {
        dist = t_wall2;
        let hit = ro + rd * t_wall2;
        col = wall_light(hit.z, hit.y, sign(rd.x));
    } else {
        // Up: city glow fading into a rainy sky.
        col = mix(vec3<f32>(0.05, 0.03, 0.08), vec3<f32>(0.005, 0.005, 0.012), clamp(rd.y * 2.0, 0.0, 1.0));
    }

    // The street vanishes into a warm haze at the far end.
    let end_glow = exp(-abs(p.x) * 3.5) * smoothstep(0.5, -0.1, p.y) * (0.3 + 0.25 * u.intensity);
    col += vec3<f32>(0.5, 0.35, 0.5) * end_glow * 0.4;

    // Rain haze with distance.
    col = mix(col, vec3<f32>(0.04, 0.035, 0.06), smoothstep(10.0, 45.0, dist) * 0.85);

    // Rain streaks: slanted dashes in two parallax layers, denser and faster
    // when the track drives.
    let speed = (8.0 + 6.0 * u.energy);
    for (var i = 0; i < 2; i++) {
        let fi = f32(i) + 1.0;
        let cells = 90.0 * fi;
        let cx = floor((p.x + p.y * 0.08) * cells);
        let ry = fract(p.y * 2.2 * fi + u.time * speed * 0.13 / fi + hash21(vec2<f32>(cx, fi * 4.0)));
        let dash = step(0.5, hash21(vec2<f32>(cx, fi))) * smoothstep(0.12, 0.0, abs(ry - 0.5)) * 0.12 / fi;
        col += vec3<f32>(0.5, 0.6, 0.7) * dash * (0.4 + 0.6 * u.intensity);
    }

    col = pow(col, vec3<f32>(1.25)) * 0.95;
    return vec4<f32>(col, 1.0);
}
