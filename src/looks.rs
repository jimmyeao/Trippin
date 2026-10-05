//! Looks: a saved snapshot of the visual state (scene, palette, effect, dancer)
//! that one key, pad or remote tap recalls (docs/design/looks-styles-packs.md).
//!
//! A Look names things that already exist; it carries no code, so a user's own
//! Looks are fine to create and share. Fields left out (`None`) leave that part
//! of the live state alone, so a Look can be "palette only".
//!
//! This module is pure state and files. Applying a Look to the running app
//! (switching the scene, recording cues) is `App::apply_look` in main.rs, which
//! calls [`apply`] for the settings half.

use crate::config::{Fx, Settings};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The file format version this build writes and understands.
pub const VERSION: u32 = 1;
/// Pads/keys bound to a Look: `Action::Look1..=Look8`.
pub const SLOTS: u8 = 8;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LookFx {
    pub mode: Fx,
    pub amt: f32,
    pub auto: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LookDancer {
    pub enabled: bool,
    /// Index into `dancer::STYLES` (append-only, so indices are stable).
    pub style: Option<usize>,
    /// Routine id; `None` leaves the running routine.
    pub clip: Option<String>,
    pub size: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedLook {
    pub version: u32,
    /// Lowercase slug, also the file name (`<id>.look.json`).
    pub id: String,
    pub name: String,
    /// Which pad/key (`Action::Look1..`) recalls it, 1-based.
    pub slot: Option<u8>,
    pub scene: Option<String>,
    pub palette: Option<String>,
    pub fx: Option<LookFx>,
    pub dancer: Option<LookDancer>,
    /// Pack ids this Look's scene needs (informational: shown when the scene
    /// is missing so the DJ knows why).
    pub requires: Vec<String>,
}

/// What [`apply`] left for the app to do (the scene and routine are changed
/// through the render thread, not by writing settings).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Applied {
    pub scene: Option<String>,
    pub clip: Option<String>,
    /// The Look names a scene that isn't installed/usable here. The rest of the
    /// Look was still applied.
    pub skipped_scene: Option<String>,
    /// Settings changed (the caller marks them dirty).
    pub changed: bool,
}

/// `"Club red!"` -> `"club-red"`. Only `[a-z0-9-]`, never empty, at most 48
/// chars: safe as a file name whatever the user typed.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_end_matches('-');
    let out: String = out.chars().take(48).collect();
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "look".into() } else { out }
}

/// Snapshot the current visual state as a Look called `name`. `scene` and
/// `clip` are what is on screen now (they live on the render thread, not in
/// `Settings`).
pub fn capture(name: &str, s: &Settings, scene: Option<&str>, clip: Option<&str>) -> SavedLook {
    SavedLook {
        version: VERSION,
        id: slugify(name),
        name: name.trim().to_string(),
        slot: None,
        scene: scene.map(str::to_string),
        palette: Some(s.palette.clone()),
        fx: Some(LookFx { mode: s.fx, amt: s.fx_amt, auto: s.fx_auto }),
        dancer: Some(LookDancer {
            enabled: s.dancer_enabled,
            style: s.dancer_style,
            clip: clip.map(str::to_string),
            size: Some(s.dancer_size),
        }),
        requires: Vec::new(),
    }
}

/// Write the settings half of `look` into `s`, and report the scene/routine
/// the app should switch to. `scenes` and `clips` are the ids that exist and are
/// usable now; anything else is skipped rather than guessed, so a Look from
/// another machine, or one using a pack that isn't installed, still applies
/// everything it can.
pub fn apply(look: &SavedLook, s: &mut Settings, scenes: &[String], clips: &[String]) -> Applied {
    let mut out = Applied::default();
    let before = (s.palette.clone(), s.fx, s.fx_amt, s.fx_auto, s.dancer_enabled, s.dancer_style, s.dancer_size);

    if let Some(p) = &look.palette {
        if crate::palettes::is_valid(p) {
            s.palette = p.clone();
        }
    }
    if let Some(fx) = &look.fx {
        s.fx = fx.mode;
        s.fx_amt = fx.amt.clamp(0.0, 1.0);
        s.fx_auto = fx.auto;
    }
    if let Some(d) = &look.dancer {
        s.dancer_enabled = d.enabled;
        if let Some(i) = d.style {
            if i < crate::dancer::STYLES.len() {
                s.dancer_style = Some(i);
            }
        }
        if let Some(z) = d.size {
            s.dancer_size = z.clamp(0.2, 3.0);
        }
        if let Some(c) = &d.clip {
            if clips.iter().any(|x| x == c) {
                out.clip = Some(c.clone());
            }
        }
    }
    match &look.scene {
        Some(sc) if scenes.iter().any(|x| x == sc) => out.scene = Some(sc.clone()),
        Some(sc) => out.skipped_scene = Some(sc.clone()),
        None => {}
    }
    out.changed = before != (s.palette.clone(), s.fx, s.fx_amt, s.fx_auto, s.dancer_enabled, s.dancer_style, s.dancer_size);
    out
}

// ---- storage ----------------------------------------------------------------

pub fn looks_dir() -> PathBuf {
    crate::config::data_dir().join("looks")
}

