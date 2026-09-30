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
}
