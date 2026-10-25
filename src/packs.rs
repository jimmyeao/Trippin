//! Effects packs: scenes shipped apart from the app (the video clip scenes
//! first). A pack is a folder, or a `.trippinpack` zip of one:
//!
//! ```text
//! pack.json          {"id","name","version","api","about"}
//! scenes/*.wgsl      scenes, compiled like the built-in ones
//! videos/*.tvid      clips for its video_* scenes (video.rs)
//! scene_tags.json    tags for its scenes (tags.rs)
//! scene_energy.json  energy for its scenes (ai.rs, `--snap-energy`)
//! ```
//!
//! Packs live in `<data dir>/packs/<id>/` (installed from the catalog or a
//! file) and, for dev builds, in `packs/` in the repo (videos git-ignored).
//! The list is read once at startup: the scene list is fixed for the run
//! (status, remote and timeline hold scene indices), so installing, removing
//! or switching a pack takes effect on the next launch.
//!
//! A pack scene whose name is already taken (built in, or an earlier pack)
//! is skipped. `api` is the version of the shader helpers a pack's scenes
//! call (`vid()` and friends in common.wgsl); packs newer than `API` are
//! refused rather than failing to compile.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, bail, Context, Result};

/// The pack API this build provides. Bump when common.wgsl loses or changes
/// a helper packs may use.
pub const API: u32 = 1;

/// Packs on the release page. An empty checksum means not published yet.
pub struct CatalogEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub about: &'static str,
    pub size_mb: u32,
    /// Release asset name and its sha256.
    pub asset: (&'static str, &'static str),
}

const RELEASE_BASE: &str = "https://github.com/jimmyeao/Trippin/releases/download/packs-v1";

pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        id: "night-city",
        name: "Night City",
        about: "Nine video scenes: night drives through a neon city and a road tunnel, flights \
                down an avenue, over the rooftops, between towers and under a bridge, a neon \
                alley and highway light trails.",
        size_mb: 290,
        asset: ("night-city-1.trippinpack", ""),
    },
    CatalogEntry {
        id: "club-nights",
        name: "Club Nights",
        about: "Four video scenes: club lasers, a club light rig, and flights over a club crowd \
                and through a warehouse rave.",
        size_mb: 105,
        asset: ("club-nights-1.trippinpack", ""),
    },
    CatalogEntry {
        id: "liquid",
        name: "Liquid",
        about: "Three video scenes: ink blooming in water, coloured smoke and liquid chrome, \
                swirled by the music.",
        size_mb: 90,
        asset: ("liquid-1.trippinpack", ""),
    },
    CatalogEntry {
        id: "club-nights-2",
        name: "Club Nights 2",
        about: "Eight club scenes locked to the bar: a disco ball, the decks, a speaker cone,                 warehouse strobes, an LED wall, cryo jets, a laser tunnel and a jumping crowd.                 Kicks punch the camera in, fills stutter, drops scratch.",
        size_mb: 220,
        asset: ("club-nights-2-1.trippinpack", ""),
    },
];

#[derive(Clone, Debug)]
pub struct Pack {
    pub id: String,
    pub name: String,
    pub version: String,
    pub about: String,
    pub dir: PathBuf,
    /// Installed under the data dir (removable), not a dev folder in the repo.
    pub installed: bool,
    /// Off in Settings: on disk, but its scenes aren't loaded.
    pub disabled: bool,
    /// Why it isn't loaded, when it isn't (`api` too new).
    pub problem: Option<String>,
}

impl Pack {
    pub fn active(&self) -> bool {
        !self.disabled && self.problem.is_none()
    }
}

#[derive(serde::Deserialize)]
struct Manifest {
    id: String,
    name: String,
    #[serde(default)]
    version: serde_json::Value,
    #[serde(default = "one")]
    api: u32,
    #[serde(default)]
    about: String,
}

fn one() -> u32 {
    1
}

/// Ids switched off in Settings, set once at startup (before `all()`).
static DISABLED: OnceLock<Vec<String>> = OnceLock::new();
static PACKS: OnceLock<Vec<Pack>> = OnceLock::new();

pub fn init(disabled: &[String]) {
    let _ = DISABLED.set(disabled.to_vec());
}

/// Where installed packs go.
pub fn root() -> PathBuf {
    crate::config::data_dir().join("packs")
}

