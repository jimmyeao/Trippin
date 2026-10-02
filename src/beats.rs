//! Neural beat + downbeat tracking with Beat This! (`beat-this`, ONNX run
//! through pure-Rust `rten`) — the same model and grid fit BeatDis uses.
//!
//! The spectral-flux autocorrelation in `song.rs` finds the tempo well but
//! guesses the bar phase from bass energy, which drum-roll and beatless
//! intros throw off by 1-3 beats. Beat This! outputs real downbeats, so the
//! grid's "one" comes from the music instead of a heuristic.
//!
//! The ~80 MB of model weights aren't shipped: they download on first use
//! from the public BeatDis-models release into `<data dir>/models`, checked
//! against the same SHA-256 BeatDis pins.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use beat_this::{BeatThis, RtenRuntime};
use sha2::{Digest, Sha256};

const MODELS_BASE_URL: &str = "https://github.com/jimmyeao/BeatDis-models/releases/download/v1";
const BEAT_MODEL: &str = "beat_this.onnx";
const MEL_MODEL: &str = "mel_spectrogram.onnx";
const MODEL_FILES: &[(&str, Option<&str>)] = &[
    (
        BEAT_MODEL,
        Some("5f810debe53459b559127fb55bbad40035bb47cc567b20e501670f968c770f02"),
    ),
    (MEL_MODEL, None),
];

/// Logit frame rate (the mel spectrogram runs at 50 fps).
const FPS: f32 = 50.0;
/// Beats below this confidence are dropped before fitting — beat-this emits
/// garbage in beatless intros and breakdowns (BeatDis `CONF_MIN`).
const CONF_MIN: f32 = 0.7;

/// Download progress / state, shown on the Settings tab.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ModelState {
    #[default]
    Unknown,
    Missing,
    Downloading { file: String, received: u64, total: u64 },
    Ready,
    Failed(String),
}

static STATE: Mutex<ModelState> = Mutex::new(ModelState::Unknown);

pub fn state() -> ModelState {
    let s = STATE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if s == ModelState::Unknown {
        let s = if models_dir().is_some() {
            ModelState::Ready
        } else {
            ModelState::Missing
        };
        set_state(s.clone());
        return s;
    }
    s
}

fn set_state(s: ModelState) {
    *STATE.lock().unwrap_or_else(|e| e.into_inner()) = s;
}

/// Where downloads go: `<data dir>/models`.
fn download_dir() -> PathBuf {
    crate::config::data_dir().join("models")
}

/// The first directory holding every model file — the download dir, then a
/// `models/` next to the exe or in the working dir (dev / bundled builds).
pub fn models_dir() -> Option<PathBuf> {
    let mut c = vec![download_dir(), PathBuf::from("models")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            c.push(dir.join("models"));
        }
    }
    c.into_iter()
        .find(|d| MODEL_FILES.iter().all(|(n, _)| d.join(n).is_file()))
}

static ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Mirror of `Settings::beat_model` — off means song analysis skips the
/// model even when it's downloaded.
pub fn set_enabled(on: bool) {
    ENABLED.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Model downloaded and the feature switched on.
pub fn ready() -> bool {
    ENABLED.load(std::sync::atomic::Ordering::Relaxed) && state() == ModelState::Ready
}

/// Start the model download on a worker thread unless the models are
/// present or a download is already running. Returns immediately.
pub fn ensure_models() {
    match state() {
        ModelState::Ready | ModelState::Downloading { .. } => return,
        _ => {}
    }
    if models_dir().is_some() {
        set_state(ModelState::Ready);
        return;
    }
    set_state(ModelState::Downloading {
        file: String::new(),
        received: 0,
        total: 0,
    });
    std::thread::spawn(|| {
        let res = std::panic::catch_unwind(download)
            .unwrap_or_else(|_| Err(anyhow!("download thread panicked")));
        set_state(match res {
            Ok(()) => ModelState::Ready,
            Err(e) => {
                eprintln!("beat model download failed: {e:#}");
                ModelState::Failed(format!("{e:#}"))
            }
        });
    });
}

fn download() -> Result<()> {
    let dir = download_dir();
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(600)))
        .build()
        .new_agent();
    for (name, sha) in MODEL_FILES {
        let dest = dir.join(name);
        if dest.is_file() {
            continue;
        }
        let url = format!("{MODELS_BASE_URL}/{name}");
        let resp = agent
            .get(&url)
            .call()
            .with_context(|| format!("downloading {name}"))?;
        let total: u64 = resp
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let mut reader = resp.into_body().into_reader();
        let tmp = dest.with_extension("part");
        let mut file = std::fs::File::create(&tmp)?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 1 << 16];
        let mut received = 0u64;
        loop {
            let n = reader.read(&mut buf).with_context(|| format!("reading {name}"))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            hasher.update(&buf[..n]);
            received += n as u64;
            set_state(ModelState::Downloading {
                file: name.to_string(),
                received,
                total,
            });
        }
        drop(file);
        if let Some(expected) = sha {
            let got = format!("{:x}", hasher.finalize());
            if got != *expected {
                let _ = std::fs::remove_file(&tmp);
                return Err(anyhow!("{name}: checksum mismatch"));
            }
        }
        std::fs::rename(&tmp, &dest)?;
    }
    Ok(())
}

