// @sim 150000 @bloom 0.5 @tonemap agx
// @title Now Playing
// Now Playing: the track that's playing (artist - title, or the DJ name when
// nothing is detected) spelt out in 150,000 points of light. When the track
// changes, the points fly from the old title into the new one. The letters
// ripple with the bass (shape), each kick blows them apart and they spring
// back together, the text swings gently on the phrase (motion), and at the
// end of each phrase it can melt into a swirl and re-form on the one. A
// starfield of loose points drifts behind. Configurable in the scene
// controls (camera::NOW_PLAYING): which text, size, height, scatter,
// dissolve, colour, wave and backdrop.

struct POut {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) col: vec3<f32>,
};

// The scene controls (camera.rs NOW_PLAYING, in order; x of roto4 is the
// text choice, which main.rs applies to the title texture).
fn c_size() -> f32 { return clamp(u.roto4.y, 0.4, 1.6); }
fn c_height() -> f32 { return clamp(u.roto4.z, -0.7, 0.7); }
fn c_scatter() -> f32 { return clamp(u.roto4.w, 0.0, 2.0); }
fn c_dissolve() -> f32 { return clamp(u.roto5.x, 0.0, 1.0); }
fn c_colour() -> f32 { return clamp(u.roto5.y, 0.0, 1.0); }
fn c_wave() -> f32 { return clamp(u.roto5.z, 0.0, 2.0); }
fn c_backdrop() -> f32 { return clamp(u.roto5.w, 0.0, 1.0); }

// World units per title-texture pixel: the line fits 4 wide and its
// 160-pixel row at most 1 tall, both scaled by Size.
fn px_scale() -> f32 {
    let wpx = max(u.look4.w, 0.0005) * 2048.0;
    return min(4.0 / wpx, 1.0 / 160.0) * c_size();
}

// A point in the title texture (uv 0..1 over the text's own box) in world
// space, before the ripple.
fn text_world(uv: vec2<f32>) -> vec3<f32> {
    let s = px_scale();
    let wpx = max(u.look4.w, 0.0005) * 2048.0;
    return vec3<f32>((uv.x - 0.5) * wpx * s, (0.5 - uv.y) * 160.0 * s + c_height(), 0.0);
}

// How far into the phrase-end dissolve we are (0 = readable): a melt over
// the last bars of each eight-bar phrase, re-formed by the one.
fn dissolve() -> f32 {
    let bar = u32(max(floor((u.beat - u.bar_phase * 4.0) / 4.0 + 0.5), 0.0));
    let pb = f32(bar % 8u) * 4.0 + u.bar_phase * 4.0;
    return smoothstep(26.0, 29.0, pb) * (1.0 - smoothstep(30.5, 32.0, pb)) * c_dissolve();
}

// The letters' home for a texture point: ripples ride the bass.
fn home_of(uv: vec2<f32>) -> vec3<f32> {
    var p = text_world(uv);
    let w = c_wave();
    p.y += w * 0.05 * sin(p.x * 2.6 - u.clock4.x * 0.9) * (0.35 + u.pres4.x);
    p.z += w * 0.18 * kick_swell() * cos(p.x * 1.7 + u.clock4.y * 0.3);
    return p;
}

fn is_backdrop(i: u32) -> bool {
    return sim_rand(i, 9u) < 0.3 * c_backdrop();
}

fn back_spawn(i: u32, far: bool) -> vec3<f32> {
    let z = select(sim_rand_f(i, 2u) * 4.0, 3.8 + sim_rand_f(i, 2u) * 0.4, far);
    return vec3<f32>((sim_rand_f(i, 0u) * 2.0 - 1.0) * 4.5, (sim_rand_f(i, 1u) * 2.0 - 1.0) * 2.5, z);
}

// Try a few random texture points; return the first inside a letter, or -1.
fn find_home(i: u32, tries: u32) -> vec2<f32> {
    for (var k = 0u; k < tries; k++) {
        let uv = vec2<f32>(sim_rand_f(i, 10u + 2u * k), sim_rand_f(i, 11u + 2u * k));
        if title_mask(uv) > 0.5 {
            return uv;
        }
    }
    return vec2<f32>(-1.0);
}

fn sim_spawn(i: u32) -> Particle {
    if is_backdrop(i) {
        return Particle(vec4<f32>(back_spawn(i, false), -1.0), vec4<f32>(0.0, 0.0, 0.0, -1.0));
    }
    let uv = find_home(i, 12u);
    var x = back_spawn(i, false);
    if uv.x >= 0.0 {
        x = home_of(uv);
    }
    return Particle(vec4<f32>(x, uv.x), vec4<f32>(0.0, 0.0, 0.0, uv.y));
}

