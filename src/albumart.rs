//! Album art for the `art` palette (and the now-playing card).
//!
//! The now-playing sources attach a cheap [`ArtHint`] to each track: a file
//! path, a URL or the image bytes (`nowplaying.rs`). A worker thread here
//! watches the shown track and turns its hint into a picture:
//!
//! 1. image files are read as they are (rekordbox's thumbnails, Mixxx's
//!    cover files);
//! 2. audio files give their embedded front cover (ID3 APIC, FLAC PICTURE,
//!    MP4 `covr`, via `lofty`), else a `cover`/`folder`/`front` image in
//!    their folder;
//! 3. URLs are fetched (Spotify on macOS);
//! 4. with nothing found, and only when the user turned it on, the iTunes
//!    Search API is asked by artist and title (it sends the track name to
//!    Apple, so it's off by default). Results are cached on disk.
//!
//! The picture becomes a palette by k-means in Lab ([`stops_from_image`]):
//! the main colours, greys and specks dropped, ordered dark to light behind
//! a dark stop, a little more saturated (scenes square the palette). It is
//! published through `palettes::set_art`, and the render loop fades to it
//! over a bar.

use crate::nowplaying::{ArtHint, SharedNowPlaying, Track};
use crate::palettes::{self, Lut};
use image::{RgbImage, RgbaImage};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A resolved cover.
pub struct Art {
    pub lut: Arc<Lut>,
    /// 256x256, centre-cropped square, for the now-playing card.
    pub thumb: Arc<RgbaImage>,
    /// Where it came from, for `--nowplaying` and the panel.
    pub from: String,
}

#[derive(Default)]
pub struct ArtState {
    /// Bumped on every change of `art` (the card re-uploads on it).
    pub serial: u64,
    pub art: Option<Arc<Art>>,
}

pub type SharedArt = Arc<Mutex<ArtState>>;

/// Start the resolver. `online` is the Settings toggle for the iTunes
/// lookup, read on every new track.
pub fn start(np: SharedNowPlaying, online: Arc<AtomicBool>, cache: PathBuf) -> SharedArt {
    let out: SharedArt = Arc::default();
    let shared = out.clone();
    std::thread::Builder::new()
        .name("albumart".into())
        .spawn(move || run(np, online, cache, shared))
        .ok();
    out
}

fn run(np: SharedNowPlaying, online: Arc<AtomicBool>, cache: PathBuf, out: SharedArt) {
    let mut seen = (u64::MAX, false);
    let mut misses = std::collections::HashSet::new();
    loop {
        std::thread::sleep(Duration::from_millis(250));
        let on = online.load(Ordering::Relaxed);
        let (serial, track) = {
            let st = np.lock().unwrap_or_else(|e| e.into_inner());
            (st.art_serial, st.track.clone())
        };
        if (serial, on) == seen {
            continue;
        }
        seen = (serial, on);
        // Nothing playing (Spotify paused, a gap between tracks): keep the
        // last cover's colours rather than drop back to the auto palette.
        let Some(t) = track else { continue };
        let art = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| resolve(&t, on, &cache, &mut misses))).ok().flatten();
        let art = art.and_then(|(bytes, from)| std::panic::catch_unwind(|| build(&bytes, from)).ok().flatten());
        let mut st = out.lock().unwrap_or_else(|e| e.into_inner());
        st.serial += 1;
        palettes::set_art(art.as_ref().map(|a| (st.serial, a.lut.clone())));
        if let Some(a) = &art {
            println!("Album art: {} ({})", t.line(), a.from);
        }
        st.art = art.map(Arc::new);
    }
}

/// The encoded image for a track, and where it was found.
fn resolve(
    t: &Track,
    online: bool,
    cache: &Path,
    misses: &mut std::collections::HashSet<String>,
) -> Option<(Vec<u8>, String)> {
    let found = match &t.art {
        ArtHint::None => None,
        ArtHint::Bytes(b) => Some((b.to_vec(), format!("{} thumbnail", t.source))),
        ArtHint::Url(u) => fetch(u).map(|b| (b, "artwork URL".to_string())),
        ArtHint::Files(fs) => crate::artfind::from_files(fs),
    };
    if found.is_some() || !online || t.title.trim().is_empty() {
        return found;
    }
    let key = t.line().to_lowercase();
    if misses.contains(&key) {
        return None;
    }
    let file = cache.join(format!("{:016x}.img", fnv(&key)));
    if let Ok(b) = std::fs::read(&file) {
        return Some((b, "iTunes (cached)".into()));
    }
    match itunes(&t.artist, &t.title) {
        Some(b) => {
            let _ = std::fs::create_dir_all(cache);
            let _ = std::fs::write(&file, &b);
            Some((b, "iTunes".into()))
        }
        None => {
            misses.insert(key);
            None
        }
    }
}

