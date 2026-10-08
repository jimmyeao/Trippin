//! Audio capture (WASAPI loopback or a chosen input device) and live analysis:
//! band energies, onsets, tempo and beat phase. The render loop reads a
//! `Features` snapshot each frame and extrapolates the beat phase from it.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;

#[path = "beateval.rs"]
mod eval;
pub use eval::{beat_eval, beat_eval_live};

pub const SPECTRUM_BINS: usize = 32;

pub(crate) const FFT_SIZE: usize = 2048;
pub(crate) const HOP: usize = 512;
const ENV_SECONDS: f32 = 8.0;
const MIN_BPM: f32 = 70.0;
const MAX_BPM: f32 = 180.0;
/// DJ convention: detected tempos above this are almost always a
/// double-time read (DnB, hard techno) — the tracker runs them at
/// half-tempo so visuals breathe on the half-time pulse (a 160 read is 80).
/// Every path that sets the tempo obeys it, and `frame` enforces it last.
pub(crate) const HALF_TEMPO_ABOVE: f32 = 150.0;
/// Neural downbeat check: seconds of audio per Beat This! window, and how
/// often one is sent (~0.5 s of one background core per window).
const NN_WINDOW_S: f32 = 15.0;
const NN_EVERY_S: f32 = 5.0;
/// The first window after start-up is sent once this much audio is in.
const NN_FIRST_S: f32 = 8.0;
/// Seconds of beats (not breakdown) a window needs before it's worth sending.
const NN_MIN_BEATS_S: f32 = 4.5;
/// How long a neural fit keeps the onset comb and the autocorrelation's
/// fine tempo out of it.
const NN_FRESH_S: f32 = 45.0;
/// Time constant of a phase correction: the live beat slews onto the
/// measured one (rate-limited, so it never runs backwards) rather than
/// jumping.
const SLEW_S: f32 = 0.5;

/// A window of recent audio for the neural downbeat worker; `start` is the
/// absolute sample index of `samples[0]`.
struct NnJob {
    samples: Vec<f32>,
    sr: u32,
    start: u64,
}

/// Detected beats in a window.
type NnResult = Result<Vec<NnBeat>, String>;

/// The worker: loads the model once, then turns windows into downbeats.
/// Always replies, even when inference panics, so the analyser never waits
/// on a dead job.
fn spawn_nn_worker() -> (mpsc::Sender<NnJob>, mpsc::Receiver<NnResult>) {
    let (tx, rx) = mpsc::channel::<NnJob>();
    let (rtx, rrx) = mpsc::channel::<NnResult>();
    let _ = std::thread::Builder::new()
        .name("beat-nn".into())
        .spawn(move || {
            let mut tracker: Option<crate::beats::Tracker> = None;
            while let Ok(job) = rx.recv() {
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if tracker.is_none() {
                        tracker = Some(crate::beats::Tracker::load().map_err(|e| format!("{e:#}"))?);
                    }
                    let beats = tracker
                        .as_mut()
                        .unwrap()
                        .detect(&job.samples, job.sr)
                        .map_err(|e| format!("{e:#}"))?;
                    // The window's edges lack context — skip their first and
                    // last second.
                    let dur = job.samples.len() as f32 / job.sr as f32;
                    Ok(beats
                        .iter()
                        .filter(|b| b.conf >= 0.5 && b.t > 1.0 && b.t < dur - 1.0)
                        .map(|b| (job.start + (b.t * job.sr as f32) as u64, b.conf, b.down))
                        .collect())
                }))
                .unwrap_or_else(|_| Err("beat worker panicked".into()));
                if rtx.send(res).is_err() {
                    break;
                }
            }
        });
    (tx, rrx)
}

/// A beat from the neural worker: absolute sample index, confidence, and
/// whether the model marked it a downbeat.
type NnBeat = (u64, f32, bool);

/// A straight beat grid through one window's beats: beat `n` at sample
/// `icpt + period * n`.
#[derive(Debug, Clone)]
struct WindowFit {
    /// Samples per beat.
    period: f64,
    icpt: f64,
    /// Grid index of the last inlier beat.
    n_last: i64,
    /// Inlier count, and their summed confidence (the fit's weight).
    count: usize,
    score: f32,
    /// Residual RMS, as a fraction of the period.
    rms: f64,
    /// Grid index mod 4 of the downbeats, when they agree.
    down_class: Option<i64>,
    /// The downbeats' summed confidence per grid index mod 4.
    down_votes: [f32; 4],
}

impl WindowFit {
    /// Sample index of grid beat `n`.
    fn at(&self, n: i64) -> f64 {
        self.icpt + self.period * n as f64
    }
    fn bpm(&self, sr: f32) -> f64 {
        60.0 * sr as f64 / self.period
    }
}

/// Candidate grids through a window's beats, one per trial period: the
/// window's median interval, and `hint` (the live period) when given.
/// Beats are numbered along the period from each of the first few beats
/// (so an off-beat first detection can't drag the whole fit onto the
/// off-beat); beats far from a grid line are left out, which keeps a
/// double-time hi-hat run or a second track in the mix from breaking the fit.
fn fit_window(pts: &[NnBeat], sr: f32, min: usize, hint: Option<f64>) -> Vec<WindowFit> {
    if pts.len() < min {
        return Vec::new();
    }
    let t: Vec<f64> = pts.iter().map(|b| b.0 as f64).collect();
    let mut ibis: Vec<f64> = t.windows(2).map(|w| w[1] - w[0]).filter(|&d| d > 0.0).collect();
    if ibis.is_empty() {
        return Vec::new();
    }
    ibis.sort_by(f64::total_cmp);
    let med = ibis[ibis.len() / 2];
    let mut periods = vec![med];
    if let Some(h) = hint.filter(|h| (h / med - 1.0).abs() > 0.02) {
        periods.push(h);
    }
    let mut out = Vec::new();
    for p0 in periods {
        if !(55.0..=290.0).contains(&(60.0 * sr as f64 / p0)) {
            continue;
        }
        let best = (0..pts.len().min(4))
            .filter_map(|start| fit_from(&t, pts, start, p0, min, sr))
            .max_by(|a, b| a.score.total_cmp(&b.score));
        out.extend(best);
    }
    out
}

fn fit_from(t: &[f64], pts: &[NnBeat], start: usize, p0: f64, min: usize, sr: f32) -> Option<WindowFit> {
    let len = pts.len();
    // Recent beats weigh more (time constant 6 s), so a tempo ramp (a
    // pitch-fader move) is followed rather than averaged over the window.
    let tau = 6.0 * sr as f64;
    let end = t[len - 1];
    let mut n = vec![0i64; len];
    let mut inl = vec![false; len];
    inl[start] = true;
    let mut prev = start;
    for i in start + 1..len {
        let x = (t[i] - t[prev]) / p0;
        let k = x.round();
        if k >= 1.0 && (x - k).abs() < 0.25 {
            n[i] = n[prev] + k as i64;
            inl[i] = true;
            prev = i;
        }
    }
    let numbered = inl.clone();
    let (mut slope, mut icpt) = (p0, t[start]);
    for _ in 0..3 {
        let (mut sw, mut swn, mut swx, mut swnn, mut swnx) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
        for i in (0..len).filter(|&i| inl[i]) {
            let (x, k) = (t[i], n[i] as f64);
            let w = pts[i].1.max(0.05) as f64 * ((x - end) / tau).exp();
            sw += w;
            swn += w * k;
            swx += w * x;
            swnn += w * k * k;
            swnx += w * k * x;
        }
        let den = sw * swnn - swn * swn;
        if den.abs() < 1e-9 {
            return None;
        }
        slope = (sw * swnx - swn * swx) / den;
        icpt = (swx - slope * swn) / sw;
        if slope <= 0.0 {
            return None;
        }
        for i in 0..len {
            inl[i] = numbered[i] && (t[i] - icpt - slope * n[i] as f64).abs() < 0.12 * slope;
        }
    }
    let idx: Vec<usize> = (0..len).filter(|&i| inl[i]).collect();
    let (&fi, &li) = (idx.first()?, idx.last()?);
    let slots = (n[li] - n[fi] + 1) as f32;
    // Most grid lines in the span must hold a beat.
    if idx.len() < min || (idx.len() as f32) < 0.6 * slots {
        return None;
    }
    let rms = (idx.iter().map(|&i| (t[i] - icpt - slope * n[i] as f64).powi(2)).sum::<f64>()
        / idx.len() as f64)
        .sqrt()
        / slope;
    if rms > 0.05 {
        return None;
    }
    let mut votes = [0.0f32; 4];
    let mut count = [0u32; 4];
    for &i in idx.iter().filter(|&&i| pts[i].2) {
        let c = n[i].rem_euclid(4) as usize;
        votes[c] += pts[i].1;
        count[c] += 1;
    }
    let total: f32 = votes.iter().sum();
    let best = (0..4).max_by(|&a, &b| votes[a].total_cmp(&votes[b])).unwrap_or(0);
    let down_class = (count[best] >= 2 && votes[best] >= 0.75 * total).then_some(best as i64);
    Some(WindowFit {
        period: slope,
        icpt,
        n_last: n[li],
        count: idx.len(),
        score: idx.iter().map(|&i| pts[i].1).sum(),
        rms,
        down_class,
        down_votes: votes,
    })
}

/// Snapshot of the analysis, shared with the renderer.
#[derive(Clone, Debug)]
pub struct Features {
    /// Normalised 0..1 band levels.
    pub bass: f32,
    pub mid: f32,
    pub high: f32,
    pub energy: f32,
    /// Slow energy trend: >0 while building, <0 while dropping away.
    pub build: f32,
    /// Decaying 0..1 pulses fired on detected onsets.
    pub onset: f32,
    pub kick: f32,
    pub spectrum: [f32; SPECTRUM_BINS],
    /// Time-domain trace: 64 samples over the last ~21 ms, for scope scenes.
    pub waveform: [f32; 64],
    pub bpm: f32,
    /// Beat phase 0..1 at `phase_at`, and the beat count at that moment.
    pub beat_phase: f32,
    pub beat_count: u64,
    pub phase_at: Instant,
    /// Which beat (0..3) of the bar is the downbeat.
    pub downbeat: u64,
    /// True when there is effectively no signal.
    pub silent: bool,
    pub tempo_confidence: f32,
    /// How steadily drum hits (kicks) are landing, 0..1 over the last few
    /// seconds — high in a four-on-the-floor section, ~0 when the drums drop.
    pub groove: f32,
    /// Breakdown state, 0 = beats playing, 1 = breakdown (no drums: pads,
    /// vocals, pure instrumental). Hysteretic and smoothed, so it never
    /// flickers; falls fast when the drums come back in (the drop).
    pub calm: f32,
    /// Synesthesia-style four-band vocabulary: bass (20-150 Hz), mid
    /// (150 Hz-2 kHz), mid-high (2-6 kHz), high (6-16 kHz).
    /// `lvl4`: loudness 0..1 (auto-gained, fast attack / slower release).
    pub lvl4: [f32; 4],
    /// `hits4`: 0..1 spikes on transients in each band, decaying (~0.15 s).
    pub hits4: [f32; 4],
    /// `pres4`: slow (~1.5 s) rise/fall of each band — builds and swells,
    /// not individual notes.
    pub pres4: [f32; 4],
}

impl Default for Features {
    fn default() -> Self {
        Self {
            bass: 0.0,
            mid: 0.0,
            high: 0.0,
            energy: 0.0,
            build: 0.0,
            onset: 0.0,
            kick: 0.0,
            spectrum: [0.0; SPECTRUM_BINS],
            waveform: [0.0; 64],
            bpm: 120.0,
            beat_phase: 0.0,
            beat_count: 0,
            phase_at: Instant::now(),
            downbeat: 0,
            silent: true,
            tempo_confidence: 0.0,
            groove: 0.0,
            calm: 1.0,
            lvl4: [0.0; 4],
            hits4: [0.0; 4],
            pres4: [0.0; 4],
        }
    }
}