/// One detected beat: time (s), confidence (sigmoid of the beat logit), and
/// whether the model marked it as a downbeat.
#[derive(Clone, Copy, Debug)]
pub struct Beat {
    pub t: f32,
    pub conf: f32,
    pub down: bool,
}

/// A loaded tracker. Loading parses ~80 MB of weights, so hold one for a
/// batch of calls rather than building it per call.
pub struct Tracker {
    bt: BeatThis<<RtenRuntime as beat_this::Runtime>::Model>,
}

impl Tracker {
    pub fn load() -> Result<Self> {
        let dir = models_dir().ok_or_else(|| anyhow!("beat model not downloaded"))?;
        let bt = BeatThis::new(&RtenRuntime, &dir.join(MEL_MODEL), &dir.join(BEAT_MODEL))
            .context("loading beat model")?;
        Ok(Self { bt })
    }

    /// Beats in `mono` (any sample rate), times relative to its start.
    pub fn detect(&mut self, mono: &[f32], sr: u32) -> Result<Vec<Beat>> {
        let r = self.bt.analyze_audio(mono, sr).context("beat-this inference")?;
        let logits = &r.beat_logits;
        let n = logits.len() as isize;
        let conf = |t: f32| {
            if n == 0 {
                return 0.0;
            }
            let f = ((t * FPS).round() as isize).clamp(0, n - 1) as usize;
            1.0 / (1.0 + (-logits[f]).exp())
        };
        // Sub-frame position: parabolic interpolation of the logit peak
        // (BeatDis `refine`). Peaks come out on the 20 ms frame grid, which
        // is too coarse to count beats across a long breakdown.
        let refine = |t: f32| -> f32 {
            let f0 = (t * FPS).round() as isize;
            let (mut fm, mut vm) = (f0, f32::NEG_INFINITY);
            for f in (f0 - 2)..=(f0 + 2) {
                if (0..n).contains(&f) && logits[f as usize] > vm {
                    vm = logits[f as usize];
                    fm = f;
                }
            }
            if fm <= 0 || fm >= n - 1 {
                return fm as f32 / FPS;
            }
            let (a, b, c) = (
                logits[(fm - 1) as usize],
                logits[fm as usize],
                logits[(fm + 1) as usize],
            );
            let d = a - 2.0 * b + c;
            let delta = if d.abs() < 1e-9 { 0.0 } else { 0.5 * (a - c) / d };
            (fm as f32 + delta.clamp(-0.5, 0.5)) / FPS
        };
        Ok(r.beats
            .iter()
            .map(|&t| Beat {
                t: refine(t),
                conf: conf(t),
                down: r.downbeats.iter().any(|&d| (d - t).abs() <= 0.025),
            })
            .collect())
    }
}

/// Weighted least squares `t = icpt + slope * n`.
fn regress(pts: &[(f64, f64, f64)]) -> Option<(f64, f64)> {
    let (mut sw, mut swn, mut swx, mut swnn, mut swnx) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
    for &(n, x, w) in pts {
        sw += w;
        swn += w * n;
        swx += w * x;
        swnn += w * n * n;
        swnx += w * n * x;
    }
    let den = sw * swnn - swn * swn;
    if den.abs() < 1e-12 || sw <= 0.0 {
        return None;
    }
    let slope = (sw * swnx - swn * swx) / den;
    (slope > 0.0).then(|| (slope, (swx - slope * swn) / sw))
}

