// Fireworks over a city skyline: a rocket climbs and bursts on the beat every
// two beats; sparks fly out, fall under gravity, fade and leave glittering
// trails. Bigger bursts when the track drives. Seasonal (New Year, Bonfire
// Night; see config.rs).

const SPARKS: i32 = 64;
const EVERY: f32 = 2.0;        // beats between bursts
const LIFE: f32 = 5.0;         // beats a burst stays visible

fn burst_colour(k: f32) -> vec3<f32> {
    let h = hash21(vec2<f32>(k, 17.0));
    if h < 0.2 { return vec3<f32>(1.0, 0.25, 0.15); }
    if h < 0.4 { return vec3<f32>(1.0, 0.8, 0.3); }
    if h < 0.6 { return vec3<f32>(0.3, 1.0, 0.45); }
    if h < 0.8 { return vec3<f32>(0.35, 0.5, 1.0); }
    return vec3<f32>(1.0, 0.4, 0.9);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);            // y grows downward
    let asp = aspect();
    let secs_per_beat = 60.0 / max(u.bpm, 60.0);

    // Night sky with a faint city glow on the horizon.
    var sky = mix(vec3<f32>(0.01, 0.01, 0.04), vec3<f32>(0.08, 0.04, 0.1), smoothstep(-0.2, 0.7, p.y));

    var sparks = vec3<f32>(0.0);
    let now = u.flow / EVERY;
    for (var j = 0; j < 3; j++) {
        let k = floor(now) - f32(j);
        let age_beats = (now - k) * EVERY;              // beats since this burst
        if age_beats > LIFE {
            continue;
        }
        let centre = vec2<f32>((hash21(vec2<f32>(k, 1.0)) - 0.5) * 1.4 * asp, -0.15 - 0.5 * hash21(vec2<f32>(k, 2.0)));
        let colour = burst_colour(k);
        let size = 0.28 + 0.18 * hash21(vec2<f32>(k, 3.0)) + 0.12 * u.intensity;
        let tsec = age_beats * secs_per_beat;
        // Rocket: in the last half-beat before the burst it climbs from the ground.
        let climb = clamp(age_beats / 0.5, 0.0, 1.0);
        if age_beats < 0.5 {
            let rp = mix(vec2<f32>(centre.x, 0.9), centre, climb * climb * (3.0 - 2.0 * climb));
            sparks += vec3<f32>(1.0, 0.8, 0.5) * 0.00008 / (dot(p - rp, p - rp) + 0.00002);
            continue;
        }
        let a = (age_beats - 0.5) * secs_per_beat;      // seconds since the bang
        let fade = pow(clamp(1.0 - (age_beats - 0.5) / (LIFE - 0.5), 0.0, 1.0), 1.6);
        for (var i = 0; i < SPARKS; i++) {
            let fi = f32(i);
            let dir_a = fi / f32(SPARKS) * TAU + hash21(vec2<f32>(k, fi)) * 0.3;
            let speed = size * (0.75 + 0.5 * hash21(vec2<f32>(fi, k + 5.0)));
            // Fast out, slowing with drag, then drifting down under gravity.
            let travel = speed * (1.0 - exp(-a * 3.0));
            let sp = centre + vec2<f32>(cos(dir_a), sin(dir_a)) * travel + vec2<f32>(0.0, 0.09 * a * a);
            let twinkle = 0.6 + 0.4 * sin(tsec * 30.0 + fi * 3.1);
            let d2 = dot(p - sp, p - sp);
            sparks += colour * fade * twinkle * 0.00006 / (d2 + 0.000015);
        }
        // Flash of the burst itself, briefly lighting the sky.
        sky += colour * 0.06 * exp(-a * 6.0);
    }

    // Skyline silhouette with a scattering of lit windows.
    let bx = floor((p.x + 3.0) * 9.0);
    let tall = step(0.8, hash21(vec2<f32>(bx, 2.0)));      // the odd tower
    let bh = 0.72 - 0.16 * hash21(vec2<f32>(bx, 9.0)) - 0.22 * tall;
    let building = step(bh, p.y);
    let wc = floor(vec2<f32>(p.x * 70.0, p.y * 55.0));
    let win = step(0.86, hash21(wc)) * step(0.35, fract(p.x * 70.0)) * step(0.35, fract(p.y * 55.0));
    var col = sky + sparks * (1.0 - building);
    col = mix(col, vec3<f32>(0.008, 0.006, 0.016) + win * vec3<f32>(0.6, 0.45, 0.2) * 0.3, building);

    // Glittering trails: the previous frame lingers only where it is brighter
    // than the current sky, so the sky itself never builds up.
    let trail = prev(uncentred(p - vec2<f32>(0.0, -0.0015))) * 0.9;
    col = max(col, trail * (1.0 - building));
    return vec4<f32>(col, 1.0);
}
