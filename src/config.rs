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
    ReloadShaders,
    TogglePanel,
    LeaveFullscreen,
}

impl Action {
    pub const ALL: [Action; 18] = [
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
    pub dancer_enabled: bool,
    /// None = auto-pilot picks the look; Some(i) = always that look.
    pub dancer_style: Option<usize>,
    pub canon: Tristate,
    pub disabled_clips: Vec<String>,
    /// Dancer height as a fraction of the screen.
    pub dancer_size: f32,
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
            dancer_enabled: true,
            dancer_style: None,
            canon: Tristate::Auto,
            disabled_clips: Vec::new(),
            dancer_size: 0.85,
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
