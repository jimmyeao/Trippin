//! Stream overlays: the "now playing" card, the always-on branding block
//! (logo + DJ name + handles) and the scrolling ticker. Each is drawn once on
//! the CPU into an RGBA image (ab_glyph text, rounded panels) whenever its
//! content changes, uploaded as a texture, and composited by
//! `shaders/overlay.wgsl` in the present pass — so post FX never warp them
//! and the NDI/Spout tap carries them too. Per frame only the quads move
//! (card slide-in, ticker scroll).

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};

use crate::config::Settings;
use crate::nowplaying::NowPlayingState;

/// GPU layers — must match `O` in `shaders/overlay.wgsl`.
pub const OV_LAYERS: usize = 4;
pub const L_CARD: usize = 0;
/// Logo image only — its own layer so it can fade without touching the name.
pub const L_LOGO: usize = 1;
pub const L_TICKER: usize = 2;
/// DJ name + handles only — same corner, own fade.
pub const L_NAME: usize = 3;

/// One layer — must match `OL` in `shaders/overlay.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OvLayerU {
    /// Centre x/y + half width/height, in centred coords (y down).
    pub quad: [f32; 4],
    /// x: u scale, y: u offset (ticker scroll), z: opacity, w: 1 = ticker band.
    pub uvx: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OvUniforms {
    pub layers: [OvLayerU; OV_LAYERS],
}

/// Straight-alpha sRGB RGBA8 — the shader premultiplies after decode.
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Image {
    fn new(w: u32, h: u32) -> Self {
        Self {
            w: w.max(1),
            h: h.max(1),
            px: vec![0; (w.max(1) * h.max(1) * 4) as usize],
        }
    }

    /// Source-over blend of a straight-alpha colour at coverage `a`.
    fn blend(&mut self, x: i32, y: i32, c: [f32; 3], a: f32) {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h || a <= 0.0 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        let d = &mut self.px[i..i + 4];
        let da = d[3] as f32 / 255.0;
        let oa = a + da * (1.0 - a);
        if oa <= 0.0 {
            return;
        }
        for k in 0..3 {
            let dc = d[k] as f32 / 255.0;
            let v = (c[k] * a + dc * da * (1.0 - a)) / oa;
            d[k] = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
        d[3] = (oa.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    }

    /// Anti-aliased rounded rectangle; `col(fx, fy)` gets the 0..1 position
    /// inside the box (for gradients and strips).
    fn round_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, col: impl Fn(f32, f32) -> [f32; 4]) {
        for y in y0.floor() as i32..y1.ceil() as i32 {
            for x in x0.floor() as i32..x1.ceil() as i32 {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                // Signed distance to the rounded box.
                let cx = (x0 + x1) * 0.5;
                let cy = (y0 + y1) * 0.5;
                let qx = (px - cx).abs() - ((x1 - x0) * 0.5 - r);
                let qy = (py - cy).abs() - ((y1 - y0) * 0.5 - r);
                let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
                let cov = (0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    let c = col((px - x0) / (x1 - x0).max(1.0), (py - y0) / (y1 - y0).max(1.0));
                    self.blend(x, y, [c[0], c[1], c[2]], c[3] * cov);
                }
            }
        }
    }
}

/// A laid-out line of text at a pixel size.
struct Run {
    glyphs: Vec<ab_glyph::OutlinedGlyph>,
    width: f32,
    ascent: f32,
}

fn layout(font: &FontVec, text: &str, px: f32, tracking: f32) -> Run {
    let scaled = font.as_scaled(PxScale::from(px));
    let mut caret = 0.0f32;
    let mut last = None;
    let mut glyphs = Vec::new();
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(l) = last {
            caret += scaled.kern(l, id);
        }
        let g = ab_glyph::Glyph {
            id,
            scale: PxScale::from(px),
            position: ab_glyph::point(caret, scaled.ascent()),
        };
        caret += scaled.h_advance(id) + tracking * px;
        last = Some(id);
        if let Some(o) = font.outline_glyph(g) {
            glyphs.push(o);
        }
    }
    Run {
        glyphs,
        width: (caret - tracking * px).max(0.0),
        ascent: scaled.ascent(),
    }
}