impl Features {
    /// Beat position (whole beats + fraction) extrapolated to `now`.
    pub fn beat_position(&self, now: Instant) -> f64 {
        let dt = now.saturating_duration_since(self.phase_at).as_secs_f64();
        // Extrapolate only to the end of the current beat — if the analyser
        // stalls (e.g. a paused show engine feeding nothing), the beat clock
        // must freeze rather than run away on a stale tempo.
        let budget = (1.0 - self.beat_phase as f64).max(0.0) * 60.0 / self.bpm.max(1.0) as f64;
        self.beat_count as f64 + self.beat_phase as f64 + dt.min(budget) * self.bpm as f64 / 60.0
    }

    /// Beat index within the bar (0 = downbeat) for a given beat position.
    pub fn beat_in_bar(&self, pos: f64) -> u64 {
        ((pos.floor() as i64 - self.downbeat as i64).rem_euclid(4)) as u64
    }
}

pub type SharedFeatures = Arc<Mutex<Features>>;

/// Rolling log of the raw onset (spectral-flux) envelope — one entry per
/// analysis hop. The timeline matcher cross-correlates this against a
/// loaded song's envelope to find where in the track the live audio is.
pub struct EnvLog {
    /// Onset strength per hop, newest last, capped ~90 s.
    pub env: VecDeque<f32>,
    /// Total hops ever pushed (env index of the newest sample is hops-1).
    pub hops: u64,
    /// Hops per second (sample_rate / HOP).
    pub fps: f32,
}

impl EnvLog {
    pub fn new() -> SharedEnv {
        Arc::new(Mutex::new(Self {
            env: VecDeque::new(),
            hops: 0,
            fps: 0.0,
        }))
    }
    fn push(&mut self, v: f32, fps: f32) {
        self.fps = fps;
        let cap = (90.0 * fps).max(1.0) as usize;
        self.env.push_back(v);
        while self.env.len() > cap {
            self.env.pop_front();
        }
        self.hops += 1;
    }
}

pub type SharedEnv = Arc<Mutex<EnvLog>>;

/// Commands from the UI thread to the analyser.
pub enum Command {
    /// Treat the beat nearest to now as the downbeat.
    MarkDownbeat,
}

enum Backend {
    // Kept for its lifetime only — dropping the stream stops capture.
    Cpal(#[allow(dead_code)] cpal::Stream),
    #[cfg(target_os = "macos")]
    System(#[allow(dead_code)] crate::sysaudio::SystemCapture),
    /// Timeline playback — the player is owned by the render loop; the
    /// backend just marks which source the analyser is hearing.
    Song,
}

pub struct AudioEngine {
    _backend: Backend,
    pub features: SharedFeatures,
    pub commands: mpsc::Sender<Command>,
    pub device_name: String,
    /// The stream reported a fatal error (`DeviceNotAvailable`). The
    /// render loop rebuilds the engine when this fires — a dead stream
    /// never recovers on its own.
    pub dead: Arc<AtomicBool>,
}

pub fn list_devices() -> Result<()> {
    let host = cpal::default_host();
    #[cfg(target_os = "windows")]
    {
        println!("Output devices (captured via loopback, the default):");
        for d in host.output_devices()? {
            println!("  {}", device_name(&d));
        }
    }
    #[cfg(target_os = "macos")]
    println!("(system audio is captured directly by default — these are the fallbacks)");
    println!("Input devices (use --device \"<part of name>\"):");
    for d in host.input_devices()? {
        println!("  {}", device_name(&d));
    }
    Ok(())
}

/// Device names for the panel's audio-in picker. On Windows the outputs
/// are listed too — they capture via loopback (same as `--device`).
pub fn capture_device_names() -> Vec<String> {
    let host = cpal::default_host();
    let names: Vec<String> = host
        .input_devices()
        .map(|ds| ds.map(|d| device_name(&d)).collect())
        .unwrap_or_default();
    #[cfg(windows)]
    let names = {
        let mut names = names;
        if let Ok(outs) = host.output_devices() {
            names.extend(outs.map(|d| device_name(&d)));
        }
        names
    };
    names
}

/// Label for `Settings::audio_in == ""` — the platform default tap.
pub fn system_audio_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "System audio (output mix)"
    } else if cfg!(windows) {
        "System output (loopback)"
    } else {
        "Default input"
    }
}

fn device_name(d: &cpal::Device) -> String {
    d.description()
        .map(|desc| desc.name().to_string())
        .unwrap_or_else(|_| "<unknown>".into())
}

impl AudioEngine {
    /// `device`: None = system audio — WASAPI loopback on Windows, the
    /// ScreenCaptureKit output mix on macOS (`mic` forces the default input
    /// there instead). Otherwise the first device whose name contains the
    /// string — inputs plus, on Windows, outputs via loopback.
    pub fn start(device: Option<&str>, mic: bool, tap: Option<SharedEnv>) -> Result<Self> {
        let (rx, sample_rate, name, backend, dead) = Self::open(device, mic)?;
        Self::spawn(rx, sample_rate, name, backend, tap, dead)
    }

    /// Open the capture stream: mono chunks on the returned channel.
    fn open(
        device: Option<&str>,
        mic: bool,
    ) -> Result<(mpsc::Receiver<Vec<f32>>, f32, String, Backend, Arc<AtomicBool>)> {
        let _ = mic; // only consulted on macOS (system-audio vs input choice)
        let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(64);

        #[cfg(target_os = "macos")]
        if device.is_none() && !mic {
            match crate::sysaudio::start(tx.clone()) {
                Ok(cap) => {
                    return Ok((
                        rx,
                        crate::sysaudio::SAMPLE_RATE,
                        "system audio".into(),
                        Backend::System(cap),
                        Arc::new(AtomicBool::new(false)),
                    ));
                }
                Err(e) => eprintln!("system audio unavailable: {e:#} — using the default input"),
            }
        }

        let host = cpal::default_host();
        let dev = match device {
            #[cfg(target_os = "windows")]
            None => host
                .default_output_device()
                .ok_or_else(|| anyhow!("no default output device"))?,
            #[cfg(not(target_os = "windows"))]
            None => host
                .default_input_device()
                .ok_or_else(|| anyhow!("no default input device"))?,
            Some(needle) => {
                let needle = needle.to_lowercase();
                let mut inputs = host.input_devices()?;
                let found = inputs.find(|d| device_name(d).to_lowercase().contains(&needle));
                #[cfg(target_os = "windows")]
                let found = found.or_else(|| {
                    host.output_devices().ok().and_then(|mut outs| {
                        outs.find(|d| device_name(d).to_lowercase().contains(&needle))
                    })
                });
                found.ok_or_else(|| {
                    anyhow!("no audio device matching {needle:?}; try --list-devices")
                })?
            }
        };
        let name = device_name(&dev);
        // Output devices only expose an output config; building an input
        // stream on them makes WASAPI capture in loopback mode.
        let supported = dev
            .default_input_config()
            .or_else(|_| dev.default_output_config())
            .context("querying device config")?;
        let config = supported.config();
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate as f32;

        let dead = Arc::new(AtomicBool::new(false));
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build_stream::<f32>(&dev, &config, channels, tx, dead.clone())?,
            SampleFormat::I16 => build_stream::<i16>(&dev, &config, channels, tx, dead.clone())?,
            SampleFormat::I32 => build_stream::<i32>(&dev, &config, channels, tx, dead.clone())?,
            SampleFormat::U16 => build_stream::<u16>(&dev, &config, channels, tx, dead.clone())?,
            f => return Err(anyhow!("unsupported sample format {f:?}")),
        };
        stream.play()?;
        Ok((rx, sample_rate, name, Backend::Cpal(stream), dead))
    }

    /// `--record-audio [device] secs out.wav`: capture the analyser's mono
    /// feed to a 32-bit float WAV, for replaying a live session offline
    /// (`--beat-eval`).
    pub fn record(device: Option<&str>, secs: f32, out: &std::path::Path) -> Result<()> {
        use std::io::{Seek, SeekFrom, Write};
        let (rx, sr, name, _backend, _dead) = Self::open(device, true)?;
        println!("Recording {secs:.0} s from {name} ({sr} Hz) -> {}", out.display());
        let mut w = std::io::BufWriter::new(std::fs::File::create(out)?);
        let header = |n: u32| -> Vec<u8> {
            let mut h = Vec::with_capacity(44);
            h.extend_from_slice(b"RIFF");
            h.extend_from_slice(&(36 + n * 4).to_le_bytes());
            h.extend_from_slice(b"WAVEfmt ");
            h.extend_from_slice(&16u32.to_le_bytes());
            h.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
            h.extend_from_slice(&1u16.to_le_bytes());
            h.extend_from_slice(&(sr as u32).to_le_bytes());
            h.extend_from_slice(&(sr as u32 * 4).to_le_bytes());
            h.extend_from_slice(&4u16.to_le_bytes());
            h.extend_from_slice(&32u16.to_le_bytes());
            h.extend_from_slice(b"data");
            h.extend_from_slice(&(n * 4).to_le_bytes());
            h
        };
        w.write_all(&header(0))?;
        let total = (secs * sr) as u32;
        let mut n = 0u32;
        let mut next_note = sr as u32 * 60;
        while n < total {
            let chunk = rx.recv_timeout(Duration::from_secs(5))?;
            for s in chunk {
                w.write_all(&s.to_le_bytes())?;
                n += 1;
            }
            if n >= next_note {
                println!("  {} min", n / (sr as u32 * 60));
                next_note += sr as u32 * 60;
            }
        }
        w.seek(SeekFrom::Start(0))?;
        w.write_all(&header(n))?;
        w.flush()?;
        println!("{:.1} s written", n as f32 / sr);
        Ok(())
    }

    /// Play a timeline's clip regions: the player drives the analyser and
    /// the speakers together so the visuals react to the show. Played audio
    /// is resampled to the first region's rate before analysis. Returns the
    /// engine plus the controllable player.
    pub fn start_show(
        regions: Vec<crate::song::Region>,
        start_s: f64,
        tap: Option<SharedEnv>,
    ) -> Result<(Self, crate::song::ShowPlayer)> {
        let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(64);
        let analysis_sr = regions.first().map(|r| r.song.sr as f64).unwrap_or(48000.0);
        let name = format!(
            "show: {}",
            regions
                .iter()
                .map(|r| r.song.name.as_str())
                .collect::<Vec<_>>()
                .join(" + ")
        );
        let player = crate::song::ShowPlayer::start(regions, analysis_sr, start_s, tx)?;
        let eng = Self::spawn(
            rx,
            analysis_sr as f32,
            name,
            Backend::Song,
            tap,
            Arc::new(AtomicBool::new(false)),
        )?;
        Ok((eng, player))
    }

    /// Shared tail: the analysis thread and the feature snapshot channel.
    fn spawn(
        rx: mpsc::Receiver<Vec<f32>>,
        sample_rate: f32,
        device_name: String,
        backend: Backend,
        tap: Option<SharedEnv>,
        dead: Arc<AtomicBool>,
    ) -> Result<Self> {
        let features: SharedFeatures = Arc::new(Mutex::new(Features::default()));
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let shared = features.clone();
        std::thread::Builder::new()
            .name("analysis".into())
            .spawn(move || Analyzer::new(sample_rate, shared, cmd_rx, tap).run(rx))?;

        Ok(Self {
            _backend: backend,
            features,
            commands: cmd_tx,
            device_name,
            dead,
        })
    }
}