/// A constant-tempo grid: seconds per beat and the time of a downbeat (the
/// earliest at or after t = 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub bpm: f64,
    pub first_downbeat: f64,
    /// How strongly the downbeats agreed on the bar phase (0.25 = no
    /// agreement among four phases, 1 = unanimous).
    pub phase_agreement: f32,
}

/// Fit one tempo + bar phase to detected beats (BeatDis `fit_constant`):
/// confident beats only, double-time hits dropped, integer beat numbering
/// across gaps, confidence-weighted least squares, then the bar phase by a
/// vote of every detected downbeat. `max_bpm` folds faster tempos to half
/// time (Trippin's grids run 87 for drum & bass, like the live tracker).
pub fn fit_grid(beats: &[Beat], max_bpm: f64) -> Option<Grid> {
    let conf: Vec<&Beat> = beats.iter().filter(|b| b.conf >= CONF_MIN).collect();
    if conf.len() < 16 {
        return None;
    }
    let mut ivl: Vec<f32> = conf.windows(2).map(|w| w[1].t - w[0].t).collect();
    ivl.sort_by(|a, b| a.total_cmp(b));
    let median = ivl[ivl.len() / 2];
    if median <= 0.0 {
        return None;
    }
    let mut kept: Vec<&Beat> = vec![conf[0]];
    for &b in &conf[1..] {
        if b.t - kept.last().unwrap().t >= 0.6 * median {
            kept.push(b);
        }
    }
    // Pass 1 — the period from steady runs (every gap within 15% of the
    // median), each run's own regression slope weighted by its span. Runs
    // whose slope disagrees with the median are junk — beat-this sometimes
    // emits confident beats at 1.5x tempo through a breakdown.
    let one = |gap: f32| gap > 0.85 * median && gap < 1.15 * median;
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut s = 0usize;
    for i in 1..=kept.len() {
        if i == kept.len() || !one(kept[i].t - kept[i - 1].t) {
            if i - s >= 8 {
                runs.push((s, i - 1));
            }
            s = i;
        }
    }
    let (mut num_w, mut den_w) = (0.0f64, 0.0f64);
    for &(a, b) in &runs {
        let pts: Vec<(f64, f64, f64)> = (a..=b)
            .map(|i| ((i - a) as f64, kept[i].t as f64, kept[i].conf as f64))
            .collect();
        if let Some((sl, _)) = regress(&pts) {
            if (sl / median as f64 - 1.0).abs() < 0.05 {
                let w = ((b - a) as f64).powi(2);
                num_w += sl * w;
                den_w += w;
            }
        }
    }
    let mut period = if den_w > 0.0 { num_w / den_w } else { median as f64 };

    // Pass 2 — number each beat against the previous accepted one using that
    // precise period; a beat landing between grid lines (off by > 0.2 of a
    // beat) is an outlier and doesn't count. Regress, drop beats far off the
    // fitted line, refit with the refined period.
    let mut numbered: Vec<(usize, i64)> = Vec::new();
    let mut fit: Option<(f64, f64)> = None;
    for _ in 0..3 {
        numbered.clear();
        let (mut ref_t, mut ref_n) = (kept[0].t as f64, 0i64);
        numbered.push((0, 0));
        for (i, b) in kept.iter().enumerate().skip(1) {
            let x = (b.t as f64 - ref_t) / period;
            let k = x.round();
            if k >= 1.0 && (x - k).abs() < 0.2 {
                ref_n += k as i64;
                ref_t = b.t as f64;
                numbered.push((i, ref_n));
            }
        }
        if let Some((sl, ic)) = fit {
            // Residual rejection against the previous pass's line.
            numbered.retain(|&(i, n)| {
                ((kept[i].t as f64 - (ic + sl * n as f64)) / sl).abs() < 0.12
            });
        }
        if numbered.len() < 16 {
            return None;
        }
        let pts: Vec<(f64, f64, f64)> = numbered
            .iter()
            .map(|&(i, n)| (n as f64, kept[i].t as f64, kept[i].conf as f64))
            .collect();
        fit = regress(&pts);
        if let Some((sl, _)) = fit {
            period = sl;
        }
    }
    let (slope, icpt) = fit?;
    // Bar phase: downbeats on accepted beats vote for their beat number
    // mod 4 — junk downbeats in a breakdown's beat burst don't get a say.
    let mut votes = [0.0f32; 4];
    for &(i, n) in &numbered {
        if kept[i].down {
            votes[n.rem_euclid(4) as usize] += kept[i].conf;
        }
    }
    // `numbered` counts from kept[0]; re-express the phase against icpt.
    let n0 = ((kept[numbered[0].0].t as f64 - icpt) / slope).round() as i64 - numbered[0].1;
    let votes = {
        let mut v = [0.0f32; 4];
        for (p, &w) in votes.iter().enumerate() {
            v[(p as i64 + n0).rem_euclid(4) as usize] += w;
        }
        v
    };
    let total: f32 = votes.iter().sum();
    let (phase, best) = votes
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, &v)| (i, v))?;
    if total <= 0.0 {
        return None;
    }
    let mut period = slope;
    let mut bar = 4.0 * slope;
    if 60.0 / slope > max_bpm {
        // Half time: a "beat" spans two real beats and a bar two real bars.
        period *= 2.0;
        bar *= 2.0;
    }
    let mut t0 = icpt + slope * phase as f64;
    t0 -= (t0 / bar).floor() * bar;
    Some(Grid {
        bpm: 60.0 / period,
        first_downbeat: t0,
        phase_agreement: best / total,
    })
}