/// Pack folders in search order: dev folders in the repo first (so edits
/// show), then installed ones. The first folder with an id wins.
/// `TRIPPIN_NO_DEV_PACKS=1` leaves out the repo's, to see what an
/// installed build sees.
fn search_dirs() -> Vec<(PathBuf, bool)> {
    let mut d = Vec::new();
    if std::env::var_os("TRIPPIN_NO_DEV_PACKS").is_none() {
        d.push((Path::new(env!("CARGO_MANIFEST_DIR")).join("packs"), false));
    }
    if let Some(p) = std::env::var_os("TRIPPIN_PACKS") {
        d.insert(0, (PathBuf::from(p), false));
    }
    d.push((root(), true));
    d
}

fn read_manifest(dir: &Path) -> Result<Manifest> {
    let t = std::fs::read_to_string(dir.join("pack.json")).context("no pack.json")?;
    let m: Manifest = serde_json::from_str(t.trim_start_matches('\u{feff}')).context("bad pack.json")?;
    if !valid_id(&m.id) {
        bail!("pack id {:?}: use lower-case letters, digits and dashes", m.id);
    }
    Ok(m)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn version_str(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "1".into(),
        other => other.to_string(),
    }
}

fn scan() -> Vec<Pack> {
    let disabled = DISABLED.get().cloned().unwrap_or_default();
    let mut out: Vec<Pack> = Vec::new();
    for (base, installed) in search_dirs() {
        let Ok(rd) = std::fs::read_dir(&base) else { continue };
        let mut dirs: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect();
        dirs.sort();
        for dir in dirs {
            let m = match read_manifest(&dir) {
                Ok(m) => m,
                Err(e) => {
                    if dir.join("pack.json").exists() {
                        eprintln!("pack {}: {e:#}", dir.display());
                    }
                    continue;
                }
            };
            if out.iter().any(|p| p.id == m.id) {
                continue;
            }
            let problem = (m.api > API).then(|| "needs a newer Trippin".to_string());
            out.push(Pack {
                disabled: disabled.contains(&m.id),
                id: m.id,
                name: m.name,
                version: version_str(&m.version),
                about: m.about,
                dir,
                installed,
                problem,
            });
        }
    }
    out
}

/// Every pack found at startup, loaded or not.
pub fn all() -> &'static [Pack] {
    PACKS.get_or_init(scan)
}

pub fn active() -> impl Iterator<Item = &'static Pack> {
    all().iter().filter(|p| p.active())
}

fn wgsl_in(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    v.retain(|p| p.extension().is_some_and(|e| e == "wgsl"));
    v.sort();
    v
}

fn stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Scene files: the built-in `<shader_dir>/scenes` plus each loaded pack's,
/// sorted by name. A pack scene with a taken name is skipped (logged once).
pub fn scene_files(shader_dir: &Path) -> Vec<PathBuf> {
    let mut files = wgsl_in(&shader_dir.join("scenes"));
    let mut names: Vec<String> = files.iter().map(|p| stem(p)).collect();
    static WARNED: AtomicBool = AtomicBool::new(false);
    let warn = !WARNED.swap(true, Ordering::Relaxed);
    for pack in active() {
        for p in wgsl_in(&pack.dir.join("scenes")) {
            let n = stem(&p);
            if names.contains(&n) {
                if warn {
                    eprintln!("pack {}: scene {n} already exists, skipped", pack.id);
                }
                continue;
            }
            names.push(n);
            files.push(p);
        }
    }
    files.sort_by_key(|p| stem(p));
    files
}

/// Scene names, as `scene_files`.
pub fn scene_names(shader_dir: &Path) -> Vec<String> {
    scene_files(shader_dir).iter().map(|p| stem(p)).collect()
}

/// The file for scene `name`, built in or from a pack.
pub fn scene_file(shader_dir: &Path, name: &str) -> Option<PathBuf> {
    let base = shader_dir.join("scenes").join(format!("{name}.wgsl"));
    if base.is_file() {
        return Some(base);
    }
    active().map(|p| p.dir.join("scenes").join(format!("{name}.wgsl"))).find(|p| p.is_file())
}

/// Which loaded pack a scene comes from (None: built in).
pub fn owner(name: &str) -> Option<&'static Pack> {
    let shader_dir = crate::render::find_shader_dir().ok()?;
    if shader_dir.join("scenes").join(format!("{name}.wgsl")).is_file() {
        return None;
    }
    active().find(|p| p.dir.join("scenes").join(format!("{name}.wgsl")).is_file())
}

