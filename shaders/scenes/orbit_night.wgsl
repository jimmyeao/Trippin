// @heavy — 2026 tier. @bloom 0.6 @tonemap agx
// Low Earth orbit before dawn: city lights spread across the night side
// below, storms flicker inside the cloud deck, and ahead the atmosphere's
// limb burns blue-to-orange where the sun is about to rise. The planet is an
// analytic sphere; the atmosphere is a 10-sample single-scattering march
// with an analytic sun optical depth (no inner loop), so it's cheap.
// The ground drifts under the camera on the tempo clock; the music brings
// the dawn up, and storm cells flash on the kicks.

const R: f32 = 1.0;
const ATM: f32 = 0.035;   // shell thickness (exaggerated for the glow)
const HR: f32 = 0.0045;   // Rayleigh scale height
const HM: f32 = 0.0015;   // Mie scale height
const B_R: vec3<f32> = vec3<f32>(3.4, 7.9, 19.0);
const B_M: f32 = 5.0;

fn sphere(ro: vec3<f32>, rd: vec3<f32>, r: f32) -> vec2<f32> {
    let b = dot(ro, rd);
    let c = dot(ro, ro) - r * r;
    let h = b * b - c;
    if h < 0.0 {
        return vec2<f32>(-1.0, -1.0);
    }
    let s = sqrt(h);
    return vec2<f32>(-b - s, -b + s);
}

// Optical depth factor toward the sun from height `hgt` whose local up
// makes cosine `mu` with the sun — Chapman-ish approximation, going to
// "fully shadowed" once the sun is well below that point's horizon.
fn sun_depth(hgt: f32, mu: f32, H: f32) -> f32 {
    let horizon = -sqrt(max(2.0 * hgt / (R + hgt), 0.0));
    let m = max(mu - horizon, 0.0);
    return exp(-hgt / H) * H / (m + 0.02) * 1.2 + select(0.0, 1e3, mu < horizon - 0.02);
}

fn stars(rd: vec3<f32>) -> vec3<f32> {
    let g = rd * 240.0;
    let cell = floor(g);
    let r1 = hash22(cell.xy + cell.z * 17.3);
    let r2 = hash21(cell.yz + cell.x * 7.1);
    let sp = cell + vec3<f32>(r1.y, r2, fract(r2 * 13.1)) * 0.6 + 0.2;
    let b = step(0.88, r1.x) * (pow(r2, 8.0) * 2.5 + 0.04);
    return mix(vec3<f32>(0.7, 0.8, 1.0), vec3<f32>(1.0, 0.85, 0.7), r1.y) * b * smoothstep(0.22, 0.0, length(g - sp));
}

