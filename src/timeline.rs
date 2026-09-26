//! Timelines: a song plus a cue list of visual actions pinned to beats.
//!
//! Docs are JSON in `<config>/timelines/*.json`. Cues fire while the song
//! plays (its own playhead drives dispatch) or, with follow-live on, when the
//! onset-envelope matcher recognises the track playing in the room and locks
//! to the right bar.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{Action, Fx, Mode, Settings, Tristate};
use crate::panel::Status;

/// A visual action pinned to a beat. `beat` counts from 0 at `first_beat`,
/// so integer beats are the grid and quarters are `x.25`/`x.5`/`x.75`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Cue {
    pub beat: f64,
    pub kind: CueKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum CueKind {
    /// Jump straight to a named scene.
    Scene(String),
    NextScene,
    PrevScene,
    /// Auto / Manual / Static.
    Mode(Mode),
    /// Dancer overlay on/off.
    Dancer(bool),
    /// Jump to a named clip.
    Clip(String),
    NextClip,
    NextLook,
    /// `None` = back to auto-pilot look.
    Look(Option<usize>),
    Canon(Tristate),
    Blackout(bool),
    /// Apply a transform (also turns auto-pick off).
    Fx(Fx),
    FxAuto(bool),
}

impl CueKind {
    /// One representative of each variant for the "add cue" picker.
    pub fn picker() -> Vec<CueKind> {
        vec![
            Self::Scene(String::new()),
            Self::NextScene,
            Self::PrevScene,
            Self::Mode(Mode::Auto),
            Self::Dancer(true),
            Self::Clip(String::new()),
            Self::NextClip,
            Self::NextLook,
            Self::Look(None),
            Self::Canon(Tristate::Auto),
            Self::Blackout(true),
            Self::Fx(Fx::Off),
            Self::FxAuto(true),
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Scene(_) => "Scene",
            Self::NextScene => "Next scene",
            Self::PrevScene => "Prev scene",
            Self::Mode(_) => "Mode",
            Self::Dancer(_) => "Dancer",
            Self::Clip(_) => "Clip",
            Self::NextClip => "Next clip",
            Self::NextLook => "Next look",
            Self::Look(_) => "Look",
            Self::Canon(_) => "Canon",
            Self::Blackout(_) => "Blackout",
            Self::Fx(_) => "Transform",
            Self::FxAuto(_) => "Auto FX",
        }
    }

    /// One-line summary for the cue list/strip labels.
    pub fn detail(&self) -> String {
        match self {
            Self::Scene(n) | Self::Clip(n) => n.clone(),
            Self::Mode(m) => format!("{m:?}"),
            Self::Dancer(b) | Self::Blackout(b) | Self::FxAuto(b) => {
                if *b { "on".into() } else { "off".into() }
            }
            Self::Look(None) => "auto".into(),
            Self::Look(Some(i)) => format!("look {}", i + 1),
            Self::Canon(t) => format!("{t:?}"),
            Self::Fx(f) => f.label().to_string(),
            _ => String::new(),
        }
    }
}

/// What a hotkey/UI action becomes when recorded into the timeline.
/// `s`/`st` are the state *after* the action was applied, so toggles record
/// the state they moved to (a blackout keypress while blacked out records
/// `Blackout(false)` — pressing play reproduces what you saw).
pub fn cue_for_action(action: Action, s: &Settings, st: &Status) -> Option<CueKind> {
    Some(match action {
        Action::NextScene => CueKind::NextScene,
        Action::PrevScene => CueKind::PrevScene,
        Action::ModeAuto => CueKind::Mode(Mode::Auto),
        Action::ModeStatic => CueKind::Mode(Mode::Static),
        Action::ModeManual => CueKind::Mode(Mode::Manual),
        // Toggles record POST-action state: the render thread applies the
        // action asynchronously, so `s` still holds the pre-action values.
        Action::ToggleDancer => CueKind::Dancer(!s.dancer_enabled),
        Action::NextClip => CueKind::NextClip,
        Action::NextStyle => CueKind::NextLook,
        Action::CycleCanon => CueKind::Canon(match s.canon {
            Tristate::Auto => Tristate::On,
            Tristate::On => Tristate::Off,
            Tristate::Off => Tristate::Auto,
        }),
        Action::Blackout => CueKind::Blackout(!st.blackout),
        Action::CycleFx => CueKind::Fx(s.fx.next()),
        _ => return None,
    })
}

