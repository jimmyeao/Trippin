//! Per-machine GPU baseline (`<data dir>/perf.json`), written by the
//! Settings "performance check". The render loop steps through every
//! `@heavy` scene live and measures real frame ms, and `unity_*` shows are
//! timed by the engine's frame counter instead (the scene itself is a
//! blit; the cost is inside Unity). Anything under 30 fps-equivalent is
//! deselected — landed in `Settings.disabled_scenes`, so the user can
//! always re-enable it by hand — and flagged on the picker tile.
//!
//! `ms` is what wgpu scenes cost here; `fps` is what the Unity engine
//! managed for `unity_*` shows. Scenes measured but never stored keep
//! their user setting.

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Slower than this and the scene leaves rotation (30 fps).
pub const MAX_MS: f64 = 1000.0 / 30.0;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Report {
    /// Unix seconds when the check finished (displayed in Settings).
    pub checked: u64,
    /// Render size the numbers were measured at, so stale results are
    /// recognisable after a resolution change.
    pub size: (u32, u32),
    /// `@heavy` scenes: mean ms per rendered frame.
    pub ms: BTreeMap<String, f64>,
    /// `unity_*` scenes: engine-produced frames/sec.
    pub fps: BTreeMap<String, f64>,
}

impl Report {
    pub fn path() -> PathBuf {
        crate::config::data_dir().join("perf.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::path(), s);
        }
    }

    /// Scenes measured below 30 fps-equivalent, for the picker's "slow"
    /// flag and the auto-deselect.
    pub fn slow(&self) -> Vec<String> {
        self.ms
            .iter()
            .filter(|(_, ms)| **ms > MAX_MS)
            .map(|(n, _)| n.clone())
            .chain(
                self.fps
                    .iter()
                    // A show that never got frames (engine wouldn't boot)
                    // counts as failed too — 0 fps.
                    .filter(|(_, f)| **f < 30.0)
                    .map(|(n, _)| n.clone()),
            )
            .collect()
    }
}

/// Frames skipped after a cut before sampling (pipeline warm-up, the
/// fade-in, and vsync queue refill).
const WARM: u32 = 30;
/// wgpu scenes: samples per scene — ~1 s at 60 fps.
const SAMPLES: usize = 60;
/// unity_* scenes: engine frames are counted over this window.
const UNITY_WINDOW: std::time::Duration = std::time::Duration::from_secs(3);
/// …after letting the engine boot the show for this long first.
const UNITY_SETTLE: std::time::Duration = std::time::Duration::from_secs(2);
/// No engine frames at all by then → the show can't run here (0 fps).
const UNITY_DEAD: std::time::Duration = std::time::Duration::from_secs(15);
/// …but once the engine has already proven dead, the rest of the unity_*
/// queue fails fast instead of each burning the full boot budget.
const UNITY_DEAD_FAST: std::time::Duration = std::time::Duration::from_secs(3);

/// The GPU baseline in progress, driven one scene at a time from the
/// render loop (it owns `dir`, `dt` and the engine's frame counter, so the
/// state lives here and the stepping lives in main.rs).
pub struct Check {
    /// Compiled scenes left to measure (Renderer indices): every @heavy
    /// plus every `unity_*` stub.
    pub queue: VecDeque<usize>,
    /// The scene under test now (None = between scenes).
    pub cur: Option<usize>,
    /// Scene ids, parallel with the renderer list — progress + report keys.
    pub names: Vec<String>,
    /// Display titles for the progress line, same index-space as `names`.
    pub titles: Vec<String>,
    /// The user's scene before the check — restored on finish/cancel.
    pub home: usize,
    /// Warm-up frames remaining before wgpu sampling starts.
    pub warm: u32,
    /// wgpu scenes: collected per-frame ms.
    pub dts: Vec<f32>,
    /// unity scenes: (engine seq, Instant) when the measure window opened.
    pub seq0: Option<(u64, Instant)>,
    /// The engine already timed out once this check — remaining unity_*
    /// scenes get the short budget.
    pub unity_dead: bool,
    /// When `cur` was entered — the unity boot budget counts from here.
    pub entered: Instant,
    pub rep: Report,
    pub total: usize,
    pub done: usize,
}