/// Shorten `text` with an ellipsis until it fits `max_w` pixels.
fn fit(font: &FontVec, text: &str, px: f32, tracking: f32, max_w: f32) -> Run {
    let r = layout(font, text, px, tracking);
    if r.width <= max_w {
        return r;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut n = chars.len();
    while n > 1 {
        n -= 1;
        let s: String = chars[..n].iter().collect::<String>().trim_end().to_string() + "…";
        let r = layout(font, &s, px, tracking);
        if r.width <= max_w {
            return r;
        }
    }
    layout(font, "…", px, tracking)
}

/// Draw a run with its top-left (line box) at (x, y). `shadow` adds a soft
/// dark outline so text reads on any scene.
fn draw(img: &mut Image, run: &Run, x: f32, y: f32, col: [f32; 3], shadow: bool) {
    let passes: &[(f32, f32, [f32; 3], f32)] = if shadow {
        &[
            (2.0, 2.0, [0.0, 0.0, 0.0], 0.55),
            (-1.5, 0.0, [0.0, 0.0, 0.0], 0.35),
            (1.5, 0.0, [0.0, 0.0, 0.0], 0.35),
            (0.0, -1.5, [0.0, 0.0, 0.0], 0.35),
            (0.0, 1.5, [0.0, 0.0, 0.0], 0.35),
            (0.0, 0.0, [1.0, 1.0, 1.0], 1.0),
        ]
    } else {
        &[(0.0, 0.0, [1.0, 1.0, 1.0], 1.0)]
    };
    for &(dx, dy, pc, pa) in passes {
        let c = if pa >= 1.0 { col } else { pc };
        for g in &run.glyphs {
            let b = g.px_bounds();
            g.draw(|gx, gy, cov| {
                img.blend(
                    (x + dx + b.min.x) as i32 + gx as i32,
                    (y + dy + b.min.y) as i32 + gy as i32,
                    c,
                    cov.min(1.0) * pa,
                );
            });
        }
    }
    let _ = run.ascent;
}

/// Hex "#rrggbb" → linear-ish sRGB triple (0..1); falls back to `def`.
pub fn parse_hex(s: &str, def: [f32; 3]) -> [f32; 3] {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return def;
    }
    let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map(|b| b as f32 / 255.0);
    match (v(0), v(2), v(4)) {
        (Ok(r), Ok(g), Ok(b)) => [r, g, b],
        _ => def,
    }
}

/// The lower-third card: a dark glass panel with an accent bar, a small
/// "NOW PLAYING" label, the title large and the artist below.
pub fn render_card(artist: &str, title: &str, accent: [f32; 3]) -> Option<Image> {
    let font = crate::text::font()?;
    let (pad, bar) = (34.0f32, 12.0f32);
    let label = layout(font, "NOW PLAYING", 26.0, 0.18);
    let max_text = 1500.0;
    let t = fit(font, title, 68.0, 0.0, max_text);
    let a = fit(font, artist, 46.0, 0.0, max_text);
    let text_w = label.width.max(t.width).max(a.width);
    let w = (bar + pad * 2.0 + text_w).ceil().max(420.0);
    let h = if artist.is_empty() { 170.0 } else { 222.0f32 };
    let mut img = Image::new(w as u32 + 8, h as u32 + 8);
    // Soft drop shadow, then the panel.
    img.round_rect(6.0, 8.0, w + 2.0, h + 4.0, 22.0, |_, _| [0.0, 0.0, 0.0, 0.35]);
    // Panel, with the left strip in an accent → complement gradient.
    let acc2 = [accent[2], accent[0], accent[1]];
    img.round_rect(0.0, 0.0, w, h, 22.0, |fx, fy| {
        if fx * w < bar {
            let mut c = [0.0, 0.0, 0.0, 1.0];
            for k in 0..3 {
                c[k] = accent[k] + (acc2[k] - accent[k]) * fy;
            }
            c
        } else {
            [0.035, 0.035, 0.06, 0.78]
        }
    });
    let x = bar + pad;
    draw(&mut img, &label, x, 22.0, accent, false);
    draw(&mut img, &t, x, 56.0, [1.0, 1.0, 1.0], false);
    if !artist.is_empty() {
        draw(&mut img, &a, x, 136.0, [0.72, 0.74, 0.82], false);
    }
    Some(img)
}

