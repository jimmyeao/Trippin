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
    rng: u64,
    pending_cut: bool,
    /// The scene the next cut will land on — picked when the current scene
    /// starts (every mode except Manual), or set by a "play next" click.
    /// Consumed by the cut; re-picked afterwards.
    pub next: Option<usize>,
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
}

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
            rng,
            pending_cut: false,
            next: None,
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

    /// (current bar in the scene, bars the scene will run) — for the
    /// panel's "bar 2 of 4" readout. Breakdowns double the phrase length.
    pub fn bars_progress(&self, phrase_bars: u32) -> (u32, u32) {
        let total = if self.calm > 0.5 {
            phrase_bars.max(1) * 2
        } else {
            phrase_bars.max(1)
        };
        ((self.bars_in_scene + 1).min(total), total)
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
        self.recent_low = 1.0;
    }

    pub fn cut_to(&mut self, scene: usize) {
        self.scene = scene;
        self.scene_started = Instant::now();
        self.bars_in_scene = 0;
        self.hue = (self.hue + 0.2 + self.rand() * 0.5).fract();
        self.seed = self.rand() * 100.0;
        // A cut in a breakdown is a soft dissolve-ish lift, not a white-out.
        self.flash = 1.0 - 0.7 * self.calm;
        self.pending_cut = true;
        // New scene → the next pick is stale; `update` re-picks it.
        self.next = None;
    }

    /// The scene that would play next — random (never the current one) or
    /// in rotation order. Doesn't cut.
    pub fn pick_next(&mut self, usable: &[usize], random: bool) -> Option<usize> {
        if usable.is_empty() {
            return None;
        }
        Some(if random && usable.len() > 1 {
            let others: Vec<usize> = usable
                .iter()
                .copied()
                .filter(|&s| s != self.scene)
                .collect();
            others[(self.rand() * others.len() as f32) as usize % others.len()]
        } else {
            let i = usable
                .iter()
                .position(|&s| s == self.scene)
                .map_or(0, |i| i + 1);
            usable[i % usable.len()]
        })
    }

    /// Queue a specific scene for the next cut — the panel's "play next".
    /// A pick that's left `usable` by cut time is dropped for a fresh one
    /// (a disabled scene would be cut away again immediately anyway).
    pub fn queue_next(&mut self, scene: usize) {
        self.next = Some(scene);
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
        };
        self.flash = (self.flash - dt * 2.5).max(0.0);
        self.calm = f.calm;
        // A fresh scene gets its next pick right away so the panel can show
        // what's coming — in every mode that ever cuts by itself.
        if self.next.is_none() && s.mode != Mode::Manual {
            self.next = self.pick_next(usable, s.random_order);
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
        }
        // Drums slamming back in after a breakdown: that's the drop.
        if the_drop && s.cut_on_drops && s.mode == Mode::Auto && !usable.is_empty() {
            self.next_scene(usable, s.random_order);
            self.flash = 1.0;
            self.pending_cut = false;
            ev.cut = true;
            return ev;
        }

        // The current scene was switched off in the playlist (or failed to compile).
        if !usable.is_empty() && !usable.contains(&self.scene) && s.mode != Mode::Manual {
            self.next_scene(usable, s.random_order);
        }

        self.update_fill(f, pos, usable, s, &mut ev);
        if ev.cut {
            return ev;
        }

        let bar = ((pos - f.downbeat as f64) / 4.0).floor() as i64;
        // A re-marked downbeat re-anchors the grid — not a new bar.
        if f.downbeat != self.last_downbeat {
            self.last_downbeat = f.downbeat;
            self.last_bar = bar;
        }
        if bar == self.last_bar {
            self.recent_low = self.recent_low.min(f.energy);
            return ev;
        }
        self.last_bar = bar;
        self.bars_in_scene += 1;
        if s.mode == Mode::Manual || f.silent {
            self.recent_low = f.energy;
            return ev;
        }

        // A drop: energy on this downbeat is well above the recent breakdown
        // (loudness-based fallback; the groove detector handles most drops).
        let drop = s.cut_on_drops
            && f.calm < 0.5
            && f.energy - self.recent_low > 0.35
            && f.energy > 0.55
            && self.bars_in_scene >= 2;
        // Breakdowns breathe: phrases run twice as long before a cut.
        let bars = if f.calm > 0.5 { s.phrase_bars.max(1) * 2 } else { s.phrase_bars.max(1) };
        let phrase_end = self.bars_in_scene >= bars;
        if drop || phrase_end {
            if s.mode == Mode::Auto {
                self.next_scene(usable, s.random_order);
                ev.cut = true;
                self.pending_cut = false;
            } else {
                self.bars_in_scene = 0;
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
        let bars = if f.calm > 0.5 {
            s.phrase_bars.max(1) * 2
        } else {
            s.phrase_bars.max(1)
        };
        // A fill into the phrase boundary always cuts — that IS the
        // section change. A really big fill can cut mid-phrase too.
        let due = self.bars_in_scene + 1 >= bars || self.fill_peak > 3.0;
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
            ev.cut = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::Features;

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
}