fn file_for(dir: &Path, id: &str) -> PathBuf {
    // Always re-slug: the id may have come from a hand-edited or imported file.
    dir.join(format!("{}.look.json", slugify(id)))
}

/// Every readable Look in `dir`, by name. Unreadable files, files from a newer
/// format and non-Looks are skipped (never an error: a broken file must not
/// stop the app starting).
pub fn load_all(dir: &Path) -> Vec<SavedLook> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<SavedLook> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().ends_with(".look.json")))
        .filter_map(|p| read_file(&p))
        .collect();
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
    v
}

/// One Look file, normalised: BOM stripped (Notepad), id re-slugged, name filled.
pub fn read_file(path: &Path) -> Option<SavedLook> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut l: SavedLook = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    if l.version == 0 || l.version > VERSION {
        return None;
    }
    l.id = slugify(if l.id.is_empty() { &l.name } else { &l.id });
    if l.name.trim().is_empty() {
        l.name = l.id.clone();
    }
    l.slot = l.slot.filter(|n| (1..=SLOTS).contains(n));
    Some(l)
}

/// Save `look` (replacing the file with its id). Written to a temp file and
/// renamed, so a crash mid-write can't leave half a Look.
pub fn save(dir: &Path, look: &SavedLook) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut l = look.clone();
    l.version = VERSION;
    l.id = slugify(&l.id);
    let path = file_for(dir, &l.id);
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&l).map_err(std::io::Error::other)?)?;
    std::fs::rename(&tmp, &path)
}

