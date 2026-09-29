// @heavy — 2026 tier. @bloom 0.8 @tonemap agx
// From a wet rooftop above a rain-soaked megacity at night: towers of lit
// windows march to the horizon under a low cloud deck glowing with the
// city's light, giant LED billboards run the spectrum, and police
// helicopters sweep searchlights through the rain.
// The city is traversed with a 2D grid DDA over city blocks (exact ray-box
// test per block, ≤72 blocks) — far cheaper than marching an SDF out to the
// horizon. Searchlights are the analytic Gaussian-tube beams.
// The camera pans slowly on the tempo clock. Window blocks ripple through
// the spectrum, billboards show the analyser, searchlights follow the bars.

const S: f32 = 14.0;           // block size

struct Hit {
    t: f32,
    n: vec3<f32>,
    cell: vec2<f32>,
    h: f32,
};

fn block(c: vec2<f32>) -> vec4<f32> {
    // (centre x, centre z, half-size, height); height 0 = empty lot.
    let h = hash22(c * 1.31 + 0.7);
    let ctr = (c + 0.5) * S + (h - 0.5) * S * 0.12;
    let half = S * (0.3 + 0.12 * h.y);
    let downtown = exp(-length(ctr - vec2<f32>(0.0, 300.0)) / 260.0);
    let n = tnoise(vec3<f32>(c * 0.07, 0.3)).b;
    var hgt = 8.0 + pow(h.x, 3.0) * 50.0 + pow(hash21(c + 5.5), 8.0) * 160.0 + downtown * 180.0 * n * n;
    // Empty lots — and a clearing round our own tower at the origin.
    if hash21(c + 9.1) < 0.08 || length(ctr - vec2<f32>(3.0, -20.0)) < 40.0 {
        hgt = 0.0;
    }
    return vec4<f32>(ctr, half, hgt);
}

fn trace_city(ro: vec3<f32>, rd: vec3<f32>) -> Hit {
    var res: Hit;
    res.t = -1.0;
    var cell = floor(ro.xz / S);
    let stp = sign(rd.xz);
    let inv = 1.0 / max(abs(rd.xz), vec2<f32>(1e-5));
    var tmax = ((cell + max(stp, vec2<f32>(0.0))) * S - ro.xz) / rd.xz;
    let tdelta = S * inv;
    for (var i = 0; i < 72; i++) {
        let b = block(cell);
        if b.w > 0.0 {
            let c3 = vec3<f32>(b.x, b.w * 0.5, b.y);
            let h3 = vec3<f32>(b.z, b.w * 0.5, b.z);
            let ib = 1.0 / rd;
            let t0 = (c3 - h3 - ro) * ib;
            let t1 = (c3 + h3 - ro) * ib;
            let tmin = min(t0, t1);
            let tmx = max(t0, t1);
            let tn = max(max(tmin.x, tmin.y), tmin.z);
            let tf = min(min(tmx.x, tmx.y), tmx.z);
            if tn < tf && tn > 0.0 {
                res.t = tn;
                res.cell = cell;
                res.h = b.w;
                if tn == tmin.x {
                    res.n = vec3<f32>(-sign(rd.x), 0.0, 0.0);
                } else if tn == tmin.y {
                    res.n = vec3<f32>(0.0, 1.0, 0.0);
                } else {
                    res.n = vec3<f32>(0.0, 0.0, -sign(rd.z));
                }
                return res;
            }
        }
        if tmax.x < tmax.y {
            cell.x += stp.x;
            tmax.x += tdelta.x;
        } else {
            cell.y += stp.y;
            tmax.y += tdelta.y;
        }
        if min(tmax.x, tmax.y) > 1400.0 {
            break;
        }
    }
    return res;
}

fn sky(rd: vec3<f32>) -> vec3<f32> {
    // Low cloud deck lit from below by the city: orange-violet, darker up.
    let h = max(rd.y, 0.0);
    let cl = tnoise(vec3<f32>(rd.xz / max(rd.y + 0.15, 0.05) * 0.03, u.time * 0.003)).r;
    let base = mix(vec3<f32>(0.07, 0.035, 0.045), vec3<f32>(0.008, 0.007, 0.016), smoothstep(0.0, 0.4, h));
    return base * (0.6 + 1.2 * cl);
}

