//! Song files for timelines: decode (mp3/flac/wav/aac/ogg via symphonia),
//! offline analysis (onset envelope, tempo, beat-grid origin, strip
//! overview), and a player that feeds the analyser and the speakers at once
//! — the visuals react to the track exactly like a live input, and cue
//! timing comes from the real playhead.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;

use crate::audio::{FFT_SIZE, HOP};

/// A decoded song plus everything the timeline UI and the live matcher need.
pub struct Song {
    pub name: String,
    pub path: PathBuf,
    pub sr: u32,
    /// Mono mix, source sample rate.
    pub mono: Arc<Vec<f32>>,
    pub duration: f64,
    pub bpm: f64,
    /// Seconds where the beat grid starts (the "one" nearest t=0).
    pub first_beat: f64,
    /// Spectral-flux onset envelope, one value per `HOP` samples.
    pub onsets: Vec<f32>,
    pub onset_fps: f64,
    /// ~1500 peak amplitudes — the waveform strip in the timeline UI.
    pub overview: Vec<f32>,
}

/// Audio file extensions we try to load.
pub const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "opus", "m4a", "aac", "aiff", "aif",
];

pub fn is_audio_file(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTS.contains(&e.to_lowercase().as_str()))
}

pub fn load(path: &Path) -> Result<Song> {
    let (mono, sr) = decode(path)?;
    if mono.len() < FFT_SIZE * 2 {
        return Err(anyhow!("too short to analyse"));
    }
    let duration = mono.len() as f64 / sr as f64;
    let fps = sr as f64 / HOP as f64;
    let (onsets, bass) = onset_envelope(&mono, sr);
    let (bpm, first_beat) = estimate_bpm(&onsets, fps as f32).unwrap_or((120.0, 0.0));
    let first_beat = align_downbeat(&bass, fps, bpm, first_beat);
    let overview = make_overview(&mono, 1600);
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("song")
        .to_string();
    Ok(Song {
        name,
        path: path.to_path_buf(),
        sr,
        mono: Arc::new(mono),
        duration,
        bpm,
        first_beat,
        onsets,
        onset_fps: fps,
        overview,
    })
}

fn decode(path: &Path) -> Result<(Vec<f32>, u32)> {
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::errors::Error as SymphoniaError;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    let file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .context("unrecognised audio format")?;
    let track = format
        .default_track(TrackType::Audio)
        .context("no audio track")?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .cloned()
        .context("no audio codec parameters")?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .context("unsupported codec")?;

    let mut mono = Vec::new();
    let mut sr = 0u32;
    let mut interleaved = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(SymphoniaError::IoError(_)) | Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(e)) => {
                eprintln!("decode warning: {e}");
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        let spec = decoded.spec();
        sr = spec.rate();
        let channels = spec.channels().count().max(1);
        interleaved.clear();
        decoded.copy_to_vec_interleaved::<f32>(&mut interleaved);
        for frame in interleaved.chunks(channels) {
            mono.push(frame.iter().sum::<f32>() / channels as f32);
        }
    }
    if sr == 0 || mono.is_empty() {
        return Err(anyhow!("no decodable audio"));
    }
    Ok((mono, sr))
}

/// The same log spectral-flux the live analyser computes — the live-match
/// correlation needs envelopes measured the same way. Also returns a
/// low-band (<150 Hz) level envelope for downbeat voting.
fn onset_envelope(mono: &[f32], sr: u32) -> (Vec<f32>, Vec<f32>) {
    let fft = FftPlanner::new().plan_fft_forward(FFT_SIZE);
    let window: Vec<f32> = (0..FFT_SIZE)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos())
        .collect();
    let low_bin = ((200.0 / sr as f32 * FFT_SIZE as f32) as usize).clamp(1, FFT_SIZE / 2 - 1);
    let bass_bin = ((150.0 / sr as f32 * FFT_SIZE as f32) as usize).clamp(1, FFT_SIZE / 2 - 1);
    let mut prev = vec![0.0f32; FFT_SIZE / 2];
    let mut env = Vec::with_capacity(mono.len() / HOP);
    let mut bass = Vec::with_capacity(mono.len() / HOP);
    let mut spec = vec![Complex::new(0.0f32, 0.0); FFT_SIZE];
    let mut hops = 0usize;
    while hops * HOP + FFT_SIZE <= mono.len() {
        let frame = &mono[hops * HOP..hops * HOP + FFT_SIZE];
        for (i, (&s, &w)) in frame.iter().zip(&window).enumerate() {
            spec[i] = Complex::new(s * w, 0.0);
        }
        fft.process(&mut spec);
        let mut flux = 0.0f32;
        let mut low = 0.0f32;
        for (i, p) in prev.iter_mut().enumerate() {
            let lm = (1.0 + 100.0 * spec[i].norm() / FFT_SIZE as f32).ln();
            let d = lm - *p;
            if d > 0.0 {
                flux += d * if i < low_bin { 2.0 } else { 1.0 };
            }
            if i < bass_bin {
                low += lm;
            }
            *p = lm;
        }
        env.push(flux);
        bass.push(low / bass_bin as f32);
        hops += 1;
    }
    (env, bass)
}