pub fn delete(dir: &Path, id: &str) -> std::io::Result<()> {
    match std::fs::remove_file(file_for(dir, id)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// An id based on `name` that no Look in `existing` uses (`club-red`, then
/// `club-red-2`, ...).
pub fn unique_id(existing: &[SavedLook], name: &str) -> String {
    let base = slugify(name);
    let taken = |id: &str| existing.iter().any(|l| l.id == id);
    if !taken(&base) {
        return base;
    }
    (2..).map(|n| format!("{base}-{n}")).find(|id| !taken(id)).unwrap()
}

/// Put `id` on `slot` (1..=SLOTS) and take the slot off any other Look,
/// like MIDI-learn: one slot, one Look. `slot` of `None` clears it.
pub fn assign_slot(looks: &mut [SavedLook], id: &str, slot: Option<u8>) {
    let slot = slot.filter(|n| (1..=SLOTS).contains(n));
    for l in looks.iter_mut() {
        if l.id == id {
            l.slot = slot;
        } else if slot.is_some() && l.slot == slot {
            l.slot = None;
        }
    }
}

pub fn by_slot(looks: &[SavedLook], slot: u8) -> Option<&SavedLook> {
    looks.iter().find(|l| l.slot == Some(slot))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "trippin-looks-{}-{}-{tag}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    /// A real palette that isn't the default.
    fn other_palette() -> String {
        let d = Settings::default().palette;
        crate::palettes::names().find(|n| *n != d).unwrap().to_string()
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn slugs_are_safe_file_names() {
        assert_eq!(slugify("Club red!"), "club-red");
        assert_eq!(slugify("  --Hello   World--  "), "hello-world");
        assert_eq!(slugify("../../etc/passwd"), "etc-passwd");
        assert_eq!(slugify("***"), "look");
        assert_eq!(slugify("Ünïcode Ñame"), "n-code-ame");
        assert!(slugify(&"a".repeat(200)).len() <= 48);
        assert!(!slugify("a/b\\c:d").contains(['/', '\\', ':']));
    }

    #[test]
    fn capture_then_apply_round_trips() {
        let mut s = Settings::default();
        s.palette = other_palette();
        s.fx = Fx::Kaleido6;
        s.fx_amt = 0.4;
        s.dancer_enabled = true;
        s.dancer_style = Some(2);
        s.dancer_size = 1.5;
        let look = capture("Club red", &s, Some("laser_show"), Some("stock_disco"));
        assert_eq!(look.id, "club-red");

        let mut t = Settings::default();
        let a = apply(&look, &mut t, &names(&["laser_show", "tunnel"]), &names(&["stock_disco"]));
        assert!(a.changed);
        assert_eq!(a.scene.as_deref(), Some("laser_show"));
        assert_eq!(a.clip.as_deref(), Some("stock_disco"));
        assert_eq!((t.palette.clone(), t.fx, t.dancer_style), (other_palette(), Fx::Kaleido6, Some(2)));
        assert!((t.fx_amt - 0.4).abs() < 1e-6 && (t.dancer_size - 1.5).abs() < 1e-6);
    }

    #[test]
    fn applying_twice_changes_nothing_the_second_time() {
        let look = capture("x", &Settings { palette: other_palette(), ..Settings::default() }, None, None);
        let mut s = Settings::default();
        assert!(apply(&look, &mut s, &[], &[]).changed);
        assert!(!apply(&look, &mut s, &[], &[]).changed, "idempotent");
    }

    #[test]
    fn a_partial_look_leaves_everything_else_alone() {
        let mut s = Settings::default();
        s.fx = Fx::Quad;
        s.dancer_enabled = true;
        s.dancer_size = 2.0;
        let palette_only = SavedLook { version: 1, id: "p".into(), palette: Some(other_palette()), ..Default::default() };
        let a = apply(&palette_only, &mut s, &[], &[]);
        assert_eq!(s.palette, other_palette());
        assert_eq!((s.fx, s.dancer_enabled, s.dancer_size), (Fx::Quad, true, 2.0));
        assert_eq!((a.scene, a.clip, a.skipped_scene), (None, None, None));
    }

    #[test]
    fn a_missing_scene_is_skipped_but_the_rest_still_applies() {
        // The Look came from a machine with a pack this one lacks.
        let look = SavedLook {
            version: 1,
            id: "xmas".into(),
            scene: Some("xmas/snowfall".into()),
            palette: Some(other_palette()),
            requires: vec!["xmas".into()],
            ..Default::default()
        };
        let mut s = Settings::default();
        let a = apply(&look, &mut s, &names(&["tunnel"]), &[]);
        assert_eq!(a.scene, None);
        assert_eq!(a.skipped_scene.as_deref(), Some("xmas/snowfall"));
        assert_eq!(s.palette, other_palette(), "palette still applied");
    }

    #[test]
    fn junk_values_are_ignored_not_trusted() {
        let look = SavedLook {
            version: 1,
            id: "j".into(),
            palette: Some("no-such-palette".into()),
            fx: Some(LookFx { mode: Fx::Off, amt: 99.0, auto: false }),
            dancer: Some(LookDancer { enabled: true, style: Some(999), clip: Some("nope".into()), size: Some(-5.0) }),
            ..Default::default()
        };
        let mut s = Settings::default();
        let pal = s.palette.clone();
        let a = apply(&look, &mut s, &[], &names(&["real"]));
        assert_eq!(s.palette, pal, "unknown palette ignored");
        assert_eq!(s.fx_amt, 1.0, "amount clamped");
        assert_ne!(s.dancer_style, Some(999));
        assert_eq!(s.dancer_size, 0.2, "size clamped");
        assert_eq!(a.clip, None, "unknown routine ignored");
    }

    #[test]
    fn save_load_delete_round_trip_and_survive_junk_files() {
        let dir = tmp("io");
        let mut a = capture("Alpha", &Settings::default(), Some("tunnel"), None);
        a.slot = Some(3);
        save(&dir, &a).unwrap();
        save(&dir, &capture("beta", &Settings::default(), None, None)).unwrap();
        std::fs::write(dir.join("broken.look.json"), "{not json").unwrap();
        std::fs::write(dir.join("future.look.json"), r#"{"version":99,"id":"f","name":"F"}"#).unwrap();
        std::fs::write(dir.join("notes.txt"), "ignore me").unwrap();
        std::fs::write(dir.join("bom.look.json"), "\u{feff}{\"version\":1,\"id\":\"bom\",\"name\":\"Bom\"}").unwrap();

        let all = load_all(&dir);
        let ids: Vec<&str> = all.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids, ["alpha", "beta", "bom"], "sorted by name; broken/future/non-look files skipped");
        assert_eq!(by_slot(&all, 3).map(|l| l.id.as_str()), Some("alpha"));
        assert_eq!(all[0].scene.as_deref(), Some("tunnel"));

        delete(&dir, "alpha").unwrap();
        delete(&dir, "alpha").unwrap(); // already gone: fine
        assert_eq!(load_all(&dir).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hostile_id_cannot_escape_the_looks_folder() {
        let dir = tmp("escape");
        let evil = SavedLook { version: 1, id: "../../evil".into(), name: "evil".into(), ..Default::default() };
        save(&dir, &evil).unwrap();
        assert!(dir.join("evil.look.json").exists());
        assert!(!dir.parent().unwrap().join("evil.look.json").exists());
        delete(&dir, "../../evil").unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ids_stay_unique_and_slots_are_exclusive() {
        let mk = |id: &str| SavedLook { version: 1, id: id.into(), name: id.into(), ..Default::default() };
        let mut v = vec![mk("club-red"), mk("club-red-2")];
        assert_eq!(unique_id(&v, "Club red"), "club-red-3");
        assert_eq!(unique_id(&v, "Fresh"), "fresh");

        assign_slot(&mut v, "club-red", Some(2));
        assign_slot(&mut v, "club-red-2", Some(2));
        assert_eq!(by_slot(&v, 2).map(|l| l.id.as_str()), Some("club-red-2"), "learning steals the slot");
        assert_eq!(v[0].slot, None);
        assign_slot(&mut v, "club-red-2", Some(99));
        assert_eq!(v[1].slot, None, "out-of-range slot clears it");
    }

    #[test]
    fn old_files_missing_new_fields_still_load() {
        // `#[serde(default)]` everywhere: a Look written by an older or hand-edited
        // build with only a name and a palette must still read.
        let dir = tmp("old");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.look.json"), r#"{"version":1,"name":"Only palette","palette":"auto"}"#).unwrap();
        let all = load_all(&dir);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "only-palette");
        assert!(all[0].scene.is_none() && all[0].fx.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
