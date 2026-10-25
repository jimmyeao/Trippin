//! Video clip scenes. A scene named `video_<clip>` plays `<clip>.tvid`: a
//! short loop (generated offline, packed by `tools/video_pack.py`) stored as
//! JPEG frames plus an index, so playback needs no codec or ffmpeg on the
//! user's machine. The clips ship in effects packs (packs.rs) beside their
//! scenes, and a `video_*` scene is only in rotation while its clip is on
//! disk (`scene_ready`).
//!
//! Playback never jumps with the audio: the rate follows a smoothed drive
//! (`Player::tick`), and the shader blends the two frames around the play
//! head (`vid()` in common.wgsl), so slow rates stay fluid instead of
//! stepping.
//!
//! `.tvid` layout (little-endian): `TVID`, u32 version (1), u32 width,
//! u32 height, u32 frames, f32 fps, then `frames` x (u32 offset, u32 len)
//! into the file, then the JPEG data.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};

use crate::gfx::{VID_H, VID_W};

/// Clip names found on disk (refreshed by `rescan`).
static AVAILABLE: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// One packed clip, held in memory (a few MB of JPEG).
pub struct Clip {
    pub fps: f32,
    index: Vec<(u32, u32)>,
    data: Vec<u8>,
}

impl Clip {
    pub fn open(path: &Path) -> Result<Clip> {
        let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let u = |o: usize| -> Result<u32> {
            data.get(o..o + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or_else(|| anyhow!("truncated header"))
        };
        if data.get(0..4) != Some(b"TVID".as_slice()) || u(4)? != 1 {
            bail!("{}: not a v1 .tvid", path.display());
        }
        let (w, h, n) = (u(8)?, u(12)?, u(16)? as usize);
        let fps = f32::from_bits(u(20)?);
        if (w, h) != (VID_W, VID_H) || n == 0 || !(1.0..=120.0).contains(&fps) {
            bail!("{}: {w}x{h}, {n} frames at {fps} fps", path.display());
        }
        let mut index = Vec::with_capacity(n);
        for i in 0..n {
            let (off, len) = (u(24 + i * 8)?, u(28 + i * 8)?);
            if off as usize + len as usize > data.len() {
                bail!("{}: frame {i} out of range", path.display());
            }
            index.push((off, len));
        }
        Ok(Clip { fps, index, data })
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Frame `i` as RGBA8, VID_W x VID_H.
    pub fn decode(&self, i: usize) -> Result<Vec<u8>> {
        let (off, len) = self.index[i % self.index.len()];
        let jpg = &self.data[off as usize..(off + len) as usize];
        let img = image::load_from_memory_with_format(jpg, image::ImageFormat::Jpeg)?.to_rgba8();
        if img.dimensions() != (VID_W, VID_H) {
            bail!("frame {i} is {:?}", img.dimensions());
        }
        Ok(img.into_raw())
    }
}

/// Folders searched for `<clip>.tvid`: `TRIPPIN_VIDEOS`, then each loaded
/// pack's `videos/`.
fn dirs() -> Vec<PathBuf> {
    let mut d = Vec::new();
    if let Some(p) = std::env::var_os("TRIPPIN_VIDEOS") {
        d.push(PathBuf::from(p));
    }
    d.extend(crate::packs::video_dirs());
    d
}

pub fn clip_path(name: &str) -> Option<PathBuf> {
    dirs().into_iter().map(|d| d.join(format!("{name}.tvid"))).find(|p| p.is_file())
}

/// Re-list the clips on disk (startup, and after a download).
pub fn rescan() {
    let mut names: Vec<String> = dirs()
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.extension()? == "tvid").then(|| p.file_stem()?.to_str().map(str::to_string))?
        })
        .collect();
    names.sort();
    names.dedup();
    *lock(&AVAILABLE) = names;
}

/// Is the clip for scene `video_<clip>` on disk? Non-video scenes: true.
pub fn scene_ready(scene: &str) -> bool {
    match scene.strip_prefix("video_") {
        Some(clip) => lock(&AVAILABLE).iter().any(|c| c == clip),
        None => true,
    }
}

/// What the decoder has ready for the clip on air.
#[derive(Default)]
struct Ready {
    clip: String,
    frames: usize,
    fps: f32,
    decoded: HashMap<usize, Arc<Vec<u8>>>,
}

