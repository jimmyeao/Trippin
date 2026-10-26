//! Per-show controls for the Unity shows (Perform tab > show controls, and
//! the remote). Two kinds:
//!
//! - **Built in** (`builtin`), for every `unity_*` show:
//!   - Speed: Trippin runs the travel clocks faster or slower (`link.rs`),
//!     and the engine (v18+) runs the camera's travel at it too.
//!   - Reactivity: sent in the feed; the engine (v18+) applies it to the
//!     levels as they arrive, after which no show's auto-gain can undo it
//!     (a gain on the levels here was cancelled out, or saturated at 1).
//!   - Brightness, Saturation, Hue: applied by Trippin to the frame the
//!     engine hands back (`look`, `ext_look` in common.wgsl), so they work
//!     with any engine build.
//! - **Show-specific**: knobs a show declares in C# with `[TrippinParam]`.
//!   The engine writes them to a manifest at startup (`-trippinParams`,
//!   see `engine.rs`); their values go back in the feed's `knobs` array, in
//!   manifest order.
//!
//! Values live in `Settings::show_params` (scene → id → value); a missing
//! value is the default, so "reset" just removes the entry.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ParamDef {
    pub id: String,
    pub label: String,
    pub min: f32,
    pub max: f32,
    #[serde(rename = "def")]
    pub default: f32,
}

/// Ids of the built-in controls (Settings keys).
pub const SPEED: &str = "speed";
pub const REACT: &str = "react";
pub const BRIGHT: &str = "bright";
pub const SAT: &str = "sat";
pub const HUE: &str = "hue";

pub fn builtin() -> [ParamDef; 5] {
    [
        ParamDef { id: SPEED.into(), label: "Speed".into(), min: 0.25, max: 2.5, default: 1.0 },
        ParamDef { id: REACT.into(), label: "Reactivity".into(), min: 0.0, max: 3.0, default: 1.0 },
        ParamDef { id: BRIGHT.into(), label: "Brightness".into(), min: 0.3, max: 2.0, default: 1.0 },
        ParamDef { id: SAT.into(), label: "Saturation".into(), min: 0.0, max: 2.0, default: 1.0 },
        ParamDef { id: HUE.into(), label: "Hue shift".into(), min: -0.5, max: 0.5, default: 0.0 },
    ]
}

#[derive(Deserialize)]
struct ManifestShow {
    scene: String,
    #[serde(default)]
    params: Vec<ParamDef>,
}

#[derive(Deserialize)]
struct Manifest {
    shows: Vec<ManifestShow>,
}

/// Show-specific params from the engine's manifest, by scene.
static SHOWS: Mutex<Option<HashMap<String, Vec<ParamDef>>>> = Mutex::new(None);
/// The manifest file's mtime when it was last read.
static SEEN: Mutex<Option<SystemTime>> = Mutex::new(None);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// (Re)read the engine's manifest if it changed. Cheap: one stat.
pub fn poll_manifest(path: &Path) {
    // Called every frame while the engine runs: look once a second.
    static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    {
        let mut l = lock(&LAST);
        if l.is_some_and(|t| t.elapsed().as_secs_f32() < 1.0) {
            return;
        }
        *l = Some(std::time::Instant::now());
    }
    let Ok(mtime) = std::fs::metadata(path).and_then(|m| m.modified()) else { return };
    if *lock(&SEEN) == Some(mtime) {
        return;
    }
    *lock(&SEEN) = Some(mtime);
    let Ok(text) = std::fs::read_to_string(path) else { return };
    match parse_manifest(&text) {
        Ok(m) => *lock(&SHOWS) = Some(m),
        Err(e) => eprintln!("show params: {}: {e}", path.display()),
    }
}

fn parse_manifest(text: &str) -> Result<HashMap<String, Vec<ParamDef>>, serde_json::Error> {
    let m: Manifest = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    Ok(m.shows
        .into_iter()
        .map(|s| {
            let ok = s.params.into_iter().filter(|p| p.min < p.max && p.default.is_finite()).take(8);
            (s.scene, ok.collect())
        })
        .collect())
}

