//! Timelines: a multi-song cue sheet — Clipchamp-style song regions ("clips")
//! laid out on one global timeline in seconds, with visual cues pinned to
//! beats inside each clip.
//!
//! Docs are JSON in `<config>/timelines/*.json`. Cues fire while the show
//! plays (the player's playhead drives dispatch) or, with follow-live on,
//! when the onset-envelope matcher recognises a clip's track playing in the
//! room and locks to the right bar.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{Action, Fx, Mode, Settings, Tristate};
use crate::panel::Status;

/// A song region on the global timeline — where a track sits in the show,
/// plus the offline analysis the strip and live matcher need.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Clip {
    /// Audio file path (playback needs it; the strip doesn't).
    pub song: PathBuf,
    pub name: String,
    /// Global timeline seconds where this clip starts.
    pub offset_s: f64,
    pub bpm: f64,
    /// Seconds *within the song* where beat 0 sits.
    pub first_beat: f64,
    pub duration_s: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub onsets: Vec<f32>,
    #[serde(default)]
    pub onset_fps: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overview: Vec<f32>,
}

impl Clip {
    pub fn from_song(song: &crate::song::Song, offset_s: f64) -> Self {
        Self {
            song: song.path.clone(),
            name: song.name.clone(),
            offset_s,
            bpm: song.bpm,
            first_beat: song.first_beat,
            duration_s: song.duration,
            onsets: song.onsets.clone(),
            onset_fps: song.onset_fps,
            overview: song.overview.clone(),
        }
    }

    pub fn beats_per_sec(&self) -> f64 {
        self.bpm / 60.0
    }
    /// Clip-local seconds → beat position (beat 0 = first_beat).
    pub fn beat_at(&self, local_s: f64) -> f64 {
        (local_s - self.first_beat) * self.beats_per_sec()
    }
    /// Beat position → clip-local seconds.
    pub fn time_at(&self, beat: f64) -> f64 {
        self.first_beat + beat / self.beats_per_sec()
    }
    /// Cue beat → global timeline seconds.
    pub fn cue_time(&self, beat: f64) -> f64 {
        self.offset_s + self.time_at(beat)
    }
    pub fn end_s(&self) -> f64 {
        self.offset_s + self.duration_s
    }
    /// Beats spanned by the clip (from first_beat to its end).
    #[cfg(test)]
    pub fn total_beats(&self) -> f64 {
        self.beat_at(self.duration_s).max(0.0)
    }
    pub fn contains_s(&self, t: f64) -> bool {
        t >= self.offset_s && t < self.end_s()
    }
}

/// A visual action pinned to a beat inside one clip. `beat` counts from 0 at
/// the clip's `first_beat`, so integer beats are the grid and quarters are
/// `x.25`/`x.5`/`x.75`. The cue moves with its clip if the clip is dragged.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Cue {
    /// Index into `Timeline::clips`.
    #[serde(default)]
    pub clip: usize,
    pub beat: f64,
    /// Block length in beats — cues are Resolve-style blocks on the lane.
    /// Point actions ignore it; toggle kinds fire their `end_kind` when the
    /// block ends (e.g. `Blackout(true)` switches back off).
    #[serde(default = "default_beats")]
    pub beats: f64,
    pub kind: CueKind,
}

fn default_beats() -> f64 {
    4.0
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
    /// Jump to a named routine clip.
    Clip(String),
    NextClip,
    NextLook,
    /// `None` = back to auto-pilot look.
    Look(Option<usize>),
    /// Motion-trail echoes on the dancer (all looks).
    Trails(bool),
    Canon(Tristate),
    Blackout(bool),
    /// Apply a transform (also turns auto-pick off).
    Fx(Fx),
    FxAuto(bool),
    /// Switch the global colour palette (latches until the next palette cue).
    Palette(String),
    /// Show a text overlay (lane 0/1 chooses which of the two text tracks).
    Text(crate::text::TextSpec),
    /// Internal: fired when a Text block's end passes — fades that lane out.
    TextOff(u8),
}

