// @title Camera Kaleidoscope
// @bloom 0.35
// @no-dancer
// Camera Kaleidoscope: the webcam (Settings > Camera) folded into an
// eight-way kaleidoscope, a mandala made of the DJ. It zooms in and out with
// the bass presence and its petals open on the kick (shape), it turns on the
// energy clock (motion), winding one way, then the other, every eight bars
// (direction), and the camera's colours are lifted toward the palette. Only
// in rotation while a camera is sending frames.

// Its controls (Perform > scene controls): u.roto4 = segments, zoom, spin,
// palette.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    let spin = (u.clock4.x * 0.03 + 0.6 * sin(TAU * u.flow / 32.0)) * u.roto4.z;
    let r = length(p);
    let seg = TAU / clamp(round(u.roto4.x), 3.0, 12.0);
    var a = angle(p) + spin;
    a = abs(fract(a / seg) * seg - seg * 0.5);
    // The zoom breathes with the bass presence; the kick opens the petals.
    let zoom = (0.42 + 0.14 * u.pres4.x + 0.05 * kick_swell()) / max(u.roto4.y, 0.1);
    let q = vec2<f32>(cos(a), sin(a)) * r * zoom;
    // Into the camera frame, around its centre (mirrored like a selfie).
    let uv = vec2<f32>(0.5 - q.x * 9.0 / 16.0, 0.5 + q.y);
    var col = untonemap(cam(uv));
    let pal = palette(fract(r * 0.4 + u.clock4.x * 0.01));
    col = mix(col, pal * pal * dot(col, vec3<f32>(0.4, 0.4, 0.2)) * 2.0, clamp(u.roto4.w, 0.0, 1.0));
    // A soft dark edge round the mandala.
    col *= 1.0 - smoothstep(0.9, 1.6, r);
    return vec4<f32>(finite(col), 1.0);
}