/// Branding block: optional logo, then DJ name over the handles line.
pub fn render_brand(logo: Option<&image::RgbaImage>, name: &str, handles: &str, accent: [f32; 3]) -> Option<Image> {
    let font = crate::text::font()?;
    let n = layout(font, name, 80.0, 0.02);
    let hd = layout(font, handles, 40.0, 0.03);
    let has_text = !name.is_empty() || !handles.is_empty();
    let text_h = match (name.is_empty(), handles.is_empty()) {
        (false, false) => 150.0,
        (false, true) => 100.0,
        (true, false) => 56.0,
        (true, true) => 0.0,
    };
    let logo_h = 170.0f32;
    let (lw, lh) = logo
        .map(|l| (l.width() as f32 * logo_h / l.height().max(1) as f32, logo_h))
        .unwrap_or((0.0, 0.0));
    let gap = if logo.is_some() && has_text { 30.0 } else { 0.0 };
    let w = (lw + gap + n.width.max(hd.width) + 12.0).ceil();
    let h = lh.max(text_h).ceil() + 24.0;
    if w < 2.0 {
        return None;
    }
    let mut img = Image::new(w as u32, h as u32);
    if let Some(l) = logo {
        let r = image::imageops::resize(l, lw.max(1.0) as u32, lh as u32, image::imageops::FilterType::Lanczos3);
        let oy = ((h - lh) * 0.5) as i32;
        for (x, y, p) in r.enumerate_pixels() {
            let a = p[3] as f32 / 255.0;
            img.blend(x as i32, y as i32 + oy, [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0], a);
        }
    }
    let x = lw + gap + 4.0;
    let mut y = (h - text_h) * 0.5;
    if !name.is_empty() {
        draw(&mut img, &n, x, y, [1.0, 1.0, 1.0], true);
        y += 96.0;
    }
    if !handles.is_empty() {
        draw(&mut img, &hd, x, y, accent, true);
    }
    Some(img)
}

/// Ticker strip: the message once plus a trailing gap; the shader repeats it.
pub fn render_ticker(text: &str, accent: [f32; 3]) -> Option<Image> {
    let font = crate::text::font()?;
    let msg = format!("{}     •", text.trim());
    let r = layout(font, &msg, 44.0, 0.02);
    let gap = 60.0;
    let w = (r.width + gap).ceil();
    let mut img = Image::new(w as u32, 64);
    draw(&mut img, &r, gap * 0.5, 4.0, [1.0, 1.0, 1.0], false);
    // Tint the separator diamond with the accent.
    let _ = accent;
    Some(img)
}

/// Render-thread state: what's uploaded, plus the show/hide envelopes —
/// each overlay piece eases its own visibility so pads/hotkeys fade
/// rather than pop.
#[derive(Default)]
pub struct Overlays {
    card_serial: u64,
    card_key: String,
    card_aspect: f32,
    card_born: f32,
    card_on: bool,
    logo_key: String,
    /// Uploaded pixel size [w, h] — zero width = nothing uploaded.
    logo_size: [f32; 2],
    name_key: String,
    name_size: [f32; 2],
    ticker_key: String,
    ticker_aspect: f32,
    logo_vis: f32,
    name_vis: f32,
    ticker_vis: f32,
    last_t: f32,
}

/// Ease `vis` toward `target` — in over ~0.45 s, out over ~0.3 s.
fn ease_vis(vis: &mut f32, target: f32, dt: f32) {
    let d = target - *vis;
    // signum(+0.0) is 1.0, not 0.0: an at-rest overlay would otherwise ping
    // `rate*dt` up and back every other frame (the logo's fast judder).
    if d == 0.0 {
        return;
    }
    let rate = if d > 0.0 { 2.2 } else { 3.2 };
    *vis = (*vis + rate * dt * d.signum()).clamp(0.0, 1.0);
    if (target - *vis).abs() < 0.02 {
        *vis = target;
    }
}

