//! Styles: a curated vibe (Pop, Rock, Dance...) that steers the auto-pilot
//! (docs/design/looks-styles-packs.md). A Style narrows the scenes the
//! director may pick, restricts the `auto` palette to a family, and can set the
//! pacing, without the DJ touching anything.
//!
//! It is an overlay, not a setting rewrite: only the chosen id is saved
//! (`Settings::style`); the pool, palettes and pacing are applied to the
//! per-frame settings clone, so nothing leaks into `trippin.json`.
//!
//! Precedence, highest first: the user's disabled scenes and the GPU/heavy
//! gating (`usable_scenes` applies those before a Style sees the list), Manual
//! mode (a Style never overrides a manual choice), then the Style's pool, then
//! the global defaults. So a Style can never enable something the user turned off.
//!
//! In code a Style is a [`Theme`] (the dancer's looks already own the word
//! "style": `dancer::STYLES`, `Action::NextStyle`).

use crate::config::{Mode, Settings};
use crate::tags::SceneTags;
use serde::Deserialize;
use std::path::Path;
use std::sync::OnceLock;

/// A Style never leaves the director fewer scenes than this: a pool that
/// narrow is widened with the closest remaining scenes.
pub const MIN_POOL: usize = 4;

/// A set of scenes picked by tag. All four lists are optional; an empty pool
/// matches everything.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Pool {
    /// Scenes always in, whatever their tags.
    pub ids: Vec<String>,
    /// At least one of these tags.
    pub include: Vec<String>,
    /// All of these tags.
    pub require: Vec<String>,
    /// None of these tags.
    pub exclude: Vec<String>,
}

impl Pool {
    pub fn matches(&self, scene: &str, tags: &SceneTags) -> bool {
        if self.ids.iter().any(|i| i == scene) {
            return true;
        }
        let has = |t: &String| tags.has(scene, t);
        if self.exclude.iter().any(has) || !self.require.iter().all(has) {
            return false;
        }
        self.include.is_empty() || self.include.iter().any(has)
    }

    /// How many of the pool's wanted tags (`include` + `require`) a scene has;
    /// the order in which a too-small pool is widened.
    fn overlap(&self, scene: &str, tags: &SceneTags) -> usize {
        self.include.iter().chain(&self.require).filter(|t| tags.has(scene, t)).count()
    }
}

/// `auto` palette families per mood: breakdown, steady groove, driving, peak.
/// An empty list leaves that mood on the built-in rotation.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MoodPalettes {
    pub calm: Vec<String>,
    pub groove: Vec<String>,
    pub driving: Vec<String>,
    pub peak: Vec<String>,
}

/// Pacing a Style sets (each `None` leaves the user's own setting).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Pacing {
    pub phrase_bars: Option<u32>,
    pub cut_on_drops: Option<bool>,
    pub fx_auto: Option<bool>,
    pub fx_amt: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub about: String,
    pub scenes: Pool,
    /// Preferred scenes for breakdowns (the mood fitter's calm picks).
    pub calm_scenes: Pool,
    pub palettes: MoodPalettes,
    pub director: Pacing,
}

#[derive(Debug, Default)]
pub struct Catalog {
    pub tags: SceneTags,
    pub themes: Vec<Theme>,
}

impl Catalog {
    /// Read `scene_tags.json` and `styles.json` from the shaders directory. A
    /// missing or broken styles file means no Styles (the app runs as before).
    pub fn load(shader_dir: &Path) -> Catalog {
        let themes = std::fs::read_to_string(shader_dir.join("styles.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Vec<Theme>>(t.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default();
        Catalog { tags: SceneTags::load(shader_dir), themes }
    }

    pub fn theme(&self, id: &str) -> Option<&Theme> {
        self.themes.iter().find(|t| t.id == id)
    }
}

/// The catalog next to the running shaders, loaded once.
pub fn catalog() -> &'static Catalog {
    static CAT: OnceLock<Catalog> = OnceLock::new();
    CAT.get_or_init(|| {
        crate::render::find_shader_dir().map(|d| Catalog::load(&d)).unwrap_or_default()
    })
}

/// The Style `s` has selected, if it names one that exists.
pub fn active<'a>(s: &Settings, cat: &'a Catalog) -> Option<&'a Theme> {
    s.style.as_deref().and_then(|id| cat.theme(id))
}