/// Autocorrelate the onset envelope for the tempo (log-Gaussian prior on
/// 124 BPM, same as the live tracker), then comb-filter for the grid phase.
/// Returns (bpm, seconds-of-first-beat).
fn estimate_bpm(env: &[f32], fps: f32) -> Option<(f64, f64)> {
    let min_lag = (60.0 * fps / 180.0) as usize;
    let max_lag = ((60.0 * fps / 70.0) as usize).min(env.len() / 2);
    if max_lag <= min_lag + 2 {
        return None;
    }
    let mean = env.iter().sum::<f32>() / env.len() as f32;
    let x: Vec<f32> = env.iter().map(|v| v - mean).collect();
    let ac = |lag: usize| -> f32 {
        x[lag..].iter().zip(&x).map(|(a, b)| a * b).sum::<f32>() / (x.len() - lag) as f32
    };
    let acs: Vec<f32> = (0..=max_lag + 1)
        .map(|l| if l >= min_lag { ac(l) } else { 0.0 })
        .collect();
    let mut best = (0usize, f32::MIN);
    for lag in min_lag..=max_lag {
        let bpm = 60.0 * fps / lag as f32;
        let prior = (-0.5 * ((bpm / 124.0).log2() / 0.5).powi(2)).exp();
        let dbl = acs.get(lag * 2).copied().unwrap_or(0.0);
        let score = (acs[lag] + 0.5 * dbl.max(0.0)) * prior;
        if score > best.1 {
            best = (lag, score);
        }
    }
    let lag = best.0;
    let (a, b, c) = (acs[lag - 1], acs[lag], acs[lag + 1]);
    let denom = a - 2.0 * b + c;
    let off = if denom.abs() > 1e-12 {
        (0.5 * (a - c) / denom).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    let period = lag as f32 + off;
    let bpm = 60.0 * fps as f64 / period as f64;

    // Comb-filter the whole envelope at that period: the offset with the most
    // onset energy on the grid is where the beats fall.
    let steps = period.ceil() as usize;
    let mut phase = 0usize;
    let mut best_sum = f32::MIN;
    for o in 0..steps {
        let mut sum = 0.0f32;
        let mut i = o as f32;
        while (i as usize) < env.len() {
            let j = i.round() as usize;
            let v = env
                .get(j)
                .copied()
                .unwrap_or(0.0)
                .max(env.get(j.saturating_sub(1)).copied().unwrap_or(0.0))
                .max(env.get(j + 1).copied().unwrap_or(0.0) * 0.8);
            sum += v;
            i += period;
        }
        if sum > best_sum {
            best_sum = sum;
            phase = o;
        }
    }
    Some((bpm, phase as f64 / fps as f64))
}

/// Vote which beat-of-4 carries the most low-band energy — kicks and
/// basslines land on the one — and shift the grid origin so beat 0 is a
/// downbeat. The comb filter finds where *a* beat falls, not the bar phase:
/// without this, every bar boundary sits up to 3 beats off (the "¼-bar
/// early transitions" symptom).
fn align_downbeat(bass: &[f32], fps: f64, bpm: f64, first_beat: f64) -> f64 {
    if bass.is_empty() || bpm <= 0.0 {
        return first_beat;
    }
    let period_s = 60.0 / bpm;
    let mut votes = [0.0f32; 4];
    let mut n = 0usize;
    let mut t = first_beat;
    while t * fps < bass.len() as f64 - 1.0 {
        if t >= 0.0 {
            let h = (t * fps).round() as usize;
            let v = bass[h]
                .max(bass.get(h.wrapping_sub(1)).copied().unwrap_or(0.0) * 0.9)
                .max(bass.get(h + 1).copied().unwrap_or(0.0) * 0.9);
            votes[n % 4] += v;
        }
        n += 1;
        t += period_s;
    }
    let slot = (0..4)
        .max_by(|&a, &b| votes[a].total_cmp(&votes[b]))
        .unwrap_or(0);
    // No clear winner (ambient/sparse material): keep the detected origin.
    let total: f32 = votes.iter().sum();
    if votes[slot] < total * 0.28 {
        return first_beat;
    }
    // Prefer shifting backward (keeps early intro coverage); wrap forward
    // if that would land before t=0.
    let shifted = first_beat + (slot as i64 - 4) as f64 * period_s;
    if shifted >= 0.0 {
        shifted
    } else {
        shifted + 4.0 * period_s
    }
}

fn make_overview(mono: &[f32], points: usize) -> Vec<f32> {
    let chunk = (mono.len() / points).max(1);
    mono.chunks(chunk)
        .take(points)
        .map(|c| c.iter().map(|s| s.abs()).fold(0.0f32, f32::max))
        .collect()
}

// ---------------------------------------------------------------------------
// Playback: a multi-clip show. One cpal output stream walks the global
// timeline (clip regions with gaps as silence); a feeder thread copies the
// played audio — resampled to the analyser's rate — into the analyser
// channel. So what the visuals react to is what comes out of the speakers.
// ---------------------------------------------------------------------------

/// One song region for playback: a decoded song at a global offset.
pub struct Region {
    pub offset_s: f64,
    pub song: Arc<Song>,
}

/// Global seconds → mono sample at the region's own rate.
fn sample_at(regions: &[Region], t: f64) -> f32 {
    for r in regions {
        let local = t - r.offset_s;
        if local < 0.0 {
            break;
        }
        if local >= r.song.duration {
            continue;
        }
        let pos = local * r.song.sr as f64;
        let i = pos as usize;
        let f = (pos - i as f64) as f32;
        let m = &r.song.mono;
        return m.get(i).copied().unwrap_or(0.0) * (1.0 - f)
            + m.get(i + 1).copied().unwrap_or(0.0) * f;
    }
    0.0
}

/// Plays a timeline's clip regions: a cpal output stream reads the regions
/// at the device rate; a feeder thread copies played samples into the
/// analyser channel (resampled to `analysis_sr`, since regions may differ).
/// With no output device the feeder drives a virtual clock instead.
pub struct ShowPlayer {
    /// Global timeline position, in device-rate frames.
    cursor: Arc<AtomicU64>,
    playing: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    seek_gen: Arc<AtomicU64>,
    /// Seek target in global seconds (f64 bits).
    seek_s: Arc<AtomicU64>,
    /// Device (or virtual) frames per second — cursor units.
    rate: f64,
    total_s: f64,
    _stream: Option<cpal::Stream>,
    _feeder: Option<std::thread::JoinHandle<()>>,
}

impl ShowPlayer {
    /// `regions`: decoded songs at their timeline offsets (must be sorted by
    /// offset; overlaps resolve to the earliest region). `analysis_sr` is the
    /// rate the analyser was spawned with — played audio is resampled to it.
    /// `start_s` = global seconds to begin from.
    pub fn start(
        mut regions: Vec<Region>,
        analysis_sr: f64,
        start_s: f64,
        tx: mpsc::SyncSender<Vec<f32>>,
    ) -> Result<Self> {
        if regions.is_empty() {
            return Err(anyhow!("no playable clips"));
        }
        regions.sort_by(|a, b| a.offset_s.total_cmp(&b.offset_s));
        let total_s = regions
            .iter()
            .map(|r| r.offset_s + r.song.duration)
            .fold(0.0, f64::max);

        let cursor = Arc::new(AtomicU64::new(0));
        let playing = Arc::new(AtomicBool::new(true));
        let finished = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let seek_gen = Arc::new(AtomicU64::new(0));
        let seek_s = Arc::new(AtomicU64::new(start_s.max(0.0).to_bits()));

        let regions = Arc::new(regions);
        let (stream, rate) = match build_output(
            &regions, total_s, &cursor, &playing, &finished, &seek_gen, &seek_s,
        ) {
            Some((s, r)) => (Some(s), r),
            None => {
                eprintln!("show playback: no output device — visuals only");
                (None, 48000.0)
            }
        };
        cursor.store((start_s.max(0.0) * rate) as u64, Ordering::Relaxed);

        let feeder = {
            let (regions, cursor, playing, finished, stop, seek_gen, seek_s) = (
                regions.clone(),
                cursor.clone(),
                playing.clone(),
                finished.clone(),
                stop.clone(),
                seek_gen.clone(),
                seek_s.clone(),
            );
            let has_output = stream.is_some();
            std::thread::Builder::new()
                .name("show-feed".into())
                .spawn(move || {
                    let mut sent_t = start_s.max(0.0);
                    let mut vt = start_s.max(0.0);
                    let mut vlast = Instant::now();
                    let mut seen_seek = 0u64;
                    while !stop.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(8));
                        let g = seek_gen.load(Ordering::Relaxed);
                        if g != seen_seek {
                            seen_seek = g;
                            sent_t = f64::from_bits(seek_s.load(Ordering::Relaxed)).max(0.0);
                            vt = sent_t;
                        }
                        if !has_output {
                            // No output stream to pace us — run a virtual clock.
                            let now = Instant::now();
                            if playing.load(Ordering::Relaxed) {
                                vt += now.duration_since(vlast).as_secs_f64();
                            }
                            vlast = now;
                            cursor.store((vt.min(total_s) * rate) as u64, Ordering::Relaxed);
                            if vt >= total_s {
                                finished.store(true, Ordering::Relaxed);
                                playing.store(false, Ordering::Relaxed);
                            }
                        }
                        let pos_t = (cursor.load(Ordering::Relaxed) as f64 / rate).min(total_s);
                        if pos_t > sent_t {
                            // Emit the played span at the analyser's rate —
                            // silence for gaps between regions.
                            let n = ((pos_t - sent_t) * analysis_sr) as usize;
                            if n > 0 {
                                let chunk: Vec<f32> = (0..n)
                                    .map(|i| sample_at(&regions, sent_t + i as f64 / analysis_sr))
                                    .collect();
                                let _ = tx.try_send(chunk); // drop rather than stall the analyser
                            }
                            sent_t = pos_t;
                        }
                    }
                })?
        };

        if let Some(s) = &stream {
            let _ = s.play();
        }
        Ok(Self {
            cursor,
            playing,
            finished,
            stop,
            seek_gen,
            seek_s,
            rate,
            total_s,
            _stream: stream,
            _feeder: Some(feeder),
        })
    }

    /// Global timeline seconds the output has reached.
    pub fn position_s(&self) -> f64 {
        (self.cursor.load(Ordering::Relaxed) as f64 / self.rate).min(self.total_s)
    }
    /// Global seconds.
    pub fn seek(&self, s: f64) {
        let s = s.clamp(0.0, self.total_s);
        self.seek_s.store(s.to_bits(), Ordering::Relaxed);
        self.cursor.store((s * self.rate) as u64, Ordering::Relaxed);
        self.seek_gen.fetch_add(1, Ordering::Relaxed);
        self.finished.store(false, Ordering::Relaxed);
    }
    pub fn set_playing(&self, on: bool) {
        self.playing.store(on, Ordering::Relaxed);
    }
    pub fn finished(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }
}

