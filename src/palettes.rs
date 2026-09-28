//! Named gradient palettes (WLED-style) compiled into a 256-entry LUT that
//! the GPU samples in `palette()`. Scenes never see the names — they just
//! call `palette(t)` and get whatever the user picked globally.
//!
//! A palette is a list of `(position, [r,g,b])` stops, position in 0..1,
//! interpolated linearly in sRGB. The shader ping-pong mirrors the index,
//! so non-cyclic palettes don't need their first/last stops to match.

pub const LUT_SIZE: usize = 256;

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

/// 256×4 RGBA bytes for the named palette (falls back to `rainbow`).
pub fn lut(name: &str) -> [u8; LUT_SIZE * 4] {
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
}