/// Keep the scenes of `usable` that suit the selected Style. `name` maps a
/// scene index to its id. Unchanged when no Style is selected or in Manual
/// mode; widened with the nearest scenes when the pool would be under
/// [`MIN_POOL`]. Never adds a scene that wasn't already in `usable`.
pub fn narrow(usable: &[usize], name: &dyn Fn(usize) -> String, s: &Settings, cat: &Catalog) -> Vec<usize> {
    let Some(theme) = active(s, cat) else { return usable.to_vec() };
    if s.mode == Mode::Manual {
        return usable.to_vec();
    }
    let mut keep: Vec<usize> = usable.iter().copied().filter(|&i| theme.scenes.matches(&name(i), &cat.tags)).collect();
    if keep.len() >= MIN_POOL.min(usable.len()) {
        return keep;
    }
    // Widen: the closest remaining scenes first (most of the Style's wanted tags).
    let mut rest: Vec<(usize, usize)> = usable
        .iter()
        .copied()
        .filter(|i| !keep.contains(i))
        .map(|i| (theme.scenes.overlap(&name(i), &cat.tags), i))
        .collect();
    rest.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    keep.extend(rest.into_iter().map(|(_, i)| i).take(MIN_POOL.saturating_sub(keep.len())));
    keep.sort_unstable();
    keep
}

/// The scenes to prefer in a breakdown: the Style's calm pool within
/// `allowed` (the user's scenes before the Style narrowed them: a calm pool
/// usually sits outside the main pool, e.g. Dance excludes `slow` and its calm
/// pool requires it), or `usable` when there is no Style, no calm pool, or it
/// would leave fewer than two scenes.
pub fn calm_candidates(
    usable: &[usize],
    allowed: &[usize],
    name: &dyn Fn(usize) -> String,
    s: &Settings,
    cat: &Catalog,
) -> Vec<usize> {
    let Some(theme) = active(s, cat) else { return usable.to_vec() };
    let calm: Vec<usize> = allowed.iter().copied().filter(|&i| theme.calm_scenes.matches(&name(i), &cat.tags)).collect();
    if calm.len() >= 2 { calm } else { usable.to_vec() }
}

/// Apply the Style's pacing to a (per-frame, never persisted) settings clone.
pub fn overlay(s: &mut Settings, cat: &Catalog) {
    let Some(theme) = active(s, cat).cloned() else { return };
    let d = &theme.director;
    if let Some(v) = d.phrase_bars {
        s.phrase_bars = v.clamp(1, 64);
    }
    if let Some(v) = d.cut_on_drops {
        s.cut_on_drops = v;
    }
    if let Some(v) = d.fx_auto {
        s.fx_auto = v;
    }
    if let Some(v) = d.fx_amt {
        s.fx_amt = v.clamp(0.0, 1.0);
    }
}

/// The Style's `auto` palette lists as the renderer's static names, in mood
/// order (calm, groove, driving, peak). Unknown names are dropped.
pub fn mood_palettes(theme: &Theme) -> [Vec<&'static str>; 4] {
    let conv = |l: &Vec<String>| -> Vec<&'static str> {
        l.iter().filter_map(|n| crate::palettes::names().find(|p| p == n)).collect()
    };
    [conv(&theme.palettes.calm), conv(&theme.palettes.groove), conv(&theme.palettes.driving), conv(&theme.palettes.peak)]
}

