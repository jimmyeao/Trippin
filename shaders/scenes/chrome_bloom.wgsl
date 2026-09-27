// @heavy — raymarched. A chrome bloom: a metal flower whose petals are
// ripples carved into a sphere, turning under two studio lights. The kick
// opens the bloom a little; the spectrum rolls colour across the metal.

fn map(p: vec3<f32>) -> f32 {
    var q = p;
    // Slow tumble; the whole bloom nods with the phrase.
    let a = u.time * 0.15 + u.bar_phase * 0.6;
    q = vec3<f32>(rot(a) * q.xy, q.z);
    q = vec3<f32>(q.x, rot(a * 0.7 + 0.8) * q.yz);
    let np = normalize(q);
    let az = angle(q.xz);
    let pol = acos(clamp(np.y, -1.0, 1.0));
    // Petals: lobes around the equator modulated over the pole angle.
    let petal = sin(az * 6.0 + u.flow * 0.9) * (0.5 + 0.5 * sin(pol * 3.0 - u.time * 0.6));
    let r = 1.05 + petal * (0.16 + 0.10 * u.kick) + u.bass * 0.03;
    return (length(q) - r) * 0.75;
}

fn normal_at(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.004, 0.0);
    return normalize(vec3<f32>(
        map(p + e.xyy) - map(p - e.xyy),
        map(p + e.yxy) - map(p - e.yxy),
        map(p + e.yyx) - map(p - e.yyx)));
}

// Studio-strip environment: metal reflects broad soft bands keyed to the mix.
fn env(d: vec3<f32>) -> vec3<f32> {
    let band = d.y * 0.5 + 0.5;
    var c = mix(palette(0.0 + u.hue), palette(0.5 + u.hue), band);
    // Squared palette for saturated metal, not milk.
    c *= c;
    // Two softboxes: one warm key, one cool fill.
    c += vec3<f32>(1.0, 0.85, 0.6) * pow(max(d.y, 0.0), 8.0) * 0.9;
    c += palette(0.75) * pow(max(-d.y, 0.0), 6.0) * (0.3 + 0.5 * u.mid);
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let drift = u.time * 0.13 + u.flow * 0.08;
    let ro = vec3<f32>(sin(drift) * 3.0, sin(u.time * 0.19) * 0.8, cos(drift) * 3.0);
    let fw = normalize(-ro);
    let rt = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), fw));
    let up = cross(fw, rt);
    // centred() has +y pointing down the screen — negate so up is up.
    let rd = normalize(fw * 1.5 + rt * p.x - up * p.y);

    var t = 0.0;
    var hit = false;
    var steps = 0.0;
    for (var i = 0; i < 80; i++) {
        let d = map(ro + rd * t);
        steps = f32(i);
        if d < 0.002 * (1.0 + t) {
            hit = true;
            break;
        }
        t += max(d * 0.55, 0.01);
        if t > 8.0 {
            break;
        }
    }

    var col = vec3<f32>(0.0);
    if hit {
        let pos = ro + rd * t;
        let n = normal_at(pos);
        // Chrome: shade by the reflected ray's environment, rim-lit.
        let rdir = reflect(rd, n);
        col = env(rdir) * (0.55 + 0.45 * u.intensity);
        // Sharp key-light specular so it reads as metal.
        let lit_dir = normalize(vec3<f32>(0.5, 0.8, 0.6));
        col += vec3<f32>(1.0, 0.95, 0.85) * pow(max(dot(rdir, lit_dir), 0.0), 60.0) * 1.6;
        let fres = pow(1.0 - max(dot(n, -rd), 0.0), 4.0);
        col += palette(0.6 + u.hue) * fres * (0.7 + u.high * 0.8);
        // Darken the petal valleys so the lobes read.
        var q = pos;
        let a = u.time * 0.15 + u.bar_phase * 0.6;
        q = vec3<f32>(rot(a) * q.xy, q.z);
        q = vec3<f32>(q.x, rot(a * 0.7 + 0.8) * q.yz);
        let np = normalize(q);
        let az = angle(q.xz);
        let pol = acos(clamp(np.y, -1.0, 1.0));
        let petal = sin(az * 6.0 + u.flow * 0.9) * (0.5 + 0.5 * sin(pol * 3.0 - u.time * 0.6));
        col *= 0.55 + 0.45 * (petal * 0.5 + 0.5);
    }
    // Aura behind the bloom.
    col += palette(0.55) * exp(-length(p) * 2.4) * 0.08 * (0.4 + u.energy);
    col += prev(uncentred(centred(in.uv) * 0.99)) * 0.12;
    return vec4<f32>(col, 1.0);
}
