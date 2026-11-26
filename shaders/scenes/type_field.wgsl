// @bloom 0.5
// @title Type Field
// Type Field: the track playing now as a field of moving type. Rows of
// "Artist - Title" scroll across the frame in three depths, neighbouring
// rows in opposite directions, on the smooth energy clock (motion); each
// row's baseline waves with the mids and the near rows swell with the bass
// presence (shape); the whole field drifts up and down over eight bars
// (direction). The DJ name (or TRIPPIN) when nothing is detected. After
// Imaginando VS's "Type Field". Text comes from title_tex (common.wgsl).

// One depth layer of scrolling rows. `rowh` is the row height (screen
// units), `speed` in text-widths per beat of the energy clock.
// Returns colour and coverage, so nearer rows can hide the ones behind.
// `keep` = one row in `keep` carries text (the rest stay dark).
fn type_layer(p: vec2<f32>, rowh: f32, speed: f32, layer: f32, keep: i32) -> vec4<f32> {
    let drift = 0.25 * sin(TAU * u.flow / 32.0 + layer);
    let yy = (p.y + drift) / rowh + layer * 0.37;
    let row = floor(yy);
    let ri = i32(row);
    if ((ri % keep) + keep) % keep != 0 {
        return vec4<f32>(0.0);
    }
    let ly = fract(yy);
    // The text's box: 70% of the row tall; its width from the title's
    // aspect (title_tex is 2048 x 160, look4.w of it used).
    let th = rowh * 0.7;
    let tw = th * (2048.0 * max(u.look4.w, 0.01)) / 160.0;
    let gap = th * 1.2;
    let tile = tw + gap;
    let dir = select(1.0, -1.0, ((ri / keep) & 1) == 1);
    let off = hash21(vec2<f32>(row, layer * 7.0)) * tile;
    let x = p.x + dir * u.clock4.x * speed * tile + off;
    let local = x - floor(x / tile) * tile;
    if local > tw {
        return vec4<f32>(0.0);
    }
    // Baseline wave along the row: deeper with the mids.
    let wave = (0.06 + 0.16 * (u.pres4.y * 0.6 + u.lvl4.y * 0.4)) * sin(local / th * 1.3 + u.flow * 0.5 + row);
    let v = (ly - 0.15 - wave) / 0.7;
    let m = title_mask(vec2<f32>(local / tw, v));
    let c = palette(row * 0.07 + layer * 0.31 + local / tile * 0.15);
    return vec4<f32>(c * c * m, m);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = centred(in.uv);
    var col = vec3<f32>(0.004, 0.004, 0.01);
    // Back to front; each nearer layer hides what's behind its letters.
    // Far: small, faint, one row in two.
    let far = type_layer(p, 0.1, 0.05, 0.0, 2);
    col += far.rgb * 0.12;
    // Mid: one row in three.
    let mid = type_layer(p, 0.2, 0.08, 1.0, 3);
    col = col * (1.0 - mid.a * 0.9) + mid.rgb * 0.35;
    // Near: two big readable rows, swelling with the bass presence.
    let nh = 0.62 * (1.0 + 0.1 * u.pres4.x);
    let near = type_layer(p, nh, 0.12, 2.0, 2);
    col = col * (1.0 - near.a * 0.95) + near.rgb * 1.1;
    return vec4<f32>(finite(col), 1.0);
}