/// Detect + fit a whole song, cached on disk by path, size and mtime — a
/// full track takes a few seconds of CPU and the answer never changes.
pub fn song_grid(path: &Path, mono: &[f32], sr: u32, max_bpm: f64) -> Result<Grid> {
    let key = cache_key(path);
    let cache = crate::config::data_dir().join("beatcache");
    if let Some(k) = &key {
        if let Ok(t) = std::fs::read_to_string(cache.join(format!("{k}.json"))) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
                if let (Some(bpm), Some(t0), Some(a)) = (
                    v["bpm"].as_f64(),
                    v["first_downbeat"].as_f64(),
                    v["phase_agreement"].as_f64(),
                ) {
                    return Ok(Grid {
                        bpm,
                        first_downbeat: t0,
                        phase_agreement: a as f32,
                    });
                }
            }
        }
    }
    let mut tr = Tracker::load()?;
    let beats = tr.detect(mono, sr)?;
    let g = fit_grid(&beats, max_bpm).ok_or_else(|| anyhow!("too few confident beats"))?;
    if let Some(k) = key {
        let _ = std::fs::create_dir_all(&cache);
        let _ = std::fs::write(
            cache.join(format!("{k}.json")),
            serde_json::json!({
                "bpm": g.bpm,
                "first_downbeat": g.first_downbeat,
                "phase_agreement": g.phase_agreement,
            })
            .to_string(),
        );
    }
    Ok(g)
}