fn facade(p: vec3<f32>, n: vec3<f32>, cell: vec2<f32>, hgt: f32) -> vec3<f32> {
    let u_ = select(p.x, p.z, abs(n.x) > 0.5);
    let g = vec2<f32>(u_ * 0.75, p.y * 0.5);
    let wc = floor(g);
    let f = fract(g);
    let pane = smoothstep(0.42, 0.34, abs(f.x - 0.5)) * smoothstep(0.4, 0.3, abs(f.y - 0.5));
    let h = hash21(wc + cell * 7.3 + n.xz * 3.0);
    var lit = step(0.78, h);
    // Spectrum ripple: bands of windows light up in waves across the city.
    let band = fract(cell.x * 0.13 + cell.y * 0.07);
    let wave = smoothstep(0.7, 1.0, sin(length(cell) * 0.4 - u.flow * 0.5) * 0.5 + 0.5);
    lit = max(lit, step(0.35, h) * wave * spec(band * 0.8) * 1.5 * u.intensity);
    let warm = mix(vec3<f32>(1.0, 0.72, 0.4), vec3<f32>(0.6, 0.8, 1.0), step(0.85, hash21(wc + 3.3)));
    var c = vec3<f32>(0.012, 0.012, 0.016) + warm * lit * pane * 0.9 * (0.4 + 0.6 * hash21(wc + 1.7));
    // Billboards: a big LED screen near the top of some towers.
    if hash21(cell + 4.4) < 0.22 && abs(n.y) < 0.5 && hgt > 40.0 {
        let top = hgt - 4.0;
        let bb = step(top - 24.0, p.y) * step(p.y, top);
        let bw = fract(u_ / S + 0.5);
        if bb > 0.5 && abs(bw - 0.5) < 0.42 {
            let x = abs(bw - 0.5) / 0.42;
            let col_id = floor(x * 12.0);
            let lvl = spec(col_id / 12.0 * 0.8);
            let yy = (p.y - (top - 24.0)) / 24.0;
            let bar = step(yy, lvl * 0.95) * smoothstep(0.45, 0.35, abs(fract(x * 12.0) - 0.5));
            let pal = palette(col_id / 12.0 * 0.7 + hash21(cell) * 0.5);
            c = pal * (0.08 + bar * 2.2) * (0.6 + 0.8 * u.intensity);
        }
    }
    return c;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let yaw = sin(u.flow * 0.012) * 0.45 + u.seed * 0.1;
    let ro = vec3<f32>(3.0, 150.0, -20.0);
    let fwd = vec3<f32>(sin(yaw), -0.2 + sin(u.flow * 0.009) * 0.05, cos(yaw));
    let rd = cam_ray(p, ro, ro + fwd, 0.0, 1.45);
    let drive = 0.5 + 0.8 * u.intensity;

    var col = sky(rd);
    var t_hit = 2000.0;
    // Our rooftop's parapet ledge, just in front of and below the camera.
    let fwd_xz = normalize(vec3<f32>(fwd.x, 0.0, fwd.z));
    let lc = ro + fwd_xz * 1.4 + vec3<f32>(0.0, -1.55, 0.0);
    let lrd = vec3<f32>(dot(rd, vec3<f32>(fwd_xz.z, 0.0, -fwd_xz.x)), rd.y, dot(rd, fwd_xz));
    let lro = vec3<f32>(dot(ro - lc, vec3<f32>(fwd_xz.z, 0.0, -fwd_xz.x)), ro.y - lc.y, dot(ro - lc, fwd_xz));
    let lh = vec3<f32>(60.0, 0.12, 0.25);
    let lt0 = (-lh - lro) / lrd;
    let lt1 = (lh - lro) / lrd;
    let ltn = max(max(min(lt0.x, lt1.x), min(lt0.y, lt1.y)), min(lt0.z, lt1.z));
    let ltf = min(min(max(lt0.x, lt1.x), max(lt0.y, lt1.y)), max(lt0.z, lt1.z));
    if ltn < ltf && ltn > 0.0 {
        t_hit = ltn;
        let lp = lro + lrd * ltn;
        // Wet concrete coping: the top face mirrors the sky glow.
        let top = step(lh.y - 0.01, lp.y);
        let wet = smoothstep(0.4, 0.6, tnoise(vec3<f32>(lp.xz * 1.5, 0.1)).g);
        col = vec3<f32>(0.01) + top * sky(vec3<f32>(rd.x, -rd.y, rd.z)) * mix(0.15, 0.5, wet);
    } else {
        let h = trace_city(ro, rd);
        if h.t > 0.0 {
            t_hit = h.t;
            let hp = ro + rd * h.t;
            col = facade(hp, h.n, h.cell, h.h);
            if h.n.y > 0.5 {
                // Rooftops: dark, a few aircraft-warning lights.
                col = vec3<f32>(0.01) + vec3<f32>(1.0, 0.1, 0.05) * step(0.9, hash21(h.cell + 2.0)) * smoothstep(1.2, 0.0, length(hp.xz - block(h.cell).xy)) * (0.5 + 0.5 * sin(u.time * 3.0 + h.cell.x));
            }
            // Rain haze + city glow with distance.
            let haze = 1.0 - exp(-h.t * 0.0022);
            col = mix(col, vec3<f32>(0.05, 0.028, 0.04) * drive, haze);
        }
    }

    // Helicopter searchlights: two beams sweeping down into the streets.
    for (var k = 0; k < 2; k++) {
        let fk = f32(k);
        // Placed relative to the view heading (the yaw varies per cut).
        let hf = normalize(vec3<f32>(sin(yaw), 0.0, cos(yaw)));
        let hr = vec3<f32>(hf.z, 0.0, -hf.x);
        let bp = ro + hf * (200.0 + fk * 120.0 + cos(u.flow * 0.017 + fk) * 40.0)
            + hr * (sin(u.flow * 0.02 + fk * 3.0) * 90.0 + fk * 70.0 - 35.0) + vec3<f32>(0.0, 110.0 + fk * 20.0, 0.0);
        let bd = normalize(vec3<f32>(sin(u.flow * 0.0625 * PI + fk * 2.0) * 0.35, -1.0, cos(u.flow * 0.05 + fk) * 0.3));
        let w0 = ro - bp;
        let b = dot(rd, bd);
        let dd = dot(rd, w0);
        let e = dot(bd, w0);
        let den = max(1.0 - b * b, 1e-4);
        let tc = (b * e - dd) / den;
        let sc = clamp((e - b * dd) / den, 0.0, 300.0);
        let dist = length(ro + rd * tc - (bp + bd * sc));
        let rr = 0.8 + sc * 0.035;
        let vis = step(0.0, tc) * smoothstep(0.0, 5.0, t_hit - tc);
        let lat = exp(-dist * dist / (rr * rr)) / (1.7725 * rr * max(sqrt(den), 0.15));
        let pw = (0.6 + 1.2 * spec(0.1 + fk * 0.4)) * drive;
        col += vec3<f32>(0.85, 0.9, 1.0) * lat * pw * 2.5 * vis;
        // The helicopter's light itself.
        let lv = bp - ro;
        let lt = dot(lv, rd);
        let ld = length(lv - rd * lt) / max(lt, 1.0);
        col += vec3<f32>(1.0) * smoothstep(0.004, 0.0, ld) * 3.0 * step(0.0, lt);
        col += vec3<f32>(1.0, 0.1, 0.1) * smoothstep(0.003, 0.0, length(lv + vec3<f32>(3.0, 0.5, 0.0) - rd * dot(lv, rd)) / max(lt, 1.0)) * step(0.5, fract(u.time * 1.5 + fk));
    }

    // Rain streaks in two parallax layers.
    for (var i = 0; i < 2; i++) {
        let fi = f32(i) + 1.0;
        let cells = 120.0 * fi;
        let cx = floor((p.x + p.y * 0.06) * cells);
        let ry = fract(p.y * 1.5 * fi - u.time * 1.6 / fi + hash21(vec2<f32>(cx, fi * 4.0)));
        let dash = step(0.9, hash21(vec2<f32>(cx, fi))) * smoothstep(0.06, 0.0, abs(ry - 0.5)) * 0.05 / fi;
        col += vec3<f32>(0.6, 0.55, 0.6) * dash * (0.4 + 0.6 * u.energy);
    }
    col += (bluen(in.pos.xy) - 0.5) * 0.003;
    return vec4<f32>(col, 1.0);
}
