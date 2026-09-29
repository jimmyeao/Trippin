// @heavy — raymarched crystal cavern. The camera drifts down a dark cave
// tube; crystals stud the rock walls and flare with the highs. Rocks stay
// dim so the gems carry the colour.

// Returns (distance, material): material 0 = rock, 1 + h = crystal with
// hash h. (It used to return the crystal hash even when the rock was the
// nearer surface, so the shader picked rock vs crystal at random — flat
// colour patchwork.)
fn map(p: vec3<f32>) -> vec2<f32> {
    // Cave tube along z: rock where wall_d > 0, open bore where < 0.
    let wall_d = length(p.xy) - (2.0 + fbm(p.xz * 0.8 + p.yy * 0.15) * 0.7);
    let rock = -wall_d; // inside the bore → positive distance to rock
    // Crystal clusters: hashed cells, only grown where the rock is near
    // the bore surface (clipped so nothing floats in the air).
    let id = floor(p * 2.2);
    let h = hash21(id.xy + id.z);
    let h2 = hash22(id.xy + id.z * 3.7);
    // Sparse: only ~40% of cells grow a crystal (a packed grid of them read
    // as patchwork).
    if h > 0.4 {
        let g0 = rock;
        return vec2<f32>(g0, 0.0);
    }
    // Crystal centred (with jitter) in its cell, cell-local coordinates.
    let cc = (id + 0.5 + (vec3<f32>(h2, fract(h * 7.3)) - 0.5) * 0.4) / 2.2;
    let c = p - cc;
    // Faceted crystal: a hexagonal-ish prism with a pointed tip, pointing
    // out of the rock toward the bore axis. Shape: grows with its band and
    // pops on kicks.
    let size = 0.1 * (0.5 + h * 2.0) * (0.6 + 0.4 * spec(h * 2.5)) + 0.008 * u.hits4.x;
    let out_dir = normalize(vec3<f32>(-cc.xy, 0.0) + vec3<f32>(0.0, 0.0, 1e-4));
    let along = dot(c, out_dir);
    let side = length(c - out_dir * along);
    // Prism of radius size*0.45 and length size*2, tapering to a point.
    let taper = size * 0.45 * clamp(1.0 - along / (size * 2.0), 0.0, 1.0);
    let gem = max(side - taper, max(-along - size * 0.3, along - size * 2.0)) * 0.7;
    let in_shell = wall_d + 0.35; // crystals live just inside the rock face
    let g = max(gem, -in_shell);
    if g < rock {
        return vec2<f32>(g, 1.0 + h * 2.4);
    }
    return vec2<f32>(rock, 0.0);
}

fn normal(p: vec3<f32>) -> vec3<f32> {
    let e = 0.004;
    let k = vec2<f32>(1.0, -1.0);
    return normalize(k.xyy * map(p + k.xyy * e).x + k.yyx * map(p + k.yyx * e).x + k.yxy * map(p + k.yxy * e).x + k.xxx * map(p + k.xxx * e).x);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let z = u.clock4.x * 2.5;
    let ro = vec3<f32>(sin(z * 0.1) * 0.5, -cos(z * 0.07) * 0.35, z);
    // -p.y is up: rays at the top of the screen point up.
    var rd = normalize(vec3<f32>(p.x * 0.7, -p.y * 0.7, 1.0));
    // Direction: the view rolls one way, then back.
    rd = vec3<f32>(rot(0.35 * sin(u.clock4.z * 0.02)) * rd.xy, rd.z);
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
        let n = normal(pos);
        let fade = exp(-t * 0.16);
        // Headlamp from the camera + a cool fill from the bore.
        let l = normalize(ro - pos);
        let dif = max(dot(n, l), 0.0);
        if hit < 0.5 {
            // Rock: dark slate, lit by the headlamp and the crystals' glow.
            let alb = vec3<f32>(0.05, 0.055, 0.07) * (0.6 + 0.8 * fbm(pos.xz * 3.0 + pos.yy));
            let bounce = palette(0.3 + u.hue) * 0.06 * (0.5 + u.lvl4.w);
            col = mix(col, alb * (dif * dif * 1.1 + 0.05) + bounce, fade);
        } else {
            // Crystal: emissive core in its band colour, faceted highlights
            // and a bright fresnel rim; flares with the highs and kicks.
            let h = (hit - 1.0) / 2.4 * 2.5;
            let cc = palette(h * 1.7 + u.hue);
            let glow = 0.25 + 1.6 * spec(h) + 0.8 * u.hits4.x + 0.5 * u.lvl4.w;
            let fres = pow(1.0 - max(dot(n, -rd), 0.0), 3.0);
            let spec_l = pow(max(dot(reflect(rd, n), l), 0.0), 40.0);
            let gc = cc * (0.2 + dif * 0.6) * glow + cc * fres * 1.5 + vec3<f32>(1.0) * spec_l * 0.8;
            col = mix(col, gc, fade);
        }
    }
    return vec4<f32>(col, 1.0);
}