impl CueKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Scene(_) => "Scene",
            Self::NextScene => "Next scene",
            Self::PrevScene => "Prev scene",
            Self::Mode(_) => "Mode",
            Self::Dancer(_) => "Dancer",
            Self::Clip(_) => "Routine",
            Self::NextClip => "Next routine",
            Self::NextLook => "Next look",
            Self::Look(_) => "Look",
            Self::Trails(_) => "Trails",
            Self::Canon(_) => "Canon",
            Self::Blackout(_) => "Blackout",
            Self::Fx(_) => "Transform",
            Self::FxAuto(_) => "Auto FX",
            Self::Palette(_) => "Palette",
            Self::Text(_) => "Text",
            Self::TextOff(_) => "Text off",
        }
    }

    /// Editor cue track: 0 scenes · 1 dancer · 2 fx · 3 show · 4-5 text lanes.
    /// A block's track is derived from its kind, so it can't be dragged sideways.
    pub fn track(&self) -> usize {
        match self {
            Self::Scene(_) | Self::NextScene | Self::PrevScene => 0,
            Self::Dancer(_)
            | Self::Clip(_)
            | Self::NextClip
            | Self::NextLook
            | Self::Look(_)
            | Self::Trails(_)
            | Self::Canon(_) => 1,
            Self::Fx(_) | Self::FxAuto(_) => 2,
            Self::Mode(_) | Self::Blackout(_) | Self::Palette(_) => 3,
            Self::Text(s) => 4 + (s.lane as usize % crate::text::TEXT_SLOTS),
            Self::TextOff(lane) => 4 + (*lane as usize % crate::text::TEXT_SLOTS),
        }
    }

    /// What fires when the cue's block ends. Only *durational* kinds
    /// release — a blackout flash or a text card lives for the block's
    /// span. State kinds latch instead: `Dancer(true)`, `Fx`, `Canon` stay
    /// in effect until an explicit off/auto cue, so a block's length is
    /// just visual.
    pub fn end_kind(&self) -> Option<CueKind> {
        match self {
            Self::Blackout(true) => Some(Self::Blackout(false)),
            Self::Text(s) => Some(Self::TextOff(s.lane)),
            _ => None,
        }
    }

    /// Short text for a block on the lane.
    pub fn text(&self) -> String {
        match self {
            Self::Scene(n) => n.clone(),
            Self::Clip(n) => n.clone(),
            Self::Mode(m) => format!("{m:?}"),
            Self::Canon(c) => format!("canon {c:?}"),
            Self::Trails(on) => format!("trails {}", if *on { "on" } else { "off" }),
            Self::Dancer(on) => format!("dancer {}", if *on { "on" } else { "off" }),
            Self::Blackout(on) => format!("blackout {}", if *on { "on" } else { "off" }),
            Self::Fx(f) => f.label().to_string(),
            Self::FxAuto(on) => format!("auto FX {}", if *on { "on" } else { "off" }),
            Self::Look(l) => match l {
                Some(i) => format!("look {}", i + 1),
                None => "auto look".into(),
            },
            Self::Palette(n) => format!("palette {n}"),
            Self::Text(s) => format!("\"{}\"", s.text),
            _ => self.label().to_lowercase(),
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

/// The show state the playhead implies at a given time — see
/// [`Timeline::state_at`]. Baseline is *everything off*: dancers, FX and
/// text start disabled and the director stays Manual until a cue on some
/// track explicitly turns them on.
#[derive(Clone, Debug)]
pub struct PlayheadState {
    /// Last absolute scene cue, if any.
    pub scene: Option<String>,
    /// Net next/prev steps after the last absolute scene cue (or from the
    /// start of the song when `scene` is `None`).
    pub scene_steps: i64,
    pub mode: Mode,
    pub dancer: bool,
    /// Last absolute routine cue, if any.
    pub clip: Option<String>,
    /// Net next-routine steps after `clip`.
    pub clip_steps: i64,
    /// `Some(l)` = a Look cue ran (`l = None` means "auto look");
    /// `None` = no Look cue yet — leave the setting alone.
    pub look: Option<Option<usize>>,
    pub look_steps: i64,
    pub trails: bool,
    pub canon: Tristate,
    pub blackout: bool,
    pub fx: Fx,
    pub fx_auto: bool,
    /// Last palette cue, if any — `None` leaves the user's pick alone.
    pub palette: Option<String>,
    /// Live text card per lane.
    pub text: [Option<crate::text::TextSpec>; crate::text::TEXT_SLOTS],
}

impl Default for PlayheadState {
    fn default() -> Self {
        Self {
            scene: None,
            scene_steps: 0,
            mode: Mode::Manual,
            dancer: false,
            clip: None,
            clip_steps: 0,
            look: None,
            look_steps: 0,
            trails: false,
            canon: Tristate::Auto,
            blackout: false,
            fx: Fx::Off,
            fx_auto: false,
            palette: None,
            text: Default::default(),
        }
    }
}

/// A timeline document — the serialisable show. Each clip carries its
/// `onsets`/`overview` so the strip and the live matcher work without
/// re-decoding audio (playback still needs the audio files themselves).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Timeline {
    pub name: String,
    #[serde(default)]
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub cues: Vec<Cue>,
    // ---- legacy single-song fields (v1 docs); migrated on load ----
    #[serde(default, skip_serializing)]
    song: Option<PathBuf>,
    #[serde(default, skip_serializing)]
    bpm: Option<f64>,
    #[serde(default, skip_serializing)]
    first_beat: Option<f64>,
    #[serde(default, skip_serializing)]
    duration: Option<f64>,
    #[serde(default, skip_serializing)]
    onsets: Option<Vec<f32>>,
    #[serde(default, skip_serializing)]
    onset_fps: Option<f64>,
    #[serde(default, skip_serializing)]
    overview: Option<Vec<f32>>,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            name: "untitled".into(),
            clips: Vec::new(),
            cues: Vec::new(),
            song: None,
            bpm: None,
            first_beat: None,
            duration: None,
            onsets: None,
            onset_fps: None,
            overview: None,
        }
    }
}