fn build_stream<T>(
    dev: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    tx: mpsc::SyncSender<Vec<f32>>,
    dead: Arc<AtomicBool>,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let rate = config.sample_rate;
    // The analyser keeps time by counting samples, so lost audio leaves
    // the beat late for good (the neural check can't see it: it counts
    // samples too). An overrun drops captured audio, and so does a full
    // channel when analysis stalls; both are put back as silence of the
    // same length. A gap shows as a buffer's capture time (QPC on WASAPI,
    // within ±0.03 ms) later than the last buffer's length predicts.
    let gap_debug = std::env::var_os("TRIPPIN_GAP_DEBUG").is_some();
    let mut last_capture: Option<(cpal::StreamInstant, usize)> = None;
    let mut owed: usize = 0;
    let stream = dev.build_input_stream::<T, _, _>(
        config.clone(),
        move |data: &[T], info: &cpal::InputCallbackInfo| {
            let frames = data.len() / channels.max(1);
            let ts = info.timestamp().capture;
            if let Some((lt, ln)) = last_capture {
                if let Some(d) = ts.checked_duration_since(lt) {
                    let gap = d.as_secs_f64() - ln as f64 / rate as f64;
                    // Over a second is a stall the watchdog handles, not a glitch.
                    if gap > 0.001 && gap < 1.0 {
                        owed += (gap * rate as f64).round() as usize;
                        if gap_debug {
                            eprintln!("audio gap {:.1} ms filled", gap * 1000.0);
                        }
                    }
                }
            }
            last_capture = Some((ts, frames));
            // Clip recorder: first two channels, interleaved.
            let out_ch = channels.min(2);
            crate::rec::audio_in(rate, out_ch as u16, || {
                data.chunks(channels)
                    .flat_map(|f| f[..out_ch].iter().map(|&s| s.to_sample::<f32>()))
                    .collect()
            });
            let mut mono: Vec<f32> = Vec::with_capacity(owed + frames);
            mono.resize(owed, 0.0);
            mono.extend(data.chunks(channels).map(|frame| {
                frame.iter().map(|&s| s.to_sample::<f32>()).sum::<f32>() / channels as f32
            }));
            // Drop audio rather than block the audio thread if analysis
            // stalls — but owe it, so the beat clock keeps real time.
            let n = mono.len();
            owed = match tx.try_send(mono) {
                Ok(()) => 0,
                Err(_) => n.min(rate as usize),
            };
        },
        move |e| {
            eprintln!("audio stream error: {e}");
            use cpal::ErrorKind as K;
            // Fatal kinds: the stream never recovers, the render loop
            // rebuilds. Xrun is a glitch, DeviceChanged already rerouted,
            // Busy/Realtime are transient.
            if !matches!(
                e.kind(),
                K::Xrun | K::DeviceChanged | K::DeviceBusy | K::RealtimeDenied
            ) {
                dead.store(true, Ordering::Relaxed);
            }
        },
        None,
    )?;
    Ok(stream)
}

/// Adaptive peak normaliser: tracks a slowly decaying peak so levels stay in
/// 0..1 whatever the master volume is.
struct AutoGain {
    peak: f32,
    floor: f32,
}

impl AutoGain {
    fn new(floor: f32) -> Self {
        Self { peak: floor, floor }
    }
    fn apply(&mut self, x: f32) -> f32 {
        self.peak = (self.peak * 0.9995).max(x).max(self.floor);
        (x / self.peak).clamp(0.0, 1.0)
    }
}

struct Analyzer {
    sr: f32,
    fps: f32,
    shared: SharedFeatures,
    commands: mpsc::Receiver<Command>,
    fft: Arc<dyn rustfft::Fft<f32>>,
    window: Vec<f32>,
    buf: VecDeque<f32>,
    since_hop: usize,
    prev_mag: Vec<f32>,
    prev_bass: f32,
    gains: [AutoGain; 3],
    spec_gain: AutoGain,
    flux_gain: AutoGain,
    /// Level-independent kick detection: low-band flux against its own
    /// recent peak and mean.
    bass_gain: AutoGain,
    bass_hist: VecDeque<f32>,
    /// Four-band vocabulary state (see `Features::lvl4`).
    gains4: [AutoGain; 4],
    hit_gains4: [AutoGain; 4],
    prev_log4: [f32; 4],
    flux_mean4: [f32; 4],
    /// Onset strength envelope at `fps`, newest last.
    env: VecDeque<f32>,
    /// Optional tap so the timeline matcher can watch the same envelope.
    tap: Option<SharedEnv>,
    bass_env: VecDeque<f32>,
    flux_hist: VecDeque<f32>,
    frames_since_tempo: usize,
    // Beat tracker state.
    period: f32, // frames per beat
    phase: f32,  // 0..1
    beat_count: u64,
    pending_bpm: Option<(f32, u32)>,
    /// A challenger that keeps winning but never by the margin — counted
    /// separately so a real, sustained tempo change can't be locked out.
    held_bpm: Option<(f32, u32)>,
    confidence: f32,
    downbeat_votes: [f32; 4],
    downbeat: u64,
    beat_bass: f32,
    // Smoothed outputs.
    f: Features,
    energy_slow: f32,
    energy_fast: f32,
    /// Times (s, analysis clock) of recent kick hits — the groove detector.
    kick_times: VecDeque<f32>,
    /// Analysis clock in seconds.
    clock: f32,
    /// Breakdown target the smoothed `calm` eases toward (hysteresis).
    calm_target: f32,
    /// Seconds the groove has sat below the breakdown threshold.
    quiet_for: f32,
    /// Neural downbeat check: the last `NN_WINDOW_S` of samples, the beat
    /// position (`beat_count + phase`) at each hop keyed by sample index,
    /// the total samples seen, and the worker channels (spawned lazily).
    nn_ring: VecDeque<f32>,
    /// (sample index, beat position, beatless) per hop.
    nn_hops: VecDeque<(u64, f64, bool)>,
    nn_samples: u64,
    nn_next: f32,
    nn_busy: bool,
    nn_tx: Option<mpsc::Sender<NnJob>>,
    nn_rx: Option<mpsc::Receiver<NnResult>>,
    /// Wait for each window's result (offline `--groove-test` runs faster
    /// than realtime, so a late reply would miss its hops).
    nn_sync: bool,
    /// Windows that moved the downbeat vote (for `--groove-test`).
    nn_applied: u32,
    /// Clock of the last neural fit taken (f32::MIN: none yet).
    nn_lock_at: f32,
    /// A different tempo seen by the last window(s), and how many in a row.
    nn_tempo_pending: Option<(f32, u32)>,
    /// A big phase correction seen by the last window, awaiting a second.
    nn_phase_pending: Option<(f64, u32)>,
    /// Clock of the last bar move by the model.
    nn_bar_moved_at: f32,
    /// Clock the bar evidence was last updated.
    nn_down_at: f32,
    /// Downbeat evidence per live bar slot, decaying window to window.
    nn_down_ev: [f32; 4],
    /// The bar has been set by a neural window: the bass vote stops moving it.
    nn_down_locked: bool,
    /// Seconds of continuous beats (not beatless) up to now.
    beat_s: f32,
    /// Phase correction still to apply, in beats (see `SLEW_S`).
    slew: f32,
    /// Flywheel through drumless breaks: once the tempo has been locked with
    /// beats playing (`had_lock`), a beatless stretch (silence, or the kick
    /// density below the breakdown threshold) coasts on that tempo and phase
    /// instead of re-estimating from an envelope with no rhythm in it — which
    /// wandered the BPM and slipped the beat count, resetting bars/phrases.
    had_lock: bool,
    /// Clock when the current beatless stretch began (None while beats play).
    beatless_since: Option<f32>,
    /// Seconds the kick density has been back at beat level (leaving a
    /// beatless stretch needs a bar of it).
    beats_back_for: f32,
    /// The flywheel is coasting this hop (for `--beat-eval`).
    coasting: bool,
}

/// Longest a drumless stretch coasts on the old tempo. Past it the lock is
/// forgotten, so a track that genuinely has no kick still gets tracked.
const FLYWHEEL_MAX_S: f32 = 90.0;

