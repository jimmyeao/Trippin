// @heavy — cathedral run: endless pointed arches sweeping past, lit from
// within on the beat. Unlike rave_hall's pillars these are solid vaults —
// heavier, more monumental.

fn arch2d(q: vec2<f32>) -> f32 {
    // Pointed-arch opening centred on x=0: two mirrored arcs meeting at a
    // point above. Negative inside the opening.
    // Shape: openings widen as the bass builds.
    let w = 0.9 + 0.5 * u.pres4.x;
    let r = 1.6;
    let d1 = length(q - vec2<f32>(-w * 0.5, -0.2)) - r;
    let d2 = length(q - vec2<f32>(w * 0.5, -0.2)) - r;
    return max(min(d1, d2), -q.y - 0.4); // cap the opening's bottom
}

fn map(p: vec3<f32>) -> f32 {
    // Corridor clearance: distance to side walls / floor / ceiling.
    let corr = min(min(2.6 - abs(p.x), p.y + 1.0), 2.4 - p.y);
    // Arch wall slabs every 2.4z, opening cut out of each.
    let slab = abs(fract(p.z / 2.4 + 0.5) * 2.4 - 1.2) - 0.10;
    let opening = arch2d(vec2<f32>(p.x, p.y));
    // Solid = inside slab AND outside opening; clip to the corridor box.
    let wall = max(max(slab, -opening), -corr);
    return min(corr, wall);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.clock4.x * 2.8;
    let ro = vec3<f32>(sin(z * 0.08) * 0.4, 0.2 + sin(u.time * 0.5) * 0.1, z);
    var rd = normalize(vec3<f32>(p.x * 0.7, -p.y * 0.6, 1.0));
    // Direction: a slow roll that swings back and forth.
    rd = vec3<f32>(rot(0.15 * sin(u.clock4.x * 0.03)) * rd.xy, rd.z);

    var t = 0.0;
    var pos = ro;
    for (var i = 0; i < 72; i++) {
        pos = ro + rd * t;
        let d = map(pos) * 0.7;
        if d < 0.01 { break; }
        t += clamp(d, 0.03, 0.9);
        if t > 30.0 { break; }
    }

    var col = vec3<f32>(0.01, 0.008, 0.02);
    if t < 30.0 {
        let fade = exp(-t * 0.16);
        let lamp = palette(fract(pos.z / 2.4) * 0.13 + u.hue);
        // Arch rims glow: nearer the opening edge, hotter the light.
        let rim = smoothstep(0.35, 0.0, arch2d(vec2<f32>(pos.x, pos.y)));
        var stone = vec3<f32>(0.04, 0.032, 0.05);
        stone += lamp * rim * (0.3 + beat_pulse(3.0) * 1.6);
        // Floor sheen.
        if pos.y < -0.95 {
            stone = vec3<f32>(0.03, 0.025, 0.04) + palette(0.8 + u.hue) * 0.05;
        }
        col = mix(col, stone, fade);
    }
    // Light at the end of the corridor.
    col += palette(0.6 + u.hue) * exp(-length(p) * 2.2) * (0.15 + u.energy * 0.6);
    return vec4<f32>(col, 1.0);
}