impl Timeline {
    /// The clip (if any) covering global time `t`.
    pub fn clip_at(&self, t: f64) -> Option<(usize, &Clip)> {
        self.clips.iter().enumerate().find(|(_, c)| c.contains_s(t))
    }

    /// End of the last clip on the timeline.
    pub fn end_s(&self) -> f64 {
        self.clips.iter().map(|c| c.end_s()).fold(0.0, f64::max)
    }

    /// Where a dropped/added clip would go: the current end of the timeline.
    pub fn next_offset_s(&self) -> f64 {
        self.end_s()
    }

    /// Append a decoded song as a clip at the end of the timeline. A fresh
    /// "untitled" doc takes the song's name.
    pub fn add_song(&mut self, song: &crate::song::Song) {
        let clip = Clip::from_song(song, self.next_offset_s());
        if self.clips.is_empty() && self.name == "untitled" {
            self.name = song.name.clone();
        }
        self.clips.push(clip);
    }

    /// Playback regions for the show player — clips whose audio file has
    /// been decoded into `songs`, in offset order.
    pub fn regions(
        &self,
        songs: &std::collections::HashMap<PathBuf, Arc<crate::song::Song>>,
    ) -> Vec<crate::song::Region> {
        let mut out: Vec<_> = self
            .clips
            .iter()
            .filter_map(|c| {
                songs.get(&c.song).map(|s| crate::song::Region {
                    offset_s: c.offset_s,
                    song: s.clone(),
                })
            })
            .collect();
        out.sort_by(|a, b| a.offset_s.total_cmp(&b.offset_s));
        out
    }

    /// Global seconds for a cue (None if its clip index is dangling).
    pub fn cue_time(&self, cue: &Cue) -> Option<f64> {
        self.clips.get(cue.clip).map(|c| c.cue_time(cue.beat))
    }