/// The Style after `current` in the list (None = off), wrapping through off:
/// off, first, second, ..., last, off.
pub fn next_id(current: Option<&str>, cat: &Catalog) -> Option<String> {
    match current.and_then(|c| cat.themes.iter().position(|t| t.id == c)) {
        None => cat.themes.first().map(|t| t.id.clone()),
        Some(i) => cat.themes.get(i + 1).map(|t| t.id.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat() -> Catalog {
        Catalog::load(&crate::render::find_shader_dir().expect("shaders dir"))
    }

    fn all_scenes(c: &Catalog) -> Vec<String> {
        c.tags.scenes().filter(|s| *s != "void").cloned().collect()
    }

    fn settings(style: Option<&str>, mode: Mode) -> Settings {
        Settings { style: style.map(str::to_string), mode, ..Settings::default() }
    }

    #[test]
    fn the_seven_built_in_styles_load() {
        let c = cat();
        let ids: Vec<&str> = c.themes.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["dance", "house_techno", "pop", "rock", "hiphop_rnb", "chill", "party"]);
        for t in &c.themes {
            assert!(!t.name.is_empty() && !t.about.is_empty(), "{}: needs a name and a blurb", t.id);
        }
    }

    #[test]
    fn styles_use_only_real_tags_palettes_and_sane_pacing() {
        let c = cat();
        for t in &c.themes {
            for pool in [&t.scenes, &t.calm_scenes] {
                for tag in pool.include.iter().chain(&pool.require).chain(&pool.exclude) {
                    assert!(crate::tags::VOCAB.contains(&tag.as_str()), "{}: unknown tag {tag:?}", t.id);
                }
                for id in &pool.ids {
                    assert!(c.tags.of(id).is_some(), "{}: unknown scene id {id:?}", t.id);
                }
            }
            for list in [&t.palettes.calm, &t.palettes.groove, &t.palettes.driving, &t.palettes.peak] {
                assert!(!list.is_empty(), "{}: every mood needs palettes", t.id);
                for p in list {
                    assert!(crate::palettes::names().any(|n| n == p), "{}: unknown palette {p:?}", t.id);
                }
            }
            if let Some(b) = t.director.phrase_bars {
                assert!((1..=32).contains(&b), "{}: phrase_bars {b}", t.id);
            }
            if let Some(a) = t.director.fx_amt {
                assert!((0.0..=1.0).contains(&a), "{}: fx_amt {a}", t.id);
            }
        }
    }

    #[test]
    fn every_style_has_a_healthy_distinct_pool() {
        let c = cat();
        let scenes: Vec<String> = all_scenes(&c).into_iter().filter(|s| !s.starts_with("unity_")).collect();
        let pools: Vec<(String, Vec<&String>)> = c
            .themes
            .iter()
            .map(|t| (t.id.clone(), scenes.iter().filter(|s| t.scenes.matches(s, &c.tags)).collect()))
            .collect();
        for (id, p) in &pools {
            // Unity scenes only count when the engine is live, so the pool must stand on the wgpu scenes alone.
            assert!(p.len() >= 15, "{id}: only {} wgpu scenes in the pool", p.len());
            assert!(p.len() * 10 <= scenes.len() * 8, "{id}: {} of {} scenes is not a narrowing", p.len(), scenes.len());
        }
        for (i, (a, pa)) in pools.iter().enumerate() {
            for (b, pb) in pools.iter().skip(i + 1) {
                let both = pa.iter().filter(|s| pb.contains(s)).count();
                let union = pa.len() + pb.len() - both;
                assert!(both * 100 < union * 85, "{a} and {b} are nearly the same pool ({both}/{union})");
            }
            let calm = c.themes[i].calm_scenes.clone();
            let n = scenes.iter().filter(|s| calm.matches(s, &c.tags)).count();
            assert!(n >= 6, "{a}: only {n} calm scenes");
        }
    }

    #[test]
    fn narrowing_respects_what_is_usable_and_never_adds() {
        let c = cat();
        let names: Vec<String> = all_scenes(&c);
        let name = |i: usize| names[i].clone();
        let usable: Vec<usize> = (0..names.len()).filter(|i| i % 2 == 0).collect();
        let s = settings(Some("chill"), Mode::Auto);
        let out = narrow(&usable, &name, &s, &c);
        assert!(!out.is_empty() && out.len() < usable.len());
        assert!(out.iter().all(|i| usable.contains(i)), "a Style never adds a scene the user had off");
        assert!(out.iter().all(|&i| c.tags.has(&names[i], "slow")), "chill keeps only slow scenes here");
    }

    #[test]
    fn no_style_manual_mode_or_an_unknown_style_change_nothing() {
        let c = cat();
        let names = all_scenes(&c);
        let name = |i: usize| names[i].clone();
        let usable: Vec<usize> = (0..names.len()).collect();
        assert_eq!(narrow(&usable, &name, &settings(None, Mode::Auto), &c), usable);
        assert_eq!(narrow(&usable, &name, &settings(Some("rock"), Mode::Manual), &c), usable, "Manual is never narrowed");
        assert_eq!(narrow(&usable, &name, &settings(Some("no-such-style"), Mode::Auto), &c), usable);
    }

    #[test]
    fn a_tiny_pool_is_widened_never_left_empty() {
        let c = cat();
        let names = all_scenes(&c);
        let name = |i: usize| names[i].clone();
        // Only scenes the chill Style rejects (loud stage scenes) are usable.
        let stage: Vec<usize> = (0..names.len()).filter(|&i| c.tags.has(&names[i], "stage") && c.tags.has(&names[i], "driving")).take(8).collect();
        assert!(stage.len() >= MIN_POOL);
        let out = narrow(&stage, &name, &settings(Some("chill"), Mode::Auto), &c);
        assert!(out.len() >= MIN_POOL, "widened to at least {MIN_POOL}, got {}", out.len());
        assert!(out.iter().all(|i| stage.contains(i)));
        // And with fewer than MIN_POOL usable, everything usable stays.
        let two = &stage[..2];
        assert_eq!(narrow(two, &name, &settings(Some("chill"), Mode::Auto), &c), two.to_vec());
    }

    #[test]
    fn calm_candidates_prefer_the_calm_pool_with_a_fallback() {
        let c = cat();
        let names = all_scenes(&c);
        let name = |i: usize| names[i].clone();
        let all: Vec<usize> = (0..names.len()).collect();
        let s = settings(Some("dance"), Mode::Auto);
        // As the render loop calls it: `usable` is already narrowed to the
        // Dance pool, which has no slow scene at all.
        let usable = narrow(&all, &name, &s, &c);
        assert!(!usable.iter().any(|&i| c.tags.has(&names[i], "slow")));
        let calm = calm_candidates(&usable, &all, &name, &s, &c);
        assert!(calm.len() >= 6 && calm.len() < all.len());
        assert!(calm.iter().all(|&i| c.tags.has(&names[i], "slow")));
        let loud: Vec<usize> = (0..names.len()).filter(|&i| c.tags.has(&names[i], "driving")).take(10).collect();
        assert_eq!(calm_candidates(&loud, &loud, &name, &s, &c), loud, "no calm scene allowed: fall back");
        assert_eq!(calm_candidates(&usable, &all, &name, &settings(None, Mode::Auto), &c), usable);
    }

    #[test]
    fn the_overlay_sets_pacing_only_when_a_style_is_selected() {
        let c = cat();
        let mut s = settings(None, Mode::Auto);
        s.phrase_bars = 7;
        overlay(&mut s, &c);
        assert_eq!(s.phrase_bars, 7, "no style, no change");
        s.style = Some("chill".into());
        overlay(&mut s, &c);
        assert_eq!((s.phrase_bars, s.cut_on_drops, s.fx_auto), (16, false, false));
        assert_eq!(s.fx_amt, 0.0);
        s.style = Some("dance".into());
        s.fx_auto = true;
        overlay(&mut s, &c);
        assert_eq!(s.phrase_bars, 4);
        assert!(s.fx_auto, "dance leaves fx_auto alone");
    }

    #[test]
    fn mood_palettes_are_real_and_cycling_wraps_through_off() {
        let c = cat();
        for t in &c.themes {
            let m = mood_palettes(t);
            assert!(m.iter().all(|l| !l.is_empty()), "{}: a mood lost its palettes", t.id);
        }
        let n = c.themes.len();
        let mut cur: Option<String> = None;
        for i in 0..n {
            cur = next_id(cur.as_deref(), &c);
            assert_eq!(cur.as_deref(), Some(c.themes[i].id.as_str()));
        }
        assert_eq!(next_id(cur.as_deref(), &c), None, "after the last comes off");
        assert_eq!(next_id(Some("gone"), &c).as_deref(), Some("dance"), "an unknown id restarts at the first");
    }

    #[test]
    fn a_missing_or_broken_file_means_no_styles() {
        assert!(Catalog::load(Path::new("/nonexistent")).themes.is_empty());
    }
}