impl Analyzer {
    fn new(
        sr: f32,
        shared: SharedFeatures,
        commands: mpsc::Receiver<Command>,
        tap: Option<SharedEnv>,
    ) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(FFT_SIZE);
        let window = (0..FFT_SIZE)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos())
            .collect();
        let fps = sr / HOP as f32;
        Self {
            sr,
            fps,
            shared,
            commands,
            fft,
            window,
            buf: VecDeque::with_capacity(FFT_SIZE * 2),
            since_hop: 0,
            prev_mag: vec![0.0; FFT_SIZE / 2],
            prev_bass: 0.0,
            gains: [
                AutoGain::new(1e-3),
                AutoGain::new(1e-4),
                AutoGain::new(1e-5),
            ],
            spec_gain: AutoGain::new(1e-4),
            flux_gain: AutoGain::new(1e-4),
            bass_gain: AutoGain::new(1e-3),
            bass_hist: VecDeque::new(),
            gains4: [
                AutoGain::new(1e-3),
                AutoGain::new(1e-4),
                AutoGain::new(1e-5),
                AutoGain::new(1e-5),
            ],
            hit_gains4: [
                AutoGain::new(1e-3),
                AutoGain::new(1e-3),
                AutoGain::new(1e-3),
                AutoGain::new(1e-3),
            ],
            prev_log4: [0.0; 4],
            flux_mean4: [0.0; 4],
            env: VecDeque::new(),
            tap,
            bass_env: VecDeque::new(),
            flux_hist: VecDeque::new(),
            frames_since_tempo: 0,
            period: fps * 60.0 / 120.0,
            phase: 0.0,
            beat_count: 0,
            pending_bpm: None,
            held_bpm: None,
            confidence: 0.0,
            downbeat_votes: [0.0; 4],
            downbeat: 0,
            beat_bass: 0.0,
            f: Features::default(),
            energy_slow: 0.0,
            energy_fast: 0.0,
            kick_times: VecDeque::new(),
            clock: 0.0,
            calm_target: 1.0,
            quiet_for: 0.0,
            nn_ring: VecDeque::new(),
            nn_hops: VecDeque::new(),
            nn_samples: 0,
            nn_next: NN_EVERY_S,
            nn_busy: false,
            nn_tx: None,
            nn_rx: None,
            nn_sync: false,
            nn_applied: 0,
            nn_lock_at: f32::MIN,
            nn_tempo_pending: None,
            nn_down_ev: [0.0; 4],
            nn_phase_pending: None,
            nn_bar_moved_at: f32::MIN,
            nn_down_at: 0.0,
            nn_down_locked: false,
            beat_s: 0.0,
            slew: 0.0,
            had_lock: false,
            beatless_since: None,
            beats_back_for: 0.0,
            coasting: false,
        }
    }

    /// Feed one sample; runs an analysis hop when one is due. Returns true
    /// when a hop ran.
    fn push(&mut self, s: f32) -> bool {
        self.buf.push_back(s);
        if self.buf.len() > FFT_SIZE {
            self.buf.pop_front();
        }
        self.nn_samples += 1;
        if crate::beats::ready() {
            self.nn_ring.push_back(s);
            let cap = (NN_WINDOW_S * self.sr) as usize;
            while self.nn_ring.len() > cap {
                self.nn_ring.pop_front();
            }
        } else if !self.nn_ring.is_empty() {
            self.nn_ring.clear();
        }
        self.since_hop += 1;
        if self.since_hop >= HOP && self.buf.len() == FFT_SIZE {
            self.since_hop = 0;
            self.frame();
            return true;
        }
        false
    }

    /// Beat position (`beat_count + phase`) at an absolute sample index,
    /// interpolated between logged hops; None outside the log.
    fn beat_pos_at(&self, sample: u64) -> Option<f64> {
        let i = self.nn_hops.partition_point(|h| h.0 <= sample);
        if i == 0 || i >= self.nn_hops.len() {
            return None;
        }
        let (s0, p0, _) = self.nn_hops[i - 1];
        let (s1, p1, _) = self.nn_hops[i];
        let f = (sample - s0) as f64 / (s1 - s0).max(1) as f64;
        Some(p0 + (p1 - p0) * f)
    }

    /// Whether the beat was missing (silence, or a drumless stretch) at a
    /// logged sample index.
    fn beatless_at(&self, sample: u64) -> bool {
        let i = self.nn_hops.partition_point(|h| h.0 <= sample);
        i == 0 || self.nn_hops.get(i - 1).is_none_or(|h| h.2)
    }

    /// A neural fit landed recently: it owns tempo, phase and the bar, and
    /// the onset comb and autocorrelation stand back.
    fn nn_fresh(&self) -> bool {
        self.clock - self.nn_lock_at < NN_FRESH_S
    }

    /// Send a window to the neural worker when one is due, and fold any
    /// finished window into the beat grid.
    fn nn_tick(&mut self, silent: bool, beatless: bool) {
        // Log this hop's beat position at the newest sample — the time the
        // render loop reads it at (`phase_at`), so a neural fit puts the live
        // beat on the real one as the app shows it.
        self.nn_hops
            .push_back((self.nn_samples, self.beat_count as f64 + self.phase as f64, beatless));
        let cap = ((NN_WINDOW_S + NN_EVERY_S * 3.0) * self.fps) as usize;
        while self.nn_hops.len() > cap {
            self.nn_hops.pop_front();
        }
        if beatless {
            self.beat_s = 0.0;
        } else {
            self.beat_s += 1.0 / self.fps;
        }
        let results: Vec<NnResult> = self
            .nn_rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for r in results {
            self.nn_busy = false;
            match r {
                Ok(beats) => self.nn_apply(&beats),
                Err(e) => eprintln!("neural beat check: {e}"),
            }
        }
        // A window is worth sending once it holds a few seconds of beats —
        // or, while coasting through a breakdown, to check the flywheel.
        // Before the first lock a shorter window gets the grid right sooner.
        let min_s = if self.nn_lock_at == f32::MIN { NN_FIRST_S } else { NN_WINDOW_S };
        let have = self.nn_ring.len() as f32 / self.sr;
        if self.nn_busy
            || silent
            || self.clock < self.nn_next
            || have < min_s
            || (self.beat_s < NN_MIN_BEATS_S && !self.coasting)
        {
            return;
        }
        self.nn_next = self.clock + NN_EVERY_S;
        if self.nn_tx.is_none() {
            let (tx, rx) = spawn_nn_worker();
            self.nn_tx = Some(tx);
            self.nn_rx = Some(rx);
        }
        let job = NnJob {
            samples: self.nn_ring.iter().copied().collect(),
            sr: self.sr as u32,
            start: self.nn_samples - self.nn_ring.len() as u64,
        };
        if self.nn_tx.as_ref().is_some_and(|tx| tx.send(job).is_ok()) {
            self.nn_busy = true;
            if self.nn_sync {
                if let Some(Ok(r)) = self.nn_rx.as_ref().map(|rx| rx.recv()) {
                    self.nn_busy = false;
                    if let Ok(beats) = r {
                        self.nn_apply(&beats);
                    }
                }
            }
        }
    }

    /// Fold one window's beats into the live grid. The window's beats are
    /// fitted to a straight grid (period + phase, outliers rejected); the fit
    /// sets the tempo outright, and the live phase slews onto it so the beat
    /// count never jumps (a jump re-labels bars and restarts phrases).
    /// While coasting through a breakdown only a fit that agrees with the
    /// flywheel is taken — and then for phase alone, never the bar.
    fn nn_apply(&mut self, beats: &[NnBeat]) {
        let coasting = self.coasting;
        // Beats from where the music had a beat; through a breakdown, all
        // of them (the stricter coasting gates apply).
        let mut pts: Vec<NnBeat> = beats
            .iter()
            .copied()
            .filter(|b| coasting || !self.beatless_at(b.0))
            .collect();
        pts.sort_by_key(|b| b.0);
        let min = if coasting { 16 } else { 8 };
        let locked = self.nn_lock_at != f32::MIN;
        let live_p = self.period as f64 * HOP as f64;
        let fits = fit_window(&pts, self.sr, min, locked.then_some(live_p));
        // Each candidate at the live metrical level: a fit at double or half
        // the locked tempo is the same grid counted differently (the model
        // flips between levels in builds and on hats), not a tempo change.
        // Unlocked, a double-time read runs at half tempo by convention.
        struct Cand {
            fit: WindowFit,
            period: f64,
            /// Anchor beat: sample index and its grid index in `fit`.
            anchor: f64,
            anchor_n: i64,
            /// Converted between levels: its downbeats don't map onto bars.
            converted: bool,
            p_e: f64,
        }
        let mut cands: Vec<Cand> = Vec::new();
        for f in fits {
            let r = f.period / live_p;
            // Over the half-tempo line always (locked or not: a clean 160
            // read once locked used to be taken as a tempo change).
            let double = f.bpm(self.sr) > HALF_TEMPO_ABOVE as f64
                || (locked && (r - 0.5).abs() < 0.015);
            let half = locked && (r - 2.0).abs() < 0.06;
            let mut period = f.period;
            let mut anchor_n = f.n_last;
            if double {
                period *= 2.0;
                // Every other beat: the parity on the live beat when locked,
                // else the downbeats' parity.
                let pick = |n: i64| -> f64 {
                    if locked {
                        self.beat_pos_at(f.at(n).max(0.0) as u64)
                            .map(|p| (p - p.round()).abs())
                            .unwrap_or(1.0)
                    } else {
                        f.down_class.map_or(0.0, |c| ((n - c).rem_euclid(2)) as f64)
                    }
                };
                if pick(f.n_last - 1) < pick(f.n_last) {
                    anchor_n = f.n_last - 1;
                }
            } else if half {
                period /= 2.0;
            }
            let anchor = f.at(anchor_n);
            let Some(p_e) = self.beat_pos_at(anchor.max(0.0) as u64) else { continue };
            cands.push(Cand { fit: f, period, anchor, anchor_n, converted: double || half, p_e });
        }
        // Prefer a candidate at the live tempo, and among those one that
        // agrees with the live phase unless another is much stronger.
        let agrees = |c: &Cand| (c.period / live_p - 1.0).abs() < 0.03;
        let phase_ok = |c: &Cand| (c.p_e - c.p_e.round()).abs() < 0.25;
        let pick = if locked && cands.iter().any(|c| agrees(c)) {
            let best = cands
                .iter()
                .filter(|c| agrees(c))
                .map(|c| c.fit.score)
                .fold(0.0f32, f32::max);
            cands
                .iter()
                .filter(|c| agrees(c) && phase_ok(c) && c.fit.score >= 0.7 * best)
                .max_by(|a, b| a.fit.score.total_cmp(&b.fit.score))
                .or_else(|| cands.iter().filter(|c| agrees(c)).max_by(|a, b| a.fit.score.total_cmp(&b.fit.score)))
        } else {
            cands.iter().max_by(|a, b| a.fit.score.total_cmp(&b.fit.score))
        };
        let Some(c) = pick else {
            self.nn_debug("no fit", &pts, None);
            return;
        };
        let fit = &c.fit;
        let period = (c.period / HOP as f64) as f32;
        let agrees = agrees(c);
        let rel = period / self.period - 1.0;
        // A clean fit: tight residuals over enough beats. Busy mixes (two
        // tracks in a transition, mic bleed) give looser ones, which only
        // nudge the phase and never set the tempo.
        let clean = fit.rms <= 0.025 && fit.count >= if locked { 12 } else { 8 };
        // Through a breakdown the flywheel's tempo stands: the model finds
        // "beats" in pure pads too (129.2 on a 128 synthetic break, which
        // slipped the grid a beat by the drop). A fit may only correct the
        // phase, and only when it agrees with the flywheel closely.
        let mean_conf = fit.score / fit.count.max(1) as f32;
        if coasting && !(fit.rms < 0.04 && mean_conf >= 0.7 && rel.abs() < 0.005) {
            self.nn_debug("coasting: fit disagrees", &pts, Some(fit));
            return;
        }
        // The live position the anchor beat should have had: the nearest
        // whole beat, so a correction is at most half a beat either way.
        let n_e = c.p_e.round();
        let e = c.p_e - n_e;
        if !agrees {
            // A different tempo (a new track) must be a clean fit, and once
            // anything is locked, seen twice running.
            let strong = fit.rms < 0.035 && fit.count >= if locked { 16 } else { 12 };
            let votes = match self.nn_tempo_pending {
                Some((p, n)) if (p / period - 1.0).abs() < 0.02 => n + 1,
                _ => 1,
            };
            self.nn_tempo_pending = strong.then_some((period, votes));
            if !strong || (locked && votes < 2) {
                self.nn_debug("tempo change pending", &pts, Some(fit));
                return;
            }
        } else {
            // A big phase correction once locked (over a quarter beat; through
            // a breakdown, over 0.12) must be measured twice running, a flip
            // of nearly half a beat three times: one window over a transition
            // between two tracks a fraction of a beat apart pulled the beat
            // half a beat off for 19 s, and windows overlap by 10 s, so two
            // in a row can share one confusing stretch (a half-beat flip and
            // back 20 s later on a mic recording).
            let lim = if coasting { 0.12 } else { 0.25 };
            if locked && e.abs() > lim {
                let seen = match self.nn_phase_pending {
                    Some((p, n)) if {
                        let d = (e - p).rem_euclid(1.0);
                        d.min(1.0 - d) < 0.1
                    } => n + 1,
                    _ => 1,
                };
                let need = if e.abs() > 0.4 { 3 } else { 2 };
                if seen < need {
                    self.nn_phase_pending = Some((e, seen));
                    self.nn_debug("phase change pending", &pts, Some(fit));
                    return;
                }
            }
            self.nn_phase_pending = None;
        }
        // Tempo: a clean fit within 1% sets it; a bigger step (a pitch
        // change) needs a second clean window to agree. Never while coasting.
        let new_tempo = !agrees;
        if agrees && clean && !coasting {
            if rel.abs() < 0.01 {
                // Weighted by how many beats back the fit: a full window
                // sets the tempo, a short one (the run-out of a build) only
                // moves it part way.
                let w = (fit.count as f32 / 28.0).clamp(0.3, 1.0);
                self.period += (period - self.period) * w;
                self.nn_tempo_pending = None;
            } else {
                let votes = match self.nn_tempo_pending {
                    Some((p, n)) if (p / period - 1.0).abs() < 0.005 => n + 1,
                    _ => 1,
                };
                self.nn_tempo_pending = Some((period, votes));
                if votes >= 2 {
                    self.period = period;
                    self.nn_tempo_pending = None;
                }
            }
        } else if new_tempo {
            self.period = period;
            self.nn_tempo_pending = None;
        }
        // Where the live count should be now, against where it is.
        let now = self.nn_samples;
        let truth = n_e + (now as f64 - c.anchor) / c.period;
        let live = self.beat_count as f64 + self.phase as f64;
        let err = (truth - live) as f32;
        if !new_tempo {
            // A loose fit only goes half way.
            self.slew = if clean || coasting { err } else { err * 0.5 };
        } else {
            // A tempo change re-anchors: jump onto the new grid, forward to
            // the next beat so the count never runs backwards.
            self.slew = 0.0;
            let t = live + err.rem_euclid(1.0) as f64;
            self.beat_count = t.floor() as u64;
            self.phase = (t - t.floor()) as f32;
            self.nn_phase_pending = None;
        }
        self.nn_lock_at = self.clock;
        self.had_lock = true;
        self.confidence = self.confidence.max(0.6);
        self.nn_applied += 1;
        self.nn_debug(
            if new_tempo { "fit: new tempo" } else if clean { "fit" } else { "fit (loose)" },
            &pts,
            Some(fit),
        );
        // Bar evidence comes from clean fits only.
        if !clean && !new_tempo {
            return;
        }
        // The bar: each window's downbeats vote for live bar slots, into
        // evidence that decays over ~5 windows. The bar is a phrase-level
        // fact, so it moves only when another slot clearly outweighs the
        // current one — the model's downbeats wobble between the one and
        // the three on some tracks (Deadmau5 – Not Exactly), and every move
        // re-labels bars and restarts a phrase. Never moved while coasting,
        // nor from a fit counted at another metrical level.
        if !agrees {
            // A new tempo is a new track: its bar starts from scratch.
            self.nn_down_ev = [0.0; 4];
            self.nn_down_locked = false;
        }
        if coasting || c.converted {
            return;
        }
        // Decay by time (x0.8 per 5 s), not per window: after a stretch
        // with no usable windows, a minute-old track's evidence mustn't
        // outvote the one playing now.
        let decay = 0.8f32.powf(((self.clock - self.nn_down_at) / NN_EVERY_S).max(1.0));
        self.nn_down_at = self.clock;
        for (class, &v) in fit.down_votes.iter().enumerate() {
            let slot = ((n_e as i64) - c.anchor_n + class as i64).rem_euclid(4) as usize;
            self.nn_down_ev[slot] = self.nn_down_ev[slot] * decay + v;
        }
        let ev = self.nn_down_ev;
        let total: f32 = ev.iter().sum();
        let best = (0..4).max_by(|&a, &b| ev[a].total_cmp(&ev[b])).unwrap_or(0);
        let cur = self.downbeat as usize;
        // After a move, hold for 30 s: a transition between two tracks whose
        // bars disagree swung it 3>0>3>0 within 45 s on a mic recording.
        let take = if !self.nn_down_locked {
            ev[best] >= 2.0 && ev[best] >= 0.6 * total
        } else {
            best != cur
                && ev[best] >= 4.0
                && ev[best] > 2.0 * ev[cur]
                && self.clock - self.nn_bar_moved_at >= 30.0
        };
        if std::env::var_os("TRIPPIN_NN_DEBUG").is_some() {
            eprintln!(
                "nn t={:.0} bar evidence {:?} cur {cur}{}",
                self.clock,
                ev.map(|v| (v * 10.0).round() / 10.0),
                if take && best != cur { format!(" -> {best}") } else { String::new() }
            );
        }
        if take {
            if best != cur {
                self.nn_bar_moved_at = self.clock;
            }
            self.set_downbeat(best as u64);
            self.nn_down_locked = true;
        } else if best == cur && ev[best] >= 2.0 {
            self.nn_down_locked = true;
        }
    }

    /// Move the bar's one, keeping the bass vote in agreement so the
    /// fallback doesn't drag it straight back.
    fn set_downbeat(&mut self, slot: u64) {
        let sum: f32 = self.downbeat_votes.iter().sum();
        self.downbeat_votes = [0.0; 4];
        self.downbeat_votes[slot as usize] = sum.max(1.0);
        self.downbeat = slot;
    }

    fn nn_debug(&self, what: &str, pts: &[NnBeat], fit: Option<&WindowFit>) {
        if std::env::var_os("TRIPPIN_NN_DEBUG").is_none() {
            return;
        }
        let live = 60.0 * self.fps / self.period;
        match fit {
            Some(f) => eprintln!(
                "nn t={:.0} {what}: {} beats, {:.2} BPM (live {live:.2}) rms {:.3} down {:?} slew {:+.3} bar {}{}",
                self.clock,
                pts.len(),
                60.0 * self.sr as f64 / f.period,
                f.rms,
                f.down_class,
                self.slew,
                self.downbeat,
                if self.coasting { " coasting" } else { "" }
            ),
            None => eprintln!("nn t={:.0} {what}: {} beats (live {live:.2})", self.clock, pts.len()),
        }
    }

    fn run(mut self, rx: mpsc::Receiver<Vec<f32>>) {
        loop {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(chunk) => {
                    for s in chunk {
                        self.push(s);
                    }
                }
                // A stalled capture shouldn't freeze the features mid-set:
                // feed silence at the sample rate so levels decay and the
                // UI reports "no signal" until the watchdog rebuilds.
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    for _ in 0..self.sr as usize {
                        self.push(0.0);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    fn bin(&self, hz: f32) -> usize {
        ((hz / self.sr * FFT_SIZE as f32) as usize).clamp(1, FFT_SIZE / 2 - 1)
    }

    /// One analysis hop (~10.7 ms at 48 kHz).
    fn frame(&mut self) {
        let mut spec: Vec<Complex<f32>> = self
            .buf
            .iter()
            .zip(&self.window)
            .map(|(s, w)| Complex::new(s * w, 0.0))
            .collect();
        self.fft.process(&mut spec);
        let mag: Vec<f32> = spec[..FFT_SIZE / 2]
            .iter()
            .map(|c| c.norm() / FFT_SIZE as f32)
            .collect();

        let band = |lo: f32, hi: f32, a: &Self| -> f32 {
            let (l, h) = (a.bin(lo), a.bin(hi));
            (mag[l..h].iter().map(|m| m * m).sum::<f32>() / (h - l) as f32).sqrt()
        };
        let raw = [
            band(20.0, 150.0, self),
            band(150.0, 2000.0, self),
            band(2000.0, 16000.0, self),
        ];
        let rms = (self.buf.iter().map(|s| s * s).sum::<f32>() / FFT_SIZE as f32).sqrt();
        let silent = rms < 1e-4;

        // Spectral flux (half-wave rectified, log-compressed), weighted to lows.
        let mut flux = 0.0;
        let low_bin = self.bin(200.0);
        for (i, (&m, p)) in mag.iter().zip(self.prev_mag.iter_mut()).enumerate() {
            let lm = (1.0 + 100.0 * m).ln();
            let d = lm - *p;
            if d > 0.0 {
                flux += d * if i < low_bin { 2.0 } else { 1.0 };
            }
            *p = lm;
        }
        let bass_now = (1.0 + 100.0 * raw[0]).ln();
        let bass_flux = (bass_now - self.prev_bass).max(0.0);
        self.prev_bass = bass_now;

        let max_env = (ENV_SECONDS * self.fps) as usize;
        push_capped(&mut self.env, flux, max_env);
        push_capped(&mut self.bass_env, bass_flux, max_env);
        if let Some(t) = &self.tap {
            if let Ok(mut t) = t.lock() {
                t.push(flux, self.fps);
            }
        }

        // Onset pulses: flux above a local adaptive threshold.
        push_capped(&mut self.flux_hist, flux, (0.5 * self.fps) as usize);
        let mean = self.flux_hist.iter().sum::<f32>() / self.flux_hist.len() as f32;
        let fnorm = self.flux_gain.apply(flux);
        let decay = (-1.0 / (0.12 * self.fps)).exp();
        self.f.onset *= decay;
        self.f.kick *= decay;
        if !silent && flux > mean * 1.5 && fnorm > 0.3 {
            self.f.onset = self.f.onset.max(fnorm);
        }
        self.clock += 1.0 / self.fps;
        // Kicks: a low-band jump well above the recent average *and* strong
        // relative to this track's own peaks. The old fixed threshold
        // (bass_flux > 0.15) depended on playback volume — a pop mix played
        // at a normal level never cleared it, so no kicks, no groove, and the
        // show sat in breakdown mode.
        push_capped(&mut self.bass_hist, bass_flux, (0.5 * self.fps) as usize);
        let bass_mean = self.bass_hist.iter().sum::<f32>() / self.bass_hist.len() as f32;
        let bnorm = self.bass_gain.apply(bass_flux);
        if !silent && bass_flux > bass_mean * 1.8 && bnorm > 0.3 && raw[0] > 1e-6 {
            let hit = (bnorm * 1.2).min(1.0);
            // A fresh hit (not the tail of the last one): log it for the
            // groove detector, with a 0.2 s refractory period.
            let last = self.kick_times.back().copied().unwrap_or(-1.0);
            if hit > 0.3 && hit > self.f.kick * 1.5 && self.clock - last > 0.2 {
                self.kick_times.push_back(self.clock);
            }
            self.f.kick = self.f.kick.max(hit);
        }

        // Levels, smoothed with fast attack / slower release.
        let levels: Vec<f32> = raw
            .iter()
            .zip(self.gains.iter_mut())
            .map(|(&x, g)| g.apply(x))
            .collect();
        smooth(&mut self.f.bass, levels[0], 0.6, 0.15);
        smooth(&mut self.f.mid, levels[1], 0.5, 0.1);
        smooth(&mut self.f.high, levels[2], 0.5, 0.1);
        // Four-band vocabulary: level, hits (transients) and presence.
        let raw4 = [
            raw[0],
            raw[1],
            band(2000.0, 6000.0, self),
            band(6000.0, 16000.0, self),
        ];
        let hop = 1.0 / self.fps;
        let hit_decay = (-hop / 0.15).exp();
        for b in 0..4 {
            let lv = self.gains4[b].apply(raw4[b]);
            smooth(&mut self.f.lvl4[b], lv, 0.5, 0.08);
            // Transient: rise in log band energy, relative to this band's
            // running mean rise and its own auto-gained peak (volume-free).
            let lg = (1.0 + 200.0 * raw4[b]).ln();
            let fl = (lg - self.prev_log4[b]).max(0.0);
            self.prev_log4[b] = lg;
            self.flux_mean4[b] += (fl - self.flux_mean4[b]) * (hop / 0.4).min(1.0);
            let fnorm = self.hit_gains4[b].apply(fl);
            self.f.hits4[b] *= hit_decay;
            if !silent && fl > self.flux_mean4[b] * 2.0 && fnorm > 0.25 {
                self.f.hits4[b] = self.f.hits4[b].max(fnorm.min(1.0));
            }
            // Presence: level smoothed over ~1.5 s.
            self.f.pres4[b] += (lv - self.f.pres4[b]) * (hop / 1.5).min(1.0);
        }
        let energy = (levels[0] * 0.5 + levels[1] * 0.3 + levels[2] * 0.2).min(1.0);
        smooth(&mut self.f.energy, energy, 0.3, 0.08);
        self.energy_fast += (energy - self.energy_fast) * (1.0 / (1.5 * self.fps));
        self.energy_slow += (energy - self.energy_slow) * (1.0 / (12.0 * self.fps));
        self.f.build = ((self.energy_fast - self.energy_slow) * 4.0).clamp(-1.0, 1.0);

        // Groove: kick hits in the last 2.5 s against the beats expected at
        // the current tempo — ~1 for four-on-the-floor, ~0.5 for half-time
        // or broken beats, ~0 when the drums drop out (pads and vocals barely
        // move the low-band flux).
        let hop_s = 1.0 / self.fps;
        const WIN: f32 = 2.5;
        while self.kick_times.front().is_some_and(|&k| self.clock - k > WIN) {
            self.kick_times.pop_front();
        }
        let expected = WIN * self.f.bpm.clamp(60.0, 200.0) / 60.0;
        let raw_groove = (self.kick_times.len() as f32 / expected * 1.6).clamp(0.0, 1.0);
        self.f.groove += (raw_groove - self.f.groove) * (hop_s / 0.25).min(1.0);
        // Hysteresis: enter a breakdown only after two bars (8 beats) of low
        // groove — techno drops the kick for a bar or two before a drop and
        // that mustn't flip the whole show; leave as soon as it's back.
        if silent {
            self.calm_target = 1.0;
        } else if self.f.groove < 0.22 {
            self.quiet_for += hop_s;
            if self.quiet_for > 8.0 * 60.0 / self.f.bpm.clamp(60.0, 200.0) {
                self.calm_target = 1.0;
            }
        } else {
            self.quiet_for = 0.0;
            if self.f.groove > 0.38 {
                self.calm_target = 0.0;
            }
        }
        // Ease in over ~1.5 s, out over ~0.3 s (the drop should hit hard).
        let rate = if self.calm_target > self.f.calm { 1.5 } else { 0.3 };
        self.f.calm += (self.calm_target - self.f.calm) * (hop_s / rate).min(1.0);

        // Log-spaced spectrum bins for shaders.
        let mut peak = 0.0f32;
        let mut bins = [0.0f32; SPECTRUM_BINS];
        for (i, b) in bins.iter_mut().enumerate() {
            let lo = 30.0 * (16000.0f32 / 30.0).powf(i as f32 / SPECTRUM_BINS as f32);
            let hi = 30.0 * (16000.0f32 / 30.0).powf((i + 1) as f32 / SPECTRUM_BINS as f32);
            let (l, h) = (self.bin(lo), self.bin(hi).max(self.bin(lo) + 1));
            *b = mag[l..h].iter().cloned().fold(0.0, f32::max) * (1.0 + i as f32 * 0.15);
            peak = peak.max(*b);
        }
        let g = self.spec_gain.apply(peak) / peak.max(1e-9);
        for (dst, src) in self.f.spectrum.iter_mut().zip(bins) {
            smooth(dst, (src * g).min(1.0), 0.6, 0.12);
        }

        // Time-domain trace like a real oscilloscope: trigger on a rising
        // zero crossing of the low-passed signal (so periodic content holds
        // still instead of scribbling at a random phase every frame), take
        // 1024 samples (~21 ms) from there, and box-average them down to 64
        // points (no aliasing). Lightly auto-levelled so quiet tracks still
        // show a wiggle, and eased frame to frame.
        let n = self.buf.len();
        let wgain = (0.5 / (rms * 3.0 + 0.02)).clamp(0.6, 5.0);
        let search = n - 1024;
        let mut start = search;
        let mut lp = 0.0f32;
        let mut prev_lp = 0.0f32;
        for i in 0..search {
            lp += (self.buf[i] - lp) * 0.08;
            if i > 32 && prev_lp <= 0.0 && lp > 0.0 {
                start = i;
                // Keep the latest crossing that still leaves a full window.
            }
            prev_lp = lp;
        }
        let start = start.min(n - 1024);
        for (i, w) in self.f.waveform.iter_mut().enumerate() {
            let b = start + i * 16;
            let avg = (b..b + 16).map(|j| self.buf[j]).sum::<f32>() / 16.0;
            let target = (avg * wgain * 1.3).clamp(-1.0, 1.0);
            *w += (target - *w) * 0.6;
        }

        // Flywheel: no beat to follow (silence, or the kicks gone below the
        // breakdown threshold) after a confident lock → coast on the locked
        // tempo and phase, bounded by FLYWHEEL_MAX_S.
        // Hysteresis: a stray kick in a build-up lifts the groove past 0.22 for
        // a moment (D.O.D. – Set Me Free, 46-60 s), and tracking that
        // sparse-kick envelope locked a wrong 74.9 BPM. The beat only counts
        // as back after a bar of real kick density.
        if silent || self.f.groove < 0.22 {
            self.beatless_since.get_or_insert(self.clock);
            self.beats_back_for = 0.0;
        } else if self.beatless_since.is_some() {
            if self.f.groove >= 0.6 {
                self.beats_back_for += hop_s;
                if self.beats_back_for >= 2.0 {
                    self.beatless_since = None;
                }
            } else {
                self.beats_back_for = 0.0;
            }
        }
        let beatless = self.beatless_since.is_some();
        if let Some(since) = self.beatless_since {
            if self.clock - since > FLYWHEEL_MAX_S {
                self.had_lock = false;
            }
        } else if self.confidence > 0.3 {
            self.had_lock = true;
        }
        let coast = beatless && self.had_lock;
        self.coasting = coast;

        self.track_beats(silent || coast);
        self.nn_tick(silent, silent || beatless);

        self.frames_since_tempo += 1;
        if self.frames_since_tempo as f32 > self.fps * 0.5 && self.env.len() as f32 > self.fps * 4.0
        {
            self.frames_since_tempo = 0;
            if !silent && !coast {
                self.estimate_tempo();
                // A recent neural fit knows where the beat is; the comb
                // locks onto whatever onsets are strongest (off-beat bass).
                if !self.nn_fresh() {
                    self.correct_phase();
                }
            }
        }

        for cmd in self.commands.try_iter().collect::<Vec<_>>() {
            match cmd {
                Command::MarkDownbeat => {
                    let nearest = if self.phase < 0.5 {
                        self.beat_count
                    } else {
                        self.beat_count + 1
                    };
                    self.downbeat = nearest % 4;
                    self.downbeat_votes = [0.0; 4];
                    self.downbeat_votes[self.downbeat as usize] = 10.0;
                    // The operator's tap outweighs ~10 windows of the
                    // model's downbeats.
                    self.nn_down_ev = [0.0; 4];
                    self.nn_down_ev[self.downbeat as usize] = 20.0;
                    self.nn_down_locked = true;
                }
            }
        }

        self.f.silent = silent;
        // The half-tempo line, whatever set the period.
        while 60.0 * self.fps / self.period > HALF_TEMPO_ABOVE {
            self.period *= 2.0;
        }
        self.f.bpm = 60.0 * self.fps / self.period;
        self.f.beat_phase = self.phase;
        self.f.beat_count = self.beat_count;
        self.f.phase_at = Instant::now();
        self.f.downbeat = self.downbeat;
        self.f.tempo_confidence = self.confidence;
        if let Ok(mut s) = self.shared.lock() {
            *s = self.f.clone();
        }
    }

    /// Advance the phase one hop; on each beat, vote for the downbeat using
    /// how much bass the previous beat had (kicks + basslines land on the one).
    fn track_beats(&mut self, silent: bool) {
        self.beat_bass += self.bass_env.back().copied().unwrap_or(0.0);
        // Phase corrections land as a brief change of speed (between 40% and
        // 200%), so the position only ever moves forward.
        let base = 1.0 / self.period;
        let corr = (self.slew * (1.0 / (SLEW_S * self.fps)).min(1.0)).clamp(-0.6 * base, base);
        self.slew -= corr;
        self.phase += base + corr;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
            let slot = (self.beat_count % 4) as usize;
            if !silent {
                for v in self.downbeat_votes.iter_mut() {
                    *v *= 0.97;
                }
                self.downbeat_votes[slot] += self.beat_bass;
            }
            self.beat_bass = 0.0;
            self.beat_count += 1;
            let best = (0..4)
                .max_by(|&a, &b| self.downbeat_votes[a].total_cmp(&self.downbeat_votes[b]))
                .unwrap_or(0) as u64;
            // Hysteresis so the downbeat doesn't flicker between candidates.
            // Once a neural window has set the bar, it owns it.
            // (Through a long stretch with no usable windows too: the bass
            // vote took the bar after 60 s of none and swung it mid-track.)
            let nn_owns = self.nn_down_locked && self.clock - self.nn_lock_at < 180.0;
            // A few bars of votes first: on a handful of beats it flaps.
            if !nn_owns
                && self.downbeat_votes.iter().sum::<f32>() > 0.0
                && self.beat_count >= 12
                && self.downbeat_votes[best as usize]
                > self.downbeat_votes[self.downbeat as usize] * 1.3
            {
                self.downbeat = best;
            }
        }
    }

    /// Autocorrelate the onset envelope over the last few seconds and pick the
    /// best beat period, with a prior favouring typical dance tempos.
    fn estimate_tempo(&mut self) {
        let env: Vec<f32> = self.env.iter().copied().collect();
        let mean = env.iter().sum::<f32>() / env.len() as f32;
        let x: Vec<f32> = env.iter().map(|v| v - mean).collect();
        let min_lag = (60.0 * self.fps / MAX_BPM) as usize;
        let max_lag = ((60.0 * self.fps / MIN_BPM) as usize).min(x.len() / 2);
        if max_lag <= min_lag + 2 {
            return;
        }
        let ac = |lag: usize| -> f32 {
            x[lag..].iter().zip(&x).map(|(a, b)| a * b).sum::<f32>() / (x.len() - lag) as f32
        };
        let acs: Vec<f32> = (0..=max_lag + 1)
            .map(|l| if l >= min_lag - 1 { ac(l) } else { 0.0 })
            .collect();
        let zero = x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32;
        // Score of a candidate lag: reinforced by the double period (bar
        // structure), penalised when a shorter lag at 2/3 explains the same
        // periodicity — a bassline cycling every 1.5 beats otherwise peaks
        // at the 1.5x shadow of the beat (127 -> 85, "Injected With a
        // Poison"). All in units of autocorr before the tempo prior.
        let score_of = |lag: usize| -> f32 {
            let bpm = 60.0 * self.fps / lag as f32;
            // Log-Gaussian tempo prior centred on 124 BPM.
            let prior = (-0.5 * ((bpm / 124.0).log2() / 0.5).powi(2)).exp();
            let dbl = acs.get(lag * 2).copied().unwrap_or(0.0);
            let tri = lag * 2 / 3;
            let shadow = if tri >= min_lag { acs[tri].max(0.0) } else { 0.0 };
            (acs[lag] + 0.5 * dbl.max(0.0) - 1.25 * shadow).max(0.0) * prior
        };
        let mut best = (0usize, f32::MIN);
        for lag in min_lag..=max_lag {
            let score = score_of(lag);
            if score > best.1 {
                best = (lag, score);
            }
        }
        let lag = best.0;
        // Parabolic interpolation for a fractional period.
        let (a, b, c) = (acs[lag - 1], acs[lag], acs[lag + 1]);
        let denom = a - 2.0 * b + c;
        let offset = if denom.abs() > 1e-12 {
            (0.5 * (a - c) / denom).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let mut period = lag as f32 + offset;
        // Octave fix: a raw estimate above HALF_TEMPO_ABOVE is nearly always the
        // double-time harmonic — drop to half-tempo (174 → 87).
        if 60.0 * self.fps / period > HALF_TEMPO_ABOVE {
            period *= 2.0;
        }
        let conf = (acs[lag] / zero.max(1e-12)).clamp(0.0, 1.0);
        self.confidence += (conf - self.confidence) * 0.3;
        // A recent neural fit measured the tempo over a whole window of
        // beats; the autocorrelation (often 0.1-0.3 BPM off) stays out.
        if self.nn_fresh() {
            return;
        }
        // With the model on hand, a neural fit owns tempo changes: the
        // autocorrelation's big jumps (2/3 shadows on a sparse build
        // envelope: 133 -> 87.8 on a mic recording) wait until the fits
        // have been absent for a couple of minutes.
        let nn_recent = crate::beats::ready()
            && self.nn_lock_at != f32::MIN
            && self.clock - self.nn_lock_at < 120.0;

        let rel = (period - self.period).abs() / self.period;
        if rel < 0.04 {
            self.period += (period - self.period) * 0.25;
            self.pending_bpm = None;
            self.held_bpm = None;
        } else if nn_recent {
            self.pending_bpm = None;
            self.held_bpm = None;
        } else {
            // Inertia: once locked, a challenger must clearly beat the
            // incumbent period's own autocorrelation — much harder when it
            // sits on a simple harmonic of the locked tempo. A groove that
            // repeats every 1.5 beats (the classic 3-against-4 bassline)
            // otherwise wins a few windows and the readout flaps between
            // the tempo and its 1.5x shadow (127 <-> 85).
            let mut margin = 1.1f32;
            if self.confidence > 0.25 {
                let r = period / self.period;
                let harmonic = [0.5f32, 0.6667, 0.75, 1.3333, 1.5, 2.0]
                    .iter()
                    .any(|&h| (r / h - 1.0).abs() < 0.03);
                if harmonic {
                    margin = 1.75;
                }
                let inc = (self.period.round() as usize).clamp(min_lag, max_lag);
                let inc_bpm = 60.0 * self.fps / self.period;
                let inc_prior =
                    (-0.5 * ((inc_bpm / 124.0).log2() / 0.5).powi(2)).exp();
                let inc_score = ((inc - 1).max(min_lag)..=(inc + 1).min(max_lag))
                    .map(|l| score_of(l))
                    .fold(0.0f32, f32::max);
                if std::env::var("TRIPPIN_TEMPO_DEBUG").is_ok() {
                    eprintln!(
                        "tempo: cur={:.1} best={:.1}(lag {lag}) best={:.4} inc={:.4} margin={margin} zero={zero:.3}",
                        60.0 * self.fps / self.period,
                        60.0 * self.fps / period,
                        best.1,
                        inc_score
                    );
                }
                if best.1 < inc_score * margin {
                    // Suppressed — but a persistent disagreement (~6 s) is a
                    // real change that started marginal. A harmonic shadow
                    // may only escape toward the more plausible tempo, so
                    // locking on the wrong side still recovers.
                    let chal_bpm = 60.0 * self.fps / period;
                    let chal_prior =
                        (-0.5 * ((chal_bpm / 124.0).log2() / 0.5).powi(2)).exp();
                    if !harmonic || chal_prior >= inc_prior {
                        let held = match self.held_bpm {
                            Some((p, n)) if (p - period).abs() / p < 0.04 => n + 1,
                            _ => 1,
                        };
                        self.held_bpm = Some((period, held));
                        if held >= 12 {
                            self.period = period;
                            self.pending_bpm = None;
                            self.held_bpm = None;
                        }
                    }
                    return;
                }
            }
            self.held_bpm = None;
            // Require a few consistent estimates before jumping tempo.
            let votes = match self.pending_bpm {
                Some((p, n)) if (p - period).abs() / p < 0.04 => n + 1,
                _ => 1,
            };
            self.pending_bpm = Some((period, votes));
            if votes >= 3 {
                self.period = period;
                self.pending_bpm = None;
            }
        }
    }

    /// Comb-filter the recent onset envelope at the current period to find
    /// where beats actually fall, then nudge the running phase towards it.
    fn correct_phase(&mut self) {
        let env: Vec<f32> = self.env.iter().copied().collect();
        let n = env.len();
        let span = (4.0 * self.fps) as usize;
        if n < span {
            return;
        }
        let period = self.period;
        let steps = period.ceil() as usize;
        let mut best = (0usize, f32::MIN);
        for off in 0..steps {
            let mut sum = 0.0;
            let mut k = 0.0f32;
            loop {
                let idx = off as f32 + k * period;
                if idx as usize >= span {
                    break;
                }
                let i = n - 1 - idx.round() as usize;
                // Tolerate +/-1 frame of jitter.
                let v = env[i]
                    .max(env[i.saturating_sub(1)])
                    .max(env[(i + 1).min(n - 1)] * 0.8);
                sum += v;
                k += 1.0;
            }
            if sum > best.1 {
                best = (off, sum);
            }
        }
        // `off` frames ago there was a beat, so the phase now is off/period —
        // shifted by what the neural check learned about where the real
        // beat sits relative to the strongest onsets.
        let target = (best.0 as f32 / period).rem_euclid(1.0);
        let mut err = target - self.phase;
        if err > 0.5 {
            err -= 1.0;
        } else if err < -0.5 {
            err += 1.0;
        }
        let gain = if self.confidence > 0.2 { 0.35 } else { 0.1 };
        self.slew = err * gain;
    }
}

fn push_capped(q: &mut VecDeque<f32>, v: f32, cap: usize) {
    q.push_back(v);
    while q.len() > cap {
        q.pop_front();
    }
}

fn smooth(v: &mut f32, target: f32, attack: f32, release: f32) {
    let k = if target > *v { attack } else { release };
    *v += (target - *v) * k;
}

/// `trippin --groove-test track.mp3`: run the live analyser over a file
/// (as fast as possible) and print tempo, groove, calm and kick counts every
/// few seconds — for tuning the beats/breakdown detector on real music.
pub fn groove_test(path: &std::path::Path) -> anyhow::Result<()> {
    let song = crate::song::load(path)?;
    let (_tx, rx) = mpsc::channel();
    let shared: SharedFeatures = Arc::new(Mutex::new(Features::default()));
    let mut a = Analyzer::new(song.sr as f32, shared, rx, None);
    let mut next_print = 0.0f32;
    let mut kicks = 0usize;
    let mut last_kick_len = 0usize;
    println!("{} — {:.1} BPM (file analysis)", song.name, song.bpm);
    println!("   t    bpm  groove  calm  kicks/2s  energy   lvl4(b m mh h)        hits/2s(b m mh h)  clock rate x tempo");
    let mut hit_n = [0u32; 4];
    let mut prev_hit = [0.0f32; 4];
    let mut clk = 0.0f32;
    let mut rate_acc = 0.0f32;
    let mut rate_n = 0u32;
    a.nn_sync = true;
    // TRIPPIN_GATE=from-to (seconds): print the strobe gate (onset >= 0.45)
    // per hop over that span, with | at each live beat — checks flashes land
    // on a fill's hits.
    let gate: Option<(f32, f32)> = std::env::var("TRIPPIN_GATE")
        .ok()
        .and_then(|v| v.split_once('-').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))));
    let mut gate_line = String::new();
    let mut gate_beat = 0u64;
    // TRIPPIN_NO_NN=1: bass vote only, for comparing against the neural check.
    if std::env::var("TRIPPIN_NO_NN").is_ok() {
        crate::beats::set_enabled(false);
    }
    for (i, &s) in song.mono.iter().enumerate() {
        if a.push(s) {
            if a.kick_times.len() > last_kick_len {
                kicks += a.kick_times.len() - last_kick_len;
            }
            last_kick_len = a.kick_times.len();
            for b in 0..4 {
                if a.f.hits4[b] > prev_hit[b] + 0.2 {
                    hit_n[b] += 1;
                }
                prev_hit[b] = a.f.hits4[b];
            }
            // Same clock-rate maths as main.rs (whole-mix clock).
            let l = &a.f.lvl4;
            let whole = (l[0] * 0.45 + l[1] * 0.3 + l[2] * 0.15 + l[3] * 0.1).min(1.0);
            clk += (whole - clk) * ((1.0 / a.fps) / 0.35).min(1.0);
            rate_acc += 0.3 + 2.4 * clk.powf(1.6);
            rate_n += 1;
            let t = i as f32 / song.sr as f32;
            if let Some((g0, g1)) = gate {
                if t >= g0 && t < g1 {
                    if a.beat_count != gate_beat {
                        gate_beat = a.beat_count;
                        gate_line.push('|');
                    }
                    gate_line.push(if a.f.onset >= 0.45 { '#' } else { '.' });
                } else if t >= g1 && !gate_line.is_empty() {
                    println!("gate {g0}-{g1}s: {gate_line}");
                    gate_line.clear();
                }
            }
            if t >= next_print {
                // Where the live "one" sits against the file's grid, in
                // beats (0 = on the bar; needs a matching tempo to mean much).
                let spb = 60.0 / song.bpm;
                let beats_since_one =
                    ((a.beat_count as i64 - a.downbeat as i64).rem_euclid(4)) as f64 + a.phase as f64;
                let lag = (FFT_SIZE / 2) as f64 / song.sr as f64;
                let one_t = t as f64 - lag - beats_since_one * 60.0 / a.f.bpm as f64;
                let bar_off = (((one_t - song.first_beat) / spb).round() as i64).rem_euclid(4);
                println!(
                    "{:5.0}s {:6.1} {:6.2} {:5.2} {:6} {:8.2}   {:.2} {:.2} {:.2} {:.2}   {:3} {:3} {:3} {:3}   {:.2}  bar {}{}{}",
                    t, a.f.bpm, a.f.groove, a.f.calm, kicks, a.f.energy,
                    l[0], l[1], l[2], l[3], hit_n[0], hit_n[1], hit_n[2], hit_n[3],
                    rate_acc / rate_n.max(1) as f32,
                    bar_off,
                    if a.nn_applied > 0 { format!(" nn{}", a.nn_applied) } else { String::new() },
                    if a.f.calm > 0.5 { "  BREAKDOWN" } else { "" }
                );
                kicks = 0;
                hit_n = [0; 4];
                rate_acc = 0.0;
                rate_n = 0;
                next_print += 2.0;
            }
        }
    }
    Ok(())
}

