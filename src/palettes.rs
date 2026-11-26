//! Named gradient palettes (WLED-style) compiled into a 256-entry LUT that
//! the GPU samples in `palette()`. Scenes never see the names — they just
//! call `palette(t)` and get whatever the user picked globally.
//!
//! A palette is a list of `(position, [r,g,b])` stops, position in 0..1,
//! interpolated linearly in sRGB. The shader ping-pong mirrors the index,
//! so non-cyclic palettes don't need their first/last stops to match.
//!
//! `AUTO` is the pseudo-palette: the render loop feeds it through [`Auto`],
//! which maps the music's mood to one of the named gradients. `ART` is the
//! other: the colours of the playing track's cover (`albumart.rs` publishes
//! them through [`set_art`]), falling back to `AUTO` while there's none.

use crate::audio::Features;

pub const LUT_SIZE: usize = 256;

/// The "pick for me" palette — a `Settings.palette` value, not a gradient.
pub const AUTO: &str = "auto";

/// "The cover's colours" — a `Settings.palette` value, not a gradient. The
/// render loop keys the live cover as `art#<serial>` (see [`is_art`]).
pub const ART: &str = "art";

pub type Lut = [u8; LUT_SIZE * 4];

/// The cover palette now playing, with the serial it was published under.
static ART_LUT: std::sync::Mutex<Option<(u64, std::sync::Arc<Lut>)>> = std::sync::Mutex::new(None);

/// Publish (or clear) the cover palette.
pub fn set_art(v: Option<(u64, std::sync::Arc<Lut>)>) {
    *ART_LUT.lock().unwrap_or_else(|e| e.into_inner()) = v;
}

/// The cover palette now playing, if any.
pub fn art() -> Option<(u64, std::sync::Arc<Lut>)> {
    ART_LUT.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// `ART`, or a live cover's key (`art#<serial>`).
pub fn is_art(name: &str) -> bool {
    name == ART || name.starts_with("art#")
}

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

/// Every selectable palette value — `AUTO` and `ART` first, then the named
/// gradients. For pickers and name validation; the render loop resolves the
/// two pseudo-palettes itself.
pub fn all_names() -> impl Iterator<Item = &'static str> {
    [AUTO, ART].into_iter().chain(names())
}

/// `AUTO`, `ART` or a named gradient.
pub fn is_valid(name: &str) -> bool {
    name == AUTO || name == ART || names().any(|n| n == name)
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
    /// Start on `name` (a Style change puts its family on screen immediately
    /// instead of waiting for the mood picker's hold and dwell windows).
    pub fn start_with(&mut self, name: &'static str) {
        self.name = name;
    }

    /// The palette matching this frame's mood. `beat` is the running beat
    /// clock (`f.beat_position(now)`) used for the hold/dwell windows.
    #[allow(dead_code)] // the render loop uses `pick_in`; kept for the tests and tools
    pub fn pick(&mut self, f: &Features, beat: f64) -> &'static str {
        self.pick_in(f, beat, None)
    }

    /// `pick`, with a Style's own palette family per mood (`styles::mood_palettes`).
    /// A mood whose list is empty falls back to the built-in rotation.
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

/// 256×4 RGBA bytes for the named palette (falls back to `rainbow`). `ART`
/// gives the live cover's palette, or the `AUTO` swatch with no cover.
pub fn lut(name: &str) -> [u8; LUT_SIZE * 4] {
    if is_art(name) {
        return art().map_or_else(|| lut(AUTO), |(_, l)| *l);
    }
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
    lut_from_stops(pal.stops)
}

/// A LUT from `(position, colour)` stops, positions ascending in 0..1,
/// interpolated linearly in sRGB.
pub fn lut_from_stops(st: &[(f32, [u8; 3])]) -> Lut {
    let mut out = [0u8; LUT_SIZE * 4];
    if st.is_empty() {
        return out;
    }
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

/// A timed fade between LUTs, for the `ART` palette: a new cover blends in
/// over about a bar instead of the hard cut a LUT swap would be.
pub struct Fade {
    from: Lut,
    to: Lut,
    cur: Lut,
    key: String,
    start: f32,
    len: f32,
    done: bool,
}

impl Fade {
    /// Start from what's on screen now.
    pub fn new(on_screen: Lut) -> Self {
        Self { from: on_screen, to: on_screen, cur: on_screen, key: String::new(), start: 0.0, len: 1.0, done: false }
    }

    /// The fade has reached its target.
    pub fn done(&self) -> bool {
        self.done
    }

    /// Aim at `key` (a new key starts a fade from what's showing, over
    /// `len` seconds). Returns the LUT to show when it changed since the
    /// last call.
    pub fn step(&mut self, key: &str, lut: impl FnOnce() -> Lut, now: f32, len: f32) -> Option<&Lut> {
        if key != self.key {
            self.key = key.to_string();
            self.from = self.cur;
            self.to = lut();
            self.start = now;
            self.len = len.max(0.01);
            self.done = false;
        }
        if self.done {
            return None;
        }
        let t = ((now - self.start) / self.len).clamp(0.0, 1.0);
        let e = t * t * (3.0 - 2.0 * t);
        for i in 0..LUT_SIZE * 4 {
            self.cur[i] = (self.from[i] as f32 + (self.to[i] as f32 - self.from[i] as f32) * e + 0.5) as u8;
        }
        self.done = t >= 1.0;
        Some(&self.cur)
    }
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
    fn art_is_valid_and_falls_back_to_the_auto_swatch() {
        assert!(is_valid(ART));
        assert!(!is_valid("art#3"));
        assert!(is_art("art#3") && is_art(ART) && !is_art("autumn"));
        assert!(!names().any(|n| n == ART));
        // No cover published in this test binary: the auto swatch.
        if art().is_none() {
            assert_eq!(lut(ART), lut(AUTO));
        }
    }

    #[test]
    fn fade_blends_then_settles() {
        let a = lut("ocean");
        let b = lut("fire");
        let mut f = Fade::new(a);
        assert_eq!(f.step("x", || b, 0.0, 2.0).copied(), Some(a)); // starts where it was
        let mid = *f.step("x", || b, 1.0, 2.0).unwrap();
        assert!(mid != a && mid != b);
        assert_eq!(f.step("x", || b, 2.5, 2.0).copied(), Some(b));
        assert!(f.step("x", || b, 3.0, 2.0).is_none()); // settled: nothing to upload
        // A new key mid-fade starts from what's showing.
        f.step("y", || a, 4.0, 2.0);
        let m2 = *f.step("y", || a, 5.0, 2.0).unwrap();
        assert!(m2 != a && m2 != b);
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
}