impl Check {
    pub fn new(r: &crate::render::Renderer, home: usize) -> Self {
        let names = r.scene_names();
        let heavy = r.scene_heavy();
        let compiled: std::collections::HashSet<usize> =
            r.usable_scenes().into_iter().collect();
        let queue: VecDeque<usize> = (0..names.len())
            .filter(|&i| {
                compiled.contains(&i) && (heavy[i] || names[i].starts_with("unity_"))
            })
            .collect();
        Self {
            total: queue.len(),
            queue,
            cur: None,
            titles: r.scene_titles(),
            names,
            home,
            warm: WARM,
            dts: Vec::with_capacity(SAMPLES),
            seq0: None,
            unity_dead: false,
            entered: Instant::now(),
            rep: Report::default(),
            done: 0,
        }
    }

    /// Pop the next scene to test; resets the per-scene timers.
    pub fn advance(&mut self) -> Option<usize> {
        self.cur = self.queue.pop_front();
        if self.cur.is_some() {
            self.warm = WARM;
            self.dts.clear();
            self.seq0 = None;
            self.entered = Instant::now();
        }
        self.cur
    }

    pub fn is_unity(&self, i: usize) -> bool {
        self.names[i].starts_with("unity_")
    }

    /// Unity while the current test is a `unity_*` scene — forces the
    /// engine link on regardless of the saved `unity_link` setting.
    pub fn cur_is_unity(&self) -> bool {
        self.cur.is_some_and(|i| self.is_unity(i))
    }

    /// "(done/total) current scene" for the Settings progress line — the
    /// current scene's display title, falling back to the id.
    pub fn prog(&self) -> (u32, u32, String) {
        let cur = self
            .cur
            .map(|i| {
                self.titles
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| self.names[i].clone())
            })
            .unwrap_or_default();
        (self.done as u32, self.total as u32, cur)
    }

    /// wgpu scene: skip the warm-up, then collect per-frame ms; returns the
    /// mean once enough samples are in.
    pub fn step_wgpu(&mut self, dt_ms: f32) -> Option<f64> {
        if self.warm > 0 {
            self.warm -= 1;
            return None;
        }
        self.dts.push(dt_ms);
        (self.dts.len() >= SAMPLES).then(|| {
            self.dts.iter().map(|v| *v as f64).sum::<f64>() / self.dts.len() as f64
        })
    }

    /// `unity_*` scene: the scene shader is a blit — the real cost is
    /// inside the engine, so count produced frames (`seq`) over a window.
    /// Returns fps once measured; 0.0 if the engine never came up.
    pub fn step_unity(&mut self, seq: u64, live: bool) -> Option<f64> {
        if !live {
            let budget = if self.unity_dead {
                UNITY_DEAD_FAST
            } else {
                UNITY_DEAD
            };
            if self.entered.elapsed() > budget {
                self.unity_dead = true;
                return Some(0.0);
            }
            return None;
        }
        match self.seq0 {
            Some((s0, t0)) => {
                let el = t0.elapsed();
                (el >= UNITY_WINDOW)
                    .then(|| seq.saturating_sub(s0) as f64 / el.as_secs_f64())
            }
            None => {
                if self.entered.elapsed() >= UNITY_SETTLE {
                    self.seq0 = Some((seq, Instant::now()));
                }
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_marks_only_under_threshold() {
        let mut rep = Report::default();
        rep.ms.insert("cheap".into(), 8.0);
        rep.ms.insert("heavy".into(), 40.0); // >33.3 ms = under 30 fps
        rep.fps.insert("unity_ok".into(), 55.0);
        rep.fps.insert("unity_slow".into(), 22.0);
        rep.fps.insert("unity_dead".into(), 0.0); // engine never produced frames
        let mut slow = rep.slow();
        slow.sort();
        assert_eq!(slow, ["heavy", "unity_dead", "unity_slow"]);
    }

    #[test]
    fn report_round_trips() {
        let mut rep = Report::default();
        rep.checked = 1_700_000_000;
        rep.size = (1920, 1080);
        rep.ms.insert("crystal_cave".into(), 41.2);
        rep.fps.insert("unity_stage".into(), 58.0);
        let json = serde_json::to_string(&rep).unwrap();
        let back: Report = serde_json::from_str(&json).unwrap();
        assert_eq!(back.checked, 1_700_000_000);
        assert_eq!(back.size, (1920, 1080));
        assert_eq!(back.ms["crystal_cave"], 41.2);
        assert_eq!(back.fps["unity_stage"], 58.0);
    }
}