#[derive(Default)]
struct Want {
    clip: String,
    frame: usize,
    stop: bool,
}

/// Plays the on-air `video_*` scene: advances the play head, asks the
/// decoder thread for the frames around it, and hands the render loop the
/// frames to upload plus the blend weight for `vid()`.
pub struct Player {
    want: Arc<(Mutex<Want>, Condvar)>,
    ready: Arc<Mutex<Ready>>,
    clip: String,
    /// Play head in seconds of clip time.
    pos: f64,
    rate: f32,
    /// Which frame each slot of `vid_tex` holds.
    slots: [Option<usize>; 2],
    /// Weight of slot 1 in `vid()` (u.misc4.y).
    pub blend: f32,
}

impl Player {
    pub fn new() -> Player {
        let want = Arc::new((Mutex::new(Want::default()), Condvar::new()));
        let ready = Arc::new(Mutex::new(Ready::default()));
        let (w, r) = (want.clone(), ready.clone());
        std::thread::Builder::new()
            .name("video".into())
            .spawn(move || decode_loop(&w, &r))
            .ok();
        Player { want, ready, clip: String::new(), pos: 0.0, rate: 1.0, slots: [None; 2], blend: 0.0 }
    }

    /// Something else drew into `vid_tex` (a panel thumbnail): re-upload.
    pub fn invalidate(&mut self) {
        self.slots = [None; 2];
    }

    /// Advance playback for `scene` and return the frames to upload as
    /// (slot, rgba). `drive` is the smoothed intensity (0..1), `calm` the
    /// breakdown amount. Nothing happens unless a `video_*` scene is on air.
    pub fn tick(&mut self, scene: &str, dt: f32, drive: f32, calm: f32) -> Vec<(u32, Arc<Vec<u8>>)> {
        let Some(clip) = scene.strip_prefix("video_") else { return Vec::new() };
        if clip != self.clip {
            self.clip = clip.to_string();
            self.pos = 0.0;
            self.slots = [None; 2];
        }
        // Speed follows the track's drive, eased over ~1.5 s: slow and
        // floaty in breakdowns, up to 1.35x on a drop. Never raw audio.
        let target = (0.55 + 0.8 * drive.clamp(0.0, 1.0)) * (1.0 - 0.35 * calm.clamp(0.0, 1.0));
        self.rate += (target - self.rate) * (1.0 - (-dt / 1.5).exp());
        self.pos += (dt * self.rate) as f64;

        let r = lock(&self.ready);
        if r.clip != self.clip || r.frames == 0 {
            drop(r);
            self.request(0);
            return Vec::new();
        }
        let n = r.frames;
        let fpos = self.pos * r.fps as f64;
        let f0 = fpos.floor() as usize % n;
        let f1 = (f0 + 1) % n;
        let frac = fpos.fract() as f32;
        let mut up = Vec::new();
        for f in [f0, f1] {
            if self.slots.contains(&Some(f)) {
                continue;
            }
            let Some(px) = r.decoded.get(&f) else { continue };
            // Overwrite a slot that holds neither frame we need.
            let s = (0..2).find(|&s| self.slots[s] != Some(f0) && self.slots[s] != Some(f1)).unwrap_or(0);
            self.slots[s] = Some(f);
            up.push((s as u32, px.clone()));
        }
        drop(r);
        self.blend = match (self.slots[0] == Some(f0), self.slots[1] == Some(f0)) {
            (true, _) if self.slots[1] == Some(f1) => frac,
            (_, true) if self.slots[0] == Some(f1) => 1.0 - frac,
            (true, _) => 0.0,
            (_, true) => 1.0,
            // Not decoded yet (just cut to the clip): hold what's there.
            _ => self.blend,
        };
        self.request(f0);
        up
    }

