//! User settings: key bindings, modes, scene and dancer preferences. Saved as
//! `trippin.json` in the working directory and edited from the control panel.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use winit::keyboard::Key;

/// Everything a hotkey can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Action {
    NextScene,
    PrevScene,
    ModeAuto,
    ModeStatic,
    ModeManual,
    ToggleRandom,
    ToggleDancer,
    NextClip,
    NextStyle,
    CycleCanon,
    Blackout,
    Fullscreen,
    MarkDownbeat,
    LatencyDown,
    LatencyUp,
    CycleFx,
    ReloadShaders,
    TogglePanel,
    LeaveFullscreen,
}

impl Action {
    pub const ALL: [Action; 19] = [
        Action::NextScene,
        Action::PrevScene,
        Action::ModeAuto,
        Action::ModeStatic,
        Action::ModeManual,
        Action::ToggleRandom,
        Action::ToggleDancer,
        Action::NextClip,
        Action::NextStyle,
        Action::CycleCanon,
        Action::Blackout,
        Action::Fullscreen,
        Action::MarkDownbeat,
        Action::LatencyDown,
        Action::LatencyUp,
        Action::CycleFx,
        Action::ReloadShaders,
        Action::TogglePanel,
        Action::LeaveFullscreen,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::NextScene => "Next scene",
            Action::PrevScene => "Previous scene",
            Action::ModeAuto => "Mode: auto (cuts on phrases and drops)",
            Action::ModeStatic => "Mode: static (hold the current scene)",
            Action::ModeManual => "Mode: manual (nothing changes by itself)",
            Action::ToggleRandom => "Random / sequential scene order",
            Action::ToggleDancer => "Dancer on / off",
            Action::NextClip => "Next dancer routine",
            Action::NextStyle => "Next dancer look",
            Action::CycleCanon => "Canon: auto / on / off",
            Action::Blackout => "Blackout (fade to black)",
            Action::Fullscreen => "Fullscreen on / off",
            Action::MarkDownbeat => "Mark this beat as the downbeat",
            Action::LatencyDown => "Latency -5 ms (visuals later)",
            Action::LatencyUp => "Latency +5 ms (visuals earlier)",
            Action::CycleFx => "Effect: off / mirror / kaleido",
            Action::ReloadShaders => "Reload shaders",
            Action::TogglePanel => "Show / hide this control panel",
            Action::LeaveFullscreen => "Leave fullscreen",
        }
    }

    pub fn default_key(self) -> &'static str {
        match self {
            Action::NextScene => "ArrowRight",
            Action::PrevScene => "ArrowLeft",
            Action::ModeAuto => "A",
            Action::ModeStatic => "H",
            Action::ModeManual => "M",
            Action::ToggleRandom => "R",
            Action::ToggleDancer => "D",
            Action::NextClip => "C",
            Action::NextStyle => "S",
            Action::CycleCanon => "V",
            Action::Blackout => "B",
            Action::Fullscreen => "F",
            Action::MarkDownbeat => "Space",
            Action::LatencyDown => "[",
            Action::LatencyUp => "]",
            Action::CycleFx => "X",
            Action::ReloadShaders => "F5",
            Action::TogglePanel => "F1",
            Action::LeaveFullscreen => "Escape",
        }
    }
}

/// How much the auto-pilot is allowed to change on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Cut scenes every phrase and on drops; the dancer follows the track.
    Auto,
    /// Hold the current scene; the dancer still changes with the phrases.
    Static,
    /// Nothing changes unless you do it.
    Manual,
}

/// Whether the seasonal scenes (Halloween, Christmas, fireworks) rotate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Seasonal {
    /// Only in season (see `SEASONS`).
    Auto,
    Always,
    Off,
}