pub use crate::artfind::embedded;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(6)))
        .tls_config(crate::config::tls())
        .build()
        .new_agent()
}

fn fetch(url: &str) -> Option<Vec<u8>> {
    agent()
        .get(url)
        .call()
        .ok()?
        .body_mut()
        .with_config()
        .limit(16 << 20)
        .read_to_vec()
        .ok()
}

/// iTunes Search: the first song match's artwork at 600 px.
fn itunes(artist: &str, title: &str) -> Option<Vec<u8>> {
    let term = format!("{artist} {title}");
    let v: serde_json::Value = agent()
        .get("https://itunes.apple.com/search")
        .query("term", term.trim())
        .query("entity", "song")
        .query("limit", "1")
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    let url = v["results"][0]["artworkUrl100"].as_str()?;
    fetch(&url.replace("100x100bb", "600x600bb"))
}

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// Decode a cover and make its palette and thumbnail.
pub fn build(bytes: &[u8], from: String) -> Option<Art> {
    let img = image::load_from_memory(bytes).ok()?;
    let (w, h) = (img.width(), img.height());
    if w < 8 || h < 8 {
        return None;
    }
    let side = w.min(h);
    let sq = img.crop_imm((w - side) / 2, (h - side) / 2, side, side);
    let small = sq.resize_exact(64, 64, image::imageops::FilterType::Triangle).to_rgb8();
    let thumb = sq.resize_exact(256, 256, image::imageops::FilterType::CatmullRom).to_rgba8();
    let stops = stops_from_image(&small);
    Some(Art { lut: Arc::new(palettes::lut_from_stops(&stops)), thumb: Arc::new(thumb), from })
}

// ---- colour ---------------------------------------------------------------

type Lab = [f32; 3];

