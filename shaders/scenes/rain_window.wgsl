// @bloom 0.6 @tonemap agx
// Looking out through a rain-streaked window at night: the city beyond is a
// soft field of out-of-focus bokeh lights, and every raindrop on the glass
// is a tiny lens holding a sharp, upside-down view of it. Drops slide down
// in stop-start runs, leaving trails of droplets.
// Pure screen-space — cheap enough for any GPU (no @heavy).
// The bokeh lights pulse with their spectrum band and swell on the kick;
// more drops run the harder the track drives (density, never speed).

// City lights. `sharp` 0 = heavily defocused, 1 = nearly in focus (seen
// through a drop). Three layers of bokeh discs of different sizes.
fn city(uv: vec2<f32>, sharp: f32) -> vec3<f32> {
    var c = mix(vec3<f32>(0.006, 0.005, 0.012), vec3<f32>(0.03, 0.018, 0.03), smoothstep(0.3, 1.0, uv.y));
    for (var l = 0; l < 3; l++) {
        let fl = f32(l);
        let cells = 6.0 + fl * 5.0;
        let g = uv * vec2<f32>(cells * aspect(), cells) + vec2<f32>(fl * 3.7, fl * 1.3);
        let base = floor(g);
        for (var j = 0; j < 9; j++) {
            let cell = base + vec2<f32>(f32(j % 3) - 1.0, f32(j / 3) - 1.0);
            let h = hash22(cell + fl * 17.0);
            // Lights live mostly in the lower two-thirds (the city).
            let cy = (cell.y + 0.5) / cells;
            if h.x > 0.12 + 0.3 * smoothstep(0.25, 0.85, cy) {
                continue;
            }
            let ctr = cell + 0.5 + (hash22(cell + 5.0) - 0.5) * 0.9;
            let band = fract(h.y * 7.3);
            let rad = mix(0.75 - fl * 0.18, 0.12, sharp) * (0.7 + 0.6 * h.y) * (0.85 + 0.35 * u.pres4.x + 0.12 * u.hits4.x);
            let d = length(g - ctr);
            let edge = mix(0.05, 0.03, sharp);
            let disc = smoothstep(rad, rad - edge, d);
            // Defocused highlights are brighter toward the rim.
            let rim = mix(0.85 + 0.4 * smoothstep(rad * 0.5, rad, d), 1.0, sharp);
            let warm = mix(vec3<f32>(1.0, 0.62, 0.28), palette(band), step(0.5, h.y));
            let lvl = (0.2 + 1.1 * spec(band * 0.85)) * (0.5 + 0.7 * u.intensity);
            c += warm * disc * rim * lvl * mix(0.07, 0.4, sharp) / (1.0 + fl * 0.4);
        }
    }
    return c;
}

// A layer of sliding drops: returns (refraction normal .xy, coverage .z).
// Each column's drop slides down in stop-start runs, leaving a trail.
fn drops(uv: vec2<f32>, t: f32, scale: f32, density: f32) -> vec3<f32> {
    let a = vec2<f32>(6.0, 1.0);
    var st = uv * scale * a;
    let cid = floor(st.x);
    let colh = hash21(vec2<f32>(cid, scale));
    st.y += t * 0.05 * (0.5 + colh) + colh * 7.0;
    let id = floor(st);
    let n = hash22(id + scale * 3.1);
    if n.x > density {
        return vec3<f32>(0.0);
    }
    let f = fract(st) - vec2<f32>(0.5, 0.0);
    let tt = fract(t * 0.12 + n.y);
    let y = smoothstep(0.0, 0.85, tt) * 0.75 + 0.12;
    let x = (n.y - 0.5) * 0.5 + sin(tt * 16.0 + n.x * 7.0) * 0.04;
    // Aspect-correct offset (units: cell height).
    let k = aspect() * a.y / a.x;
    let q = vec2<f32>((f.x - x) * k, (f.y - y));
    let r = 0.05 + n.y * 0.025;
    let dd = length(q * vec2<f32>(1.0, 0.85));
    let main_d = smoothstep(r, r * 0.8, dd);
    // Trail beads behind (above) the drop.
    let ty = fract((f.y - y) * 9.0) - 0.5;
    let tq = vec2<f32>((f.x - x) * k, ty / 9.0);
    let tr = r * 0.3;
    let trail = smoothstep(tr, tr * 0.7, length(tq)) * step(f.y, y - 0.02) * smoothstep(0.0, 0.3, f.y);
    let cov = max(main_d, trail);
    let nrm = select(tq / tr, q / r, main_d > trail);
    return vec3<f32>(nrm * cov, cov);
}

// Static beads that cling to the glass and twinkle out.
fn beads(uv: vec2<f32>, t: f32) -> vec3<f32> {
    let g = uv * vec2<f32>(40.0 * aspect(), 40.0);
    let id = floor(g);
    let h = hash22(id);
    let f = fract(g) - 0.5 - (h - 0.5) * 0.6;
    let life = fract(t * 0.1 + h.x * 7.0);
    let r = 0.25 * smoothstep(0.0, 0.1, life) * smoothstep(1.0, 0.7, life) * step(0.82, h.y);
    let cov = smoothstep(r, r * 0.5, length(f));
    return vec3<f32>(f * cov * 0.6, cov);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    // Energy: drops slide faster as the track drives (smooth clock).
    let t = u.clock4.x * 0.5;
    let density = 0.25 + 0.4 * u.energy;
    let d1 = drops(uv, t, 2.0, density);
    let d2 = drops(uv * 1.4 + 0.3, t * 1.1, 3.3, density);
    let bd = beads(uv, t);
    var cov = d1.z;
    var nrm = d1.xy;
    if d2.z > cov {
        cov = d2.z;
        nrm = d2.xy;
    }
    if bd.z > cov {
        cov = bd.z;
        nrm = bd.xy * 2.0;
    }
    // Through the glass: defocused city. Through a drop: a sharper,
    // flipped, magnified view (drops are little lenses) — they pick up the
    // lights around them, so they read brighter than the glass.
    let blurred = city(uv, 0.0);
    let through = city(uv - nrm * 0.06 + vec2<f32>(0.0, 0.05), 0.6);
    var col = mix(blurred, through * 1.4 + 0.01, cov);
    // Dark refraction rim + a specular glint up-left on each drop.
    let rim = smoothstep(0.35, 0.9, length(nrm)) * cov;
    col *= 1.0 - 0.3 * rim;
    col += vec3<f32>(0.7, 0.75, 0.9) * smoothstep(0.6, 0.9, dot(nrm, normalize(vec2<f32>(-0.6, -0.8)))) * cov * 0.25;
    // Faint condensation on the pane, thicker at the bottom.
    col += vec3<f32>(0.015, 0.016, 0.022) * smoothstep(0.4, 1.0, uv.y) * (1.0 - cov);
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
