//! Finding a track's cover on this machine: an image file as it is, an
//! audio file's embedded front cover (ID3 APIC, FLAC PICTURE, MP4 `covr`,
//! via `lofty`), else a `cover`/`folder`/`front` image beside it.
//!
//! Shared by the app (`albumart.rs`) and the LAN agent (`agent/`, through a
//! `#[path]` include, as it shares `nowplaying.rs`): the agent finds the
//! cover on the DJ machine, whose files the rig can't read, and sends it.
//! So this file must depend on nothing in the app but `lofty` and std.

use std::path::{Path, PathBuf};

fn is_image(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["jpg", "jpeg", "png"].contains(&e.to_ascii_lowercase().as_str()))
}

/// The first of `files` that gives a cover, and where it was found.
pub fn from_files(files: &[PathBuf]) -> Option<(Vec<u8>, String)> {
    files.iter().find_map(|f| from_file(f))
}

/// An image file as it is; an audio file's embedded cover, else a cover
/// image in its folder.
pub fn from_file(p: &Path) -> Option<(Vec<u8>, String)> {
    if !p.is_file() {
        return None;
    }
    if is_image(p) {
        return std::fs::read(p).ok().map(|b| (b, format!("image {}", p.display())));
    }
    if let Some(b) = embedded(p) {
        return Some((b, format!("embedded in {}", p.display())));
    }
    let dir = p.parent()?;
    for stem in ["cover", "folder", "front", "Cover", "Folder", "Front"] {
        for ext in ["jpg", "jpeg", "png"] {
            let f = dir.join(format!("{stem}.{ext}"));
            if let Ok(b) = std::fs::read(&f) {
                return Some((b, format!("image {}", f.display())));
            }
        }
    }
    None
}

/// The embedded front cover (any picture if none is marked front).
pub fn embedded(p: &Path) -> Option<Vec<u8>> {
    use lofty::file::TaggedFileExt;
    use lofty::picture::PictureType;
    let tf = lofty::read_from_path(p).ok()?;
    let mut best: Option<&lofty::picture::Picture> = None;
    for tag in tf.tags() {
        for pic in tag.pictures() {
            if pic.pic_type() == PictureType::CoverFront {
                return Some(pic.data().to_vec());
            }
            best = best.or(Some(pic));
        }
    }
    best.map(|p| p.data().to_vec())
}