impl Overlays {
    /// Rebuild any changed images (via `upload`) and lay out this frame.
    /// Returns None when nothing is on screen.
    pub fn update(
        &mut self,
        s: &Settings,
        np: &NowPlayingState,
        now_t: f32,
        screen_asp: f32,
        upload: &mut dyn FnMut(usize, &Image),
    ) -> Option<OvUniforms> {
        let accent = parse_hex(&s.brand_color, [0.25, 0.85, 1.0]);
        let mut u = OvUniforms::default();
        let mut any = false;
        let m = 0.05f32; // screen margin (centred units)
        let dt = (now_t - self.last_t).clamp(0.0, 0.1);
        self.last_t = now_t;

        // --- Ticker (bottom band) --------------------------------------
        // Slides off the bottom edge while fading, instead of blinking.
        let want_ticker = s.ticker_on && !s.ticker_text.trim().is_empty();
        ease_vis(&mut self.ticker_vis, want_ticker as u8 as f32, dt);
        let ticker_half_h = 0.034f32;
        if self.ticker_vis > 0.001 {
            let key = format!("{}|{}", s.ticker_text, s.brand_color);
            if want_ticker && key != self.ticker_key {
                self.ticker_key = key;
                if let Some(img) = render_ticker(&s.ticker_text, accent) {
                    self.ticker_aspect = img.w as f32 / img.h as f32;
                    upload(L_TICKER, &img);
                }
            }
            if self.ticker_aspect > 0.0 {
                let img_w = 2.0 * ticker_half_h * self.ticker_aspect;
                let scroll = now_t * 0.18 * s.ticker_speed.max(0.1) / img_w;
                let cy = 1.0 - ticker_half_h + (1.0 - self.ticker_vis) * 2.0 * ticker_half_h;
                u.layers[L_TICKER] = OvLayerU {
                    quad: [0.0, cy, screen_asp, ticker_half_h],
                    uvx: [2.0 * screen_asp / img_w, scroll, self.ticker_vis, 1.0],
                };
                any = true;
            }
        }
        let floor_y = 1.0 - 2.0 * ticker_half_h * self.ticker_vis;

        // --- Branding (a corner) ---------------------------------------
        // Logo and name/handles are separate layers — each eases its own
        // visibility AND its share of the block's width, so the surviving
        // piece slides sideways instead of the whole block blinking.
        let logo_path = s.brand_logo.trim();
        let want_logo = s.brand_on && s.brand_logo_on && !logo_path.is_empty();
        let want_name = s.brand_on
            && s.brand_name_on
            && (!s.brand_name.trim().is_empty() || !s.brand_handles.trim().is_empty());
        ease_vis(&mut self.logo_vis, want_logo as u8 as f32, dt);
        ease_vis(&mut self.name_vis, want_name as u8 as f32, dt);
        let corner = s.brand_corner.min(3);
        if self.logo_vis > 0.001 || want_logo {
            let key = format!("{}|{}", logo_path, s.brand_color);
            if want_logo && key != self.logo_key {
                self.logo_key = key;
                let logo = match image::open(logo_path) {
                    Ok(i) => Some(i.to_rgba8()),
                    Err(e) => {
                        eprintln!("overlay: logo {logo_path}: {e}");
                        None
                    }
                };
                match render_brand(logo.as_ref(), "", "", accent) {
                    Some(img) => {
                        self.logo_size = [img.w as f32, img.h as f32];
                        upload(L_LOGO, &img);
                    }
                    None => self.logo_size = [0.0, 0.0],
                }
            }
        }
        if self.name_vis > 0.001 || want_name {
            let key = format!("{}|{}|{}", s.brand_name, s.brand_handles, s.brand_color);
            if want_name && key != self.name_key {
                self.name_key = key;
                match render_brand(
                    None,
                    s.brand_name.trim(),
                    s.brand_handles.trim(),
                    accent,
                ) {
                    Some(img) => {
                        self.name_size = [img.w as f32, img.h as f32];
                        upload(L_NAME, &img);
                    }
                    None => self.name_size = [0.0, 0.0],
                }
            }
        }
        // Layout in block pixels scaled to centred units; a piece's width
        // contribution is eased by its visibility, so the neighbour slides.
        let block_hh = 0.085 * s.brand_size.clamp(0.4, 2.5);
        let px2c = 2.0 * block_hh / 194.0; // centred units per block pixel
        let lw = self.logo_size[0] * px2c * self.logo_vis;
        let nw = self.name_size[0] * px2c * self.name_vis;
        let gap = 30.0 * px2c * self.logo_vis.min(self.name_vis);
        let total = lw + gap + nw;
        let mut brand_vis = 0.0f32;
        if total > 0.001 {
            // Full block capped at 60% of screen width, as before.
            let fit = (screen_asp * 1.2 / total).min(1.0);
            let x0 = if corner % 2 == 0 {
                -screen_asp + m
            } else {
                screen_asp - m - total * fit
            };
            let y = if corner < 2 { -1.0 + m + block_hh * fit } else { floor_y - m - block_hh * fit };
            let op = s.brand_opacity.clamp(0.0, 1.0);
            if self.logo_vis > 0.001 && self.logo_size[0] > 0.0 {
                u.layers[L_LOGO] = OvLayerU {
                    quad: [
                        x0 + lw * fit / 2.0,
                        y,
                        lw * fit / 2.0,
                        self.logo_size[1] * px2c * fit / 2.0,
                    ],
                    uvx: [1.0, 0.0, self.logo_vis * op, 0.0],
                };
                any = true;
            }
            if self.name_vis > 0.001 && self.name_size[0] > 0.0 {
                u.layers[L_NAME] = OvLayerU {
                    quad: [
                        x0 + (lw + gap) * fit + nw * fit / 2.0,
                        y,
                        nw * fit / 2.0,
                        self.name_size[1] * px2c * fit / 2.0,
                    ],
                    uvx: [1.0, 0.0, self.name_vis * op, 0.0],
                };
                any = true;
            }
            brand_vis = self.logo_vis.max(self.name_vis);
        }

        // --- Now playing card (bottom-left; bottom-right if the brand
        // sits there) --------------------------------------------------
        if s.np_card {
            if let Some(t) = &np.track {
                let key = format!("{}|{}|{}", t.artist, t.title, s.brand_color);
                if np.serial != self.card_serial || key != self.card_key {
                    let fresh = np.serial != self.card_serial;
                    self.card_serial = np.serial;
                    self.card_key = key;
                    if let Some(img) = render_card(&t.artist, &t.title, accent) {
                        self.card_aspect = img.w as f32 / img.h as f32;
                        upload(L_CARD, &img);
                        if fresh || !self.card_on {
                            self.card_born = now_t;
                        }
                        self.card_on = true;
                    }
                }
            } else {
                self.card_on = false;
            }
        }
        if s.np_card && self.card_on && self.card_aspect > 0.0 {
            let lt = now_t - self.card_born;
            let hold = s.np_hold_s;
            // Slide in over 0.6 s; out over 0.6 s after `hold` (0 = stays).
            let ein = 1.0 - (1.0 - (lt / 0.6).clamp(0.0, 1.0)).powi(3);
            let eout = if hold > 0.0 { ((lt - hold) / 0.6).clamp(0.0, 1.0) } else { 0.0 };
            let eout = eout * eout;
            let vis = ein * (1.0 - eout);
            if vis > 0.001 {
                let hh = 0.1 * s.np_size.clamp(0.4, 2.5);
                let hw = (hh * self.card_aspect).min(screen_asp * 0.9);
                let hh = hw / self.card_aspect;
                let right = brand_vis > 0.01 && corner == 2;
                let travel = 2.0 * hw + m * 2.0;
                let off = travel * (1.0 - ein + eout);
                let x = if right { screen_asp - m - hw + off } else { -screen_asp + m + hw - off };
                let y = floor_y - m - hh;
                u.layers[L_CARD] = OvLayerU {
                    quad: [x, y, hw, hh],
                    uvx: [1.0, 0.0, vis, 0.0],
                };
                any = true;
            }
        }
        any.then_some(u)
    }

