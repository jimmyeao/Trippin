//! Audio capture (WASAPI loopback or a chosen input device) and live analysis:
//! band energies, onsets, tempo and beat phase. The render loop reads a
//! `Features` snapshot each frame and extrapolates the beat phase from it.

use std::collections::VecDeque;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

pub const SPECTRUM_BINS: usize = 32;

const FFT_SIZE: usize = 2048;
const HOP: usize = 512;
const ENV_SECONDS: f32 = 8.0;
const MIN_BPM: f32 = 70.0;
const MAX_BPM: f32 = 180.0;

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
        }
    }
}

impl Features {
    /// Beat position (whole beats + fraction) extrapolated to `now`.
    pub fn beat_position(&self, now: Instant) -> f64 {
        let dt = now.saturating_duration_since(self.phase_at).as_secs_f64();
        self.beat_count as f64 + self.beat_phase as f64 + dt * self.bpm as f64 / 60.0
    }

    /// Beat index within the bar (0 = downbeat) for a given beat position.
    pub fn beat_in_bar(&self, pos: f64) -> u64 {
        ((pos.floor() as i64 - self.downbeat as i64).rem_euclid(4)) as u64
    }
}

pub type SharedFeatures = Arc<Mutex<Features>>;

/// Commands from the UI thread to the analyser.
pub enum Command {
    /// Treat the beat nearest to now as the downbeat.
    MarkDownbeat,
}

pub struct AudioEngine {
    _stream: cpal::Stream,
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
    /// `device`: None = loopback of the default output device (Windows); on
    /// platforms without output loopback, the default input (BlackHole, an
    /// audio interface, or the mic). Otherwise the first device whose name
    /// contains the string — inputs plus, on Windows, outputs via loopback.
    pub fn start(device: Option<&str>) -> Result<Self> {
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
                    host.output_devices()
                        .ok()
                        .and_then(|mut outs| outs.find(|d| device_name(d).to_lowercase().contains(&needle)))
                });
                found.ok_or_else(|| anyhow!("no audio device matching {needle:?}; try --list-devices"))?
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

        let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(64);
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build_stream::<f32>(&dev, &config, channels, tx)?,
            SampleFormat::I16 => build_stream::<i16>(&dev, &config, channels, tx)?,
            SampleFormat::I32 => build_stream::<i32>(&dev, &config, channels, tx)?,
            SampleFormat::U16 => build_stream::<u16>(&dev, &config, channels, tx)?,
            f => return Err(anyhow!("unsupported sample format {f:?}")),
        };
        stream.play()?;

        let features: SharedFeatures = Arc::new(Mutex::new(Features::default()));
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let shared = features.clone();
        std::thread::Builder::new()
            .name("analysis".into())
            .spawn(move || Analyzer::new(sample_rate, shared, cmd_rx).run(rx))?;

        Ok(Self { _stream: stream, features, commands: cmd_tx, device_name: name })
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
                .map(|frame| frame.iter().map(|&s| s.to_sample::<f32>()).sum::<f32>() / channels as f32)
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
    /// Onset strength envelope at `fps`, newest last.
    env: VecDeque<f32>,
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
}

impl Analyzer {
    fn new(sr: f32, shared: SharedFeatures, commands: mpsc::Receiver<Command>) -> Self {
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
            gains: [AutoGain::new(1e-3), AutoGain::new(1e-4), AutoGain::new(1e-5)],
            spec_gain: AutoGain::new(1e-4),
            flux_gain: AutoGain::new(1e-4),
            env: VecDeque::new(),
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
        let mag: Vec<f32> = spec[..FFT_SIZE / 2].iter().map(|c| c.norm() / FFT_SIZE as f32).collect();

        let band = |lo: f32, hi: f32, a: &Self| -> f32 {
            let (l, h) = (a.bin(lo), a.bin(hi));
            (mag[l..h].iter().map(|m| m * m).sum::<f32>() / (h - l) as f32).sqrt()
        };
        let raw = [band(20.0, 150.0, self), band(150.0, 2000.0, self), band(2000.0, 16000.0, self)];
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
        if !silent && bass_flux > 0.15 && raw[0] > 0.0 {
            self.f.kick = self.f.kick.max((bass_flux * 2.0).min(1.0));
        }

        // Levels, smoothed with fast attack / slower release.
        let levels: Vec<f32> = raw.iter().zip(self.gains.iter_mut()).map(|(&x, g)| g.apply(x)).collect();
        smooth(&mut self.f.bass, levels[0], 0.6, 0.15);
        smooth(&mut self.f.mid, levels[1], 0.5, 0.1);
        smooth(&mut self.f.high, levels[2], 0.5, 0.1);
        let energy = (levels[0] * 0.5 + levels[1] * 0.3 + levels[2] * 0.2).min(1.0);
        smooth(&mut self.f.energy, energy, 0.3, 0.08);
        self.energy_fast += (energy - self.energy_fast) * (1.0 / (1.5 * self.fps));
        self.energy_slow += (energy - self.energy_slow) * (1.0 / (12.0 * self.fps));
        self.f.build = ((self.energy_fast - self.energy_slow) * 4.0).clamp(-1.0, 1.0);

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
        if self.frames_since_tempo as f32 > self.fps * 0.5 && self.env.len() as f32 > self.fps * 4.0 {
            self.frames_since_tempo = 0;
            if !silent {
                self.estimate_tempo();
                self.correct_phase();
            }
        }

        for cmd in self.commands.try_iter().collect::<Vec<_>>() {
            match cmd {
                Command::MarkDownbeat => {
                    let nearest = if self.phase < 0.5 { self.beat_count } else { self.beat_count + 1 };
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
            if self.downbeat_votes[best as usize] > self.downbeat_votes[self.downbeat as usize] * 1.3 {
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
        let acs: Vec<f32> = (0..=max_lag + 1).map(|l| if l >= min_lag - 1 { ac(l) } else { 0.0 }).collect();
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
        let offset = if denom.abs() > 1e-12 { (0.5 * (a - c) / denom).clamp(-0.5, 0.5) } else { 0.0 };
        let period = lag as f32 + offset;
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
                let v = env[i].max(env[i.saturating_sub(1)]).max(env[(i + 1).min(n - 1)] * 0.8);
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
