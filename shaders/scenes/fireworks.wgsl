// Fireworks over a harbour skyline: scheduled bursts every two beats plus
// extra rockets that fire more often the harder the track drives, crackle
// riding the highs, a denser barrage on drops, and everything wobbling in the
// water below. Seasonal (New Year, Bonfire Night; see config.rs).

const SPARKS: i32 = 64;
const LIFE: f32 = 5.0;         // beats a burst stays visible

fn burst_colour(k: f32) -> vec3<f32> {
    let h = hash21(vec2<f32>(k, 17.0));
    if h < 0.2 { return vec3<f32>(1.0, 0.25, 0.15); }
    if h < 0.4 { return vec3<f32>(1.0, 0.8, 0.3); }
    if h < 0.6 { return vec3<f32>(0.3, 1.0, 0.45); }
    if h < 0.8 { return vec3<f32>(0.35, 0.5, 1.0); }
    return vec3<f32>(1.0, 0.4, 0.9);
}

// Sky + all live bursts at view point q. Split out so the water can reflect it.
fn sky_and_bursts(q: vec2<f32>, asp: f32, secs_per_beat: f32) -> vec3<f32> {
    var sky = mix(vec3<f32>(0.01, 0.01, 0.04), vec3<f32>(0.08, 0.04, 0.1), smoothstep(-0.2, 0.7, q.y));
    var sparks = vec3<f32>(0.0);
    let now = u.flow;
    // Every beat is a slot: even beats always fire, odd beats fire more often
    // the higher the intensity — so breakdowns stay sparse and drops barrage.
    for (var j = 0; j < 6; j++) {
        let k = floor(now) - f32(j);
        let parity = k - 2.0 * floor(k * 0.5);
        let extra = hash21(vec2<f32>(k, 91.0)) < u.intensity * 0.85;
        if parity > 0.5 && !extra {
            continue;
        }
        let age_beats = now - k;
        if age_beats > LIFE {
            continue;
        }
        let centre = vec2<f32>((hash21(vec2<f32>(k, 1.0)) - 0.5) * 1.4 * asp, -0.15 - 0.5 * hash21(vec2<f32>(k, 2.0)));
        let colour = burst_colour(k);
        // Kicks make the burst bigger; drops add a multi-shell bloom.
        let size = (0.28 + 0.18 * hash21(vec2<f32>(k, 3.0))) * (1.0 + 0.35 * u.kick + 0.15 * u.intensity);
        let tsec = age_beats * secs_per_beat;
        let climb = clamp(age_beats / 0.5, 0.0, 1.0);
        if age_beats < 0.5 {
            // Rocket climbing from the waterline in the last half-beat.
            let rp = mix(vec2<f32>(centre.x, 0.72), centre, climb * climb * (3.0 - 2.0 * climb));
            sparks += vec3<f32>(1.0, 0.8, 0.5) * 0.00008 / (dot(q - rp, q - rp) + 0.00002);
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
            // Crackle rides the high band.
            let twinkle = 0.5 + 0.5 * sin(tsec * (22.0 + 26.0 * spec(0.85)) + fi * 3.1);
            let d2 = dot(q - sp, q - sp);
            sparks += colour * fade * twinkle * 0.00006 / (d2 + 0.000015);
        }
        // Flash of the burst itself, briefly lighting the sky.
        sky += colour * 0.06 * exp(-a * 6.0) * (0.7 + 0.6 * u.energy);
    }
    return sky + sparks;
}

// Skyline silhouette height at x for a layer (0 = near, 1 = far).
fn roof(x: f32, layer: f32) -> f32 {
    let scale = mix(9.0, 6.0, layer);
    let bx = floor((x + 3.0) * scale);
    let tall = step(0.8 - 0.15 * layer, hash21(vec2<f32>(bx, 2.0 + layer * 13.0)));
    return 0.68 - 0.14 * hash21(vec2<f32>(bx, 9.0 + layer * 7.0)) - 0.2 * tall - 0.06 * layer;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);            // y grows downward
    let asp = aspect();
    let secs_per_beat = 60.0 / max(u.bpm, 60.0);
    let waterline = 0.8;

    var col = sky_and_bursts(p, asp, secs_per_beat);

    // Far skyline layer, hazier and lighter.
    let far = step(roof(p.x, 1.0), p.y);
    col = mix(col, vec3<f32>(0.03, 0.03, 0.07), far * step(p.y, waterline));

    // Near skyline: darker, with lit windows and beat-blinking tower beacons.
    let near_y = roof(p.x, 0.0);
    let near = step(near_y, p.y);
    let bx = floor((p.x + 3.0) * 9.0);
    let wc = floor(vec2<f32>(p.x * 70.0, p.y * 55.0));
    let win = step(0.86, hash21(wc)) * step(0.35, fract(p.x * 70.0)) * step(0.35, fract(p.y * 55.0));
    // Windows shimmer a little with the mids.
    let win_lit = win * vec3<f32>(0.6, 0.45, 0.2) * (0.25 + 0.3 * spec(0.5));
    let tower = step(0.8, hash21(vec2<f32>(bx, 2.0)));
    let beacon = tower * step(abs(p.y - near_y), 0.006) * step(abs(fract(p.x * 9.0) - 0.5), 0.06);
    let blink = 0.3 + 0.7 * beat_pulse(2.0);
    col = mix(col, vec3<f32>(0.008, 0.006, 0.016) + win_lit, near * step(p.y, waterline));
    col += vec3<f32>(1.0, 0.15, 0.1) * beacon * blink * near;

    // Harbour water: reflect last frame's sky through a wobble, so bursts and
    // the skyline shimmer in it. Darker with depth, fading to black.
    if p.y > waterline {
        let depth = p.y - waterline;
        let wob = (noise(vec2<f32>(p.x * 18.0, p.y * 70.0 - u.time * 1.2)) - 0.5) * 0.05 * depth * 10.0;
        let rq = vec2<f32>(p.x + wob * 0.4, 2.0 * waterline - p.y + wob * 0.1);
        var refl = prev(uncentred(rq));
        // Buildings on the far shore darken the reflection behind them.
        refl *= 1.0 - step(roof(rq.x, 1.0), rq.y) * 0.7;
        let water_col = vec3<f32>(0.004, 0.008, 0.02) + refl * 0.45;
        col = mix(col, water_col * (1.0 - depth * 0.6), smoothstep(0.0, 0.005, depth));
    }

    // Glittering trails: the previous frame lingers only where it is brighter
    // than the current sky, so the sky itself never builds up.
    let trail = prev(uncentred(p - vec2<f32>(0.0, -0.0015))) * 0.9;
    col = max(col, trail * step(p.y, waterline));
    return vec4<f32>(col, 1.0);
}