impl Drop for ShowPlayer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(f) = self._feeder.take() {
            let _ = f.join();
        }
    }
}

fn build_output(
    regions: &Arc<Vec<Region>>,
    total_s: f64,
    cursor: &Arc<AtomicU64>,
    playing: &Arc<AtomicBool>,
    finished: &Arc<AtomicBool>,
    seek_gen: &Arc<AtomicU64>,
    seek_s: &Arc<AtomicU64>,
) -> Option<(cpal::Stream, f64)> {
    let host = cpal::default_host();
    let dev = host.default_output_device()?;
    let supported = dev.default_output_config().ok()?;
    let config = supported.config();
    let rate = config.sample_rate as f64;
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build_output_stream::<f32>(
            &dev, config, regions, total_s, cursor, playing, finished, seek_gen, seek_s,
        )
        .ok(),
        SampleFormat::I16 => build_output_stream::<i16>(
            &dev, config, regions, total_s, cursor, playing, finished, seek_gen, seek_s,
        )
        .ok(),
        SampleFormat::U16 => build_output_stream::<u16>(
            &dev, config, regions, total_s, cursor, playing, finished, seek_gen, seek_s,
        )
        .ok(),
        _ => None,
    }?;
    Some((stream, rate))
}

fn build_output_stream<T>(
    dev: &cpal::Device,
    config: cpal::StreamConfig,
    regions: &Arc<Vec<Region>>,
    total_s: f64,
    cursor: &Arc<AtomicU64>,
    playing: &Arc<AtomicBool>,
    finished: &Arc<AtomicBool>,
    seek_gen: &Arc<AtomicU64>,
    seek_s: &Arc<AtomicU64>,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let rate = config.sample_rate as f64;
    let regions = regions.clone();
    let (cursor, playing, finished, seek_gen, seek_s) = (
        cursor.clone(),
        playing.clone(),
        finished.clone(),
        seek_gen.clone(),
        seek_s.clone(),
    );
    let mut pos = f64::from_bits(seek_s.load(Ordering::Relaxed)).max(0.0) * rate;
    let mut seen_seek = seek_gen.load(Ordering::Relaxed);
    let total_frames = total_s * rate;
    let stream = dev.build_output_stream::<T, _, _>(
        config.clone(),
        move |data: &mut [T], _| {
            let g = seek_gen.load(Ordering::Relaxed);
            if g != seen_seek {
                seen_seek = g;
                pos = f64::from_bits(seek_s.load(Ordering::Relaxed)).max(0.0) * rate;
            }
            let on = playing.load(Ordering::Relaxed);
            for out in data.chunks_mut(channels) {
                let s = if on && pos < total_frames {
                    sample_at(&regions, pos / rate)
                } else {
                    0.0
                };
                for o in out.iter_mut() {
                    *o = T::from_sample(s);
                }
                if on {
                    pos += 1.0;
                }
            }
            if pos >= total_frames {
                pos = total_frames;
                finished.store(true, Ordering::Relaxed);
                playing.store(false, Ordering::Relaxed);
            }
            cursor.store(pos.max(0.0) as u64, Ordering::Relaxed);
        },
        |e| eprintln!("show output error: {e}"),
        None,
    )?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A click track: a short bright burst every beat, quiet bed otherwise.
    /// Deterministic tempo should recover the BPM almost exactly.
    fn click_track(sr: u32, bpm: f64, secs: f64) -> Vec<f32> {
        let n = (secs * sr as f64) as usize;
        let beat = sr as f64 * 60.0 / bpm;
        let mut v = vec![0.02f32; n]; // quiet bed so silence doesn't dominate
        for (i, s) in v.iter_mut().enumerate() {
            let r = (i as f64) % beat;
            if r < 0.03 * sr as f64 {
                // 30 ms decaying burst — a fake kick.
                *s += 0.8 * (-r * 90.0 / sr as f64).exp() as f32;
            }
        }
        v
    }

    /// Minimal 16-bit mono WAV writer — exercises the real decode path.
    fn write_wav(path: &Path, sr: u32, data: &[f32]) {
        let bytes: Vec<u8> = data
            .iter()
            .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes())
            .collect();
        let mut w = Vec::with_capacity(44 + bytes.len());
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36u32 + bytes.len() as u32).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes()); // PCM
        w.extend_from_slice(&1u16.to_le_bytes()); // mono
        w.extend_from_slice(&sr.to_le_bytes());
        w.extend_from_slice(&(sr * 2).to_le_bytes()); // byte rate
        w.extend_from_slice(&2u16.to_le_bytes()); // block align
        w.extend_from_slice(&16u16.to_le_bytes()); // bits
        w.extend_from_slice(b"data");
        w.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        w.extend_from_slice(&bytes);
        std::fs::write(path, w).unwrap();
    }

    #[test]
    fn loads_and_analyses_a_wav() {
        let sr = 44100u32;
        let bpm = 128.0;
        let secs = 12.0;
        let mono = click_track(sr, bpm, secs);
        let path = std::env::temp_dir().join("trippin_test_click.wav");
        write_wav(&path, sr, &mono);

        let song = load(&path).expect("decode wav");
        assert!(
            (song.duration - secs).abs() < 0.2,
            "duration {}",
            song.duration
        );
        assert!(
            (song.bpm - bpm).abs() < 2.0,
            "bpm {} expected ~{bpm}",
            song.bpm
        );
        assert!(
            song.first_beat >= 0.0 && song.first_beat < 0.6,
            "first_beat {}",
            song.first_beat
        );
        // One onset per hop minus the warm-up FFT frame.
        assert!((song.onsets.len() as f64 - secs * song.onset_fps).abs() < 10.0);
        assert_eq!(song.overview.len(), 1600);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn bpm_estimate_is_stable_on_clean_grid() {
        let sr = 48000u32;
        let mono = click_track(sr, 174.0, 10.0);
        let (env, _bass) = onset_envelope(&mono, sr);
        let (bpm, phase) = estimate_bpm(&env, sr as f32 / HOP as f32).unwrap();
        assert!((bpm - 174.0).abs() < 2.0, "bpm {bpm}");
        assert!(phase < 0.35, "phase {phase}");
    }
}
