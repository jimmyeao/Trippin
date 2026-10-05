//! Named gradient palettes (WLED-style) compiled into a 256-entry LUT that
//! the GPU samples in `palette()`. Scenes never see the names — they just
//! call `palette(t)` and get whatever the user picked globally.
//!
//! A palette is a list of `(position, [r,g,b])` stops, position in 0..1,
//! interpolated linearly in sRGB. The shader ping-pong mirrors the index,
//! so non-cyclic palettes don't need their first/last stops to match.
//!
//! `AUTO` is the pseudo-palette: the render loop feeds it through [`Auto`],
//! which maps the music's mood to one of the named gradients.

use crate::audio::Features;

pub const LUT_SIZE: usize = 256;

/// The "pick for me" palette — a `Settings.palette` value, not a gradient.
pub const AUTO: &str = "auto";

pub struct Palette {
    pub name: &'static str,
    stops: &'static [(f32, [u8; 3])],
}

macro_rules! pal {
    ($name:literal, $(($p:expr, $c:expr)),+ $(,)?) => {
        Palette {
            name: $name,
            stops: &[$(($p, $c)),+],
        }
    };
}

pub const PALETTES: &[Palette] = &[
    // Classic sweep — closest to the old cosine palette.
    pal!(
        "rainbow",
        (0.00, [255, 0, 0]),
        (0.14, [255, 128, 0]),
        (0.28, [255, 230, 0]),
        (0.42, [0, 220, 40]),
        (0.57, [0, 160, 255]),
        (0.71, [60, 40, 255]),
        (0.85, [180, 0, 255]),
        (1.00, [255, 0, 128])
    ),
    // WLED "party": bright rainbow dips through dark valleys.
    pal!(
        "party",
        (0.00, [30, 0, 60]),
        (0.10, [255, 0, 90]),
        (0.24, [20, 0, 40]),
        (0.36, [255, 200, 0]),
        (0.50, [0, 20, 50]),
        (0.64, [0, 230, 160]),
        (0.78, [40, 0, 60]),
        (1.00, [80, 120, 255])
    ),
    pal!(
        "ocean",
        (0.00, [0, 8, 40]),
        (0.30, [0, 50, 140]),
        (0.60, [0, 160, 200]),
        (0.85, [80, 230, 220]),
        (1.00, [200, 255, 245])
    ),
    pal!(
        "forest",
        (0.00, [0, 20, 4]),
        (0.30, [0, 90, 20]),
        (0.60, [40, 190, 40]),
        (0.85, [160, 230, 60]),
        (1.00, [220, 255, 140])
    ),
    pal!(
        "sunset",
        (0.00, [30, 0, 60]),
        (0.25, [120, 10, 90]),
        (0.50, [220, 60, 40]),
        (0.75, [255, 160, 30]),
        (1.00, [255, 240, 160])
    ),
    pal!(
        "lava",
        (0.00, [10, 0, 0]),
        (0.30, [120, 10, 0]),
        (0.55, [230, 60, 0]),
        (0.78, [255, 170, 20]),
        (1.00, [255, 250, 220])
    ),
    pal!(
        "fire",
        (0.00, [0, 0, 0]),
        (0.25, [140, 0, 0]),
        (0.50, [255, 60, 0]),
        (0.75, [255, 190, 30]),
        (1.00, [255, 255, 235])
    ),
    pal!(
        "ice",
        (0.00, [0, 10, 40]),
        (0.35, [0, 90, 200]),
        (0.65, [60, 200, 255]),
        (1.00, [240, 255, 255])
    ),
    pal!(
        "breeze",
        (0.00, [0, 40, 80]),
        (0.35, [0, 160, 200]),
        (0.65, [120, 230, 220]),
        (1.00, [235, 250, 240])
    ),
    pal!(
        "cyber",
        (0.00, [10, 0, 30]),
        (0.30, [120, 0, 180]),
        (0.55, [255, 0, 160]),
        (0.80, [0, 220, 255]),
        (1.00, [140, 255, 250])
    ),
    pal!(
        "magenta",
        (0.00, [20, 0, 30]),
        (0.35, [140, 0, 120]),
        (0.70, [255, 30, 160]),
        (1.00, [255, 170, 220])
    ),
    pal!(
        "coral",
        (0.00, [40, 5, 20]),
        (0.35, [200, 60, 60]),
        (0.70, [255, 140, 110]),
        (1.00, [255, 220, 190])
    ),
    pal!(
        "autumn",
        (0.00, [30, 12, 0]),
        (0.30, [140, 50, 10]),
        (0.60, [220, 120, 20]),
        (0.85, [180, 160, 40]),
        (1.00, [240, 230, 160])
    ),
    pal!(
        "pastel",
        (0.00, [255, 175, 200]),
        (0.25, [255, 210, 170]),
        (0.50, [200, 255, 190]),
        (0.75, [170, 220, 255]),
        (1.00, [230, 190, 255])
    ),
    pal!(
        "smoke",
        (0.00, [0, 0, 0]),
        (0.40, [80, 80, 90]),
        (0.75, [190, 195, 205]),
        (1.00, [255, 255, 255])
    ),
    pal!(
        "halloween",
        (0.00, [40, 0, 50]),
        (0.30, [120, 20, 160]),
        (0.55, [255, 110, 10]),
        (0.80, [60, 10, 80]),
        (1.00, [255, 160, 40])
    ),
    pal!(
        "rift",
        (0.00, [20, 0, 50]),
        (0.40, [90, 20, 200]),
        (0.65, [0, 190, 200]),
        (1.00, [120, 255, 240])
    ),
    pal!(
        "gold",
        (0.00, [30, 15, 0]),
        (0.35, [160, 100, 20]),
        (0.70, [255, 190, 60]),
        (1.00, [255, 245, 200])
    ),
];

