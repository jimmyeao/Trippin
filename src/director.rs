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
}

pub struct Director {
    pub scene: usize,
    pub scene_started: Instant,
    pub hue: f32,
    pub seed: f32,
    pub flash: f32,
    pub intensity: f32,
    bars_in_scene: u32,
    last_bar: i64,
    /// Downbeat slot the bar count is aligned to — a manual re-mark shifts
    /// the bar grid and must not count as a new bar.
    last_downbeat: u64,
    /// Lowest energy seen over the last few bars (a breakdown).
    recent_low: f32,
    rng: u64,
    pending_cut: bool,
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
            bars_in_scene: 0,
            last_bar: i64::MIN,
            last_downbeat: u64::MAX,
            recent_low: 1.0,
            rng,
            pending_cut: false,
        }
    }

    pub fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn cut_to(&mut self, scene: usize) {
        self.scene = scene;
        self.scene_started = Instant::now();
        self.bars_in_scene = 0;
        self.hue = (self.hue + 0.2 + self.rand() * 0.5).fract();
        self.seed = self.rand() * 100.0;
        self.flash = 1.0;
        self.pending_cut = true;
    }

    /// Next scene among `usable`: random (never the current one) or in order.
    pub fn next_scene(&mut self, usable: &[usize], random: bool) {
        if usable.is_empty() {
            return;
        }
        let pick = if random && usable.len() > 1 {
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
        };
        self.flash = (self.flash - dt * 2.5).max(0.0);
        let target = if f.silent {
            0.15
        } else {
            (f.energy * 0.8 + f.build.max(0.0) * 0.4).min(1.0)
        };
        self.intensity += (target - self.intensity) * (dt * 2.0).min(1.0);

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

        // A drop: energy on this downbeat is well above the recent breakdown.
        let drop = s.cut_on_drops
            && f.energy - self.recent_low > 0.35
            && f.energy > 0.55
            && self.bars_in_scene >= 2;
        let phrase_end = self.bars_in_scene >= s.phrase_bars.max(1);
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