/// A JSON object file (`scene_tags.json`, `scene_energy.json`) merged from
/// the shader dir and every loaded pack: the built-in keys win.
pub fn merged_json(shader_dir: &Path, file: &str) -> serde_json::Map<String, serde_json::Value> {
    let read = |p: PathBuf| -> serde_json::Map<String, serde_json::Value> {
        std::fs::read_to_string(p)
            .ok()
            .and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default()
    };
    let mut m = read(shader_dir.join(file));
    for pack in active() {
        for (k, v) in read(pack.dir.join(file)) {
            m.entry(k).or_insert(v);
        }
    }
    m
}

/// Clip folders of the loaded packs (video.rs).
pub fn video_dirs() -> Vec<PathBuf> {
    active().map(|p| p.dir.join("videos")).collect()
}

// ---- installing -------------------------------------------------------------

/// Download / install progress for the Settings card.
pub static STATUS: Mutex<String> = Mutex::new(String::new());
static BUSY: AtomicBool = AtomicBool::new(false);
/// Something was installed or removed this run: restart to load it.
pub static CHANGED: AtomicBool = AtomicBool::new(false);

fn set_status(s: String) {
    *STATUS.lock().unwrap_or_else(|e| e.into_inner()) = s;
}

pub fn busy() -> bool {
    BUSY.load(Ordering::Acquire)
}

/// Only these files are unpacked from a pack.
fn allowed(rel: &Path) -> bool {
    let top = |want: &str, ext: &str| {
        let mut c = rel.components();
        matches!(c.next(), Some(std::path::Component::Normal(d)) if d == want)
            && rel.components().count() == 2
            && rel.extension().is_some_and(|e| e == ext)
    };
    rel == Path::new("pack.json")
        || rel == Path::new("scene_tags.json")
        || rel == Path::new("scene_energy.json")
        || top("scenes", "wgsl")
        || top("videos", "tvid")
}

/// Unpack a `.trippinpack` into `<root>/<id>`, replacing an older copy.
/// Returns the pack's name.
pub fn install_file(zip_path: &Path) -> Result<String> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_path)?).context("not a pack (zip) file")?;
    // The manifest first: it names the folder.
    let m: Manifest = {
        let mut e = zip.by_name("pack.json").map_err(|_| anyhow!("no pack.json in the pack"))?;
        let mut t = String::new();
        std::io::Read::read_to_string(&mut e, &mut t)?;
        serde_json::from_str(t.trim_start_matches('\u{feff}')).context("bad pack.json")?
    };
    if !valid_id(&m.id) {
        bail!("pack id {:?} isn't valid", m.id);
    }
    if m.api > API {
        bail!("{} needs a newer Trippin (pack API {}, this build has {API})", m.name, m.api);
    }
    let root = root();
    std::fs::create_dir_all(&root)?;
    let part = root.join(format!("{}.part", m.id));
    let _ = std::fs::remove_dir_all(&part);
    std::fs::create_dir_all(&part)?;
    let mut scenes = 0;
    for i in 0..zip.len() {
        let mut e = zip.by_index(i)?;
        // enclosed_name rejects absolute paths and `..` (zip-slip).
        let Some(rel) = e.enclosed_name() else { continue };
        if e.is_dir() || !allowed(&rel) {
            continue;
        }
        scenes += rel.extension().is_some_and(|x| x == "wgsl") as usize;
        let out = part.join(&rel);
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut f = std::fs::File::create(&out)?;
        std::io::copy(&mut e, &mut f)?;
    }
    if scenes == 0 {
        let _ = std::fs::remove_dir_all(&part);
        bail!("{} has no scenes", m.name);
    }
    let dest = root.join(&m.id);
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&part, &dest).with_context(|| format!("installing {}", m.name))?;
    forget_thumbs(&dest);
    CHANGED.store(true, Ordering::Release);
    Ok(m.name)
}

/// Thumbnails are cached by scene name: drop a pack's, so an update shows.
fn forget_thumbs(dir: &Path) {
    let thumbs = crate::config::data_dir().join("thumbs");
    for p in wgsl_in(&dir.join("scenes")) {
        let _ = std::fs::remove_file(thumbs.join(format!("scene_{}.png", stem(&p))));
    }
}