/// Display names for the panel dropdown, in definition order.
pub fn names() -> impl Iterator<Item = &'static str> {
    PALETTES.iter().map(|p| p.name)
}

/// Every selectable palette value — `AUTO` first, then the named gradients.
/// For pickers and name validation; the render loop resolves `AUTO` itself.
pub fn all_names() -> impl Iterator<Item = &'static str> {
    std::iter::once(AUTO).chain(names())
}

/// `AUTO` or a named gradient.
pub fn is_valid(name: &str) -> bool {
    name == AUTO || names().any(|n| n == name)
}

/// Mood → palette lists for `AUTO`. Curated to stay show-safe — the niche
/// looks (smoke, halloween, pastel, forest) are manual picks only.
const MOODS: [&[&str]; 4] = [
    // calm — drums out → cool and dark
    &["ocean", "ice", "breeze", "rift"],
    // steady groove → warm and broad
    &["rainbow", "sunset", "coral", "gold"],
    // driving or building → electric
    &["cyber", "magenta", "party", "sunset"],
    // drop / peak energy → hot
    &["fire", "lava", "party", "autumn"],
];

/// The mood bucket for the current features: 0 breakdown, 1 groove,
/// 2 driving, 3 peak. Ordered — calm wins over raw energy.
fn bucket(f: &Features) -> usize {
    if f.calm > 0.5 {
        return 0;
    }
    if f.energy > 0.72 {
        return 3;
    }
    if f.build > 0.2 || f.energy > 0.45 {
        return 2;
    }
    1
}

/// `pick` state for `AUTO`: the live palette and the hysteresis that stops
/// it flickering between moods — a LUT swap is a hard colour cut, so a new
/// mood must hold for ~2 beats and each pick lasts at least 16 beats.
pub struct Auto {
    name: &'static str,
    bucket: usize,
    /// Rotation cursor per bucket — consecutive visits cycle its list.
    idx: [usize; 4],
    /// Beat clock when the live pick took effect.
    since: f64,
    /// Bucket the music has moved to, and the beat it appeared on.
    cand: Option<(usize, f64)>,
}

impl Default for Auto {
    fn default() -> Self {
        Self {
            name: "rainbow",
            bucket: usize::MAX,
            idx: [0; 4],
            since: f64::MIN,
            cand: None,
        }
    }
}

