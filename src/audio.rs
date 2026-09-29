//! Audio capture (WASAPI loopback or a chosen input device) and live analysis:
//! band energies, onsets, tempo and beat phase. The render loop reads a
//! `Features` snapshot each frame and extrapolates the beat phase from it.

use std::collections::VecDeque;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;

pub const SPECTRUM_BINS: usize = 32;

pub(crate) const FFT_SIZE: usize = 2048;
pub(crate) const HOP: usize = 512;
const ENV_SECONDS: f32 = 8.0;
const MIN_BPM: f32 = 70.0;
const MAX_BPM: f32 = 180.0;
/// DJ convention: detected tempos above this are almost always a
/// double-time read (DnB, hard techno) — the tracker runs them at
/// half-tempo so visuals breathe on the half-time pulse.
const HALF_TEMPO_ABOVE: f32 = 144.0;

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
        let _ = mic; // only consulted on macOS (system-audio vs input choice)
        let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(64);

        #[cfg(target_os = "macos")]
        if device.is_none() && !mic {
            match crate::sysaudio::start(tx.clone()) {
                Ok(cap) => {
                    return Self::spawn(
                        rx,
                        crate::sysaudio::SAMPLE_RATE,
                        "system audio".into(),
                        Backend::System(cap),
                        tap,
                    );
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

        let stream = match supported.sample_format() {
            SampleFormat::F32 => build_stream::<f32>(&dev, &config, channels, tx)?,
            SampleFormat::I16 => build_stream::<i16>(&dev, &config, channels, tx)?,
            SampleFormat::I32 => build_stream::<i32>(&dev, &config, channels, tx)?,
            SampleFormat::U16 => build_stream::<u16>(&dev, &config, channels, tx)?,
            f => return Err(anyhow!("unsupported sample format {f:?}")),
        };
        stream.play()?;

        Self::spawn(rx, sample_rate, name, Backend::Cpal(stream), tap)
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
        let eng = Self::spawn(rx, analysis_sr as f32, name, Backend::Song, tap)?;
        Ok((eng, player))
    }

    /// Shared tail: the analysis thread and the feature snapshot channel.
    fn spawn(
        rx: mpsc::Receiver<Vec<f32>>,
        sample_rate: f32,
        device_name: String,
        backend: Backend,
        tap: Option<SharedEnv>,
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
        })
    }
}

fn build_stream<T>(
    dev: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    tx: mpsc::SyncSender<Vec<f32>>,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let stream = dev.build_input_stream::<T, _, _>(
        config.clone(),
        move |data: &[T], _| {
            let mono: Vec<f32> = data
                .chunks(channels)
                .map(|frame| {
                    frame.iter().map(|&s| s.to_sample::<f32>()).sum::<f32>() / channels as f32
                })
                .collect();
            // Drop audio rather than block the audio thread if analysis stalls.
            let _ = tx.try_send(mono);
        },
        |e| eprintln!("audio stream error: {e}"),
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
}

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
        }
    }

    fn run(mut self, rx: mpsc::Receiver<Vec<f32>>) {
        while let Ok(chunk) = rx.recv() {
            for s in chunk {
                self.buf.push_back(s);
                if self.buf.len() > FFT_SIZE {
                    self.buf.pop_front();
                }
                self.since_hop += 1;
                if self.since_hop >= HOP && self.buf.len() == FFT_SIZE {
                    self.since_hop = 0;
                    self.frame();
                }
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

        // Time-domain trace: 64 samples decimated from the newest 1024
        // (~21 ms at 48 kHz), lightly auto-levelled so quiet tracks still
        // show a wiggle.
        let n = self.buf.len();
        let wgain = (0.5 / (rms * 3.0 + 0.02)).clamp(0.6, 5.0);
        for (i, w) in self.f.waveform.iter_mut().enumerate() {
            let s = self.buf[n - 1024 + i * 16 + 8];
            *w = (s * wgain).clamp(-1.0, 1.0);
        }

        self.track_beats(silent);

        self.frames_since_tempo += 1;
        if self.frames_since_tempo as f32 > self.fps * 0.5 && self.env.len() as f32 > self.fps * 4.0
        {
            self.frames_since_tempo = 0;
            if !silent {
                self.estimate_tempo();
                self.correct_phase();
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
                }
            }
        }

        self.f.silent = silent;
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
        self.phase += 1.0 / self.period;
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
            if self.downbeat_votes[best as usize]
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
        let mut best = (0usize, f32::MIN);
        for lag in min_lag..=max_lag {
            let bpm = 60.0 * self.fps / lag as f32;
            // Log-Gaussian tempo prior centred on 124 BPM.
            let prior = (-0.5 * ((bpm / 124.0).log2() / 0.5).powi(2)).exp();
            // Reinforce with the double-period (bar-level structure).
            let dbl = acs.get(lag * 2).copied().unwrap_or(0.0);
            let score = (acs[lag] + 0.5 * dbl.max(0.0)) * prior;
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
        // Octave fix: a raw estimate above 144 BPM is nearly always the
        // double-time harmonic — drop to half-tempo (174 → 87).
        if 60.0 * self.fps / period > HALF_TEMPO_ABOVE {
            period *= 2.0;
        }
        let conf = (acs[lag] / zero.max(1e-12)).clamp(0.0, 1.0);
        self.confidence += (conf - self.confidence) * 0.3;

        let rel = (period - self.period).abs() / self.period;
        if rel < 0.04 {
            self.period += (period - self.period) * 0.25;
            self.pending_bpm = None;
        } else {
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
        // `off` frames ago there was a beat, so the phase now is off/period.
        let target = best.0 as f32 / period;
        let mut err = target - self.phase;
        if err > 0.5 {
            err -= 1.0;
        } else if err < -0.5 {
            err += 1.0;
        }
        let gain = if self.confidence > 0.2 { 0.35 } else { 0.1 };
        self.phase += err * gain;
        if self.phase < 0.0 {
            self.phase += 1.0;
            self.beat_count = self.beat_count.saturating_sub(1);
        } else if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.beat_count += 1;
        }
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
    for (i, &s) in song.mono.iter().enumerate() {
        a.buf.push_back(s);
        if a.buf.len() > FFT_SIZE {
            a.buf.pop_front();
        }
        a.since_hop += 1;
        if a.since_hop >= HOP && a.buf.len() == FFT_SIZE {
            a.since_hop = 0;
            a.frame();
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
            if t >= next_print {
                println!(
                    "{:5.0}s {:6.1} {:6.2} {:5.2} {:6} {:8.2}   {:.2} {:.2} {:.2} {:.2}   {:3} {:3} {:3} {:3}   {:.2}{}",
                    t, a.f.bpm, a.f.groove, a.f.calm, kicks, a.f.energy,
                    l[0], l[1], l[2], l[3], hit_n[0], hit_n[1], hit_n[2], hit_n[3],
                    rate_acc / rate_n.max(1) as f32,
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

#[cfg(test)]
mod tests {
    use super::*;

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

    const TAU_F: f32 = std::f32::consts::TAU;
}