/// A timeline document — the serialisable bit. `onsets`/`overview` travel
/// with it so the strip and the live matcher work without re-decoding the
/// song (playback still needs the audio file itself).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Timeline {
    pub name: String,
    /// Song the cues were authored against.
    pub song: PathBuf,
    pub bpm: f64,
    /// Seconds where beat 0 lives.
    pub first_beat: f64,
    pub duration: f64,
    #[serde(default)]
    pub cues: Vec<Cue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub onsets: Vec<f32>,
    #[serde(default)]
    pub onset_fps: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overview: Vec<f32>,
}

impl Timeline {
    pub fn from_song(song: &crate::song::Song) -> Self {
        Self {
            name: song.name.clone(),
            song: song.path.clone(),
            bpm: song.bpm,
            first_beat: song.first_beat,
            duration: song.duration,
            cues: Vec::new(),
            onsets: song.onsets.clone(),
            onset_fps: song.onset_fps,
            overview: song.overview.clone(),
        }
    }

    pub fn beats_per_sec(&self) -> f64 {
        self.bpm / 60.0
    }
    /// Song seconds → beat position.
    pub fn beat_at(&self, t: f64) -> f64 {
        (t - self.first_beat) * self.beats_per_sec()
    }
    /// Beat position → song seconds.
    pub fn time_at(&self, beat: f64) -> f64 {
        self.first_beat + beat / self.beats_per_sec()
    }
    pub fn total_beats(&self) -> f64 {
        self.beat_at(self.duration).max(0.0)
    }

    /// Keep cues ordered; call after any edit or load.
    pub fn sort_cues(&mut self) {
        self.cues.sort_by(|a, b| a.beat.total_cmp(&b.beat));
    }

    /// Cues lying in (after, up_to] beats — the slice to fire this frame.
    pub fn cues_between(&self, after: f64, up_to: f64) -> Vec<&Cue> {
        self.cues.iter().filter(|c| c.beat > after && c.beat <= up_to).collect()
    }

    pub fn save(&self, dir: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(dir).context("create timelines dir")?;
        let file = dir.join(format!("{}.json", sanitize(&self.name)));
        let json = serde_json::to_string_pretty(self).context("encode timeline")?;
        std::fs::write(&file, json).with_context(|| format!("write {}", file.display()))?;
        Ok(file)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let mut doc: Self = serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        doc.sort_cues();
        Ok(doc)
    }

    /// All timelines saved under the given dir (file paths, not parsed).
    pub fn list(dir: &Path) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }
}

fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' })
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() { "timeline".into() } else { s }
}

/// Playback state the panel shows and the render loop steers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayMode {
    Stopped,
    Playing,
    Paused,
}

/// Player controls the panel sends to the render thread.
#[derive(Clone, Copy, Debug)]
pub enum SongCtl {
    /// Play, or pause if playing.
    Toggle,
    Stop,
    Seek(f64),
}

/// Everything the panel and render thread share about the timeline.
pub struct TimelineState {
    pub doc: Option<Timeline>,
    pub mode: PlayMode,
    /// Playhead in song seconds (from the player, or live-match estimate).
    pub pos_s: f64,
    /// Where the edit cursor / strip click sits — used by "add cue".
    pub cursor_s: f64,
    /// Recording: actions are written into the cue list at the playhead.
    pub recording: bool,
    /// Follow-live: cross-correlate room audio onsets against the song and
    /// fire cues at the matched position.
    pub autosync: bool,
    pub live_locked: bool,
    pub live_score: f32,
    /// Cues/song changed since last save.
    pub dirty: bool,
    /// Snap cue placement and edits to quarter beats.
    pub snap: bool,
    /// Status line for the panel ("loading…", "saved x.json", errors).
    pub message: String,
    /// Loading in the background — the transport is disabled meanwhile.
    pub busy: bool,
}

impl TimelineState {
    pub fn new() -> Shared {
        Arc::new(Mutex::new(Self {
            doc: None,
            mode: PlayMode::Stopped,
            pos_s: 0.0,
            cursor_s: 0.0,
            recording: false,
            autosync: true,
            live_locked: false,
            live_score: 0.0,
            dirty: false,
            snap: true,
            message: String::new(),
            busy: false,
        }))
    }
}

pub type Shared = Arc<Mutex<TimelineState>>;

// ---------------------------------------------------------------------------
// Follow-live matcher: normalised cross-correlation of the live onset
// envelope against the song's. Cheap enough to run on the render thread every
// half-second (a few million FLOPs at ~21 Hz subsampled resolution).
// ---------------------------------------------------------------------------