/// Install a pack file in the background (Settings > Add pack from file).
pub fn install_file_bg(path: PathBuf) {
    if BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    set_status(format!("installing {}…", path.file_name().unwrap_or_default().to_string_lossy()));
    std::thread::Builder::new()
        .name("pack-install".into())
        .spawn(move || {
            match install_file(&path) {
                Ok(name) => set_status(format!("{name} installed: restart Trippin to load it")),
                Err(e) => set_status(format!("couldn't install: {e:#}")),
            }
            BUSY.store(false, Ordering::Release);
        })
        .ok();
}

/// Download a catalog pack and install it, in the background.
pub fn install_catalog_bg(id: &str) {
    let Some(c) = CATALOG.iter().find(|c| c.id == id) else { return };
    if c.asset.1.is_empty() || BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    let (name, asset) = (c.name, c.asset);
    std::thread::Builder::new()
        .name("pack-dl".into())
        .spawn(move || {
            let r = (|| -> Result<String> {
                let zip = crate::engine::download_verified(
                    &format!("{RELEASE_BASE}/{}", asset.0),
                    asset,
                    &root(),
                    name,
                    &set_status,
                )?;
                let r = install_file(&zip);
                let _ = std::fs::remove_file(&zip);
                r
            })();
            match r {
                Ok(name) => set_status(format!("{name} installed: restart Trippin to load it")),
                Err(e) => set_status(format!("{name}: {e:#}")),
            }
            BUSY.store(false, Ordering::Release);
        })
        .ok();
}

/// Delete an installed pack (takes effect on restart: its files may be open).
pub fn remove(id: &str) -> Result<()> {
    if !valid_id(id) {
        bail!("bad pack id");
    }
    let dir = root().join(id);
    // Rename first: on Windows a clip the decoder holds can't be deleted,
    // but the folder can still move out of the way.
    let gone = root().join(format!("{id}.removed"));
    let _ = std::fs::remove_dir_all(&gone);
    std::fs::rename(&dir, &gone).with_context(|| format!("removing {id}"))?;
    let _ = std::fs::remove_dir_all(&gone);
    CHANGED.store(true, Ordering::Release);
    Ok(())
}

/// Leftovers from an interrupted install or a remove that couldn't finish.
pub fn tidy() {
    let Ok(rd) = std::fs::read_dir(root()) else { return };
    for p in rd.filter_map(|e| e.ok().map(|e| e.path())) {
        if p.extension().is_some_and(|e| e == "part" || e == "removed") {
            let _ = std::fs::remove_dir_all(&p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_pack_files_are_unpacked() {
        for ok in ["pack.json", "scene_tags.json", "scenes/video_x.wgsl", "videos/x.tvid"] {
            assert!(allowed(Path::new(ok)), "{ok}");
        }
        for bad in ["trippin.exe", "scenes/x.exe", "scenes/a/b.wgsl", "videos/x.wgsl", "common.wgsl", "x/pack.json"] {
            assert!(!allowed(Path::new(bad)), "{bad}");
        }
    }

    #[test]
    fn ids_are_folder_safe() {
        assert!(valid_id("night-city"));
        for bad in ["", "..", "a/b", "Night", "a b", "x\\y"] {
            assert!(!valid_id(bad), "{bad:?}");
        }
    }

    /// Every pack in the repo has a manifest, tags for each scene, and
    /// scene names that don't collide with the built-in ones.
    #[test]
    fn repo_packs_are_well_formed() {
        let shaders = crate::render::find_shader_dir().unwrap();
        let base: Vec<String> = wgsl_in(&shaders.join("scenes")).iter().map(|p| stem(p)).collect();
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("packs");
        let mut seen: Vec<String> = Vec::new();
        for p in std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()) {
            let m = read_manifest(&p).unwrap_or_else(|e| panic!("{}: {e:#}", p.display()));
            assert!(m.api <= API);
            let tags: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&std::fs::read_to_string(p.join("scene_tags.json")).unwrap()).unwrap();
            for s in wgsl_in(&p.join("scenes")) {
                let n = stem(&s);
                assert!(!base.contains(&n), "{}: {n} is also built in", m.id);
                assert!(!seen.contains(&n), "{n} is in two packs");
                assert!(tags.contains_key(&n), "{}: {n} has no tags", m.id);
                seen.push(n);
            }
        }
    }
}
