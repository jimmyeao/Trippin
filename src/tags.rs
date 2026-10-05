//! Scene tags: a small closed vocabulary describing what each scene looks and
//! moves like (`shaders/scene_tags.json`). Styles pick scene pools by tag
//! (docs/design/looks-styles-packs.md), and the AI show builder can use them
//! to choose by feel instead of guessing from names.
//!
//! The vocabulary is closed on purpose: a typo in the JSON must fail a test,
//! not silently create a tag nothing matches. Add a scene, add its tags (the
//! `every_scene_has_tags` test fails until you do), the same rule as
//! `scene_energy.json`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Every tag a scene may carry, grouped by what it describes.
pub const VOCAB: &[&str] = &[
    // colour and light
    "neon",   // saturated glowing synth / laser look
    "warm",   // fire, ember, sunset, gold
    "cold",   // ice, deep blue, aurora, moonlight
    "dark",   // low-key, mostly black, moody
    "bright", // high-key, colourful, party
    // form
    "abstract",  // no real-world referent
    "geometric", // shapes, wireframes, grids, rings
    "organic",   // nature, fluid, fire, water
    "urban",     // city, architecture
    "space",     // stars, galaxies, nebulae
    "stage",     // laser rigs, light bars, LED walls, club hardware
    "character", // robots and figures
    "analyser",  // a literal audio display: bars, meters, scope
    "retro",     // synthwave, demoscene, pixels
    // motion
    "slow",    // calm, suits breakdowns and chill sets
    "driving", // fast and rhythmic, suits drops
    "flight",  // the camera travels through the scene
    // calendar
    "seasonal",
];

/// Scene id -> its tags.
#[derive(Clone, Debug, Default)]
#[allow(dead_code)] // first used by Styles (design doc, phase P2)
pub struct SceneTags(BTreeMap<String, BTreeSet<String>>);

#[allow(dead_code)]
impl SceneTags {
    /// Read `scene_tags.json` from the shaders directory. A missing or broken
    /// file gives an empty set (every query answers "no tags"), never an error:
    /// tags only steer choices, so the app must run without them.
    pub fn load(shader_dir: &Path) -> SceneTags {
        std::fs::read_to_string(shader_dir.join("scene_tags.json"))
            .ok()
            .map(|t| SceneTags::parse(t.trim_start_matches('\u{feff}')))
            .unwrap_or_default()
    }

    pub fn parse(json: &str) -> SceneTags {
        let raw: BTreeMap<String, Vec<String>> = serde_json::from_str(json).unwrap_or_default();
        SceneTags(raw.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect())
    }

    pub fn of(&self, scene: &str) -> Option<&BTreeSet<String>> {
        self.0.get(scene)
    }

    pub fn has(&self, scene: &str, tag: &str) -> bool {
        self.0.get(scene).is_some_and(|t| t.contains(tag))
    }

    /// True if the scene carries at least one of `any` (an empty `any` matches nothing).
    pub fn has_any(&self, scene: &str, any: &[&str]) -> bool {
        any.iter().any(|t| self.has(scene, t))
    }

    pub fn scenes(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shaders() -> std::path::PathBuf {
        crate::render::find_shader_dir().expect("shaders dir")
    }

    fn scene_ids(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir.join("scenes"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "wgsl"))
            .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn every_scene_has_tags() {
        let dir = shaders();
        let tags = SceneTags::load(&dir);
        let missing: Vec<String> = scene_ids(&dir).into_iter().filter(|s| tags.of(s).is_none()).collect();
        assert!(missing.is_empty(), "scenes missing from shaders/scene_tags.json: {missing:?}");
    }

    #[test]
    fn no_tags_for_deleted_scenes() {
        let dir = shaders();
        let ids = scene_ids(&dir);
        let tags = SceneTags::load(&dir);
        let stale: Vec<&String> = tags.scenes().filter(|s| !ids.contains(s)).collect();
        assert!(stale.is_empty(), "scene_tags.json names scenes that no longer exist: {stale:?}");
    }

    #[test]
    fn tags_stay_inside_the_vocabulary() {
        let tags = SceneTags::load(&shaders());
        for s in tags.scenes() {
            for t in tags.of(s).unwrap() {
                assert!(VOCAB.contains(&t.as_str()), "{s}: unknown tag {t:?} (vocabulary: {VOCAB:?})");
            }
        }
    }

    #[test]
    fn scenes_are_described_by_two_to_five_tags_except_void() {
        let tags = SceneTags::load(&shaders());
        for s in tags.scenes() {
            let n = tags.of(s).unwrap().len();
            if s == "void" {
                assert_eq!(n, 0, "void is pure black and carries no tags");
            } else {
                assert!((2..=5).contains(&n), "{s}: {n} tags (want 2-5 so Styles can tell scenes apart)");
            }
        }
    }

    #[test]
    fn a_style_pool_is_never_empty() {
        // The tag combinations the built-in Styles will lean on must each match a
        // healthy number of scenes, or the director would be left with nothing.
        let tags = SceneTags::load(&shaders());
        for tag in VOCAB {
            let n = tags.scenes().filter(|s| tags.has(s, tag)).count();
            assert!(n >= 3, "tag {tag:?} matches only {n} scenes");
        }
    }

    #[test]
    fn broken_or_missing_file_is_harmless() {
        assert!(SceneTags::parse("not json").of("tunnel").is_none());
        assert!(SceneTags::load(Path::new("/nonexistent")).of("tunnel").is_none());
        assert!(!SceneTags::default().has("tunnel", "flight"));
        assert!(SceneTags::parse("{\"a\":[\"slow\"]}").has_any("a", &["driving", "slow"]));
    }
}