/// Seconds of live audio needed before a match is attempted.
const MIN_WINDOW_S: f64 = 12.0;
/// Live window actually correlated (tail of the tap).
const WINDOW_S: f64 = 20.0;
/// Subsample factor — correlation runs at onset_fps/4 (~21 Hz).
const SUB: usize = 4;
/// Score above this locks the match; below LOW unlocks (sticky in between).
const LOCK_SCORE: f32 = 0.5;
const UNLOCK_SCORE: f32 = 0.28;

pub struct Matcher {
    pub locked: bool,
    pub score: f32,
    /// Song-time the latest live sample maps to when locked.
    pub pos_s: f64,
    /// Wall clock the last position estimate was taken at — between
    /// evaluations the estimate coasts forward at 1× speed.
    coasted_at: Instant,
    /// Skip evaluation until this instant (runs ~2×/sec max).
    next_eval: Instant,
}

impl Matcher {
    pub fn new() -> Self {
        Self {
            locked: false,
            score: 0.0,
            pos_s: 0.0,
            coasted_at: Instant::now(),
            next_eval: Instant::now(),
        }
    }

    /// A full correlation is expensive — run at most twice a second.
    pub fn due(&self) -> bool {
        Instant::now() >= self.next_eval
    }

    /// Advance the locked estimate by wall-clock — cheap, call every frame.
    /// Returns the live-estimated song position while locked.
    pub fn coast(&mut self) -> Option<f64> {
        let now = Instant::now();
        if self.locked {
            self.pos_s += now.duration_since(self.coasted_at).as_secs_f64();
        }
        self.coasted_at = now;
        self.locked.then_some(self.pos_s)
    }

    /// Run a correlation pass — call only when `due()`. Returns the
    /// live-estimated song position while locked. `live_env`/`live_fps` come
    /// from the analyser tap; the song envelope is stored on the doc.
    pub fn update(&mut self, live_env: &[f32], live_fps: f64, doc: &Timeline) -> Option<f64> {
        self.next_eval = Instant::now() + Duration::from_millis(500);

        let window_hops = (WINDOW_S * live_fps) as usize;
        let min_hops = (MIN_WINDOW_S * live_fps) as usize;
        if live_env.len() < min_hops || doc.onsets.len() < min_hops || doc.onset_fps <= 0.0 {
            self.locked = false;
            self.score = 0.0;
            return None;
        }
        let n_live = live_env.len().min(window_hops);
        let live = &live_env[live_env.len() - n_live..];

        // Resample the song envelope onto live-rate hops (rates differ when
        // the file isn't the same sample rate as the capture device).
        let scale = doc.onset_fps / live_fps;
        let n_song = (doc.onsets.len() as f64 / scale) as usize;
        if n_song <= n_live + SUB {
            self.locked = false;
            return None;
        }
        let song_at = |i: usize| -> f32 {
            let x = i as f64 * scale;
            let j = x as usize;
            let f = (x - j as f64) as f32;
            doc.onsets.get(j).copied().unwrap_or(0.0) * (1.0 - f)
                + doc.onsets.get(j + 1).copied().unwrap_or(0.0) * f
        };

        // Subsampled z-normalised cross-correlation. Song window norms come
        // from a prefix sum of squares so each lag is O(window).
        let lw = n_live / SUB;
        let sw = n_song / SUB;
        if lw < 16 || sw <= lw {
            self.locked = false;
            return None;
        }
        let l_sub: Vec<f32> = (0..lw).map(|i| live[i * SUB]).collect();
        let l_mean = l_sub.iter().sum::<f32>() / lw as f32;
        let l_norm = l_sub.iter().map(|v| (v - l_mean).powi(2)).sum::<f32>().sqrt().max(1e-9);

        let s_sub: Vec<f32> = (0..sw + 1).map(|i| song_at(i * SUB)).collect();
        let s_mean = s_sub.iter().take(sw).sum::<f32>() / sw as f32;
        // Prefix sums of (s - mean)² and of (s - mean) for the window norms.
        let mut ps = Vec::with_capacity(sw + 1);
        let mut pss = Vec::with_capacity(sw + 1);
        let (mut a, mut b) = (0.0f64, 0.0f64);
        ps.push(0.0f32);
        pss.push(0.0f32);
        for &v in s_sub.iter().take(sw) {
            let d = (v - s_mean) as f64;
            a += d;
            b += d * d;
            ps.push(a as f32);
            pss.push(b as f32);
        }

        let mut best = (0usize, f32::MIN);
        for lag in 0..=(sw - lw) {
            // Window norm: sqrt( Σd² - (Σd)²/w ) over song[lag..lag+lw].
            let sd = (pss[lag + lw] - pss[lag]) - (ps[lag + lw] - ps[lag]).powi(2) / lw as f32;
            if sd <= 1e-9 {
                continue;
            }
            let mut dot = 0.0f32;
            for i in 0..lw {
                dot += (l_sub[i] - l_mean) * (s_sub[lag + i] - s_mean);
            }
            let score = dot / (l_norm * sd.sqrt());
            if score > best.1 {
                best = (lag, score);
            }
        }
        if !best.1.is_finite() {
            self.locked = false;
            self.score = 0.0;
            return None;
        }

        // The last live sample ("now") sits at song hop base + (lw-1)*SUB.
        let last_hop = |lag: i64| -> i64 { lag + (lw as i64 - 1) * SUB as i64 };
        let (mut pos_hops, mut score) = (last_hop(best.0 as i64 * SUB as i64), best.1);
        for off in -3..=3i64 {
            // Full-res refinement: same subsampled stride, shifted by hops.
            let base = (best.0 as i64 * SUB as i64 + off).clamp(0, (sw - lw) as i64) as usize;
            let mut dot = 0.0f32;
            let mut ss = 0.0f32;
            for i in 0..lw {
                let v = song_at(base + i * SUB);
                dot += (l_sub[i] - l_mean) * (v - s_mean);
                ss += (v - s_mean).powi(2);
            }
            let sc = dot / (l_norm * ss.sqrt().max(1e-9));
            if sc > score {
                score = sc;
                pos_hops = last_hop(base as i64);
            }
        }

        self.score = score;
        if self.locked {
            if score < UNLOCK_SCORE {
                self.locked = false;
            }
        } else if score >= LOCK_SCORE {
            self.locked = true;
        }
        if self.locked {
            self.pos_s = pos_hops.max(0) as f64 / live_fps;
            self.coasted_at = Instant::now();
            Some(self.pos_s)
        } else {
            None
        }
    }

