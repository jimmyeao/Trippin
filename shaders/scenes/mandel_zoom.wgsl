// Mandelbrot deep-zoom: the classic set, slowly diving into a seam while
// the iteration glow rides the energy. Colours come from the palette so it
// sits in the same look family as everything else.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);

    // Dive target: the seahorse valley, zooming in exponentially.
    let zoom = pow(0.5, fract(u.flow * 0.015)) * 0.6;
    let c = vec2<f32>(-0.7436, 0.1318) + p * zoom * vec2<f32>(1.0, 1.0);

    var z = vec2<f32>(0.0);
    var it = 0.0;
    let max_it = 48.0;
    for (var i = 0; i < 48; i++) {
        z = vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
        if dot(z, z) > 4.0 {
            it = f32(i) + 1.0 - log2(log2(dot(z, z)) * 0.5 + 1e-6);
            break;
        }
        it = f32(i);
    }

    var col: vec3<f32>;
    if it >= max_it {
        // Inside the set: dark with a faint heartbeat.
        col = vec3<f32>(0.01, 0.005, 0.02) * (0.5 + u.kick * 0.5);
    } else {
        let t = it / max_it;
        col = palette(t * (2.0 + u.energy * 3.0) + u.hue) * (0.25 + u.energy * 0.9);
        // Bright edge filament on the boundary.
        col += palette(0.5 + u.hue) * exp(-abs(t - 0.5) * 30.0) * beat_pulse(4.0);
    }
    col += prev(in.uv) * 0.12;
    return vec4<f32>(col, 1.0);
}