    /// Keep cues ordered by global position; call after any edit or load.
    pub fn sort_cues(&mut self) {
        // Global time needs clip offsets — copy them out so the sort
        // doesn't borrow `self` while sorting `self.cues`.
        let offs: Vec<(f64, f64, f64)> = self
            .clips
            .iter()
            .map(|c| (c.offset_s, c.first_beat, c.bpm))
            .collect();
        let t_of = |c: &Cue| -> f64 {
            offs.get(c.clip)
                .map(|&(o, fb, bpm)| o + fb + c.beat * 60.0 / bpm)
                .unwrap_or(f64::MAX)
        };
        self.cues.sort_by(|a, b| t_of(a).total_cmp(&t_of(b)));
    }

    /// Cues whose global time lies in (after_s, up_to_s].
    #[cfg(test)]
    pub fn cues_between_s(&self, after_s: f64, up_to_s: f64) -> Vec<&Cue> {
        self.cues
            .iter()
            .filter(|c| {
                self.cue_time(c)
                    .is_some_and(|t| t > after_s && t <= up_to_s)
            })
            .collect()
    }

    /// Global end time of a cue block (start + beats→seconds in its clip).
    pub fn cue_end_s(&self, cue: &Cue) -> Option<f64> {
        let c = self.clips.get(cue.clip)?;
        Some(c.offset_s + c.first_beat + (cue.beat + cue.beats.max(0.0)) * 60.0 / c.bpm)
    }

    /// Everything the playhead crosses in (after_s, up_to_s], ordered: cue
    /// starts fire their `kind`, block ends fire the toggle's `end_kind`.
    pub fn edges_between_s(&self, after_s: f64, up_to_s: f64) -> Vec<CueKind> {
        let mut edges: Vec<(f64, CueKind)> = Vec::new();
        for c in &self.cues {
            if let Some(t) = self.cue_time(c) {
                if t > after_s && t <= up_to_s {
                    edges.push((t, c.kind.clone()));
                }
            }
            if let (Some(e), Some(off)) = (self.cue_end_s(c), c.kind.end_kind()) {
                if e > after_s && e <= up_to_s {
                    edges.push((e, off));
                }
            }
        }
        edges.sort_by(|a, b| a.0.total_cmp(&b.0));
        edges.into_iter().map(|(_, k)| k).collect()
    }

    /// What "played straight through to `t`" would leave switched on —
    /// folded from every cue edge at or before it. Applied when playback
    /// starts and after every seek, so the visuals match the playhead
    /// instead of whatever the last-fired cues happened to leave behind.
    pub fn state_at(&self, t: f64) -> PlayheadState {
        let mut st = PlayheadState::default();
        for k in self.edges_between_s(f64::NEG_INFINITY, t) {
            match k {
                CueKind::Scene(n) => {
                    st.scene = Some(n);
                    st.scene_steps = 0;
                }
                CueKind::NextScene => st.scene_steps += 1,
                CueKind::PrevScene => st.scene_steps -= 1,
                CueKind::Mode(m) => st.mode = m,
                CueKind::Dancer(b) => st.dancer = b,
                CueKind::Clip(n) => {
                    st.clip = Some(n);
                    st.clip_steps = 0;
                    st.dancer = true;
                }
                CueKind::NextClip => {
                    st.clip_steps += 1;
                    st.dancer = true;
                }
                CueKind::NextLook => st.look_steps += 1,
                CueKind::Look(l) => {
                    st.look = Some(l);
                    st.look_steps = 0;
                }
                CueKind::Trails(b) => st.trails = b,
                CueKind::Canon(c) => st.canon = c,
                CueKind::Blackout(b) => st.blackout = b,
                CueKind::Fx(f) => {
                    st.fx = f;
                    st.fx_auto = false;
                }
                CueKind::FxAuto(b) => st.fx_auto = b,
                CueKind::Palette(n) => st.palette = Some(n),
                CueKind::Text(spec) => {
                    let lane = spec.lane as usize % crate::text::TEXT_SLOTS;
                    st.text[lane] = Some(spec);
                }
                CueKind::TextOff(l) => {
                    st.text[l as usize % crate::text::TEXT_SLOTS] = None;
                }
            }
        }
        st
    }

    /// Re-index cues after a clip is removed (indices above shift down).
    pub fn remove_clip(&mut self, i: usize) {
        if i < self.clips.len() {
            self.clips.remove(i);
            self.cues.retain(|c| c.clip != i);
            for c in &mut self.cues {
                if c.clip > i {
                    c.clip -= 1;
                }
            }
        }
    }

