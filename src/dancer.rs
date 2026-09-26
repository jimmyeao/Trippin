//! Silhouette dancer layer: loads rotoscoped mask clips from `dancers/<name>/`
//! (made by tools/roto.py) and maps their loop onto the live beat so the moves
//! land on the music. Rendering happens in `Renderer` via shaders/dancer.wgsl.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::config::Tristate;

/// Largest frame count we upload (wgpu's default texture-array limit is 256).
pub const MAX_FRAMES: usize = 256;

pub const STYLES: [&str; 4] = ["shadow", "neon", "fill", "strobe"];

#[derive(Deserialize)]
struct ClipMeta {
    name: String,
    fps: f32,
    frames: usize,
    beats: f32,
    /// 0 = slow and graceful (breakdowns) .. 1 = energetic (drops).
    #[serde(default = "default_energy")]
    energy: f32,
}

fn default_energy() -> f32 {
    0.5
}

pub struct ClipEntry {
    pub name: String,
    pub path: PathBuf,
    pub energy: f32,
}

pub struct Clip {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Seconds the loop lasts at its original speed.
    pub duration: f32,
    pub beats: f32,
    /// One 8-bit mask per frame, `width * height` bytes each.
    pub frames: Vec<Vec<u8>>,
}

/// Dancer slots: 0 is the main dancer, 1 and 2 the canon companions, each
/// with its own routine so they never just copy each other.
pub const SLOTS: usize = 3;

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SlotUniforms {
    /// Fractional frame index into this slot's loop.
    pub frame: f32,
    /// Frame count; 0 = nothing loaded in this slot yet.
    pub frames: f32,
    /// Mask width / height.
    pub aspect: f32,
    pub _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DancerUniforms {
    pub slots: [SlotUniforms; SLOTS],
    pub opacity: f32,
    pub style: f32,
    /// Number of dancers: 1, or 3 for the canon.
    pub count: f32,
    /// Main dancer height as a fraction of the screen height.
    pub scale: f32,
}

pub fn find_dancer_dir() -> Option<PathBuf> {
    let mut candidates = vec![PathBuf::from("dancers")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("dancers"));
        }
    }
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("dancers"));
    candidates.into_iter().find(|p| p.is_dir())
}

pub fn list_clips(dir: &Path) -> Vec<ClipEntry> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let meta: ClipMeta = serde_json::from_str(&std::fs::read_to_string(path.join("clip.json")).ok()?).ok()?;
            let name = path.file_name()?.to_string_lossy().into_owned();
            Some(ClipEntry { name, path, energy: meta.energy })
        })
        .collect()
}

pub fn load_clip(dir: &Path) -> Result<Clip> {
    let meta: ClipMeta = serde_json::from_str(&std::fs::read_to_string(dir.join("clip.json"))?)
        .with_context(|| format!("parsing {}", dir.join("clip.json").display()))?;
    if meta.frames == 0 || meta.fps <= 0.0 || meta.beats <= 0.0 {
        return Err(anyhow!("{}: clip.json needs frames, fps and beats > 0", dir.display()));
    }
    // Evenly subsample overlong clips rather than failing.
    let keep = meta.frames.min(MAX_FRAMES);
    let mut frames = Vec::with_capacity(keep);
    let (mut width, mut height) = (0, 0);
    for k in 0..keep {
        let i = k * meta.frames / keep;
        let path = dir.join("frames").join(format!("{i:04}.png"));
        let img = image::open(&path).with_context(|| format!("loading {}", path.display()))?.into_luma8();
        if k == 0 {
            (width, height) = img.dimensions();
        } else if img.dimensions() != (width, height) {
            return Err(anyhow!("{}: frame sizes differ", path.display()));
        }
        frames.push(img.into_raw());
    }
    Ok(Clip {
        name: meta.name,
        width,
        height,
        duration: meta.frames as f32 / meta.fps,
        beats: meta.beats,
        frames,
    })
}

/// Choose how many live beats one loop spans (the clip's own count, halved or
/// doubled) so playback stays as close to its original speed as possible.
pub fn loop_beats(duration: f32, beats: f32, bpm: f32) -> f32 {
    let off_speed = |b: f32| (duration / (b * 60.0 / bpm)).ln().abs();
    [0.5, 1.0, 2.0, 4.0]
        .iter()
        .map(|m| beats * m)
        .filter(|b| *b >= 1.0)
        .min_by(|a, b| off_speed(*a).total_cmp(&off_speed(*b)))
        .unwrap_or(beats)
}