// pos.w / vel.w hold the point's place in the title texture (-1: none yet,
// or a backdrop point). A new title strands points outside the letters:
// they look again each frame, and fly to where they land.
fn sim_step(i: u32, p: Particle, dt: f32) -> Particle {
    var x = p.pos.xyz;
    var v = p.vel.xyz;
    var uv = vec2<f32>(p.pos.w, p.vel.w);
    let calm = u.calm;
    let t = u.clock4.z * 0.02;
    if is_backdrop(i) {
        // Drift on the swirl, outward from the middle on kicks.
        let flow = sim_curl(x * 0.3 + vec3<f32>(t, 0.0, -t)) * (0.12 + 0.3 * u.lvl4.y);
        v += (flow - v) * 1.5 * dt;
        let r = vec3<f32>(x.xy, 0.0);
        v += r / max(length(r), 0.2) * simc.onset.x * 0.35 * (1.0 - 0.85 * calm);
        x += v * dt;
        if abs(x.x) > 5.0 || abs(x.y) > 3.0 || x.z < -1.5 || x.z > 4.5 {
            x = back_spawn(i, true);
            v = vec3<f32>(0.0);
        }
        return Particle(vec4<f32>(x, -1.0), vec4<f32>(v, -1.0));
    }
    if uv.x < 0.0 || title_mask(uv) < 0.35 {
        uv = find_home(i, 3u);
    }
    if uv.x < 0.0 {
        // No letter yet: float with the swirl until one is found.
        v += (sim_curl(x * 0.4 + vec3<f32>(t)) * 0.3 - v) * 2.0 * dt;
        x += v * dt;
        return Particle(vec4<f32>(x, -1.0), vec4<f32>(v, -1.0));
    }
    let h = home_of(uv);
    let d = dissolve();
    let k = (90.0 + 50.0 * u.pres4.x) * (1.0 - 0.45 * calm) * (1.0 - 0.97 * min(d * 1.6, 1.0));
    var acc = (h - x) * k - v * (1.6 * sqrt(k) + 0.8 * d);
    acc += sim_curl(h * 0.7 + vec3<f32>(t, -0.4 * t, 0.7 * t)) * (0.04 + 0.3 * u.lvl4.y + 0.8 * calm + 6.0 * d);
    v += acc * dt;
    // The kick: out from the line's middle, with a scatter of its own.
    let c = vec3<f32>(0.0, c_height(), 0.0);
    let rr = h - c;
    let j = vec3<f32>(sim_rand(i, 3u), sim_rand(i, 4u), sim_rand(i, 5u)) - 0.5;
    let push = (normalize(rr + vec3<f32>(0.0, 0.0, 0.3)) + j * 1.2) * simc.onset.x * 1.6 * c_scatter();
    v += push * (1.0 - 0.85 * calm);
    x += v * dt;
    return Particle(vec4<f32>(x, uv.x), vec4<f32>(v, uv.y));
}

fn view(w_in: vec3<f32>) -> vec3<f32> {
    var w = w_in;
    // A gentle swing on the phrase: turns back and forth, never round.
    let ya = 0.22 * sin(TAU * u.flow / 64.0);
    let wy = rot(ya) * w.xz;
    w = vec3<f32>(wy.x, w.y, wy.y);
    let wp = rot(0.06 * sin(TAU * u.flow / 32.0)) * w.yz;
    return vec3<f32>(w.x, wp.x, wp.y + 3.6);
}

@vertex
fn vs_particles(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
    var o: POut;
    let pp = sim_p[ii];
    let w = pp.pos.xyz;
    let a = view(w);
    let uv = vec2<f32>(pp.pos.w, pp.vel.w);
    if uv.x < 0.0 {
        // The backdrop, and points still looking for a letter.
        let s = sim_quad(a, view(w - pp.vel.xyz * 0.05), vi, 0.012, 2.4);
        o.pos = s.pos;
        o.q = s.q;
        let c = palette(0.6 + 0.3 * sim_rand(ii, 6u));
        o.col = mix(vec3<f32>(0.6), c * c, c_colour()) * 0.9 * s.dim * clamp(6.5 - a.z, 0.15, 1.0);
        return o;
    }
    let s = sim_quad(a, view(w - pp.vel.xyz * 0.03), vi, 0.0032 * (0.7 + 0.3 * c_size()), 2.4);
    o.pos = s.pos;
    o.q = s.q;
    // Loose points (kicked, dissolving) take the palette's later colours.
    let h = home_of(uv);
    let loose = min(length(w - h) * 2.0, 1.0);
    let c = palette(0.05 + 0.5 * uv.x + 0.25 * loose);
    let col = mix(vec3<f32>(1.0, 0.97, 0.94), c * c * 1.4, c_colour());
    let b = (0.13 + 0.12 * loose) * clamp(5.0 - a.z, 0.4, 1.4);
    o.col = col * b * s.dim;
    return o;
}

@fragment
fn fs_particles(in: POut) -> @location(0) vec4<f32> {
    let r2 = dot(in.q, in.q);
    if r2 > 1.0 {
        discard;
    }
    let g = 1.0 - r2;
    return vec4<f32>(in.col * g * g, 0.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = prev(in.uv) * 0.45;
    col += vec3<f32>(0.004, 0.004, 0.01) * (1.0 - 0.45 * length(p));
    return vec4<f32>(finite(col), 1.0);
}