/// `--dump-feed track.flac out.jsonl [palette]`: run a file through the real
/// analyser (the same tempo tracker, neural downbeat check, groove and
/// breakdown detector the live app uses) and write the show-state feed it would
/// have sent the Unity engine, one JSON line per 60 fps frame. The engine
/// replays it with `-replayFeed out.jsonl`, so Unity shows can be tested on
/// real music without audio capture. Intensity is a proxy (the director's
/// own value needs the whole settings machinery).
pub fn dump_feed(path: &std::path::Path, out: &std::path::Path, palette: &str) -> anyhow::Result<()> {
    use std::io::Write;
    let song = crate::song::load(path)?;
    let (_tx, rx) = mpsc::channel();
    let shared: SharedFeatures = Arc::new(Mutex::new(Features::default()));
    let mut a = Analyzer::new(song.sr as f32, shared, rx, None);
    a.nn_sync = true;
    let pal = crate::link::palette_colours(palette);
    let mut w = std::io::BufWriter::new(std::fs::File::create(out)?);
    let dt = 1.0f32 / 60.0;
    let mut next_t = 0.0f32;
    let mut frames = 0usize;
    // Same smoothing / clock maths as the render loop (main.rs).
    let mut flow = 0.0f64;
    let mut flow_bpm = 120.0f32;
    let mut flow_speed = 1.0f32;
    let mut clock4 = [0.0f64; 4];
    let mut clock_lvl = [0.0f32; 4];
    let mut intensity = 0.5f32;
    for (i, &s) in song.mono.iter().enumerate() {
        if !a.push(s) {
            continue;
        }
        let t = i as f32 / song.sr as f32;
        while next_t <= t {
            let f = &a.f;
            flow_bpm += (f.bpm - flow_bpm) * (dt * 1.5).min(1.0);
            flow_speed += ((1.0 - 0.45 * f.calm) - flow_speed) * (dt * 0.8).min(1.0);
            flow = (flow + dt as f64 * flow_bpm as f64 / 60.0 * flow_speed as f64) % 4096.0;
            let whole = (f.lvl4[0] * 0.45 + f.lvl4[1] * 0.3 + f.lvl4[2] * 0.15 + f.lvl4[3] * 0.1).min(1.0);
            let src = [whole, f.lvl4[0], f.lvl4[1], f.lvl4[2].max(f.lvl4[3])];
            let k = (dt / 0.35).min(1.0);
            for c in 0..4 {
                let target = if f.silent { 0.0 } else { src[c] };
                clock_lvl[c] += (target - clock_lvl[c]) * k;
                let rate = 0.3 + 2.4 * clock_lvl[c].powf(1.6);
                clock4[c] = (clock4[c] + dt as f64 * flow_bpm as f64 / 60.0 * rate as f64) % 4096.0;
            }
            intensity += ((0.3 + 0.7 * f.energy.min(1.0)) - intensity) * (dt * 0.6).min(1.0);
            let pos = a.beat_count as f64 + a.phase as f64;
            let beat_in_bar = ((pos.floor() as i64 - a.downbeat as i64).rem_euclid(4)) as f32;
            let mut spectrum = [0.0f32; SPECTRUM_BINS];
            spectrum.copy_from_slice(&f.spectrum);
            let u = crate::render::Uniforms {
                time: next_t,
                dt,
                bass: f.bass,
                mid: f.mid,
                high: f.high,
                energy: f.energy,
                onset: f.onset * (1.0 - 0.6 * f.calm),
                kick: f.kick,
                beat: (pos % 4096.0) as f32,
                beat_phase: pos.fract() as f32,
                bar_phase: (beat_in_bar + pos.fract() as f32) / 4.0,
                bpm: f.bpm,
                build: f.build,
                intensity,
                flow: flow as f32,
                master: 1.0,
                spectrum,
                calm: f.calm,
                lvl4: f.lvl4,
                hits4: f.hits4,
                pres4: f.pres4,
                clock4: clock4.map(|c| c as f32),
                ..Default::default()
            };
            writeln!(w, "{}", crate::link::frame_json(&u, "", &pal, false, f.calm < 0.5))?;
            frames += 1;
            next_t += dt;
        }
    }
    w.flush()?;
    println!("{}: {} frames ({:.1} s) -> {}", song.name, frames, frames as f32 * dt, out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An analyser 0.4 beat behind the real beats at 120 BPM (24000
    /// samples a beat), with the hop log filled in to `beats` beats.
    fn behind(beats: u64) -> Analyzer {
        let (_tx, rx) = mpsc::channel();
        let shared: SharedFeatures = Arc::new(Mutex::new(Features::default()));
        let mut a = Analyzer::new(48000.0, shared, rx, None);
        a.period = 24000.0 / HOP as f32;
        let mut s = 0u64;
        while s <= beats * 24000 {
            a.nn_hops.push_back((s, s as f64 / 24000.0 - 0.4, false));
            s += HOP as u64;
        }
        a.nn_samples = beats * 24000;
        let live = beats as f64 - 0.4;
        a.beat_count = live.floor() as u64;
        a.phase = (live - live.floor()) as f32;
        a
    }

    /// Model beats at whole beats `from..to`, downbeats where `k % 4 == down`.
    fn model_beats(from: u64, to: u64, down: u64) -> Vec<NnBeat> {
        (from..to).map(|k| (k * 24000, 0.9, k % 4 == down)).collect()
    }

    /// The tracker locked 0.4 beat off the real beats (off-beat bass), with
    /// the real downbeats on its beat 2: one window must slew the phase onto
    /// the beats (no jump) and set the bar; a lone window voting another
    /// bar must not move it.
    #[test]
    fn nn_fit_fixes_offbeat_lock_and_bar() {
        let mut a = behind(41);
        a.nn_apply(&model_beats(10, 40, 2));
        assert!((a.slew - 0.4).abs() < 0.01, "slew {}", a.slew);
        assert!((a.period - 24000.0 / HOP as f32).abs() < 0.01);
        assert_eq!(a.downbeat, 2);
        // The slew lands within two seconds, moving forward all the way.
        let mut last = a.beat_count as f64 + a.phase as f64;
        for _ in 0..200 {
            a.track_beats(false);
            let p = a.beat_count as f64 + a.phase as f64;
            assert!(p > last, "position ran backwards");
            last = p;
        }
        assert!(a.slew.abs() < 0.02, "slew left {}", a.slew);
        // One window says the bar is elsewhere: evidence, not a move.
        let mut b = behind(41);
        b.nn_apply(&model_beats(10, 40, 2));
        b.nn_apply(&model_beats(10, 40, 0));
        assert_eq!(b.downbeat, 2, "a single odd window moved the bar");
    }

    /// Double-time detections (a hi-hat run read as beats): with the live
    /// tempo locked, the fit stays at the live tempo on the live phase.
    #[test]
    fn nn_fit_ignores_double_time_detections() {
        let mut a = behind(41);
        a.nn_lock_at = 0.0;
        a.clock = 1.0;
        a.beat_count = 41;
        a.phase = 0.0;
        for h in a.nn_hops.iter_mut() {
            h.1 += 0.4;
        }
        let mut beats = model_beats(10, 40, 2);
        beats.extend((10..40).map(|k| (k * 24000 + 12000, 0.8, false)));
        beats.sort_by_key(|b| b.0);
        a.nn_apply(&beats);
        let bpm = 60.0 * a.fps / a.period;
        assert!((bpm - 120.0).abs() < 0.5, "tempo {bpm}");
        assert!(a.slew.abs() < 0.05, "phase moved {}", a.slew);
    }

    /// A clean read over the half-tempo line, once locked, settles at half
    /// tempo (160 -> 80), never at 160.
    #[test]
    fn tempo_never_runs_over_the_half_tempo_line() {
        let mut a = behind(41);
        a.nn_lock_at = 0.0;
        a.clock = 1.0;
        let beats: Vec<NnBeat> = (0..40).map(|k| (240_000 + k * 18_000, 0.9, k % 4 == 0)).collect();
        a.nn_apply(&beats);
        a.nn_apply(&beats);
        let bpm = 60.0 * a.fps / a.period;
        assert!((bpm - 80.0).abs() < 0.5, "tempo {bpm}");
    }

    /// Through a breakdown the flywheel's tempo stands, even when the model
    /// finds a consistent grid in the pads.
    #[test]
    fn coasting_keeps_tempo_against_model() {
        let mut a = behind(41);
        a.nn_lock_at = 0.0;
        a.clock = 1.0;
        a.coasting = true;
        let beats: Vec<NnBeat> = (10..40).map(|k| ((k as f64 * 24000.0 * 0.99) as u64, 0.9, false)).collect();
        let before = a.period;
        a.nn_apply(&beats);
        assert_eq!(a.period, before);
    }

    /// Beats (kick + pad) → breakdown (pad only) → drop (kick + pad):
    /// `calm` must follow, entering slowly and leaving fast.
    #[test]
    fn groove_detects_breakdown_and_drop() {
        let sr = 48000.0f32;
        let (_tx, rx) = mpsc::channel();
        let shared: SharedFeatures = Arc::new(Mutex::new(Features::default()));
        let mut a = Analyzer::new(sr, shared, rx, None);
        let bpm = 124.0;
        let spb = 60.0 / bpm;
        let total = 48.0;
        let mut t = 0.0f32;
        let mut log = Vec::new();
        let mut rng = 1u32;
        while t < total {
            let with_drums = !(16.0..32.0).contains(&t);
            // Pad: a soft chord with slow swell, plus a little noise.
            let mut s = 0.08 * ((t * 220.0 * TAU_F).sin() + (t * 277.2 * TAU_F).sin() + (t * 329.6 * TAU_F).sin()) / 3.0
                * (0.7 + 0.3 * (t * 0.5).sin());
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            s += (rng as f32 / u32::MAX as f32 - 0.5) * 0.01;
            if with_drums {
                let ph = (t % spb) / spb;
                let kt = ph * spb;
                // 909-ish kick: pitch-dropping sine with a fast decay.
                let f = 50.0 + 120.0 * (-kt * 30.0).exp();
                s += 0.6 * (kt * f * TAU_F).sin() * (-kt * 9.0).exp();
            }
            a.buf.push_back(s);
            if a.buf.len() > FFT_SIZE {
                a.buf.pop_front();
            }
            a.since_hop += 1;
            if a.since_hop >= HOP && a.buf.len() == FFT_SIZE {
                a.since_hop = 0;
                a.frame();
                log.push((t, a.f.groove, a.f.calm));
            }
            t += 1.0 / sr;
        }
        let at = |x: f32| log.iter().find(|e| e.0 >= x).copied().unwrap();
        for x in [8.0, 15.0, 17.0, 19.0, 24.0, 31.0, 32.5, 33.0, 34.0, 40.0] {
            let e = at(x);
            println!("t={:5.1}s groove={:.2} calm={:.2}", e.0, e.1, e.2);
        }
        assert!(at(12.0).2 < 0.1, "beats should read as beats");
        assert!(at(17.0).2 < 0.5, "one missing bar mustn't trip a breakdown");
        assert!(at(29.0).2 > 0.9, "a drumless section is a breakdown");
        assert!(at(33.5).2 < 0.3, "the drop must be caught fast");
        assert!(at(44.0).2 < 0.05);
    }

    /// Beats → a long drumless break (pad only) → beats again: the tempo must
    /// hold through the break and the beat grid must come out the other side
    /// on the same count (no slipped beats = no reset bars/phrases).
    #[test]
    fn grid_holds_through_drumless_break() {
        let sr = 48000.0f32;
        let (_tx, rx) = mpsc::channel();
        let shared: SharedFeatures = Arc::new(Mutex::new(Features::default()));
        let mut a = Analyzer::new(sr, shared, rx, None);
        let bpm = 124.0;
        let spb = 60.0 / bpm;
        let (brk0, brk1, total) = (24.0f32, 56.0f32, 72.0f32);
        let mut k = 0u64;
        let mut t = 0.0f32;
        let mut rng = 1u32;
        // (t, bpm, beat_count + phase - true beat index)
        let mut log: Vec<(f32, f32, f64)> = Vec::new();
        while t < total {
            // Time from the sample index (an f32 += 1/sr accumulates error).
            t = (k as f64 / sr as f64) as f32;
            k += 1;
            let with_drums = !(brk0..brk1).contains(&t);
            let mut s = 0.08 * ((t * 220.0 * TAU_F).sin() + (t * 277.2 * TAU_F).sin() + (t * 329.6 * TAU_F).sin()) / 3.0
                * (0.7 + 0.3 * (t * 0.5).sin());
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            s += (rng as f32 / u32::MAX as f32 - 0.5) * 0.01;
            if with_drums {
                let ph = (t % spb) / spb;
                let kt = ph * spb;
                let f = 50.0 + 120.0 * (-kt * 30.0).exp();
                s += 0.6 * (kt * f * TAU_F).sin() * (-kt * 9.0).exp();
            }
            a.buf.push_back(s);
            if a.buf.len() > FFT_SIZE {
                a.buf.pop_front();
            }
            a.since_hop += 1;
            if a.since_hop >= HOP && a.buf.len() == FFT_SIZE {
                a.since_hop = 0;
                a.frame();
                let truth = (k - 1) as f64 / sr as f64 / spb as f64;
                log.push((t, a.f.bpm, a.beat_count as f64 + a.phase as f64 - truth));
            }
        }
        let at = |x: f32| log.iter().find(|e| e.0 >= x).copied().unwrap();
        for x in [20.0, 28.0, 36.0, 44.0, 52.0, 60.0, 68.0, 71.0] {
            let e = at(x);
            println!("t={:5.1}s bpm={:6.2} grid offset={:+.2} beats", e.0, e.1, e.2);
        }
        let worst_bpm = log
            .iter()
            .filter(|e| e.0 >= brk0 && e.0 < brk1 + 4.0)
            .map(|e| (e.1 - bpm).abs())
            .fold(0.0f32, f32::max);
        assert!(worst_bpm < 1.0, "tempo drifted {worst_bpm:.2} BPM in the break");
        // Offset before vs after: the same beat count (within a quarter beat).
        let before = at(22.0).2;
        let after = at(70.0).2;
        assert!(
            (after - before).abs() < 0.25,
            "beat grid slipped {:+.2} beats across the break ({before:+.2} -> {after:+.2})",
            after - before
        );
    }

    /// A steady tone must draw the same trace every frame (triggered scope),
    /// not a randomly phased slice.
    #[test]
    fn waveform_is_triggered() {
        let sr = 48000.0f32;
        let (_tx, rx) = mpsc::channel();
        let shared: SharedFeatures = Arc::new(Mutex::new(Features::default()));
        let mut a = Analyzer::new(sr, shared, rx, None);
        let mut snaps = Vec::new();
        for k in 0..(sr as usize) {
            let t = k as f32 / sr;
            a.buf.push_back((t * 110.0 * TAU_F).sin() * 0.4 + (t * 330.0 * TAU_F).sin() * 0.1);
            if a.buf.len() > FFT_SIZE {
                a.buf.pop_front();
            }
            a.since_hop += 1;
            if a.since_hop >= HOP && a.buf.len() == FFT_SIZE {
                a.since_hop = 0;
                a.frame();
                if k > 24000 {
                    snaps.push(a.f.waveform);
                }
            }
        }
        // Compare consecutive frames: max point difference must be small.
        let worst = snaps
            .windows(2)
            .map(|w| w[0].iter().zip(w[1].iter()).map(|(x, y)| (x - y).abs()).fold(0.0f32, f32::max))
            .fold(0.0f32, f32::max);
        println!("worst frame-to-frame deviation {worst:.3}");
        assert!(worst < 0.25, "trace jitters between frames: {worst}");
    }

    /// Build an onset envelope: weak kicks every beat, dominant stabs every
    /// `stab_mult` beats — the 3-against-4 acid line that flapped 127 <-> 85
    /// on "Injected With a Poison".
    fn stab_env(fps: f32, bpm: f32, secs: f32, stab_mult: f32) -> VecDeque<f32> {
        let period = 60.0 * fps / bpm;
        let n = (fps * secs) as usize;
        let mut env = vec![0.02f32; n];
        let mut i = 0.0f32;
        while (i as usize) < n {
            env[i as usize] += 0.5;
            i += period;
        }
        let mut i = 0.0f32;
        while (i as usize) < n {
            env[i as usize] += 1.0;
            i += period * stab_mult;
        }
        env.into()
    }

    /// Locked at 127 BPM, the estimator must not jump to the autocorr peak at
    /// 1.5x the beat period (85 BPM) even when the stab pattern dominates.
    #[test]
    fn tempo_inertia_holds_against_triplet_shadow() {
        let (_tx, rx) = mpsc::channel();
        let shared: SharedFeatures = Arc::new(Mutex::new(Features::default()));
        let mut a = Analyzer::new(48000.0, shared, rx, None);
        a.fps = 90.0;
        let bpm = 127.0;
        a.period = 60.0 * a.fps / bpm;
        a.confidence = 0.6;
        a.env = stab_env(a.fps, bpm, 6.0, 1.5);
        for _ in 0..30 {
            a.estimate_tempo();
        }
        let got = 60.0 * a.fps / a.period;
        assert!((got - bpm).abs() < 6.0, "tempo flapped to {got}");
    }

    /// If the shadow won the initial lock (84.5 instead of 127), the more
    /// plausible tempo is allowed to escape the suppression and take over.
    #[test]
    fn tempo_recovers_from_wrong_side_lock() {
        let (_tx, rx) = mpsc::channel();
        let shared: SharedFeatures = Arc::new(Mutex::new(Features::default()));
        let mut a = Analyzer::new(48000.0, shared, rx, None);
        a.fps = 90.0;
        a.period = 60.0 * a.fps / 84.5;
        a.confidence = 0.6;
        a.env = stab_env(a.fps, 127.0, 6.0, 1.5);
        for _ in 0..20 {
            a.estimate_tempo();
        }
        let got = 60.0 * a.fps / a.period;
        assert!((got - 127.0).abs() < 6.0, "stuck at {got}");
    }

    /// A real tempo change must still get through: locked at 140 while the
    /// audio is genuinely 110 (not a harmonic), it follows within a few
    /// windows.
    #[test]
    fn tempo_follows_genuine_change() {
        let (_tx, rx) = mpsc::channel();
        let shared: SharedFeatures = Arc::new(Mutex::new(Features::default()));
        let mut a = Analyzer::new(48000.0, shared, rx, None);
        a.fps = 90.0;
        a.period = 60.0 * a.fps / 140.0;
        a.confidence = 0.6;
        a.env = stab_env(a.fps, 110.0, 6.0, 1.5);
        for _ in 0..8 {
            a.estimate_tempo();
        }
        let got = 60.0 * a.fps / a.period;
        assert!((got - 110.0).abs() < 5.0, "tempo stuck at {got}");
    }

    const TAU_F: f32 = std::f32::consts::TAU;
}