/// What the layer needs to know about the clip currently on the GPU.
#[derive(Clone)]
pub struct ClipInfo {
    pub name: String,
    pub duration: f32,
    pub beats: f32,
    pub frames: usize,
    pub aspect: f32,
}

/// One dancer: which routine it shows, its background load, and its tempo mapping.
#[derive(Default)]
struct Slot {
    current: Option<usize>,
    loaded: Option<ClipInfo>,
    loader: Option<mpsc::Receiver<Result<Clip>>>,
    loop_beats: f32,
    loop_bpm: f32,
}

impl Slot {
    /// Frame uniforms for `pos`, re-picking half/double time only when the
    /// tempo really moves so the dancer doesn't jump on BPM jitter.
    fn uniforms(&mut self, pos: f64, downbeat: u64, bpm: f32) -> SlotUniforms {
        let Some(info) = self.loaded.as_ref() else { return SlotUniforms::default() };
        if (bpm - self.loop_bpm).abs() / bpm.max(1.0) > 0.05 {
            self.loop_bpm = bpm;
            self.loop_beats = loop_beats(info.duration, info.beats, bpm);
        }
        // Frame 0 of the clip sits on the downbeat.
        let t = ((pos - downbeat as f64) / self.loop_beats.max(1.0) as f64).rem_euclid(1.0);
        SlotUniforms { frame: (t * info.frames as f64) as f32, frames: info.frames as f32, aspect: info.aspect, _pad: 0.0 }
    }
}

/// Owns the clip list and the background loaders; the renderer owns the GPU copies.
pub struct DancerLayer {
    /// User switch for the whole layer.
    pub enabled: bool,
    /// Auto-pilot's choice of whether the dancer is on screen right now.
    pub showing: bool,
    pub style: usize,
    pub canon: bool,
    pub clips: Vec<ClipEntry>,
    slots: [Slot; SLOTS],
    opacity: f32,
    rng: u64,
}

impl DancerLayer {
    pub fn new() -> Self {
        let clips = find_dancer_dir().map(|d| list_clips(&d)).unwrap_or_default();
        Self {
            enabled: true,
            showing: true,
            style: 0,
            canon: false,
            clips,
            slots: Default::default(),
            opacity: 0.0,
            rng: 0x9E37_79B9_7F4A_7C15,
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    /// The main dancer's routine index.
    pub fn current(&self) -> Option<usize> {
        self.slots[0].current
    }

    pub fn loaded_name(&self) -> Option<String> {
        self.slots[0].loaded.as_ref().map(|c| c.name.clone())
    }

    /// Start loading routine `index` into `slot` on a background thread.
    fn request_slot(&mut self, slot: usize, index: usize) {
        let Some(path) = self.clips.get(index).map(|c| c.path.clone()) else { return };
        self.slots[slot].current = Some(index);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(load_clip(&path));
        });
        self.slots[slot].loader = Some(rx);
    }

    /// Show routine `index` on the main dancer.
    pub fn request(&mut self, index: usize) {
        self.request_slot(0, index);
    }

    pub fn next_clip(&mut self) {
        if !self.clips.is_empty() {
            let next = self.current().map_or(0, |c| (c + 1) % self.clips.len());
            self.request(next);
        }
    }

    pub fn clip_names(&self) -> Vec<String> {
        self.clips.iter().map(|c| c.name.clone()).collect()
    }

    /// Give each canon companion a routine that differs from the main dancer
    /// and from the other companion (keeping any that already do).
    fn refresh_companions(&mut self, disabled: &[String]) {
        for slot in 1..SLOTS {
            let taken: Vec<usize> =
                (0..SLOTS).filter(|&s| s != slot).filter_map(|s| self.slots[s].current).collect();
            let mine = self.slots[slot].current;
            if mine.is_some_and(|m| !taken.contains(&m) && !disabled.contains(&self.clips[m].name)) {
                continue;
            }
            let free: Vec<usize> = (0..self.clips.len())
                .filter(|i| !taken.contains(i) && !disabled.contains(&self.clips[*i].name))
                .collect();
            // Too few ticked routines to go round: at least differ from the main dancer.
            let pool: Vec<usize> = if free.is_empty() {
                (0..self.clips.len()).filter(|i| Some(*i) != self.slots[0].current).collect()
            } else {
                free
            };
            if !pool.is_empty() {
                let pick = pool[(self.rand() * pool.len() as f32) as usize % pool.len()];
                self.request_slot(slot, pick);
            }
        }
    }