/// The show's own (manifest) params; empty for old engines or other scenes.
pub fn show_defs(scene: &str) -> Vec<ParamDef> {
    lock(&SHOWS).as_ref().and_then(|m| m.get(scene).cloned()).unwrap_or_default()
}

/// Does this scene have controls at all?
pub fn has_controls(scene: &str) -> bool {
    scene.starts_with("unity_")
}

/// A control's current value (its default when unset), clamped to range.
pub fn value(saved: &BTreeMap<String, BTreeMap<String, f32>>, scene: &str, def: &ParamDef) -> f32 {
    saved
        .get(scene)
        .and_then(|m| m.get(&def.id))
        .copied()
        .filter(|v| v.is_finite())
        .unwrap_or(def.default)
        .clamp(def.min, def.max)
}

/// What the link applies for the scene on air.
#[derive(Clone, Debug, PartialEq)]
pub struct Tweak {
    pub speed: f32,
    pub react: f32,
    /// Show-specific values, manifest order.
    pub knobs: Vec<f32>,
}

impl Default for Tweak {
    fn default() -> Self {
        Tweak { speed: 1.0, react: 1.0, knobs: Vec::new() }
    }
}

pub fn tweak(saved: &BTreeMap<String, BTreeMap<String, f32>>, scene: &str) -> Tweak {
    if !has_controls(scene) {
        return Tweak::default();
    }
    let [sp, re, ..] = builtin();
    Tweak {
        speed: value(saved, scene, &sp),
        react: value(saved, scene, &re),
        knobs: show_defs(scene).iter().map(|d| value(saved, scene, d)).collect(),
    }
}

/// Brightness, saturation and hue shift (turns) for the frame of the Unity
/// show on air, as `Uniforms::look4` (`ext_look` in common.wgsl); identity
/// for every other scene.
pub fn look(saved: &BTreeMap<String, BTreeMap<String, f32>>, scene: &str) -> [f32; 4] {
    if !has_controls(scene) {
        return [1.0, 1.0, 0.0, 0.0];
    }
    let [_, _, b, s, h] = builtin();
    [value(saved, scene, &b), value(saved, scene, &s), value(saved, scene, &h), 0.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manifest_parses_and_bad_params_are_dropped() {
        let m = parse_manifest(
            r#"{"shows":[{"scene":"unity_fluid","params":[
                {"id":"swirl","label":"Swirl","min":0,"max":2,"def":1},
                {"id":"broken","label":"Broken","min":1,"max":1,"def":1}]},
              {"scene":"unity_stage"}]}"#,
        )
        .unwrap();
        assert_eq!(m["unity_fluid"].len(), 1);
        assert_eq!(m["unity_fluid"][0].label, "Swirl");
        assert!(m["unity_stage"].is_empty());
    }

    #[test]
    fn unset_values_are_defaults_and_saved_ones_clamp() {
        let mut saved = BTreeMap::new();
        let [sp, re, ..] = builtin();
        assert_eq!(value(&saved, "unity_orb", &sp), 1.0);
        saved.insert("unity_orb".to_string(), BTreeMap::from([(SPEED.to_string(), 9.0)]));
        assert_eq!(value(&saved, "unity_orb", &sp), 2.5);
        assert_eq!(value(&saved, "unity_orb", &re), 1.0);
        // Not a Unity show: no tweak, and the frame is left alone.
        assert_eq!(tweak(&saved, "tunnel"), Tweak::default());
        saved.insert("tunnel".to_string(), BTreeMap::from([(BRIGHT.to_string(), 2.0)]));
        assert_eq!(look(&saved, "tunnel"), [1.0, 1.0, 0.0, 0.0]);
        saved.insert("unity_orb".to_string(), BTreeMap::from([(HUE.to_string(), 0.25), (SAT.to_string(), 5.0)]));
        assert_eq!(look(&saved, "unity_orb"), [1.0, 2.0, 0.25, 0.0]);
    }
}
