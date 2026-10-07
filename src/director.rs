//! Auto-pilot: decides when to cut between scenes (every phrase, or early on a
//! drop), in the order the settings ask for, and shapes the overall intensity.

use std::time::Instant;

use crate::audio::Features;
use crate::config::{Mode, Settings};

/// What happened on this frame, for layers that follow the phrasing.
#[derive(Default, Clone, Copy)]
pub struct Events {
    /// The scene changed (by auto-pilot or by hand).
    pub cut: bool,
    /// A phrase ended without a cut (static mode).
    pub phrase: bool,
    /// The track moved between beat mode and a breakdown (either way).
    pub mode_change: bool,
    /// A drop landed (drums back after a breakdown, or a big jump in energy
    /// off a low). Raised whatever the cut settings are, for auto clips.
    pub drop: bool,
}

pub struct Director {
    pub scene: usize,
    pub scene_started: Instant,
    pub hue: f32,
    pub seed: f32,
    pub flash: f32,
    pub intensity: f32,
    /// Breakdown state 0..1 (from the audio groove detector), as the
    /// director last saw it — scales cut flashes and the camera clock.
    pub calm: f32,
    /// Hysteretic beats/breakdown flag for mode-change events.
    in_breakdown: bool,
    bars_in_scene: u32,
    last_bar: i64,
    /// Downbeat slot the bar count is aligned to — a manual re-mark shifts
    /// the bar grid and must not count as a new bar.
    last_downbeat: u64,
    /// Lowest energy seen over the last few bars (a breakdown).
    recent_low: f32,
    // --- Phrase grid -----------------------------------------------------
    // Phrases are a musical grid, not a count of bars since the last cut:
    // `phrase_at` is the beat position of a phrase's first bar line, and
    // boundaries fall every `phrase_bars` bars from it. The grid coasts
    // through breakdowns with the beat, ignores off-grid event cuts, and
    // moves only on real evidence of a new phrase: a drop after a
    // breakdown, music after silence, or a manual phrase tap.
    /// Beat position of a phrase start (NaN until the first bar line).
    phrase_at: f64,
    /// Bar index the current scene started in.
    scene_start_bar: i64,
    /// Re-anchor the grid to the beat nearest the next frame (a tap).
    mark_pending: bool,
    /// Re-anchor the grid on the next bar line (music back after silence).
    reanchor_pending: bool,
    /// Beat position silence began (MAX while not silent).
    silent_since: f64,
    /// The grid was set by a drop or a tap (not just the first bar line):
    /// a later drop may then only move it by whole 4-bar blocks.
    phrase_confirmed: bool,
    rng: u64,
    pending_cut: bool,
    /// The scene the next cut will land on — picked when the current scene
    /// starts (every mode except Manual), or set by a "play next" click.
    /// Consumed by the cut; re-picked afterwards.
    pub next: Option<usize>,
    /// `next` came from the operator's "play next" — the mood fitter must
    /// not overwrite a human's pick.
    pub next_queued: bool,
    /// The scenes that played before this one, newest last. Random picks
    /// skip them so a small Style pool doesn't land the same scene over
    /// and over (Party/Pop showed one scene 6 times in 3 minutes).
    played: std::collections::VecDeque<usize>,
    /// Beat pos of the last cut the director made (any kind). Event cuts
    /// need ≥4 beats of clear air after it — a busy run of musical events
    /// must not strobe the scene list.
    last_cut_pos: f64,
    /// Beat position of the last drop flagged for the clip recorder.
    last_drop_pos: f64,
    /// Beat position where the current breakdown was entered (MAX: none seen).
    breakdown_enter_pos: f64,
    /// Beat position of the last silent frame: music starting after silence
    /// (the first track, or the next one after a gap) isn't a drop.
    last_silent_pos: f64,
    // --- Drum-fill detection ---------------------------------------------
    // The bar grid is a skeleton, not a cage: a fill is a burst of onsets
    // far above the section's baseline density. While one runs the render
    // loop strobes on its hits (except tunnels — jerky there), and when
    // it ends the next section lands — a good spot for a sub-bar cut.
    /// Onset accumulator for the beat currently being measured.
    beat_onset_acc: f32,
    beat_onset_n: u32,
    /// Beat index (`pos.floor()`) the accumulator belongs to.
    beat_idx: i64,
    /// Slow EMA of per-beat mean onset — the baseline hit density.
    onset_base: f32,
    /// True while a fill is running.
    in_fill: bool,
    /// Beat pos the fill started on — the strobe caps at one bar.
    fill_start: f64,
    /// Peak density reached during the fill, as a multiple of baseline.
    fill_peak: f32,
    /// No new fill may start before this beat pos.
    fill_cool: f64,
    /// Read by the render loop: strobe gate while a fill plays. Scene
    /// type is checked there — the director doesn't know scene names.
    pub fill_strobe: bool,
    // --- Free cuts --------------------------------------------------------
    // The bar grid is a backstop, not the score: a breakdown start, an
    // energy surge or a vocal break is a section boundary wherever it
    // lands. Each detector has a sustained-on window plus its own
    // refractory, and every event cut goes through `try_event_cut`'s gap.
    /// Fast/slow energy EMAs for the surge detector.
    energy_fast: f32,
    energy_slow: f32,
    /// Beat pos the energy jump was first seen (MAX = no candidate).
    surge_since: f64,
    /// No surge cut before this beat pos.
    surge_cool: f64,
    /// Beat pos the bass-out/mids-hot texture was first seen.
    vocal_since: f64,
    /// No vocal-break cut before this beat pos.
    vocal_cool: f64,
    /// Beat pos the current breakdown began (MAX = none pending). Its cut
    /// waits `BREAKDOWN_CONFIRM_BEATS`, so a drum drop-out before the drop
    /// gets one cut (the drop) instead of two 2-4 s apart (i9mac).
    calm_since: f64,
}

/// How long a breakdown must last before Auto cuts into it. Real breakdowns
/// run 8-32 bars; pre-drop drum gaps one or two. A bar (~1.9 s at 128 BPM)
/// lets a gap end first; the scene already eases into breakdown mode
/// (intensity, `u.calm`) without a cut.
const BREAKDOWN_CONFIRM_BEATS: f64 = 4.0;