impl Auto {
    /// The palette matching this frame's mood. `beat` is the running beat
    /// clock (`f.beat_position(now)`) used for the hold/dwell windows.
    #[allow(dead_code)] // the render loop uses `pick_in`; kept for the tests and tools
    pub fn pick(&mut self, f: &Features, beat: f64) -> &'static str {
        self.pick_in(f, beat, None)
    }

    /// `pick`, with a Style's own palette family per mood (`styles::mood_palettes`).
    /// A mood whose list is empty falls back to the built-in rotation.
    /// The family changed (a Style was picked or cleared): re-pick from it for
    /// the current mood right away. Starting over with a fresh `Auto` showed
    /// `rainbow` until a mood held for two clean beats, which on music whose
    /// energy hovers at a bucket edge took up to ~50 s (i9 test pass).
    pub fn restyle(&mut self, over: Option<&[Vec<&'static str>; 4]>) {
        let b = self.bucket;
        if b >= MOODS.len() {
            return; // nothing picked yet: the first pick uses the new family
        }
        let list: &[&'static str] = match over {
            Some(o) if !o[b].is_empty() => &o[b],
            _ => MOODS[b],
        };
        self.name = list[self.idx[b] % list.len()];
        self.cand = None;
    }

    pub fn pick_in(&mut self, f: &Features, beat: f64, over: Option<&[Vec<&'static str>; 4]>) -> &'static str {
        const HOLD: f64 = 2.0;
        const DWELL: f64 = 16.0;
        let b = bucket(f);
        if b == self.bucket {
            self.cand = None;
        } else if let Some((_, since)) = self.cand.filter(|(cb, _)| *cb == b) {
            if beat - since >= HOLD && beat - self.since >= DWELL {
                self.bucket = b;
                self.idx[b] += 1;
                let list: &[&'static str] = match over {
                    Some(o) if !o[b].is_empty() => &o[b],
                    _ => MOODS[b],
                };
                self.name = list[self.idx[b] % list.len()];
                self.since = beat;
                self.cand = None;
            }
        } else {
            self.cand = Some((b, beat));
        }
        self.name
    }
}

/// 256×4 RGBA bytes for the named palette (falls back to `rainbow`).
pub fn lut(name: &str) -> [u8; LUT_SIZE * 4] {
    if name == AUTO {
        // Display LUT for the `auto` chip — cool/warm/hot thirds so the
        // swatch shows the moods it picks between.
        let mut out = lut("ocean");
        let t = LUT_SIZE / 3;
        out[t * 4..t * 8].copy_from_slice(&lut("sunset")[t * 4..t * 8]);
        out[t * 8..].copy_from_slice(&lut("fire")[t * 8..]);
        return out;
    }
    let pal = PALETTES
        .iter()
        .find(|p| p.name == name)
        .unwrap_or(&PALETTES[0]);
    let st = pal.stops;
    let mut out = [0u8; LUT_SIZE * 4];
    for i in 0..LUT_SIZE {
        let x = i as f32 / (LUT_SIZE - 1) as f32;
        // Find the enclosing stop pair.
        let hi = st.iter().position(|(p, _)| *p >= x).unwrap_or(st.len() - 1);
        let lo = hi.saturating_sub(1);
        let (p0, c0) = st[lo];
        let (p1, c1) = st[hi.min(st.len() - 1)];
        let t = if p1 > p0 { (x - p0) / (p1 - p0) } else { 0.0 }.clamp(0.0, 1.0);
        for ch in 0..3 {
            out[i * 4 + ch] = (c0[ch] as f32 + (c1[ch] as f32 - c0[ch] as f32) * t) as u8;
        }
        out[i * 4 + 3] = 255;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_palette_builds_a_lut() {
        for p in PALETTES {
            let l = lut(p.name);
            assert_eq!(l[3], 255); // alpha
            // First and last entries equal the declared end stops.
            assert_eq!(&l[..3], &p.stops[0].1);
            assert_eq!(
                &l[(LUT_SIZE - 1) * 4..(LUT_SIZE - 1) * 4 + 3],
                &p.stops.last().unwrap().1
            );
        }
    }

    #[test]
    fn unknown_name_falls_back_to_rainbow() {
        assert_eq!(lut("nope"), lut("rainbow"));
    }

    fn feats(calm: f32, energy: f32, build: f32) -> Features {
        Features {
            calm,
            energy,
            build,
            ..Default::default()
        }
    }

    #[test]
    fn auto_is_a_valid_name_but_not_a_gradient() {
        assert!(is_valid(AUTO));
        assert!(all_names().any(|n| n == AUTO));
        assert!(!names().any(|n| n == AUTO));
    }

    #[test]
    fn auto_picks_cool_palettes_in_breakdowns() {
        let mut a = Auto::default();
        let f = feats(1.0, 0.4, 0.0);
        a.pick(&f, 0.0); // mood appears
        let p = a.pick(&f, 2.0); // held 2 beats → switch
        assert!(MOODS[0].contains(&p), "breakdown picked {p}");
    }

    #[test]
    fn auto_ignores_flickering_moods() {
        let mut a = Auto::default();
        let g = feats(0.0, 0.0, 0.0);
        a.pick(&g, 0.0);
        let settled = a.pick(&g, 2.0); // groove pick
        // A one-beat energy spike is not a section change.
        a.pick(&feats(0.0, 0.9, 0.0), 3.0);
        a.pick(&g, 3.5);
        a.pick(&feats(0.0, 0.9, 0.0), 4.0);
        let p = a.pick(&feats(0.0, 0.9, 0.0), 5.0); // held only 1 beat
        assert_eq!(p, settled);
    }

    #[test]
    fn auto_respects_the_dwell() {
        let mut a = Auto::default();
        let g = feats(0.0, 0.0, 0.0);
        a.pick(&g, 0.0);
        a.pick(&g, 2.0); // groove pick lands at beat 2
        let peak = feats(0.0, 0.9, 0.0);
        a.pick(&peak, 3.0);
        // Held long enough but only 4 beats since the last switch.
        assert!(MOODS[1].contains(&a.pick(&peak, 6.0)));
        // Past the 16-beat dwell the peak pick finally lands.
        assert!(MOODS[3].contains(&a.pick(&peak, 20.0)));
    }

    #[test]
    fn a_new_family_applies_at_once() {
        let mut a = Auto::default();
        let g = feats(0.0, 0.0, 0.0);
        a.pick(&g, 0.0);
        a.pick(&g, 2.0); // groove pick from the built-in rotation
        let fam: [Vec<&'static str>; 4] = [vec!["ice"], vec!["gold"], vec!["fire"], vec!["lava"]];
        a.restyle(Some(&fam));
        // Same mood, one frame later, inside the dwell: already the new family.
        assert_eq!(a.pick_in(&g, 2.1, Some(&fam)), "gold");
        a.restyle(None);
        assert!(MOODS[1].contains(&a.pick(&g, 2.2)));
    }
}