/// Seasonal scenes and when they're in season: (month, day) ranges, inclusive.
pub const SEASONS: [(&str, &[((u32, u32), (u32, u32))]); 3] = [
    ("halloween", &[((10, 1), (11, 2))]),
    ("christmas", &[((12, 1), (12, 27))]),
    // Bonfire Night and New Year.
    ("fireworks", &[((11, 1), (11, 8)), ((12, 28), (12, 31)), ((1, 1), (1, 2))]),
];

/// None if `scene` isn't seasonal, otherwise whether it's in season on (month, day).
pub fn in_season(scene: &str, date: (u32, u32)) -> Option<bool> {
    let (_, ranges) = SEASONS.iter().find(|(name, _)| *name == scene)?;
    Some(ranges.iter().any(|(from, to)| *from <= date && date <= *to))
}

/// Today's (month, day) in UTC, from the system clock.
pub fn today() -> (u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    month_day(secs)
}

/// (month, day) of a Unix timestamp, via Howard Hinnant's days-to-civil algorithm.
fn month_day(secs: i64) -> (u32, u32) {
    let z = secs.div_euclid(86_400) + 719_468;
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (month, day)
}

/// Whole-frame post effect: mirrors and kaleidoscope. Applied in
/// `present.wgsl` from `u.fx`, so it transforms the scene and dancer alike.
/// `#[serde(other)]` keeps a stale value (e.g. a removed mode) from
/// invalidating the whole settings file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fx {
    MirrorX,
    MirrorY,
    Quad,
    Kaleido6,
    Kaleido8,
    /// Last so `#[serde(other)]` lands here: a removed mode in a stale
    /// trippin.json (e.g. "Invert") falls back to Off instead of being
    /// rejected with the whole settings file.
    #[serde(other)]
    #[default]
    Off,
}

impl Fx {
    pub const ALL: [Fx; 6] = [
        Fx::Off,
        Fx::MirrorX,
        Fx::MirrorY,
        Fx::Quad,
        Fx::Kaleido6,
        Fx::Kaleido8,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Fx::Off => "Off",
            Fx::MirrorX => "Mirror X",
            Fx::MirrorY => "Mirror Y",
            Fx::Quad => "Quad mirror",
            Fx::Kaleido6 => "Kaleido x6",
            Fx::Kaleido8 => "Kaleido x8",
        }
    }

    /// The number handed to the shader (`u.fx`).
    pub fn index(self) -> f32 {
        match self {
            Fx::Off => 0.0,
            Fx::MirrorX => 1.0,
            Fx::MirrorY => 2.0,
            Fx::Quad => 3.0,
            Fx::Kaleido6 => 4.0,
            Fx::Kaleido8 => 5.0,
        }
    }

    pub fn next(self) -> Fx {
        let i = Fx::ALL.iter().position(|&f| f == self).unwrap_or(0);
        Fx::ALL[(i + 1) % Fx::ALL.len()]
    }

    /// Auto pool: MirrorY is excluded — an upside-down dancer mid-set reads
    /// as a glitch rather than an effect (it stays selectable by hand).
    const AUTO: [Fx; 5] = [Fx::Off, Fx::MirrorX, Fx::Quad, Fx::Kaleido6, Fx::Kaleido8];

    /// A random pick for auto mode; never repeats the current effect.
    pub fn random(r: f32, cur: Fx) -> Fx {
        let i = (r * Fx::AUTO.len() as f32) as usize % Fx::AUTO.len();
        let f = Fx::AUTO[i];
        if f == cur { Fx::AUTO[(i + 1) % Fx::AUTO.len()] } else { f }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tristate {
    Auto,
    On,
    Off,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub keys: BTreeMap<Action, String>,
    pub mode: Mode,
    pub random_order: bool,
    /// Bars per scene in auto mode.
    pub phrase_bars: u32,
    /// Cut early when a drop lands.
    pub cut_on_drops: bool,
    pub disabled_scenes: Vec<String>,
    pub seasonal: Seasonal,
    pub dancer_enabled: bool,
    /// None = auto-pilot picks the look; Some(i) = always that look.
    pub dancer_style: Option<usize>,
    pub canon: Tristate,
    pub disabled_clips: Vec<String>,
    /// Dancer height as a fraction of the screen.
    pub dancer_size: f32,
    /// Fixed post effect (ignored while `fx_auto` is on).
    pub fx: Fx,
    /// Pick a fresh post effect on every scene cut.
    pub fx_auto: bool,
    /// Post effect strength 0..1 — blends the transform in `present.wgsl`.
    pub fx_amt: f32,
    pub latency_ms: f32,
    pub show_panel: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            keys: Action::ALL.iter().map(|a| (*a, a.default_key().to_string())).collect(),
            mode: Mode::Auto,
            random_order: true,
            phrase_bars: 16,
            cut_on_drops: true,
            disabled_scenes: Vec::new(),
            seasonal: Seasonal::Auto,
            dancer_enabled: true,
            dancer_style: None,
            canon: Tristate::Auto,
            disabled_clips: Vec::new(),
            dancer_size: 0.85,
            fx: Fx::Off,
            fx_auto: false,
            fx_amt: 1.0,
            latency_ms: 30.0,
            show_panel: true,
        }
    }
}

