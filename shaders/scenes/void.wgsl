// Void: pure black. The timeline treats this as "scenes off" — the show
// starts here and seeks land on it until a scene cue cuts to something.
// Also useful on the scenes lane for dark stretches where only the dancer
// or a text card should be on screen.

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
