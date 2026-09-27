//! Text overlays: `CueKind::Text` blocks rasterise their glyphs into a
//! coverage mask (ab_glyph), upload it to a per-slot GPU texture, and
//! `shaders/text.wgsl` colours/animates it in the present pass — after the
//! scene, so whole-frame FX (mirror, kaleido) don't garble the lettering.
//!
//! Two slots back the editor's two text lanes, so two strings can be on
//! screen at once. The cue's `end_kind` releases to `TextOff`, which fades
//! the slot out (see `TextState` in main.rs).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use serde::{Deserialize, Serialize};

/// GPU text slots — one per text lane on the timeline.
pub const TEXT_SLOTS: usize = 2;

/// What's written on screen and how it's styled — the payload of a
/// `CueKind::Text` block.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct TextSpec {
    pub text: String,
    #[serde(default)]
    pub style: TextStyle,
    #[serde(default)]
    pub pos: TextPos,
    /// Which text lane the block rides on (0 or 1) — drives `track()`.
    #[serde(default)]
    pub lane: u8,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextStyle {
    #[default]
    Neon,
    Fire,
    Wave,
    Glitch,
    Pulse,
    Chrome,
}

impl TextStyle {
    pub const ALL: [TextStyle; 6] = [
        Self::Neon,
        Self::Fire,
        Self::Wave,
        Self::Glitch,
        Self::Pulse,
        Self::Chrome,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Neon => "neon",
            Self::Fire => "fire",
            Self::Wave => "wave",
            Self::Glitch => "glitch",
            Self::Pulse => "pulse",
            Self::Chrome => "chrome",
        }
    }

    /// Shader-side index — keep in sync with `text.wgsl`.
    pub fn index(&self) -> f32 {
        *self as usize as f32
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextPos {
    Top,
    #[default]
    Center,
    Bottom,
}

impl TextPos {
    pub const ALL: [TextPos; 3] = [Self::Top, Self::Center, Self::Bottom];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Center => "middle",
            Self::Bottom => "bottom",
        }
    }

    /// Centre Y in centred coords (-1..1) — note the shaders' y axis is
    /// flipped (dancer.wgsl's `bottom = 0.98` is positive).
    pub fn y(&self) -> f32 {
        match self {
            Self::Top => -0.72,
            Self::Center => 0.0,
            Self::Bottom => 0.72,
        }
    }
}

/// One GPU slot — must match `TS` in `shaders/text.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextSlotU {
    /// Centre x/y + half width/height, in centred coords.
    pub quad: [f32; 4],
    /// Mask width / height; 0 = slot empty.
    pub aspect: f32,
    /// `TextStyle::index()`.
    pub style: f32,
    /// Fade envelope 0..1 (render loop animates it).
    pub opacity: f32,
    /// `u.time` when the block fired — styles animate from this.
    pub born: f32,
    /// Block length in seconds (0 = untimed) — loop styles wrap at this.
    pub life: f32,
    /// Colour seed so sibling slots don't share a hue.
    pub hue: f32,
    pub _pad: [f32; 2],
}

/// Both slots — must match `T` in `shaders/text.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextUniforms {
    pub slots: [TextSlotU; TEXT_SLOTS],
}

/// Live overlay state on the render thread — one per text slot.
pub struct TextState {
    pub spec: TextSpec,
    /// Mask aspect (w/h) at upload time.
    pub aspect: f32,
    /// `u.time` the block fired.
    pub born: f32,
    /// Fade-out start, set when the block's end cue passes.
    pub out_at: Option<f32>,
}

/// A rasterised line of text — single-channel coverage, coloured in shader.
pub struct TextBitmap {
    pub width: u32,
    pub height: u32,
    /// Row-major coverage, 0..=255.
    pub mask: Vec<u8>,
}