/// `trippin.json` in the working directory if one exists (handy when running
/// from the repo), otherwise `%APPDATA%\Trippin	rippin.json` so an installed
/// copy in Program Files can still save.
fn path() -> PathBuf {
    let local = PathBuf::from("trippin.json");
    if local.exists() {
        return local;
    }
    match std::env::var_os("APPDATA") {
        Some(appdata) => {
            let dir = PathBuf::from(appdata).join("Trippin");
            let _ = std::fs::create_dir_all(&dir);
            dir.join("trippin.json")
        }
        None => local,
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut s: Settings = std::fs::read_to_string(path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).map_err(|e| eprintln!("trippin.json ignored: {e}")).ok())
            .unwrap_or_default();
        // Actions added in newer versions get their default key.
        for a in Action::ALL {
            s.keys.entry(a).or_insert_with(|| a.default_key().to_string());
        }
        s
    }

    pub fn save(&self) {
        match serde_json::to_string_pretty(self) {
            Ok(t) => {
                if let Err(e) = std::fs::write(path(), t) {
                    eprintln!("could not save trippin.json: {e}");
                }
            }
            Err(e) => eprintln!("could not serialise settings: {e}"),
        }
    }

    pub fn action_for(&self, key: &str) -> Option<Action> {
        self.keys.iter().find(|(_, k)| k.as_str() == key).map(|(a, _)| *a)
    }
}

/// Stable, human-readable name for a key ("A", "Space", "ArrowRight", "F5").
pub fn key_name(key: &Key) -> Option<String> {
    match key {
        Key::Named(n) => Some(format!("{n:?}")),
        Key::Character(c) => Some(c.to_uppercase()),
        _ => None,
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_day_matches_known_dates() {
        assert_eq!(month_day(1_790_380_800), (9, 26)); // 2026-09-26
        assert_eq!(month_day(1_709_164_800), (2, 29)); // 2024-02-29 (leap day)
        assert_eq!(month_day(1_767_139_200), (12, 31)); // 2025-12-31
        assert_eq!(month_day(1_767_225_600), (1, 1)); // 2026-01-01
        assert_eq!(month_day(951_868_800), (3, 1)); // 2000-03-01
    }

    #[test]
    fn seasons() {
        assert_eq!(in_season("kaleido", (10, 31)), None);
        assert_eq!(in_season("halloween", (10, 31)), Some(true));
        assert_eq!(in_season("halloween", (9, 26)), Some(false));
        assert_eq!(in_season("christmas", (12, 24)), Some(true));
        assert_eq!(in_season("fireworks", (11, 5)), Some(true));
        assert_eq!(in_season("fireworks", (12, 31)), Some(true));
        assert_eq!(in_season("fireworks", (1, 1)), Some(true));
        assert_eq!(in_season("fireworks", (1, 3)), Some(false));
    }
}