    pub fn save(&self, dir: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(dir).context("create timelines dir")?;
        let file = dir.join(format!("{}.json", sanitize(&self.name)));
        let json = serde_json::to_string_pretty(self).context("encode timeline")?;
        std::fs::write(&file, json).with_context(|| format!("write {}", file.display()))?;
        Ok(file)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let mut doc: Self =
            serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        // v1 migration: a bare `song` field becomes clip 0, its cues point at it.
        if doc.clips.is_empty() {
            if let Some(song) = doc.song.take() {
                doc.clips.push(Clip {
                    song,
                    name: doc.name.clone(),
                    offset_s: 0.0,
                    bpm: doc.bpm.take().unwrap_or(120.0),
                    first_beat: doc.first_beat.take().unwrap_or(0.0),
                    duration_s: doc.duration.take().unwrap_or(0.0),
                    onsets: doc.onsets.take().unwrap_or_default(),
                    onset_fps: doc.onset_fps.take().unwrap_or(0.0),
                    overview: doc.overview.take().unwrap_or_default(),
                });
            }
        }
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
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() { "timeline".into() } else { s }
}

/// Playback state the panel/editor shows and the render loop steers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayMode {
    Stopped,
    Playing,
    Paused,
}

/// Player controls the UI sends to the render thread.
#[derive(Clone, Copy, Debug)]
pub enum SongCtl {
    /// Play, or pause if playing.
    Toggle,
    Stop,
    /// Global timeline seconds.
    Seek(f64),
}

/// Everything the UI and render thread share about the timeline.
pub struct TimelineState {
    pub doc: Option<Timeline>,
    pub mode: PlayMode,
    /// Playhead in global timeline seconds (player clock or live-match estimate).
    pub pos_s: f64,
    /// Where the edit cursor / strip click sits — used by "add cue".
    pub cursor_s: f64,
    /// Recording: actions are written into the cue list at the playhead.
    pub recording: bool,
    /// Follow-live: cross-correlate room audio onsets against the clips and
    /// fire cues at the matched position.
    pub autosync: bool,
    pub live_locked: bool,
    pub live_score: f32,
    /// Cues/clips changed since last save.
    pub dirty: bool,
    /// Snap cue placement and clip moves (¼-beat cues, ½-s clip edges).
    pub snap: bool,
    /// Status line for the UI ("loading…", "saved x.json", errors).
    pub message: String,
    /// Decoding in the background — the transport is disabled meanwhile.
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
// envelope against each clip's. Cheap enough to run on the render thread
// every half-second (a few million FLOPs at ~21 Hz subsampled resolution).
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
    /// Global timeline seconds the latest live sample maps to when locked.
    pub pos_s: f64,
    /// Clip the lock is on — kept sticky across evals so a jittery tie
    /// doesn't teleport the playhead between regions.
    locked_clip: Option<usize>,
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
            locked_clip: None,
            coasted_at: Instant::now(),
            next_eval: Instant::now(),
        }
    }

    /// A full correlation is expensive — run at most twice a second.
    pub fn due(&self) -> bool {
        Instant::now() >= self.next_eval
    }

    /// Advance the locked estimate by wall-clock — cheap, call every frame.
    /// Returns the live-estimated global position while locked.
    pub fn coast(&mut self) -> Option<f64> {
        let now = Instant::now();
        if self.locked {
            self.pos_s += now.duration_since(self.coasted_at).as_secs_f64();
        }
        self.coasted_at = now;
        self.locked.then_some(self.pos_s)
    }

    /// Run a correlation pass — call only when `due()`. `live_env`/`live_fps`
    /// come from the analyser tap. Returns the global position while locked.
    pub fn update(&mut self, live_env: &[f32], live_fps: f64, doc: &Timeline) -> Option<f64> {
        self.next_eval = Instant::now() + Duration::from_millis(500);

        let window_hops = (WINDOW_S * live_fps) as usize;
        let min_hops = (MIN_WINDOW_S * live_fps) as usize;
        if live_env.len() < min_hops {
            self.locked = false;
            self.locked_clip = None;
            self.score = 0.0;
            return None;
        }
        let n_live = live_env.len().min(window_hops);
        let live = &live_env[live_env.len() - n_live..];
        let lw = n_live / SUB;
        if lw < 16 {
            self.locked = false;
            return None;
        }
        let l_sub: Vec<f32> = (0..lw).map(|i| live[i * SUB]).collect();
        let l_mean = l_sub.iter().sum::<f32>() / lw as f32;
        let l_norm = l_sub
            .iter()
            .map(|v| (v - l_mean).powi(2))
            .sum::<f32>()
            .sqrt()
            .max(1e-9);

        // Correlate every clip; keep the best. A locked clip only needs to
        // stay above the unlock floor — best-overall can switch the lock.
        let mut best: Option<(usize, f64, f32)> = None; // (clip, local pos s, score)
        for (ci, clip) in doc.clips.iter().enumerate() {
            if clip.onsets.is_empty() || clip.onset_fps <= 0.0 {
                continue;
            }
            if let Some((pos_hops, score)) = correlate(&l_sub, l_mean, l_norm, clip, live_fps) {
                if best.is_none_or(|b| score > b.2) {
                    best = Some((ci, pos_hops / live_fps, score));
                }
            }
        }
        let Some((ci, local_s, score)) = best else {
            self.locked = false;
            self.locked_clip = None;
            self.score = 0.0;
            return None;
        };

        self.score = score;
        if self.locked {
            if score < UNLOCK_SCORE {
                self.locked = false;
                self.locked_clip = None;
            } else if self.locked_clip.is_some_and(|c| c != ci) {
                // Another clip matched better — only switch if it's clearly better.
                self.locked_clip = Some(ci);
            }
        } else if score >= LOCK_SCORE {
            self.locked = true;
            self.locked_clip = Some(ci);
        }
        if self.locked {
            self.pos_s = doc.clips[ci].offset_s + local_s;
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
        self.locked_clip = None;
    }
}