/// How late the breakdown detector sees a drop, in beats (the groove has
/// to fill again first: ~1-2 beats). The phrase re-anchors to the bar line
/// nearest the drop position minus this.
const DROP_LAG_BEATS: f64 = 1.5;

/// A breakdown must last this long for its drop to re-anchor the phrase
/// grid. Build-ups with a snare roll can lift the groove for a moment and
/// end a "breakdown" a few bars in (D.O.D. – Set Me Free at 149 s, 4 bars
/// before the real drop); real breakdowns run 8-32 bars.
const REANCHOR_MIN_BREAKDOWN_BEATS: f64 = 24.0;

/// How many past scenes `recent` remembers.
const RECENT_MAX: usize = 8;

impl Director {
    pub fn new() -> Self {
        let rng = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1)
            | 1;
        Self {
            scene: 0,
            scene_started: Instant::now(),
            hue: 0.0,
            seed: 0.0,
            flash: 0.0,
            intensity: 0.0,
            calm: 1.0,
            in_breakdown: true,
            bars_in_scene: 0,
            last_bar: i64::MIN,
            last_downbeat: u64::MAX,
            recent_low: 1.0,
            phrase_at: f64::NAN,
            scene_start_bar: 0,
            mark_pending: false,
            reanchor_pending: false,
            silent_since: f64::MAX,
            phrase_confirmed: false,
            rng,
            pending_cut: false,
            next: None,
            next_queued: false,
            played: std::collections::VecDeque::new(),
            last_cut_pos: f64::MIN,
            last_drop_pos: f64::MIN,
            breakdown_enter_pos: f64::MAX,
            last_silent_pos: f64::MIN,
            energy_fast: 0.0,
            energy_slow: 0.5,
            surge_since: f64::MAX,
            surge_cool: 0.0,
            vocal_since: f64::MAX,
            calm_since: f64::MAX,
            vocal_cool: 0.0,
            beat_onset_acc: 0.0,
            beat_onset_n: 0,
            beat_idx: i64::MIN,
            onset_base: 0.3,
            in_fill: false,
            fill_start: 0.0,
            fill_peak: 0.0,
            fill_cool: 0.0,
            fill_strobe: false,
        }
    }

    /// (bar, bars) for the panel's "bar 2 of 16 · cut in 14": the bar
    /// counts from the phrase boundary the scene started in, so it reads
    /// the musical position (an off-grid cut doesn't restart it at 1), and
    /// `bars` runs to the boundary the next grid cut will land on.
    pub fn bars_progress(&self, phrase_bars: u32) -> (u32, u32) {
        let l = phrase_bars.max(1) as i64;
        let Some(bar0) = self.phrase_bar0(self.last_downbeat) else {
            return (1, l as u32);
        };
        let seg_start = self.scene_start_bar - (self.scene_start_bar - bar0).rem_euclid(l);
        let cut = self.next_grid_cut(bar0, l);
        ((self.last_bar - seg_start + 1).max(1) as u32, (cut - seg_start).max(1) as u32)
    }

    /// Beat position of the phrase start the grid is anchored to (NaN
    /// before the first bar line) — for `--beat-eval`.
    pub fn phrase_anchor(&self) -> f64 {
        self.phrase_at
    }

    /// Bar index of the phrase start the grid is anchored to (under the
    /// given downbeat slot); None before the first bar line.
    fn phrase_bar0(&self, downbeat: u64) -> Option<i64> {
        if !self.phrase_at.is_finite() || self.last_bar == i64::MIN {
            return None;
        }
        Some(((self.phrase_at - downbeat as f64) / 4.0).round() as i64)
    }

    /// Bars a scene must have run before a phrase boundary may cut it:
    /// half a phrase, or in a breakdown a phrase and a half (so breakdowns
    /// breathe, cutting every other boundary).
    fn min_bars(&self, l: i64) -> i64 {
        if self.calm > 0.5 {
            l + l / 2
        } else {
            (l / 2).max(2)
        }
    }

    /// The phrase boundary (bar index) the next grid cut lands on.
    fn next_grid_cut(&self, bar0: i64, l: i64) -> i64 {
        let next = self.last_bar + 1;
        let mut b = next + (bar0 - next).rem_euclid(l);
        while b - self.scene_start_bar < self.min_bars(l) {
            b += l;
        }
        b
    }

    pub fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Tap: a new segment starts here — restart the phrase/cut clock so the
    /// next auto cut lands `phrase_bars` from now. Also clears the drop
    /// baseline so a previous section's quiet patch can't skew detection.
    pub fn mark_phrase(&mut self) {
        self.bars_in_scene = 0;
        self.scene_start_bar = self.last_bar;
        self.recent_low = 1.0;
        self.mark_pending = true;
    }

    pub fn cut_to(&mut self, scene: usize) {
        if scene != self.scene {
            self.played.push_back(self.scene);
            if self.played.len() > RECENT_MAX {
                self.played.pop_front();
            }
        }
        self.scene = scene;
        self.scene_started = Instant::now();
        self.bars_in_scene = 0;
        self.scene_start_bar = self.last_bar;
        self.hue = (self.hue + 0.2 + self.rand() * 0.5).fract();
        self.seed = self.rand() * 100.0;
        // A cut in a breakdown is a soft dissolve-ish lift, not a white-out.
        self.flash = 1.0 - 0.7 * self.calm;
        self.pending_cut = true;
        // New scene → the next pick is stale; `update` re-picks it.
        self.next = None;
        self.next_queued = false;
        // A cut disarms the section detectors — nothing they were building
        // toward belongs to the new scene.
        self.in_fill = false;
        self.fill_strobe = false;
        self.surge_since = f64::MAX;
        self.vocal_since = f64::MAX;
        self.calm_since = f64::MAX;
    }

    /// The scene that would play next — random (never the current one) or
    /// in rotation order. Doesn't cut.
    pub fn pick_next(&mut self, usable: &[usize], random: bool) -> Option<usize> {
        if usable.is_empty() {
            return None;
        }
        Some(if random && usable.len() > 1 {
            let mut others: Vec<usize> = usable
                .iter()
                .copied()
                .filter(|&s| s != self.scene && !self.recent(s, usable.len()))
                .collect();
            if others.is_empty() {
                others = usable.iter().copied().filter(|&s| s != self.scene).collect();
            }
            others[(self.rand() * others.len() as f32) as usize % others.len()]
        } else {
            let i = usable
                .iter()
                .position(|&s| s == self.scene)
                .map_or(0, |i| i + 1);
            usable[i % usable.len()]
        })
    }

    /// `scene` played within the last few cuts. The window is half the pool
    /// (capped at `RECENT_MAX`), so a small pool still has picks left.
    pub fn recent(&self, scene: usize, pool: usize) -> bool {
        let n = (pool / 2).min(RECENT_MAX);
        self.played.iter().rev().take(n).any(|&p| p == scene)
    }

    /// Queue a specific scene for the next cut — the panel's "play next".
    /// A pick that's left `usable` by cut time is dropped for a fresh one
    /// (a disabled scene would be cut away again immediately anyway).
    pub fn queue_next(&mut self, scene: usize) {
        self.next = Some(scene);
        self.next_queued = true;
    }

    /// Next scene among `usable`: the queued pick if there is one, else
    /// random (never the current one) or in order.
    pub fn next_scene(&mut self, usable: &[usize], random: bool) {
        if usable.is_empty() {
            return;
        }
        let pick = match self.next.take() {
            Some(n) if usable.contains(&n) => n,
            _ => self.pick_next(usable, random).unwrap_or(self.scene),
        };
        self.cut_to(pick);
    }

    pub fn prev_scene(&mut self, usable: &[usize]) {
        if usable.is_empty() {
            return;
        }
        let n = usable.len();
        let i = usable.iter().position(|&s| s == self.scene).unwrap_or(0);
        self.cut_to(usable[(i + n - 1) % n]);
    }

    /// Call once per frame. `pos` is the beat position including latency offset.
    pub fn update(
        &mut self,
        f: &Features,
        pos: f64,
        dt: f32,
        usable: &[usize],
        s: &Settings,
    ) -> Events {
        let mut ev = Events {
            cut: std::mem::take(&mut self.pending_cut),
            phrase: false,
            mode_change: false,
            drop: false,
        };
        self.flash = (self.flash - dt * 2.5).max(0.0);
        self.calm = f.calm;
        // A phrase tap: the beat nearest the tap starts a phrase (the
        // analyser moves the bar's one there too).
        if self.mark_pending {
            self.mark_pending = false;
            self.phrase_at = pos.round();
            self.phrase_confirmed = true;
        }
        // Long silence: the next music is a new start (the first track, or
        // the next after a gap) and its first bar line opens a phrase.
        if f.silent {
            if self.silent_since == f64::MAX {
                self.silent_since = pos;
            } else if pos - self.silent_since >= 8.0 {
                self.reanchor_pending = true;
            }
        } else {
            self.silent_since = f64::MAX;
        }
        // A fresh scene gets its next pick right away so the panel can show
        // what's coming — in every mode that ever cuts by itself.
        if self.next.is_none() && s.mode != Mode::Manual {
            self.next = self.pick_next(usable, s.random_order);
            self.next_queued = false;
        }
        // Two modes. With drums, intensity follows loudness *and* how hard
        // the groove is driving; in a breakdown it's capped low and follows
        // the (pad/vocal) energy gently, so loud pads don't read as a peak.
        let beat_t = (f.energy * 0.6 + f.groove * 0.35 + f.build.max(0.0) * 0.4).min(1.0);
        let calm_t = (0.22 + f.energy * 0.35).min(0.5);
        let target = if f.silent {
            0.15
        } else {
            beat_t + (calm_t - beat_t) * f.calm
        };
        self.intensity += (target - self.intensity) * (dt * 2.0).min(1.0);

        // Beats <-> breakdown transitions (hysteresis on the smoothed calm).
        let enter = !self.in_breakdown && f.calm > 0.7;
        let leave = self.in_breakdown && f.calm < 0.3;
        let the_drop = leave && !f.silent && self.bars_in_scene >= 1;
        if enter || leave {
            self.in_breakdown = enter;
            ev.mode_change = true;
            if std::env::var_os("TRIPPIN_DROP_DEBUG").is_some() {
                eprintln!(
                    "drop: breakdown {} at beat {pos:.1} (calm {:.2}, groove {:.2})",
                    if enter { "enter" } else { "leave" },
                    f.calm,
                    f.groove
                );
            }
        }
        // No bars-in-scene requirement here (that only stops a double cut): a
        // breakdown that just cut to a new scene still has a drop worth a clip.
        // But it must be a breakdown we watched start and last two bars: the
        // director boots "in a breakdown", which isn't one.
        if enter {
            self.breakdown_enter_pos = pos;
        }
        // A silence is not a breakdown: the music coming back after one (a
        // gap between tracks, the first track of the night) must not clip.
        if f.silent {
            self.breakdown_enter_pos = f64::MAX;
            self.last_silent_pos = pos;
        }
        ev.drop = leave && !f.silent && pos - self.breakdown_enter_pos >= 8.0;
        // Set the guard here too: this fires mid-bar, and the downbeat path
        // below returns early on most frames, so it never saw this drop and
        // flagged the same one again at the next downbeat (M2, real feed).
        if ev.drop {
            self.last_drop_pos = pos;
            // A drop opens a phrase: re-anchor the grid to its bar line
            // (the detector lags the drums by a beat or two). Once a drop
            // has confirmed the grid, a later one may move it by whole
            // 4-bar blocks (phrases run 8, 16 or 32 bars), but a shift of a
            // bar or two is the detector, not the music.
            let down = f.downbeat as f64;
            let at = down + 4.0 * ((pos - DROP_LAG_BEATS - down) / 4.0).round();
            let shift = if self.phrase_at.is_finite() { (at - self.phrase_at).rem_euclid(16.0) } else { 0.0 };
            let on_blocks = shift < 0.5 || shift > 15.5;
            if pos - self.breakdown_enter_pos >= REANCHOR_MIN_BREAKDOWN_BEATS
                && (!self.phrase_confirmed || on_blocks)
            {
                self.phrase_at = at;
                self.phrase_confirmed = true;
            } else if std::env::var_os("TRIPPIN_DROP_DEBUG").is_some() {
                eprintln!("drop: phrase grid kept (bar line {at:.0} is {shift:.0} beats off it)");
            }
            if std::env::var_os("TRIPPIN_DROP_DEBUG").is_some() {
                eprintln!(
                    "drop: breakdown exit at beat {pos:.1} (entered {:.1}, calm {:.2})",
                    self.breakdown_enter_pos, f.calm
                );
            }
        }
        // Drums slamming back in after a breakdown: that's the drop.
        if the_drop && s.cut_on_drops && s.mode == Mode::Auto && !usable.is_empty() {
            self.next_scene(usable, s.random_order);
            self.flash = 1.0;
            self.pending_cut = false;
            self.last_cut_pos = pos;
            ev.cut = true;
            return ev;
        }

        // The current scene was switched off in the playlist (or failed to compile).
        if !usable.is_empty() && !usable.contains(&self.scene) && s.mode != Mode::Manual {
            self.next_scene(usable, s.random_order);
            self.last_cut_pos = pos;
        }

        self.update_events(f, pos, dt, enter, usable, s, &mut ev);
        if ev.cut {
            return ev;
        }

        self.update_fill(f, pos, usable, s, &mut ev);
        if ev.cut {
            return ev;
        }

        let bar = ((pos - f.downbeat as f64) / 4.0).floor() as i64;
        // A re-marked downbeat re-labels the bars — not a new bar. The
        // phrase start moves to the nearest bar line under the new labels,
        // and the scene keeps its age.
        if self.last_downbeat == u64::MAX {
            self.last_downbeat = f.downbeat;
        } else if f.downbeat != self.last_downbeat {
            if self.phrase_at.is_finite() {
                let d = (f.downbeat as f64 - self.phrase_at).rem_euclid(4.0);
                self.phrase_at += if d > 2.0 { d - 4.0 } else { d };
            }
            self.last_downbeat = f.downbeat;
            self.last_bar = bar;
            self.scene_start_bar = bar - self.bars_in_scene as i64;
        }
        // Only a forward bar line counts (a position nudged back across a
        // bar line must not count as a new bar).
        if bar <= self.last_bar {
            self.recent_low = self.recent_low.min(f.energy);
            return ev;
        }
        self.last_bar = bar;
        self.bars_in_scene += 1;
        let line = f.downbeat as f64 + 4.0 * bar as f64;
        if !f.silent && (!self.phrase_at.is_finite() || self.reanchor_pending) {
            self.phrase_at = line;
            self.phrase_confirmed = false;
            self.reanchor_pending = false;
        }
        if s.mode == Mode::Manual || f.silent {
            self.recent_low = f.energy;
            return ev;
        }

        // A drop: energy on this downbeat is well above the recent breakdown
        // (loudness-based fallback; the groove detector handles most drops
        // and the surge detector catches jumps that weren't breakdowns, so
        // this stays clear of anything already cut on).
        let drop = s.cut_on_drops
            && f.calm < 0.5
            && f.energy - self.recent_low > 0.35
            && f.energy > 0.55
            && self.bars_in_scene >= 2
            && pos - self.last_cut_pos >= 8.0;
        // Same jump, without the cut settings: the clip recorder wants every
        // drop. (The energy memory below only drifts up afterwards, so one
        // jump raises this on one downbeat check, not on every frame.)
        // Clips are flagged only by a breakdown exit (above). An energy-jump
        // flag here fired on ordinary section changes mid-groove: 4 of 5
        // clips on a live D.O.D. – Set Me Free run on Windows were mid-groove
        // (calm 0, energy 0.55-0.70 against a low of 0.18-0.27). Its silence
        // guard never engaged either, because a loopback "silence" isn't
        // digital zero. The jump still cuts scenes (`drop` above) when cuts
        // on drops are on.
        if std::env::var_os("TRIPPIN_DROP_DEBUG").is_some() && drop {
            eprintln!(
                "drop: energy jump at beat {pos:.1} (energy {:.2}, recent low {:.2}): cut only, no clip",
                f.energy, self.recent_low
            );
        }
        // Phrase boundaries on the grid; a scene must have run long enough
        // (`min_bars`: breakdowns breathe, skipping every other one).
        let l = s.phrase_bars.max(1) as i64;
        let boundary = self.phrase_bar0(f.downbeat).is_some_and(|b0| (bar - b0).rem_euclid(l) == 0);
        let phrase_end = boundary
            && (s.mode != Mode::Auto || self.bars_in_scene as i64 >= self.min_bars(l));
        if drop || phrase_end {
            if s.mode == Mode::Auto {
                self.next_scene(usable, s.random_order);
                ev.cut = true;
                self.pending_cut = false;
                self.last_cut_pos = pos;
            } else {
                self.bars_in_scene = 0;
                self.scene_start_bar = bar;
                ev.phrase = true;
            }
        }
        // Let the breakdown memory drift back up so old lows don't linger.
        self.recent_low = (self.recent_low + 0.1).min(f.energy.max(self.recent_low));
        ev
    }

    /// Drum fills, evaluated every frame (not just on bar lines): the mean
    /// onset level per beat is compared to a slow baseline EMA. A beat
    /// running far hotter starts a fill — the render loop strobes it —
    /// and when the burst collapses the new section lands: a cut there is
    /// musical even mid-bar. Big fills can also cut mid-phrase.
    fn update_fill(
        &mut self,
        f: &Features,
        pos: f64,
        usable: &[usize],
        s: &Settings,
        ev: &mut Events,
    ) {
        let beat_idx = pos.floor() as i64;
        if self.beat_idx == i64::MIN {
            self.beat_idx = beat_idx;
        }
        if beat_idx > self.beat_idx {
            // The beat closed: judge its density, then reset the window.
            let mean = self.beat_onset_acc / self.beat_onset_n.max(1) as f32;
            if self.in_fill {
                // The burst collapsing means the new section has landed.
                if mean < self.onset_base * 1.4 {
                    self.end_fill(f, pos, usable, s, ev);
                }
            } else if !f.silent && f.calm < 0.5 {
                // Baseline only learns from ordinary beats.
                self.onset_base =
                    (self.onset_base * 0.92 + mean * 0.08).clamp(0.05, 0.8);
            }
            self.beat_idx = beat_idx;
            self.beat_onset_acc = 0.0;
            self.beat_onset_n = 0;
        }
        self.beat_onset_acc += f.onset;
        self.beat_onset_n += 1;

        let run = self.beat_onset_acc / self.beat_onset_n.max(1) as f32;
        if !self.in_fill {
            // Mid-beat trigger once the running mean is worth trusting.
            if !f.silent
                && f.calm < 0.5
                && pos > self.fill_cool
                && pos.fract() > 0.35
                && run > 0.3
                && run > self.onset_base * 2.2
            {
                self.in_fill = true;
                self.fill_start = pos - pos.fract();
                self.fill_peak = run / self.onset_base.max(0.05);
                // Strobes ride the fill in every mode except Manual —
                // there the operator runs the rig.
                self.fill_strobe = s.mode != Mode::Manual;
            }
        } else {
            self.fill_peak = self.fill_peak.max(run / self.onset_base.max(0.05));
            // Hard stops: a full bar of density is the new section, not a
            // fill; a breakdown or silence kills it instantly.
            if pos - self.fill_start >= 4.0 || f.calm > 0.6 || f.silent {
                self.end_fill(f, pos, usable, s, ev);
            }
        }
    }

    fn end_fill(
        &mut self,
        f: &Features,
        pos: f64,
        usable: &[usize],
        s: &Settings,
        ev: &mut Events,
    ) {
        self.in_fill = false;
        self.fill_strobe = false;
        self.fill_cool = pos + 8.0;
        if f.calm > 0.6 || f.silent {
            return;
        }
        // A fill into the phrase boundary always cuts — that IS the
        // section change (the fill ends in the phrase's last bar, or just
        // past the boundary). A really big fill can cut mid-phrase too.
        let l = s.phrase_bars.max(1) as i64;
        let into_boundary = self.phrase_bar0(f.downbeat).is_some_and(|b0| {
            let bar = ((pos - f.downbeat as f64) / 4.0).floor() as i64;
            let k = (bar - b0).rem_euclid(l);
            let into_bar = (pos - f.downbeat as f64).rem_euclid(4.0);
            k == l - 1 || (k == 0 && into_bar < 1.5)
        });
        let due = into_boundary || self.fill_peak > 3.0;
        if s.mode == Mode::Auto
            && s.cut_on_drops
            && due
            && self.bars_in_scene >= 2
            && !usable.is_empty()
            && !ev.cut
            && !self.pending_cut
        {
            self.next_scene(usable, s.random_order);
            self.pending_cut = false;
            self.last_cut_pos = pos;
            ev.cut = true;
        }
    }

    /// Section changes off the grid: breakdown entry, energy surges and
    /// vocal breaks. Every trigger routes through `try_event_cut`, which
    /// enforces Auto mode, `cut_on_drops` and the one-bar/four-beat gap —
    /// a detector arms freely but only fires through the gap.
    fn update_events(
        &mut self,
        f: &Features,
        pos: f64,
        dt: f32,
        entered_calm: bool,
        usable: &[usize],
        s: &Settings,
        ev: &mut Events,
    ) {
        if ev.cut {
            // Another path already cut this frame — the armed candidates
            // belong to the old scene.
            self.surge_since = f64::MAX;
            self.vocal_since = f64::MAX;
            return;
        }
        // The groove leaving — a breakdown or a vocal break — is a
        // boundary once it has lasted a bar; a shorter gap is the run-up to
        // a drop, which cuts on its own.
        if entered_calm && !f.silent {
            self.calm_since = pos;
        }
        if !self.in_breakdown || f.silent {
            self.calm_since = f64::MAX;
        } else if pos - self.calm_since >= BREAKDOWN_CONFIRM_BEATS {
            self.calm_since = f64::MAX;
            self.try_event_cut(usable, s, pos, ev);
        }

        // Energy surge: the fast EMA jumping well past the slow one is a
        // bigger section landing while the drums play (a chorus, or a
        // second drop with no breakdown). ~1.5 beats of sustained jump,
        // then a 24-beat refractory so a long loud section can't re-fire.
        let kf = (dt * 3.0).min(1.0);
        let ks = (dt * 0.12).min(1.0);
        self.energy_fast += (f.energy - self.energy_fast) * kf;
        self.energy_slow += (f.energy - self.energy_slow) * ks;
        let surging = !f.silent
            && f.calm < 0.5
            && self.energy_fast > 0.55
            && self.energy_fast > self.energy_slow * 1.5 + 0.1;
        if surging {
            if self.surge_since == f64::MAX {
                self.surge_since = pos;
            } else if pos - self.surge_since >= 1.5 && pos > self.surge_cool {
                self.try_event_cut(usable, s, pos, ev);
                if ev.cut {
                    self.surge_cool = pos + 24.0;
                }
                self.surge_since = f64::MAX;
            }
        } else {
            self.surge_since = f64::MAX;
        }

        // Vocal-break proxy: the groove's still on (calm low) but the bass
        // has thinned out while the mids stay hot — a verse dropping to
        // voice over air. Five beats of that is a section, not a breath.
        let vocalish = !f.silent
            && f.calm < 0.7
            && f.lvl4[0] < 0.28
            && f.lvl4[1] + f.lvl4[2] > 0.6;
        if vocalish {
            if self.vocal_since == f64::MAX {
                self.vocal_since = pos;
            } else if pos - self.vocal_since >= 5.0 && pos > self.vocal_cool {
                self.try_event_cut(usable, s, pos, ev);
                if ev.cut {
                    self.vocal_cool = pos + 24.0;
                }
                self.vocal_since = f64::MAX;
            }
        } else {
            self.vocal_since = f64::MAX;
        }

        if ev.cut {
            self.surge_since = f64::MAX;
            self.vocal_since = f64::MAX;
        }
    }

    /// The gap every off-grid cut must clear: a full bar into the scene
    /// and four beats since the last one.
    fn cut_ok(&self, pos: f64) -> bool {
        self.bars_in_scene >= 1 && pos - self.last_cut_pos >= 4.0
    }

    /// Pick + cut + bookkeeping, for every cut the director initiates.
    fn cut(&mut self, usable: &[usize], s: &Settings, pos: f64, ev: &mut Events) {
        self.next_scene(usable, s.random_order);
        self.pending_cut = false;
        self.last_cut_pos = pos;
        ev.cut = true;
    }

    fn try_event_cut(&mut self, usable: &[usize], s: &Settings, pos: f64, ev: &mut Events) {
        if s.mode == Mode::Auto
            && s.cut_on_drops
            && !usable.is_empty()
            && !ev.cut
            && !self.pending_cut
            && self.cut_ok(pos)
        {
            self.cut(usable, s, pos, ev);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::Features;

    #[test]
    fn random_picks_skip_recent_scenes() {
        let mut d = Director::new();
        let pool: Vec<usize> = (0..10).collect();
        // Over many cuts no scene comes back within the last 5 (half the pool).
        let mut last: Vec<usize> = vec![d.scene];
        for _ in 0..200 {
            let n = d.pick_next(&pool, true).unwrap();
            assert!(!last.iter().rev().take(5).any(|&p| p == n), "{n} repeated within 5: {last:?}");
            d.cut_to(n);
            last.push(n);
        }
        // A pool of two still alternates (the window shrinks with the pool).
        let mut d = Director::new();
        for _ in 0..10 {
            let n = d.pick_next(&[0, 1], true).unwrap();
            assert_ne!(n, d.scene);
            d.cut_to(n);
        }
    }

    fn settings(mode: Mode) -> Settings {
        Settings {
            mode,
            cut_on_drops: true,
            phrase_bars: 4,
            ..Default::default()
        }
    }

    fn features(onset: f32) -> Features {
        let mut f = Features::default();
        f.silent = false;
        f.calm = 0.0;
        f.onset = onset;
        f.bpm = 126.0;
        f.downbeat = 0;
        f
    }

    /// Advance `pos` by `beats` at 30 frames a beat, counting cuts.
    fn run(d: &mut Director, f: &Features, pos: &mut f64, beats: f64, s: &Settings) -> usize {
        let usable = [0usize, 1, 2, 3];
        let mut cuts = 0;
        for _ in 0..(beats * 30.0) as usize {
            *pos += 1.0 / 30.0;
            if d.update(f, *pos, 0.016, &usable, s).cut {
                cuts += 1;
            }
        }
        cuts
    }

    /// Count `Events::drop` over `beats` beats (30 frames a beat).
    fn drops(d: &mut Director, f: &Features, pos: &mut f64, beats: f64, s: &Settings) -> usize {
        let usable = [0usize, 1, 2, 3];
        let mut n = 0;
        for _ in 0..(beats * 30.0) as usize {
            *pos += 1.0 / 30.0;
            if d.update(f, *pos, 0.016, &usable, s).drop {
                n += 1;
            }
        }
        n
    }

    #[test]
    fn drop_flag_fires_once_when_drums_return_even_with_cuts_off() {
        // Auto clips want the drop whatever the cut settings say.
        let mut s = settings(Mode::Auto);
        s.cut_on_drops = false;
        let (mut d, mut pos) = (Director::new(), 0.0f64);
        let steady = features(0.2);
        assert_eq!(drops(&mut d, &steady, &mut pos, 16.0, &s), 0, "steady beats are no drop");
        let mut calm = features(0.0);
        calm.calm = 1.0;
        assert_eq!(drops(&mut d, &calm, &mut pos, 16.0, &s), 0, "entering a breakdown is no drop");
        assert_eq!(drops(&mut d, &steady, &mut pos, 16.0, &s), 1, "the drums returning is exactly one drop");
    }

    #[test]
    fn music_after_silence_is_no_drop() {
        // The first track of the night, or the next one after a gap: the energy jumps
        // from silence, and the silence looked like a breakdown. Neither is a drop.
        let s = settings(Mode::Auto);
        let (mut d, mut pos) = (Director::new(), 0.0f64);
        let mut quiet = features(0.0);
        quiet.energy = 0.3;
        assert_eq!(drops(&mut d, &quiet, &mut pos, 16.0, &s), 0);
        let mut silent = features(0.0);
        silent.silent = true;
        silent.calm = 1.0;
        silent.energy = 0.0;
        assert_eq!(drops(&mut d, &silent, &mut pos, 16.0, &s), 0);
        let mut loud = features(0.4);
        loud.energy = 0.8;
        assert_eq!(drops(&mut d, &loud, &mut pos, 48.0, &s), 0, "music starting after silence");
    }

    #[test]
    fn a_drop_is_flagged_once_not_again_at_the_next_downbeat() {
        // The breakdown path fires mid-bar; the energy path checks on downbeats. Both
        // see the same drop, so it must be flagged once.
        let s = settings(Mode::Auto);
        let (mut d, mut pos) = (Director::new(), 0.0f64);
        let mut steady = features(0.3);
        steady.energy = 0.3;
        assert_eq!(drops(&mut d, &steady, &mut pos, 16.0, &s), 0);
        let mut bd = features(0.0);
        bd.calm = 1.0;
        bd.energy = 0.05;
        assert_eq!(drops(&mut d, &bd, &mut pos, 16.0, &s), 0);
        let mut back = features(0.4);
        back.energy = 0.8;
        assert_eq!(drops(&mut d, &back, &mut pos, 32.0, &s), 1, "one drop, not one per path");
    }

    #[test]
    fn fill_strobes_then_cuts_off_grid() {
        let s = settings(Mode::Auto);
        let mut d = Director::new();
        let mut pos = 0.0;
        // A plain groove for three bars — the baseline settles low.
        let quiet = features(0.08);
        run(&mut d, &quiet, &mut pos, 12.0, &s);
        // A fill: a dense burst through one beat. Strobe on mid-beat.
        let fill = features(0.7);
        run(&mut d, &fill, &mut pos, 0.7, &s);
        assert!(d.in_fill, "a dense beat should register as a fill");
        assert!(d.fill_strobe, "the fill should strobe");
        // The burst ends — the new section lands. Cut here, mid-bar.
        let cuts = run(&mut d, &quiet, &mut pos, 1.5, &s);
        assert!(cuts >= 1, "a strong fill should end in a cut");
        assert!(!d.fill_strobe, "strobe off once the fill is over");
    }

    #[test]
    fn fills_never_fire_in_manual() {
        let s = settings(Mode::Manual);
        let mut d = Director::new();
        let mut pos = 0.0;
        let quiet = features(0.08);
        run(&mut d, &quiet, &mut pos, 12.0, &s);
        let fill = features(0.7);
        let cuts = run(&mut d, &fill, &mut pos, 2.0, &s)
            + run(&mut d, &quiet, &mut pos, 2.0, &s);
        assert_eq!(cuts, 0, "manual mode must not cut");
        assert!(!d.fill_strobe, "manual mode must not strobe by itself");
    }

    #[test]
    fn breakdown_doesnt_fill() {
        let s = settings(Mode::Auto);
        let mut d = Director::new();
        let mut pos = 0.0;
        let quiet = features(0.08);
        run(&mut d, &quiet, &mut pos, 8.0, &s);
        // Melodic hits in a breakdown are not drum fills.
        let mut calm_fill = features(0.7);
        calm_fill.calm = 0.8;
        run(&mut d, &calm_fill, &mut pos, 2.0, &s);
        assert!(!d.in_fill);
        assert!(!d.fill_strobe);
    }

    /// A steady groove — energy mid, drums on, mids/bass present.
    fn groove() -> Features {
        let mut f = features(0.05);
        f.energy = 0.4;
        f.groove = 0.8;
        f.lvl4 = [0.6, 0.4, 0.3, 0.2];
        f
    }

    /// Settings with a long grid so the phrase clock can't explain a cut.
    fn free_run() -> (Director, Settings, f64) {
        let mut s = settings(Mode::Auto);
        s.phrase_bars = 32;
        (Director::new(), s, 0.0)
    }

    #[test]
    fn breakdown_entry_cuts_off_grid() {
        let (mut d, s, mut pos) = free_run();
        run(&mut d, &groove(), &mut pos, 24.0, &s); // 6 bars, grid won't fire
        assert!(!d.in_breakdown, "drums should end the initial breakdown");
        // The groove detector commits to a breakdown mid-bar.
        let mut calm_f = groove();
        calm_f.calm = 0.8;
        calm_f.energy = 0.25;
        let cuts = run(&mut d, &calm_f, &mut pos, 3.0, &s);
        assert_eq!(cuts, 0, "a breakdown waits a bar before it cuts");
        let cuts = run(&mut d, &calm_f, &mut pos, 2.0, &s);
        assert_eq!(cuts, 1, "a breakdown that lasts is a cut point");
    }

    #[test]
    fn a_short_drum_gap_cuts_once_on_the_drop() {
        let (mut d, s, mut pos) = free_run();
        run(&mut d, &groove(), &mut pos, 24.0, &s);
        let mut gap = groove();
        gap.calm = 0.8;
        gap.energy = 0.25;
        // Two beats of drop-out, then the drums slam back.
        let cuts = run(&mut d, &gap, &mut pos, 2.0, &s);
        assert_eq!(cuts, 0, "no cut into a two-beat gap");
        let mut drop = groove();
        drop.energy = 0.9;
        let cuts = run(&mut d, &drop, &mut pos, 2.0, &s);
        assert!(cuts <= 1, "the drop cuts once at most, got {cuts}");
    }

    #[test]
    fn energy_surge_cuts_off_grid() {
        let (mut d, s, mut pos) = free_run();
        run(&mut d, &groove(), &mut pos, 24.0, &s);
        // The chorus slams in with the drums still playing — no breakdown.
        let mut hot = groove();
        hot.energy = 0.9;
        let cuts = run(&mut d, &hot, &mut pos, 4.0, &s);
        assert!(cuts >= 1, "a sustained energy jump should cut");
        // …but a long loud section can't keep re-firing it.
        let more = run(&mut d, &hot, &mut pos, 8.0, &s);
        assert_eq!(more, 0, "the surge refractory must hold");
    }

    #[test]
    fn vocal_break_cuts_off_grid() {
        let (mut d, s, mut pos) = free_run();
        run(&mut d, &groove(), &mut pos, 24.0, &s);
        // Bass out, mids still hot — voice over air while the hats ride.
        let mut vocal = groove();
        vocal.lvl4 = [0.1, 0.5, 0.4, 0.2];
        let cuts = run(&mut d, &vocal, &mut pos, 7.0, &s);
        assert!(cuts >= 1, "a sustained vocal break should cut");
    }

    #[test]
    fn event_cuts_respect_the_gap() {
        let (mut d, s, mut pos) = free_run();
        run(&mut d, &groove(), &mut pos, 24.0, &s);
        let mut calm_f = groove();
        calm_f.calm = 0.8;
        // The breakdown cuts once it has lasted a bar.
        let cuts = run(&mut d, &calm_f, &mut pos, 4.5, &s);
        assert_eq!(cuts, 1);
        // Fakeout: energy surges straight back — inside the 4-beat gap
        // nothing else may cut, however hot the detector runs. (The run
        // stays inside the gap; at +4 beats the surge could fire.)
        let mut hot = groove();
        hot.energy = 1.0;
        let cuts = run(&mut d, &hot, &mut pos, 1.5, &s);
        assert_eq!(cuts, 0, "the gap blocks back-to-back event cuts");
    }

    /// Positions of the cuts over `beats` beats (30 frames a beat).
    fn cut_log(d: &mut Director, f: &Features, pos: &mut f64, beats: f64, s: &Settings) -> Vec<f64> {
        let usable = [0usize, 1, 2, 3];
        let mut v = Vec::new();
        for _ in 0..(beats * 30.0) as usize {
            *pos += 1.0 / 30.0;
            if d.update(f, *pos, 0.016, &usable, s).cut {
                v.push(*pos);
            }
        }
        v
    }

    /// An off-grid event cut mid-phrase leaves the phrase grid alone: the
    /// next grid cut still lands on the musical boundary, and the panel's
    /// bar count keeps reading the phrase position.
    #[test]
    fn event_cuts_keep_the_phrase_grid() {
        let mut s = settings(Mode::Auto);
        s.phrase_bars = 4;
        let (mut d, mut pos) = (Director::new(), 0.0f64);
        let cuts = cut_log(&mut d, &groove(), &mut pos, 34.0, &s);
        // Grid cuts every 16 beats from the first bar line.
        assert!(cuts.iter().all(|c| (c / 16.0 - (c / 16.0).round()).abs() < 0.01), "{cuts:?}");
        // A vocal break cuts off the grid at ~beat 39-40.
        let mut vocal = groove();
        vocal.lvl4 = [0.1, 0.5, 0.4, 0.2];
        let ev = cut_log(&mut d, &vocal, &mut pos, 6.0, &s);
        assert_eq!(ev.len(), 1, "{ev:?}");
        assert!(ev[0] % 16.0 > 1.0, "the event cut should be off the grid: {ev:?}");
        let (bar, _) = d.bars_progress(4);
        assert!(bar > 1, "the bar count restarted at an off-grid cut");
        // The next cut is the boundary at beat 48, not 16 beats after the event.
        let next = cut_log(&mut d, &groove(), &mut pos, 12.0, &s);
        assert_eq!(next.len(), 1, "{next:?}");
        assert!((next[0] - 48.0).abs() < 0.05, "grid cut at {next:?}, expected 48");
    }

    /// A drop after a real breakdown opens a phrase: the grid re-anchors to
    /// the drop's bar line, and the grid cuts that follow count from there.
    #[test]
    fn a_drop_reanchors_the_phrase_grid() {
        let mut s = settings(Mode::Auto);
        s.phrase_bars = 4;
        s.cut_on_drops = true;
        let (mut d, mut pos) = (Director::new(), 0.0f64);
        cut_log(&mut d, &groove(), &mut pos, 24.0, &s);
        // A breakdown to beat 73.5: the drums came back on beat 72 (2 bars
        // off the old 16-beat grid) and the detector sees it 1.5 beats late.
        let mut bd = groove();
        bd.calm = 1.0;
        bd.energy = 0.2;
        cut_log(&mut d, &bd, &mut pos, 49.5, &s);
        let drop_bar = ((pos - 1.5) / 4.0).round() * 4.0;
        assert!((drop_bar % 16.0 - 8.0).abs() < 0.01, "test setup: drop bar {drop_bar}");
        let cuts = cut_log(&mut d, &groove(), &mut pos, 40.0, &s);
        assert!(!cuts.is_empty());
        // The drop cut, then grid cuts every 16 beats from the drop's bar.
        for c in cuts.iter().skip(1) {
            let k = (c - drop_bar) / 16.0;
            assert!((k - k.round()).abs() < 0.01, "grid cut at {c} not on the re-anchored phrase ({drop_bar}): {cuts:?}");
        }
    }

    /// Drop at `beat` + the detector lag after a breakdown from `from`.
    fn breakdown_then_drop(d: &mut Director, pos: &mut f64, s: &Settings, beat: f64) {
        let mut bd = groove();
        bd.calm = 1.0;
        bd.energy = 0.2;
        cut_log(d, &bd, pos, beat + 1.5 - *pos, s);
        cut_log(d, &groove(), pos, 2.0, s);
    }

    /// A snare roll that ends a "breakdown" four bars in is no drop for the
    /// phrase grid; and once a drop has set the grid, a later drop a bar
    /// off it leaves it alone, while one a whole 4-bar block off moves it.
    #[test]
    fn short_or_one_bar_off_drops_keep_the_grid() {
        let s = settings(Mode::Auto);
        let (mut d, mut pos) = (Director::new(), 0.0f64);
        cut_log(&mut d, &groove(), &mut pos, 32.0, &s);
        let start = d.phrase_anchor();
        breakdown_then_drop(&mut d, &mut pos, &s, 52.0); // 20 beats: too short
        assert_eq!(d.phrase_anchor(), start, "a 5-bar breakdown re-anchored");
        cut_log(&mut d, &groove(), &mut pos, 20.0, &s);
        breakdown_then_drop(&mut d, &mut pos, &s, 136.0); // 2 bars off the grid
        assert_eq!(d.phrase_anchor(), 136.0, "the first real drop sets the grid");
        cut_log(&mut d, &groove(), &mut pos, 20.0, &s);
        breakdown_then_drop(&mut d, &mut pos, &s, 204.0); // 1 bar off it
        assert_eq!(d.phrase_anchor(), 136.0, "a one-bar-off drop moved a confirmed grid");
        cut_log(&mut d, &groove(), &mut pos, 20.0, &s);
        breakdown_then_drop(&mut d, &mut pos, &s, 264.0); // 2 blocks of 4 bars on
        assert_eq!(d.phrase_anchor(), 264.0);
    }

    /// A downbeat re-label keeps the phrase where it was musically and
    /// doesn't count a bar.
    #[test]
    fn a_downbeat_change_keeps_the_phrase() {
        let mut s = settings(Mode::Auto);
        s.phrase_bars = 4;
        let (mut d, mut pos) = (Director::new(), 0.0f64);
        cut_log(&mut d, &groove(), &mut pos, 20.0, &s);
        let mut moved = groove();
        moved.downbeat = 1;
        let cuts = cut_log(&mut d, &moved, &mut pos, 30.0, &s);
        // Boundaries now at 1 mod 4: the nearest to the old 32 is 33.
        assert!(cuts.iter().any(|c| (c - 33.0).abs() < 0.05), "{cuts:?}");
    }

    #[test]
    fn events_stay_off_in_static_and_manual() {
        for mode in [Mode::Static, Mode::Manual] {
            let mut s = settings(mode);
            s.phrase_bars = 32;
            let mut d = Director::new();
            let mut pos = 0.0;
            run(&mut d, &groove(), &mut pos, 24.0, &s);
            let mut calm_f = groove();
            calm_f.calm = 0.8;
            let mut hot = calm_f.clone();
            hot.calm = 0.0;
            hot.energy = 1.0;
            let cuts = run(&mut d, &calm_f, &mut pos, 3.0, &s)
                + run(&mut d, &hot, &mut pos, 6.0, &s);
            assert_eq!(cuts, 0, "{mode:?} must not event-cut");
        }
    }
}