    fn request(&self, frame: usize) {
        let (m, cv) = &*self.want;
        let mut w = lock(m);
        if w.clip != self.clip || w.frame != frame {
            w.clip = self.clip.clone();
            w.frame = frame;
            cv.notify_one();
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let (m, cv) = &*self.want;
        lock(m).stop = true;
        cv.notify_one();
    }
}

/// Decoder thread: keeps the wanted frame and the next two decoded, drops
/// the rest. Loading a clip reads the whole file (a few MB).
fn decode_loop(want: &(Mutex<Want>, Condvar), ready: &Mutex<Ready>) {
    let (m, cv) = want;
    let mut clip: Option<(String, Clip)> = None;
    loop {
        let (name, frame) = {
            let mut w = lock(m);
            loop {
                if w.stop {
                    return;
                }
                if !w.clip.is_empty() {
                    let have = lock(ready);
                    let n = have.frames.max(1);
                    let missing = have.clip != w.clip
                        || (0..3).any(|k| !have.decoded.contains_key(&((w.frame + k) % n)));
                    drop(have);
                    if missing {
                        break;
                    }
                }
                w = cv.wait_timeout(w, Duration::from_millis(250)).unwrap_or_else(|p| p.into_inner()).0;
            }
            (w.clip.clone(), w.frame)
        };
        if clip.as_ref().is_none_or(|(n, _)| *n != name) {
            let loaded = clip_path(&name).ok_or_else(|| anyhow!("no clip {name}")).and_then(|p| Clip::open(&p));
            match loaded {
                Ok(c) => {
                    *lock(ready) = Ready { clip: name.clone(), frames: c.len(), fps: c.fps, decoded: HashMap::new() };
                    clip = Some((name.clone(), c));
                }
                Err(e) => {
                    eprintln!("video: {e:#}");
                    // Don't spin on a broken clip: wait for a different want.
                    let mut w = lock(m);
                    while w.clip == name && !w.stop {
                        w = cv.wait(w).unwrap_or_else(|p| p.into_inner());
                    }
                    continue;
                }
            }
        }
        let Some((_, c)) = clip.as_ref() else { continue };
        let n = c.len();
        for k in 0..3 {
            let f = (frame + k) % n;
            if lock(ready).decoded.contains_key(&f) {
                continue;
            }
            match c.decode(f) {
                Ok(px) => {
                    let mut r = lock(ready);
                    if r.clip == name {
                        r.decoded.insert(f, Arc::new(px));
                    }
                }
                Err(e) => {
                    eprintln!("video: {name} frame {f}: {e:#}");
                    break;
                }
            }
            // A newer request (a seek, or the next frame already) wins.
            if lock(m).frame != frame {
                break;
            }
        }
        let keep = |f: usize| (0..4).any(|k| (frame + n - 1 + k) % n == f);
        lock(ready).decoded.retain(|&f, _| keep(f));
    }
}

/// `--snap` / thumbnails: the two frames around clip time `t` (rate 1) and
/// the blend weight, decoded synchronously.
pub fn frames_at(name: &str, t: f64) -> Option<(Vec<u8>, Vec<u8>, f32)> {
    let c = Clip::open(&clip_path(name)?).ok()?;
    let fpos = t * c.fps as f64;
    let f0 = fpos.floor() as usize % c.len();
    Some((c.decode(f0).ok()?, c.decode(f0 + 1).ok()?, fpos.fract() as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_clip(path: &Path, frames: usize) {
        let mut jpgs = Vec::new();
        for i in 0..frames {
            let img = image::RgbImage::from_pixel(VID_W, VID_H, image::Rgb([(i * 40) as u8, 0, 0]));
            let mut b = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut b, 80).encode_image(&img).unwrap();
            jpgs.push(b);
        }
        let mut out = b"TVID".to_vec();
        for v in [1u32, VID_W, VID_H, frames as u32, 24.0f32.to_bits()] {
            out.extend(v.to_le_bytes());
        }
        let mut off = (24 + frames * 8) as u32;
        for j in &jpgs {
            out.extend(off.to_le_bytes());
            out.extend((j.len() as u32).to_le_bytes());
            off += j.len() as u32;
        }
        for j in &jpgs {
            out.extend(j);
        }
        std::fs::write(path, out).unwrap();
    }

    #[test]
    fn a_packed_clip_reads_back_frame_by_frame() {
        let dir = std::env::temp_dir().join(format!("tvid-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.tvid");
        write_clip(&p, 3);
        let c = Clip::open(&p).unwrap();
        assert_eq!((c.len(), c.fps), (3, 24.0));
        let f = c.decode(2).unwrap();
        assert_eq!(f.len(), (VID_W * VID_H * 4) as usize);
        assert!((f[0] as i32 - 80).abs() < 8, "red {}", f[0]);
        // Wraps rather than panicking past the end.
        assert!(c.decode(4).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_video_scenes_are_always_ready() {
        assert!(scene_ready("tunnel"));
        assert!(!scene_ready("video_definitely_not_a_clip"));
    }
}