    /// Show the current track's card again (hotkey / panel button).
    pub fn replay_card(&mut self, now_t: f32) {
        if self.card_on {
            self.card_born = now_t;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes the three overlay images to `target/overlay_*.png` for a look.
    #[test]
    fn overlay_images_render() {
        let acc = [0.25, 0.85, 1.0];
        let save = |name: &str, img: &Image| {
            let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("target/overlay_{name}.png"));
            image::save_buffer(out, &img.px, img.w, img.h, image::ColorType::Rgba8).unwrap();
        };
        if crate::text::font().is_none() {
            return; // no system font on this box
        }
        let c = render_card("Chicane", "Saltwater (Ilan Bluestone Remix)", acc).unwrap();
        assert!(c.w > 400 && c.h > 100);
        save("card", &c);
        let b = render_brand(None, "DJ Jimmy", "@jimmyeao · twitch.tv/jimmyeao", acc).unwrap();
        save("brand", &b);
        let t = render_ticker("Requests in chat · follow for the next set", acc).unwrap();
        save("ticker", &t);
    }

    /// signum(+0.0) is 1.0 — an at-rest vis used to ping `rate*dt` up and
    /// back every other frame (the logo juddered while the name was off).
    #[test]
    fn vis_ease_is_stable_at_rest() {
        let mut v = 0.0f32;
        for _ in 0..120 {
            ease_vis(&mut v, 0.0, 0.016);
        }
        assert_eq!(v, 0.0);
        // And still converges both ways.
        for _ in 0..120 {
            ease_vis(&mut v, 1.0, 0.016);
        }
        assert_eq!(v, 1.0);
        for _ in 0..120 {
            ease_vis(&mut v, 0.0, 0.016);
        }
        assert_eq!(v, 0.0);
    }
}