    /// Load a routine whose energy suits the track: one of the three closest
    /// matches other than the current one (skipping `disabled` ones), chosen
    /// by `r` in 0..1.
    pub fn pick_for(&mut self, intensity: f32, r: f32, disabled: &[String]) {
        let current = self.current();
        let mut order: Vec<usize> = (0..self.clips.len())
            .filter(|&i| Some(i) != current && !disabled.contains(&self.clips[i].name))
            .collect();
        order.sort_by(|&a, &b| {
            let d = |i: usize| (self.clips[i].energy - intensity).abs();
            d(a).total_cmp(&d(b))
        });
        order.truncate(3);
        if !order.is_empty() {
            self.request(order[(r * order.len() as f32) as usize % order.len()]);
        }
    }

    /// Auto-pilot's reaction to a scene cut or phrase. `rand` yields values in
    /// 0..1. A fixed `style` or a non-auto `canon` from the settings wins.
    pub fn on_cut(
        &mut self,
        intensity: f32,
        mut rand: impl FnMut() -> f32,
        style: Option<usize>,
        canon: Tristate,
        disabled: &[String],
    ) {
        // On screen most of the time, and always through breakdowns.
        self.showing = intensity < 0.35 || rand() < 0.7;
        // Mostly the classic black shadow; strobe only when the track drives.
        let r = rand();
        self.style = if let Some(s) = style {
            s
        } else if r < 0.45 {
            0
        } else if r < 0.7 {
            2
        } else if r < 0.9 || intensity < 0.6 {
            1
        } else {
            3
        };
        self.canon = match canon {
            Tristate::Auto => intensity > 0.6 && rand() < 0.6,
            Tristate::On => true,
            Tristate::Off => false,
        };
        let current = self.current().and_then(|i| self.clips.get(i));
        let current_ok = current.is_some_and(|c| !disabled.contains(&c.name));
        let energy = current.map_or(0.5, |c| c.energy);
        if !current_ok || (energy - intensity).abs() > 0.4 || rand() < 0.3 {
            let r = rand();
            self.pick_for(intensity, r, disabled);
        }
        // Fresh companions each time the canon comes in, so the trio keeps changing.
        if self.canon {
            for slot in 1..SLOTS {
                self.slots[slot].current = None;
            }
            self.refresh_companions(disabled);
        }
    }

    /// Finished background loads, ready to upload: (slot, clip).
    pub fn poll_loaded(&mut self) -> Vec<(usize, Clip)> {
        let mut done = Vec::new();
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let Some(result) = slot.loader.as_ref().and_then(|rx| rx.try_recv().ok()) else { continue };
            slot.loader = None;
            match result {
                Ok(clip) => {
                    println!("dancer {i}: {} ({} frames)", clip.name, clip.frames.len());
                    slot.loaded = Some(ClipInfo {
                        name: clip.name.clone(),
                        duration: clip.duration,
                        beats: clip.beats,
                        frames: clip.frames.len(),
                        aspect: clip.width as f32 / clip.height as f32,
                    });
                    slot.loop_bpm = 0.0;
                    done.push((i, clip));
                }
                Err(e) => eprintln!("dancer clip failed: {e:#}"),
            }
        }
        done
    }

    /// Per-frame uniforms, or None when nothing should be drawn.
    pub fn uniforms(
        &mut self,
        pos: f64,
        downbeat: u64,
        bpm: f32,
        dt: f32,
        size: f32,
        disabled: &[String],
    ) -> Option<DancerUniforms> {
        let target = if self.enabled && self.showing { 1.0 } else { 0.0 };
        self.opacity += (target - self.opacity) * (dt * 4.0).min(1.0);
        self.slots[0].loaded.as_ref()?;
        if self.opacity < 0.01 {
            return None;
        }
        // Canon switched on by hand, or the main routine changed to one a
        // companion was showing: make sure all three dance something different.
        if self.canon && self.slots[1..].iter().all(|s| s.loader.is_none()) {
            let clash = (1..SLOTS).any(|s| {
                let c = self.slots[s].current;
                c.is_none() || (0..s).any(|o| self.slots[o].current == c)
            });
            if clash {
                self.refresh_companions(disabled);
            }
        }
        let mut slots = [SlotUniforms::default(); SLOTS];
        for (i, slot) in self.slots.iter_mut().enumerate() {
            slots[i] = slot.uniforms(pos, downbeat, bpm);
        }
        Some(DancerUniforms {
            slots,
            opacity: self.opacity,
            style: self.style as f32,
            count: if self.canon { 3.0 } else { 1.0 },
            scale: size,
        })
    }
}