fn cache_key(path: &Path) -> Option<String> {
    let m = std::fs::metadata(path).ok()?;
    let mtime = m
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let mut h = Sha256::new();
    h.update(path.to_string_lossy().as_bytes());
    h.update(m.len().to_le_bytes());
    h.update(mtime.to_le_bytes());
    Some(format!("{:x}", h.finalize())[..24].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 124 BPM, downbeats on beats 2, 6, 10… (bar phase 2), a beatless
    /// low-confidence intro with double-time junk, one drum-roll "downbeat"
    /// on the wrong phase.
    fn synthetic() -> Vec<Beat> {
        let p = 60.0 / 124.0;
        let mut v = Vec::new();
        for i in 0..8 {
            v.push(Beat { t: 0.1 + i as f32 * 0.21, conf: 0.3, down: i == 3 });
        }
        for n in 0..200 {
            let t = 2.0 + n as f32 * p;
            v.push(Beat { t, conf: 0.95, down: n % 4 == 2 });
        }
        v.push(Beat { t: 2.0 + 41.0 * p, conf: 0.9, down: true });
        v.sort_by(|a, b| a.t.total_cmp(&b.t));
        v
    }

    #[test]
    fn fits_tempo_and_bar_phase() {
        let g = fit_grid(&synthetic(), 144.0).unwrap();
        let p = 60.0 / 124.0;
        assert!((g.bpm - 124.0).abs() < 0.05, "bpm {}", g.bpm);
        // The first downbeat is beat 2 of the grid, folded to the earliest
        // bar at or after 0.
        let want = (2.0 + 2.0 * p) % (4.0 * p);
        assert!((g.first_downbeat - want).abs() < 0.01, "t0 {} want {want}", g.first_downbeat);
        assert!(g.phase_agreement > 0.9);
    }

    #[test]
    fn folds_fast_tempos_to_half_time() {
        let p = 60.0 / 172.0;
        let beats: Vec<Beat> = (0..300)
            .map(|n| Beat { t: 1.0 + n as f32 * p, conf: 0.9, down: n % 4 == 0 })
            .collect();
        let g = fit_grid(&beats, 144.0).unwrap();
        assert!((g.bpm - 86.0).abs() < 0.05, "bpm {}", g.bpm);
    }

    #[test]
    fn too_few_beats_is_none() {
        let beats: Vec<Beat> = (0..10)
            .map(|n| Beat { t: n as f32 * 0.5, conf: 0.9, down: n % 4 == 0 })
            .collect();
        assert!(fit_grid(&beats, 144.0).is_none());
    }
}

/// `trippin --beats <file>`: the autocorrelation grid next to the Beat
/// This! grid, with timings — for checking a track whose bars land wrong.
pub fn beat_test(path: &Path) -> Result<()> {
    ensure_models();
    let mut last = String::new();
    loop {
        match state() {
            ModelState::Ready => break,
            ModelState::Failed(e) => return Err(anyhow!("model download failed: {e}")),
            ModelState::Downloading { file, received, total } => {
                let line = format!("downloading {file} {} / {} MB", received >> 20, total >> 20);
                if line != last {
                    println!("{line}");
                    last = line;
                }
            }
            _ => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let t = std::time::Instant::now();
    // The onset grid alone, for comparison.
    set_enabled(false);
    let song = crate::song::load(path)?;
    set_enabled(true);
    println!(
        "{}: decode + onset grid {:.1}s — {:.2} BPM, first beat {:.3}s",
        song.name,
        t.elapsed().as_secs_f32(),
        song.bpm,
        song.first_beat
    );
    let t = std::time::Instant::now();
    let mut tr = Tracker::load()?;
    println!("model load {:.1}s", t.elapsed().as_secs_f32());
    let t = std::time::Instant::now();
    let beats = tr.detect(&song.mono, song.sr)?;
    let secs = t.elapsed().as_secs_f32();
    println!(
        "inference {secs:.1}s for {:.0}s of audio ({:.0}x realtime)",
        song.duration,
        song.duration as f32 / secs.max(1e-3)
    );
    let confident = beats.iter().filter(|b| b.conf >= CONF_MIN).count();
    let downs: Vec<String> = beats
        .iter()
        .filter(|b| b.down)
        .take(8)
        .map(|b| format!("{:.2}", b.t))
        .collect();
    println!(
        "{} beats ({confident} confident), first downbeats: {}",
        beats.len(),
        downs.join(" ")
    );
    if std::env::var("TRIPPIN_BEATS_DEBUG").is_ok() {
        let mut w = 0.0f32;
        while w < song.duration as f32 {
            let v: Vec<&Beat> = beats.iter().filter(|b| b.t >= w && b.t < w + 30.0).collect();
            let c: Vec<f32> = v
                .windows(2)
                .filter(|p| p[0].conf >= CONF_MIN && p[1].conf >= CONF_MIN)
                .map(|p| p[1].t - p[0].t)
                .collect();
            let mut s = c.clone();
            s.sort_by(|a, b| a.total_cmp(b));
            let med = s.get(s.len() / 2).copied().unwrap_or(0.0);
            let mean = if c.is_empty() { 0.0 } else { c.iter().sum::<f32>() / c.len() as f32 };
            println!(
                "  {w:>4.0}s  beats {:>3} conf {:>3} downs {:>2}  median {:>6.2} mean {:>6.2} BPM",
                v.len(),
                v.iter().filter(|b| b.conf >= CONF_MIN).count(),
                v.iter().filter(|b| b.down).count(),
                if med > 0.0 { 60.0 / med } else { 0.0 },
                if mean > 0.0 { 60.0 / mean } else { 0.0 }
            );
            w += 30.0;
        }
    }
    match fit_grid(&beats, 144.0) {
        Some(g) => {
            let bar = 4.0 * 60.0 / g.bpm;
            let shift = ((song.first_beat - g.first_downbeat) / (60.0 / g.bpm)).round() as i64;
            println!(
                "beat-this grid: {:.2} BPM, first downbeat {:.3}s, phase agreement {:.0}% \
                 (onset grid is {} beat(s) off the bar; bar = {bar:.3}s)",
                g.bpm,
                g.first_downbeat,
                g.phase_agreement * 100.0,
                shift.rem_euclid(4)
            );
        }
        None => println!("beat-this grid: too few confident beats"),
    }
    Ok(())
}
