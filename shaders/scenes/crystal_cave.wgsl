// @heavy — raymarched crystal cavern. The camera drifts down a dark cave
// tube; crystals stud the rock walls and flare with the highs. Rocks stay
// dim so the gems carry the colour.

fn map(p: vec3<f32>) -> vec2<f32> {
    // Cave tube along z: rock where wall_d > 0, open bore where < 0.
    let wall_d = length(p.xy) - (2.0 + fbm(p.xz * 0.8 + p.yy * 0.15) * 0.7);
    var d = -wall_d; // inside the bore → positive distance to rock
    // Crystal clusters: hashed cells, only grown where the rock is near
    // the bore surface (clipped so nothing floats in the air).
    let id = floor(p * 2.2);
    let h = hash21(id.xy + id.z);
    let c = fract(p * 2.2) - 0.5;
    let gem = length(c) - 0.13 * (0.4 + h);
    let in_shell = wall_d + 0.35; // crystals live just inside the rock face
    d = min(d, max(gem, -in_shell));
    return vec2<f32>(d, h);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.flow * 2.5;
    let ro = vec3<f32>(sin(z * 0.1) * 0.5, -cos(z * 0.07) * 0.35, z);
    // -p.y is up: rays at the top of the screen point up.
    var rd = normalize(vec3<f32>(p.x * 0.7, -p.y * 0.7, 1.0));
    rd = vec3<f32>(rot(sin(z * 0.05) * 0.2) * rd.xy, rd.z);

    var t = 0.0;
    var hit = 0.0;
    var pos = ro;
    var hit_gem = false;
    for (var i = 0; i < 72; i++) {
        pos = ro + rd * t;
        let m = map(pos);
        if m.x < 0.008 + t * 0.002 {
            hit = m.y;
            hit_gem = true;
            break;
        }
        t += clamp(m.x * 0.55, 0.03, 0.7);
        if t > 26.0 { break; }
    }

    var col = vec3<f32>(0.006, 0.008, 0.02);
    // Faint cool depth glow ahead.
    col += palette(0.8 + u.hue) * (1.0 - exp(-t * 0.2)) * 0.10;

    if hit_gem && t < 26.0 {
        // Gem vs rock by how close to the bare sphere distance we got.
        let cc = palette(hit * 1.7 + u.hue);
        let fade = exp(-t * 0.28);
        let gem_glow = 0.3 + u.high * 2.2 + u.kick * 0.8;
        let rock = vec3<f32>(0.03, 0.032, 0.05);
        let shade = step(0.45, hit);
        col = mix(col, mix(rock, cc * gem_glow, shade), fade);
    }
    return vec4<f32>(col, 1.0);
}