// Surface colour at unit-sphere point n (world) with sun direction sd.
fn ground(n: vec3<f32>, rd: vec3<f32>, sd: vec3<f32>, foot: f32) -> vec3<f32> {
    // Surface coordinates rotate under us (the camera "moves").
    let a = u.flow * 0.0016 + u.seed * 0.1;
    let q = vec3<f32>(n.x, n.y * cos(a) - n.z * sin(a), n.y * sin(a) + n.z * cos(a));
    let cont = tnoise(q * 0.45 + 0.13).b + (tnoise(q * 1.3 + 0.4).r - 0.22) * 0.35;
    let land = smoothstep(0.5, 0.515, cont);
    let coast = smoothstep(0.49, 0.5, cont) - land * 0.5;
    let lit = dot(n, sd);
    let day = smoothstep(-0.03, 0.12, lit);

    // Day: ocean with sun glint, land in desaturated greens/browns.
    let hv = normalize(sd - rd);
    let glint = ggx(max(dot(n, hv), 0.0), max(lit, 0.0), 0.18) * (1.0 - land);
    let ocean = vec3<f32>(0.004, 0.012, 0.03);
    let terr = mix(vec3<f32>(0.035, 0.04, 0.02), vec3<f32>(0.07, 0.055, 0.035), tnoise(q * 3.0).b);
    var c = mix(ocean, terr, land) * max(lit, 0.0) * 3.0 + glint * vec3<f32>(1.0, 0.8, 0.6) * 0.4;

    // Night: city lights at three scales — metro regions, suburban sprawl
    // inside them, and a glitter of individual towns and roads.
    let region = smoothstep(0.46, 0.66, tnoise(q * 2.5 + 3.1).a) + coast * 0.4;
    let sprawl = smoothstep(0.45, 0.75, tnoise(q * 40.0).g) * (0.3 + 0.7 * smoothstep(0.3, 0.7, tnoise(q * 150.0 + 0.5).a));
    // Glitter points fade to their average once a cell is under ~2 pixels
    // (`foot` = world size of a pixel here) — no aliasing sparkle.
    let fine = q * 900.0;
    let fc = floor(fine);
    let pt = step(0.8, hash21(fc.xy + fc.z * 31.7)) * smoothstep(0.3, 0.05, length(fract(fine.xy) - 0.5));
    let glit = mix(pt, 0.03, smoothstep(0.3, 0.6, foot * 900.0));
    let city = land * (region * (sprawl * 1.3 + glit * 1.2) + glit * 0.08);
    let city_col = mix(vec3<f32>(1.0, 0.55, 0.2), vec3<f32>(0.9, 0.9, 1.0), hash21(fc.xz));
    let drive = 0.6 + 0.6 * u.intensity;
    c += city * city_col * 0.3 * (1.0 - day) * drive;

    // Cloud deck (drifts slowly relative to the ground).
    let cq = q * 1.6 + vec3<f32>(u.time * 0.002, 0.0, 0.0);
    let cl = smoothstep(0.45, 0.75, tnoise(cq).r * 0.7 + tnoise(cq * 3.0).g * 0.3);
    // Storm cells: a few flash on each kick, re-picked every beat.
    let cell = floor(q * 38.0);
    let pick = hash21(cell.xy + cell.z * 13.0 + floor(u.beat) * 7.31);
    let storm = step(pick, 0.012 + 0.02 * u.intensity) * u.kick * cl;
    let inner = smoothstep(0.7, 0.0, length(fract(q * 38.0) - 0.5));
    c = mix(c * (1.0 - cl * 0.85), vec3<f32>(0.9) * max(lit, 0.0) * 2.2 + vec3<f32>(0.004, 0.005, 0.008), cl * mix(0.3, 1.0, day));
    c += storm * inner * vec3<f32>(0.6, 0.65, 1.0) * 1.5 * (1.0 - day);
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let alt = 0.068;
    let ro = vec3<f32>(0.0, R + alt, 0.0);
    // Look ahead toward the horizon, gently yawing and rolling.
    let yaw = sin(u.flow * 0.009) * 0.25;
    let pitch = -0.2 + sin(u.flow * 0.013) * 0.04;
    let fwd = vec3<f32>(sin(yaw) * cos(pitch), sin(pitch), cos(yaw) * cos(pitch));
    let rd = cam_ray(p, ro, ro + fwd, sin(u.flow * 0.007) * 0.06, 1.35);

    // Dawn: the sun sits just below the horizon ahead; the music lifts it.
    let el = -0.47 + 0.08 * u.intensity + 0.015 * sin(u.time * 0.05);
    let sd = normalize(vec3<f32>(0.35, el, 1.0));

    var col = stars(rd);
    let hp = sphere(ro, rd, R);
    let ha = sphere(ro, rd, R + ATM);
    var t_end = ha.y;
    if hp.x > 0.0 {
        let n = normalize(ro + rd * hp.x);
        col = ground(n, rd, sd, hp.x / (540.0 * 1.35) / max(dot(n, -rd), 0.15));
        t_end = hp.x;
    }

    // Single scattering through the shell.
    if ha.y > 0.0 {
        let t0 = max(ha.x, 0.0);
        let steps = 10;
        let dt = (t_end - t0) / f32(steps);
        var tr_r = 0.0;
        var tr_m = 0.0;
        var sum_r = vec3<f32>(0.0);
        var sum_m = vec3<f32>(0.0);
        let off = bluen(in.pos.xy);
        for (var i = 0; i < steps; i++) {
            let pos = ro + rd * (t0 + (f32(i) + off) * dt);
            let r = length(pos);
            let hgt = max(r - R, 0.0);
            let dr = exp(-hgt / HR) * dt;
            let dm = exp(-hgt / HM) * dt;
            tr_r += dr;
            tr_m += dm;
            let mu = dot(pos / r, sd);
            let sun_r = sun_depth(hgt, mu, HR);
            let sun_m = sun_depth(hgt, mu, HM);
            let att = exp(-(B_R * (tr_r + sun_r) + B_M * 1.1 * (tr_m + sun_m)));
            sum_r += att * dr;
            sum_m += att * dm;
        }
        let mu_v = dot(rd, sd);
        let ph_r = 0.0596 * (1.0 + mu_v * mu_v);
        let g = 0.8;
        let ph_m = 0.119 * (1.0 - g * g) / pow(1.0 + g * g - 2.0 * g * mu_v, 1.5);
        let sun_i = 22.0 * (0.8 + 0.4 * u.intensity);
        let ext = exp(-(B_R * tr_r + B_M * 1.1 * tr_m));
        col = col * ext + (sum_r * B_R * ph_r + sum_m * B_M * ph_m) * sun_i;
    }
    return vec4<f32>(col, 1.0);
}
