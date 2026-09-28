// Shared by every scene and by present.wgsl (prepended before compiling).
// Must match `Uniforms` in src/render.rs.

struct U {
    time: f32,
    dt: f32,
    res_x: f32,
    res_y: f32,
    bass: f32,
    mid: f32,
    high: f32,
    energy: f32,
    onset: f32,
    kick: f32,
    beat: f32,        // beat position; fract(beat) == beat_phase
    beat_phase: f32,
    bar_phase: f32,   // 0..1 through the bar
    bpm: f32,
    build: f32,       // >0 building, <0 dropping away
    scene_time: f32,
    intensity: f32,   // overall drive 0..1
    hue: f32,         // palette offset, changes on each cut
    seed: f32,
    flash: f32,       // 1 on a cut, decays
    flow: f32,        // smooth beat clock for motion (never jumps); use for travel
    master: f32,      // overall brightness (blackout fades to 0)
    fx: f32,          // post effect mode (see present.wgsl / Fx in config.rs)
    fx_amt: f32,      // post effect strength 0..1 (uv blend in present.wgsl)
    spectrum: array<vec4<f32>, 8>,
    wave: array<vec4<f32>, 16>,   // 64 time-domain samples, -1..1 (scope scenes)
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var prev_tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
// 256×1 gradient LUT — the user-selected global palette (WLED-style).
@group(0) @binding(3) var pal_tex: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle.
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VsOut;
    o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

const PI: f32 = 3.14159265;
const TAU: f32 = 6.28318531;

fn aspect() -> f32 { return u.res_x / u.res_y; }

// uv (0..1) -> centred coords, y in -1..1, x scaled by aspect.
fn centred(uv: vec2<f32>) -> vec2<f32> {
    return (uv - 0.5) * vec2<f32>(aspect(), 1.0) * 2.0;
}

fn uncentred(p: vec2<f32>) -> vec2<f32> {
    return p / vec2<f32>(aspect(), 1.0) * 0.5 + 0.5;
}

fn prev(uv: vec2<f32>) -> vec3<f32> {
    return finite(textureSampleLevel(prev_tex, samp, uv, 0.0).rgb);
}

// Replace NaN/Inf with 0. One bad pixel would otherwise live forever in the
// feedback loop and get smeared across the screen by zooming scenes.
fn finite(c: vec3<f32>) -> vec3<f32> {
    let bad = (c != c) | (abs(c) > vec3<f32>(1e6));
    return select(c, vec3<f32>(0.0), bad);
}

// atan2 that is defined at the origin (GPU atan2(0, 0) is undefined).
fn angle(p: vec2<f32>) -> f32 {
    return atan2(p.y, p.x + 1e-9);
}

// Spectrum 0..1 across 32 log-spaced bins; x in 0..1.
fn spec(x: f32) -> f32 {
    let f = clamp(x, 0.0, 0.999) * 32.0;
    let i = u32(f);
    let a = u.spectrum[i / 4u][i % 4u];
    let j = min(i + 1u, 31u);
    let b = u.spectrum[j / 4u][j % 4u];
    return mix(a, b, fract(f));
}

// Time-domain audio sample: -1..1 across 64 samples of the last ~21 ms;
// x in 0..1 maps left (older) to right (newest). The real waveform.
fn wave(x: f32) -> f32 {
    let f = clamp(x, 0.0, 0.999) * 64.0;
    let i = u32(f);
    let a = u.wave[i / 4u][i % 4u];
    let j = min(i + 1u, 63u);
    let b = u.wave[j / 4u][j % 4u];
    return mix(a, b, fract(f));
}

// Sharp pulse at each beat, decaying through it.
fn beat_pulse(sharpness: f32) -> f32 { return exp(-u.beat_phase * sharpness); }

fn rot(a: f32) -> mat2x2<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat2x2<f32>(c, s, -s, c);
}

// Global palette LUT, shifted by the director's hue. The sampler is
// MirrorRepeat, so t outside 0..1 ping-pongs back through the gradient —
// seamless even for non-cyclic palettes.
fn palette(t: f32) -> vec3<f32> {
    return textureSampleLevel(pal_tex, samp, vec2<f32>(t + u.hue, 0.5), 0.0).rgb;
}

// Integer PCG hash: stable at any coordinate magnitude (float hashes go
// blocky once inputs get large).
fn pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}

fn hash21(p: vec2<f32>) -> f32 {
    let i = vec2<i32>(floor(p));
    return f32(pcg(u32(i.x) + pcg(u32(i.y)))) / 4294967295.0;
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    let n = hash21(p);
    return vec2<f32>(n, hash21(p + vec2<f32>(n * 7919.0, 1.0)));
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash21(i), hash21(i + vec2<f32>(1.0, 0.0)), w.x),
        mix(hash21(i + vec2<f32>(0.0, 1.0)), hash21(i + vec2<f32>(1.0, 1.0)), w.x),
        w.y
    );
}

fn fbm(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var v = 0.0;
    var a = 0.5;
    for (var i = 0; i < 5; i++) {
        v += a * noise(p);
        p = rot(0.5) * p * 2.02 + 17.0;
        a *= 0.5;
    }
    return v;
}