fn to_lin(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn from_lin(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

fn lab_f(t: f32) -> f32 {
    if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 }
}

fn lab_finv(t: f32) -> f32 {
    let t3 = t * t * t;
    if t3 > 0.008856 { t3 } else { (t - 16.0 / 116.0) / 7.787 }
}

pub fn rgb_to_lab(c: [u8; 3]) -> Lab {
    let (r, g, b) = (to_lin(c[0]), to_lin(c[1]), to_lin(c[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let (fx, fy, fz) = (lab_f(x), lab_f(y), lab_f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

pub fn lab_to_rgb(l: Lab) -> [u8; 3] {
    let fy = (l[0] + 16.0) / 116.0;
    let fx = fy + l[1] / 500.0;
    let fz = fy - l[2] / 200.0;
    let (x, y, z) = (lab_finv(fx) * 0.95047, lab_finv(fy), lab_finv(fz) * 1.08883);
    let r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
    let g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
    let b = 0.0557 * x - 0.2040 * y + 1.0570 * z;
    [from_lin(r), from_lin(g), from_lin(b)]
}

fn chroma(l: &Lab) -> f32 {
    (l[1] * l[1] + l[2] * l[2]).sqrt()
}

fn dist2(a: &Lab, b: &Lab) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Lab with its chroma scaled and lightness set, hue kept.
fn with_lc(c: &Lab, l: f32, chroma_scale: f32) -> Lab {
    [l, c[1] * chroma_scale, c[2] * chroma_scale]
}

/// Rotate a colour's hue by `deg`.
fn hue_shift(c: &Lab, deg: f32) -> Lab {
    let (s, co) = deg.to_radians().sin_cos();
    [c[0], c[1] * co - c[2] * s, c[1] * s + c[2] * co]
}

/// k-means in Lab, deterministic: k-means++ seeding from a fixed LCG.
/// Returns (centre, share of pixels), heaviest first.
fn kmeans(px: &[Lab], k: usize) -> Vec<(Lab, f32)> {
    let mut rng = 0x9e3779b97f4a7c15u64;
    let mut next = || {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (rng >> 33) as f32 / (1u64 << 31) as f32
    };
    let mut cs: Vec<Lab> = vec![px[px.len() / 2]];
    while cs.len() < k {
        let d: Vec<f32> = px.iter().map(|p| cs.iter().map(|c| dist2(p, c)).fold(f32::MAX, f32::min)).collect();
        let total: f32 = d.iter().sum();
        if total <= 0.0 {
            break;
        }
        let mut r = next() * total;
        let mut pick = px.len() - 1;
        for (i, v) in d.iter().enumerate() {
            r -= v;
            if r <= 0.0 {
                pick = i;
                break;
            }
        }
        cs.push(px[pick]);
    }
    let mut assign = vec![0usize; px.len()];
    for _ in 0..12 {
        for (i, p) in px.iter().enumerate() {
            assign[i] = (0..cs.len())
                .min_by(|&a, &b| dist2(p, &cs[a]).total_cmp(&dist2(p, &cs[b])))
                .unwrap_or(0);
        }
        let mut sum = vec![[0.0f32; 4]; cs.len()];
        for (i, p) in px.iter().enumerate() {
            let s = &mut sum[assign[i]];
            s[0] += p[0];
            s[1] += p[1];
            s[2] += p[2];
            s[3] += 1.0;
        }
        for (c, s) in cs.iter_mut().zip(&sum) {
            if s[3] > 0.0 {
                *c = [s[0] / s[3], s[1] / s[3], s[2] / s[3]];
            }
        }
    }
    let mut count = vec![0usize; cs.len()];
    for a in &assign {
        count[*a] += 1;
    }
    let n = px.len() as f32;
    let mut out: Vec<(Lab, f32)> = cs.into_iter().zip(count).filter(|(_, c)| *c > 0).map(|(c, k)| (c, k as f32 / n)).collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
}

/// A cover's palette stops: its main colours ordered dark to light behind a
/// dark stop of the darkest one's hue. Greys, near-blacks and specks are
/// dropped; a cover with one colour gets two neighbours of it, and a grey
/// one a tinted monochrome ramp.
pub fn stops_from_image(img: &RgbImage) -> Vec<(f32, [u8; 3])> {
    let px: Vec<Lab> = img.pixels().map(|p| rgb_to_lab(p.0)).collect();
    let clusters = kmeans(&px, 7);
    // Colourful, visible and not a speck.
    let mut keep: Vec<Lab> = Vec::new();
    for (c, share) in &clusters {
        if *share < 0.02 || chroma(c) < 14.0 || c[0] < 8.0 || c[0] > 97.0 {
            continue;
        }
        // Merge near-duplicates (the heavier one, earlier in the list, wins).
        if keep.iter().any(|k| dist2(k, c) < 15.0 * 15.0) {
            continue;
        }
        keep.push(*c);
        if keep.len() == 5 {
            break;
        }
    }
    let boost = 1.2;
    let mut cols: Vec<Lab> = keep.iter().map(|c| with_lc(c, c[0].clamp(25.0, 88.0), boost)).collect();
    match cols.len() {
        0 => {
            // Grey cover: a monochrome ramp, tinted by whatever cast it has
            // (or cool, if none).
            let mean = clusters.iter().fold([0.0f32; 3], |a, (c, w)| [a[0] + c[0] * w, a[1] + c[1] * w, a[2] + c[2] * w]);
            let tint = if chroma(&mean) > 2.0 {
                let s = 22.0 / chroma(&mean);
                [0.0, mean[1] * s, mean[2] * s]
            } else {
                [0.0, -4.0, -20.0]
            };
            cols = vec![
                [35.0, tint[1] * 1.2, tint[2] * 1.2],
                [62.0, tint[1], tint[2]],
                [88.0, tint[1] * 0.5, tint[2] * 0.5],
            ];
        }
        1 => {
            let c = cols[0];
            cols.push(hue_shift(&with_lc(&c, (c[0] + 22.0).min(90.0), 0.8), 30.0));
            cols.push(hue_shift(&with_lc(&c, (c[0] - 18.0).max(28.0), 1.0), -30.0));
        }
        2 => {
            let (a, b) = (cols[0], cols[1]);
            cols.push(with_lc(&a, ((a[0] + b[0]) / 2.0 + 25.0).min(92.0), 0.6));
        }
        _ => {}
    }
    cols.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let dark = with_lc(&cols[0], 9.0, 0.7);
    let all: Vec<Lab> = std::iter::once(dark).chain(cols).collect();
    let n = all.len() - 1;
    all.iter().enumerate().map(|(i, c)| (i as f32 / n as f32, lab_to_rgb(*c))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(f: impl Fn(u32, u32) -> [u8; 3]) -> RgbImage {
        RgbImage::from_fn(64, 64, |x, y| image::Rgb(f(x, y)))
    }

    #[test]
    fn lab_round_trips() {
        for c in [[0, 0, 0], [255, 255, 255], [200, 30, 90], [10, 200, 140], [40, 60, 250]] {
            let back = lab_to_rgb(rgb_to_lab(c));
            for i in 0..3 {
                assert!((back[i] as i32 - c[i] as i32).abs() <= 2, "{c:?} -> {back:?}");
            }
        }
    }

    #[test]
    fn two_colour_cover_keeps_both_hues() {
        // Half red, half blue.
        let s = stops_from_image(&img(|x, _| if x < 32 { [220, 30, 40] } else { [30, 60, 220] }));
        assert!(s.len() >= 3, "{s:?}");
        assert_eq!(s[0].0, 0.0);
        assert_eq!(s.last().unwrap().0, 1.0);
        let reddish = s.iter().any(|(_, c)| c[0] > 150 && c[2] < 120);
        let blueish = s.iter().any(|(_, c)| c[2] > 150 && c[0] < 120);
        assert!(reddish && blueish, "{s:?}");
        // Starts dark.
        let d = s[0].1;
        assert!(d.iter().map(|&v| v as u32).sum::<u32>() < 150, "{s:?}");
    }

    #[test]
    fn grey_cover_gives_a_ramp_not_a_crash() {
        let s = stops_from_image(&img(|x, y| {
            let v = ((x + y) * 2) as u8;
            [v, v, v]
        }));
        assert_eq!(s.len(), 4, "{s:?}");
        // Dark to light.
        let lum = |c: [u8; 3]| c.iter().map(|&v| v as u32).sum::<u32>();
        assert!(s.windows(2).all(|w| lum(w[0].1) <= lum(w[1].1)), "{s:?}");
    }

    #[test]
    fn single_colour_cover_gets_neighbours() {
        let s = stops_from_image(&img(|_, _| [240, 160, 20]));
        assert_eq!(s.len(), 4, "{s:?}");
    }

    #[test]
    fn specks_are_ignored() {
        // Black cover with a 4x4 green speck (<0.5%): the speck is no stop.
        let s = stops_from_image(&img(|x, y| if x < 4 && y < 4 { [0, 255, 0] } else { [180, 40, 160] }));
        assert!(!s.iter().any(|(_, c)| c[1] > 200 && c[0] < 100), "{s:?}");
    }

    #[test]
    fn build_decodes_png_and_makes_a_thumb() {
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(img(|x, _| [x as u8 * 4, 50, 200]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let a = build(&png, "test".into()).unwrap();
        assert_eq!(a.thumb.dimensions(), (256, 256));
        assert_eq!(a.lut[3], 255);
        assert!(build(b"not an image", "x".into()).is_none());
    }

    /// Covers embedded in real files: set `TRIPPIN_ART_FILE` to an audio file.
    #[test]
    fn embedded_cover_from_env_file() {
        let Ok(f) = std::env::var("TRIPPIN_ART_FILE") else { return };
        let b = embedded(Path::new(&f)).expect("no embedded cover");
        let a = build(&b, f.clone()).expect("cover didn't decode");
        let stops = stops_from_image(&image::load_from_memory(&b).unwrap().resize_exact(64, 64, image::imageops::FilterType::Triangle).to_rgb8());
        println!("{f}: {} bytes, stops {stops:?}", b.len());
        assert_eq!(a.thumb.dimensions(), (256, 256));
    }
}