/// z-normalised cross-correlation of the live window against one clip's
/// onset envelope. Returns (clip-local position in hops at live rate, score).
fn correlate(
    l_sub: &[f32],
    l_mean: f32,
    l_norm: f32,
    clip: &Clip,
    live_fps: f64,
) -> Option<(f64, f32)> {
    let lw = l_sub.len();
    let scale = clip.onset_fps / live_fps;
    let n_song = (clip.onsets.len() as f64 / scale) as usize;
    if n_song <= lw * SUB + SUB {
        return None;
    }
    let sw = n_song / SUB;
    let song_at = |i: usize| -> f32 {
        let x = i as f64 * scale;
        let j = x as usize;
        let f = (x - j as f64) as f32;
        clip.onsets.get(j).copied().unwrap_or(0.0) * (1.0 - f)
            + clip.onsets.get(j + 1).copied().unwrap_or(0.0) * f
    };

    let s_sub: Vec<f32> = (0..sw + 1).map(|i| song_at(i * SUB)).collect();
    let s_mean = s_sub.iter().take(sw).sum::<f32>() / sw as f32;
    // Prefix sums of (s - mean) and (s - mean)² for per-lag window norms.
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
        return None;
    }

    // The last live sample ("now") sits at song hop lag + (lw-1)*SUB.
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
    Some((pos_hops.max(0) as f64, score))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(offset: f64) -> Clip {
        Clip {
            song: PathBuf::from("x.wav"),
            name: "x".into(),
            offset_s: offset,
            bpm: 120.0,
            first_beat: 0.5,
            duration_s: 120.0,
            onsets: Vec::new(),
            onset_fps: 0.0,
            overview: Vec::new(),
        }
    }

    fn doc() -> Timeline {
        Timeline {
            clips: vec![clip(0.0), clip(130.0)],
            cues: vec![
                Cue {
                    clip: 0,
                    beat: 8.0,
                    beats: 4.0,
                    kind: CueKind::NextScene,
                },
                Cue {
                    clip: 0,
                    beat: 4.0,
                    beats: 4.0,
                    kind: CueKind::Blackout(true),
                },
                // Same local beat in clip 1 lands 130 s later.
                Cue {
                    clip: 1,
                    beat: 0.0,
                    beats: 4.0,
                    kind: CueKind::Dancer(true),
                },
            ],
            ..doc_default("test")
        }
    }

    fn doc_default(name: &str) -> Timeline {
        Timeline {
            name: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn clip_beat_time_and_global_windows() {
        let mut d = doc();
        d.sort_cues();
        let c0 = &d.clips[0];
        assert_eq!(c0.beat_at(0.5), 0.0);
        assert_eq!(c0.beat_at(1.0), 1.0);
        assert_eq!(c0.time_at(4.0), 2.5);
        assert_eq!(c0.total_beats(), 239.0);
        assert_eq!(d.end_s(), 250.0);
        // clip 1 starts at 130 s — clip_at picks the right region.
        assert_eq!(d.clip_at(5.0).unwrap().0, 0);
        assert_eq!(d.clip_at(135.0).unwrap().0, 1);
        assert!(d.clip_at(125.0).is_none()); // the gap between the clips
        // Global window: clip-0 cues at t 2.5 and 4.5; clip-1 cue at 130.5.
        assert_eq!(d.cues_between_s(0.0, 5.0).len(), 2);
        assert_eq!(d.cues_between_s(2.5, 4.5).len(), 1);
        assert_eq!(d.cues_between_s(130.0, 131.0).len(), 1);
        assert_eq!(d.cues_between_s(5.0, 130.0).len(), 0);
    }

    #[test]
    fn end_kinds_release_to_baseline() {
        // Durational kinds release at the block's end…
        assert_eq!(
            CueKind::Blackout(true).end_kind(),
            Some(CueKind::Blackout(false))
        );
        // …state kinds latch — nothing fires at the block end; they stay
        // until an explicit off/auto cue.
        assert_eq!(CueKind::Dancer(true).end_kind(), None);
        assert_eq!(CueKind::Clip("x".into()).end_kind(), None);
        assert_eq!(CueKind::Canon(Tristate::On).end_kind(), None);
        assert_eq!(CueKind::Mode(Mode::Manual).end_kind(), None);
        assert_eq!(CueKind::Mode(Mode::Auto).end_kind(), None);
        assert_eq!(CueKind::Look(Some(2)).end_kind(), None);
        assert_eq!(CueKind::Fx(Fx::Quad).end_kind(), None);
        assert_eq!(CueKind::FxAuto(true).end_kind(), None);
        assert_eq!(CueKind::Scene("x".into()).end_kind(), None);
        // Palette cues latch too — no revert at the block end.
        assert_eq!(CueKind::Palette("fire".into()).end_kind(), None);
        // Tracks stay fixed per kind.
        assert_eq!(CueKind::Scene("x".into()).track(), 0);
        assert_eq!(CueKind::Canon(Tristate::On).track(), 1);
        assert_eq!(CueKind::FxAuto(true).track(), 2);
        assert_eq!(CueKind::Blackout(true).track(), 3);
        assert_eq!(CueKind::Palette("fire".into()).track(), 3);
    }

    #[test]
    fn state_at_folds_edges_to_baseline() {
        let mut d = doc();
        d.sort_cues();
        // t=0: untouched — the baseline is everything off and Manual.
        let st = d.state_at(0.0);
        assert_eq!(st.mode, Mode::Manual);
        assert!(!st.dancer && !st.blackout && st.fx == Fx::Off && !st.fx_auto);
        // Beat 4 = 2s: the blackout block is live…
        assert!(d.state_at(3.0).blackout);
        // …ends at beat 8 (4s), but NextScene also landed at beat 8.
        let st = d.state_at(5.0);
        assert!(!st.blackout && st.scene_steps == 1);
        // Clip 1's Dancer(true) starts at 130s and LATCHES — its 4-beat
        // block ending must not switch the dancer back off.
        let st = d.state_at(200.0);
        assert!(st.dancer);
        assert_eq!(st.scene_steps, 1);
    }

    #[test]
    fn palette_cues_latch_through_state_at() {
        let mut d = doc();
        d.cues.push(Cue {
            clip: 0,
            beat: 12.0, // 120 BPM + first_beat 0.5 → global t = 6.5 s
            beats: 4.0,
            kind: CueKind::Palette("fire".into()),
        });
        d.sort_cues();
        // Before the cue: no opinion — the user's live pick stays.
        assert_eq!(d.state_at(0.0).palette, None);
        assert_eq!(d.state_at(6.0).palette, None);
        // After it: latched, and still latched well past the block end.
        assert_eq!(d.state_at(7.0).palette.as_deref(), Some("fire"));
        assert_eq!(d.state_at(200.0).palette.as_deref(), Some("fire"));
        // And it survives JSON.
        let dir = std::env::temp_dir().join("trippin_tl_pal");
        let path = d.save(&dir).unwrap();
        let back = Timeline::load(&path).unwrap();
        assert!(
            back.cues
                .iter()
                .any(|c| c.kind == CueKind::Palette("fire".into()))
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn remove_clip_reindexes_cues() {
        let mut d = doc();
        d.remove_clip(0);
        assert_eq!(d.clips.len(), 1);
        assert_eq!(d.cues.len(), 1);
        assert_eq!(d.cues[0].clip, 0);
        assert_eq!(d.cues[0].kind, CueKind::Dancer(true));
    }

    #[test]
    fn timeline_json_roundtrip() {
        let mut d = doc();
        d.cues.clear();
        d.cues.push(Cue {
            clip: 0,
            beat: 0.0,
            beats: 4.0,
            kind: CueKind::Scene("clouds".into()),
        });
        d.cues.push(Cue {
            clip: 0,
            beat: 16.5,
            beats: 4.0,
            kind: CueKind::Fx(Fx::Kaleido6),
        });
        d.cues.push(Cue {
            clip: 1,
            beat: 4.0,
            beats: 4.0,
            kind: CueKind::Look(Some(2)),
        });
        d.cues.push(Cue {
            clip: 1,
            beat: 5.0,
            beats: 4.0,
            kind: CueKind::Canon(Tristate::On),
        });
        d.sort_cues();
        let dir = std::env::temp_dir().join("trippin_tl_test");
        let path = d.save(&dir).unwrap();
        let back = Timeline::load(&path).unwrap();
        assert_eq!(back.clips.len(), 2);
        assert_eq!(back.cues.len(), 4);
        assert_eq!(back.cues[0].kind, CueKind::Scene("clouds".into()));
        assert_eq!(back.cues[1].kind, CueKind::Fx(Fx::Kaleido6));
        // Clip-1 cues sort by global time (offset 130 + local) — after clip-0's.
        assert_eq!(back.cues[2].clip, 1);
        assert_eq!(back.cues[2].kind, CueKind::Look(Some(2)));
        assert_eq!(back.cues[3].kind, CueKind::Canon(Tristate::On));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn v1_doc_migrates_to_clips() {
        // The single-song shape written before clip regions existed —
        // cue JSON built from the real serializer so the format is exact.
        let legacy = serde_json::json!({
            "name": "old set",
            "song": "a.wav",
            "bpm": 128.0,
            "first_beat": 0.25,
            "duration": 200.0,
            "cues": [{"beat": 8.0, "kind": CueKind::Blackout(true)}],
            "onsets": [0.1, 0.2],
            "onset_fps": 86.0,
            "overview": [0.5]
        })
        .to_string();
        let dir = std::env::temp_dir().join("trippin_tl_v1");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("old.json");
        std::fs::write(&p, legacy).unwrap();
        let d = Timeline::load(&p).unwrap();
        assert_eq!(d.name, "old set");
        assert_eq!(d.clips.len(), 1);
        assert_eq!(d.clips[0].offset_s, 0.0);
        assert_eq!(d.clips[0].bpm, 128.0);
        assert_eq!(d.clips[0].onsets.len(), 2);
        assert_eq!(d.cues.len(), 1);
        assert_eq!(d.cues[0].clip, 0);
        assert_eq!(d.cues[0].beat, 8.0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