fn find_font() -> Option<PathBuf> {
    // A user-dropped `fonts/*.ttf` beside the exe (or in the repo) wins.
    let mut dirs = vec![PathBuf::from("fonts")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.join("fonts"));
            if let Some(contents) = dir.parent() {
                dirs.push(contents.join("Resources/fonts"));
            }
        }
    }
    dirs.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("fonts"));
    for dir in dirs {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            let mut hits: Vec<PathBuf> = rd
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                        e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf")
                    })
                })
                .collect();
            hits.sort();
            if let Some(p) = hits.into_iter().next() {
                return Some(p);
            }
        }
    }
    // Platform fallbacks — every target ships something usable.
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
            "/System/Library/Fonts/Supplemental/Impact.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
        ]
    } else if cfg!(windows) {
        &[
            "C:\\Windows\\Fonts\\bahnschrift.ttf",
            "C:\\Windows\\Fonts\\arialbd.ttf",
            "C:\\Windows\\Fonts\\impact.ttf",
        ]
    } else {
        &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
        ]
    };
    candidates
        .iter()
        .map(Path::new)
        .find(|p| p.is_file())
        .map(PathBuf::from)
}

fn font() -> Option<&'static FontVec> {
    static FONT: OnceLock<Option<FontVec>> = OnceLock::new();
    FONT.get_or_init(|| {
        let path = find_font()?;
        let bytes = std::fs::read(&path).ok()?;
        let f = FontVec::try_from_vec(bytes).ok()?;
        eprintln!("Text: font {}", path.display());
        Some(f)
    })
    .as_ref()
}

/// Rasterise `text` into a coverage mask at `px` cap height. Kerning and
/// side bearings come from the font; missing glyphs are skipped.
pub fn rasterize(text: &str, px: f32) -> Option<TextBitmap> {
    let font = font()?;
    let scaled = font.as_scaled(PxScale::from(px));
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let line_h = ascent - descent;

    // Layout pass: advance + kern.
    let mut glyphs = Vec::new();
    let mut caret = 0.0f32;
    let mut last = None;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(l) = last {
            caret += scaled.kern(l, id);
        }
        let g = ab_glyph::Glyph {
            id,
            scale: PxScale::from(px),
            position: ab_glyph::point(caret + scaled.h_side_bearing(id), ascent),
        };
        caret += scaled.h_advance(id);
        last = Some(id);
        if let Some(q) = font.outline_glyph(g) {
            glyphs.push(q);
        }
    }
    if glyphs.is_empty() {
        return None;
    }
    // Tight bounds over the outlined glyphs.
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for g in &glyphs {
        let b = g.px_bounds();
        min_x = min_x.min(b.min.x);
        min_y = min_y.min(b.min.y);
        max_x = max_x.max(b.max.x);
        max_y = max_y.max(b.max.y);
    }
    let _ = line_h;
    let pad = (px * 0.12).ceil();
    let w = ((max_x - min_x).ceil() as i32 + (pad * 2.0) as i32).max(1) as u32;
    let h = ((max_y - min_y).ceil() as i32 + (pad * 2.0) as i32).max(1) as u32;
    if w > 4096 || h > 1024 {
        eprintln!("Text: bitmap too big ({w}x{h}) — shorten the cue text");
        return None;
    }
    let mut mask = vec![0u8; (w * h) as usize];
    for g in &glyphs {
        let b = g.px_bounds();
        g.draw(|gx, gy, cov| {
            let x = (b.min.x + gx as f32 - min_x + pad) as i32;
            let y = (b.min.y + gy as f32 - min_y + pad) as i32;
            if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                let i = (y as u32 * w + x as u32) as usize;
                mask[i] = mask[i].saturating_add((cov * 255.0) as u8);
            }
        });
    }
    Some(TextBitmap {
        width: w,
        height: h,
        mask,
    })
}

/// Same mask as an egui-friendly RGBA image — for the block thumbnail.
pub fn rasterize_rgba(text: &str, px: f32) -> Option<(u32, u32, Vec<u8>)> {
    let b = rasterize(text, px)?;
    let mut rgba = Vec::with_capacity((b.width * b.height * 4) as usize);
    for a in &b.mask {
        rgba.extend_from_slice(&[255, 255, 255, *a]);
    }
    Some((b.width, b.height, rgba))
}