    /// Forget the lock — e.g. the timeline was unloaded or switched.
    pub fn reset(&mut self) {
        self.locked = false;
        self.score = 0.0;
        self.pos_s = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> Timeline {
        Timeline {
            name: "test".into(),
            song: PathBuf::from("x.wav"),
            bpm: 120.0,
            first_beat: 0.5,
            duration: 120.0,
            cues: vec![
                Cue { beat: 8.0, kind: CueKind::NextScene },
                Cue { beat: 4.0, kind: CueKind::Blackout(true) },
            ],
            onsets: Vec::new(),
            onset_fps: 0.0,
            overview: Vec::new(),
        }
    }

    #[test]
    fn beat_time_roundtrip_and_windows() {
        let mut d = doc();
        d.sort_cues();
        assert_eq!(d.beat_at(0.5), 0.0);
        assert_eq!(d.beat_at(1.0), 1.0);
        assert_eq!(d.time_at(4.0), 2.5);
        assert_eq!(d.total_beats(), 239.0);
        // cues are (after, up_to]: [4, 8] both land inside (3, 8].
        assert_eq!(d.cues_between(3.0, 8.0).len(), 2);
        assert_eq!(d.cues_between(4.0, 8.0).len(), 1);
        assert_eq!(d.cues_between(8.0, 9.0).len(), 0);
    }

    #[test]
    fn timeline_json_roundtrip() {
        let mut d = doc();
        d.cues.clear();
        d.cues.push(Cue { beat: 0.0, kind: CueKind::Scene("clouds".into()) });
        d.cues.push(Cue { beat: 16.5, kind: CueKind::Fx(Fx::Kaleido6) });
        d.cues.push(Cue { beat: 20.0, kind: CueKind::Look(Some(2)) });
        d.cues.push(Cue { beat: 21.0, kind: CueKind::Canon(Tristate::On) });
        d.sort_cues();
        let dir = std::env::temp_dir().join("trippin_tl_test");
        let path = d.save(&dir).unwrap();
        let back = Timeline::load(&path).unwrap();
        assert_eq!(back.cues.len(), d.cues.len());
        assert_eq!(back.cues[0].kind, CueKind::Scene("clouds".into()));
        assert_eq!(back.cues[1].kind, CueKind::Fx(Fx::Kaleido6));
        assert_eq!(back.cues[2].kind, CueKind::Look(Some(2)));
        assert_eq!(back.cues[3].kind, CueKind::Canon(Tristate::On));
        assert!((back.bpm - 120.0).abs() < 1e-9);
        let _ = std::fs::remove_dir_all(dir);
    }
}
