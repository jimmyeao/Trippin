//! The external Unity engine (unity/TrippinStage): Trippin starts it, keeps
//! it running and reads its frames — the user never launches anything.
//!
//! - **Process:** when `Settings::unity_link` is on, the player is spawned
//!   headless (`-batchmode`, renders offscreen) with the frame-file path and
//!   the feed port on its command line; it's restarted if it exits and
//!   killed when the link goes off or Trippin quits. The player quits by
//!   itself if Trippin's feed stops (so a crashed Trippin leaves no orphan).
//! - **Frames:** a shared-memory file (memory-mapped on both Windows and
//!   macOS — no Spout/Syphon) that Unity fills via async GPU readback:
//!   a 64-byte header then RGBA8 pixels at `EXT_W` x `EXT_H`. The header's
//!   `seq` is odd while Unity writes and bumps by 2 per frame, so a reader
//!   that sees the same even `seq` before and after its copy has a clean
//!   frame (a seqlock). A reader thread copies new frames out so the render
//!   thread only uploads.
//! - **Feed:** `link.rs` sends the show state over UDP to the port passed.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use memmap2::MmapMut;

use crate::gfx::{EXT_H, EXT_W};

const MAGIC: u32 = 0x4654_5254; // "TRTF"
const HEADER: usize = 64;

/// The latest frame from the engine, RGBA8 at `EXT_W` x `EXT_H`.
pub struct InFrame {
    pub seq: u64,
    pub rgba: Arc<Vec<u8>>,
    /// When it arrived — the source counts as gone after a short silence.
    pub at: Instant,
}

/// Where the player executable lives: bundled next to Trippin (installer /
/// .app), downloaded into the data dir, or the dev build in the repo.
pub fn player_path() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let data = crate::config::data_dir().join("unity");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("unity").join("TrippinStage");
    let mut c: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        let app = |d: &Path| d.join("TrippinStage.app/Contents/MacOS/Trippin Stage");
        c.push(app(&exe_dir.join("../Resources/unity")));
        c.push(app(&data));
        c.push(app(&repo.join("BuildMac")));
    } else {
        c.push(exe_dir.join("unity").join("TrippinStage.exe"));
        c.push(data.join("TrippinStage.exe"));
        c.push(repo.join("Build").join("TrippinStage.exe"));
        c.push(repo.join("Build2").join("TrippinStage.exe"));
    }
    c.into_iter().find(|p| p.is_file())
}

/// Supervises the engine process and owns the frame file + reader thread.
pub struct Engine {
    child: Option<Child>,
    started: Instant,
    restarts: u32,
    path: PathBuf,
    port: u16,
    pub latest: Arc<Mutex<Option<InFrame>>>,
    stop: Arc<AtomicBool>,
    /// Why the engine isn't running, for the Settings tab.
    pub status: String,
}

impl Engine {
    /// Create the frame file and start the reader thread; the process is
    /// spawned by `tick`.
    pub fn new(port: u16) -> Result<Engine> {
        let path = std::env::temp_dir().join(format!("trippin-engine-{}.frame", std::process::id()));
        let len = HEADER + (EXT_W * EXT_H * 4) as usize;
        let f = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .with_context(|| format!("creating {}", path.display()))?;
        f.set_len(len as u64)?;
        let mut map = unsafe { MmapMut::map_mut(&f)? };
        map[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        map[4..8].copy_from_slice(&1u32.to_le_bytes());
        map[8..12].copy_from_slice(&EXT_W.to_le_bytes());
        map[12..16].copy_from_slice(&EXT_H.to_le_bytes());
        map[16..24].copy_from_slice(&0u64.to_le_bytes());
        let latest = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (l, s) = (latest.clone(), stop.clone());
        let _ = std::thread::Builder::new().name("engine-frames".into()).spawn(move || {
            read_loop(map, l, s);
        });
        Ok(Engine {
            child: None,
            started: Instant::now(),
            restarts: 0,
            path,
            port,
            latest,
            stop,
            status: "starting".into(),
        })
    }

    /// Keep the player running: spawn it, notice when it exits, restart it
    /// (backing off so a player that crashes on launch doesn't spin).
    pub fn tick(&mut self) {
        if let Some(c) = self.child.as_mut() {
            match c.try_wait() {
                Ok(None) => return,
                Ok(Some(st)) => {
                    eprintln!("unity engine exited ({st}); restarting");
                    self.child = None;
                    self.restarts += 1;
                }
                Err(e) => {
                    self.status = format!("{e}");
                    self.child = None;
                }
            }
        }
        let backoff = Duration::from_secs((2u64 << self.restarts.min(5)).min(60));
        if self.restarts > 0 && self.started.elapsed() < backoff {
            return;
        }
        match self.spawn() {
            Ok(c) => {
                self.child = Some(c);
                self.started = Instant::now();
                self.status = "running".into();
            }
            Err(e) => {
                self.status = format!("{e:#}");
                self.started = Instant::now();
                self.restarts += 1;
            }
        }
    }

    fn spawn(&self) -> Result<Child> {
        let exe = player_path().ok_or_else(|| anyhow!("Unity engine not installed"))?;
        let log = crate::config::data_dir().join("unity-engine.log");
        Command::new(&exe)
            .arg("-batchmode")
            .arg("-trippinFrame")
            .arg(&self.path)
            .arg("-trippinPort")
            .arg(self.port.to_string())
            .arg("-logFile")
            .arg(&log)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("starting {}", exe.display()))
    }

    /// Frames arrived within the last 1.5 s.
    pub fn live(&self) -> bool {
        self.latest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|f| f.at.elapsed() < Duration::from_millis(1500))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        // The reader thread holds the mapping; the file goes once it's
        // unmapped (Windows refuses to delete a mapped file — retry later).
        let p = self.path.clone();
        std::thread::spawn(move || {
            for _ in 0..20 {
                std::thread::sleep(Duration::from_millis(100));
                if std::fs::remove_file(&p).is_ok() {
                    break;
                }
            }
        });
    }
}

/// Poll the frame file; copy out each new, complete frame.
fn read_loop(map: MmapMut, latest: Arc<Mutex<Option<InFrame>>>, stop: Arc<AtomicBool>) {
    let len = (EXT_W * EXT_H * 4) as usize;
    let seq_at = |m: &MmapMut| {
        // Volatile read: the other process writes this behind our back.
        unsafe { std::ptr::read_volatile(m.as_ptr().add(16) as *const u64) }
    };
    let mut last = 0u64;
    while !stop.load(Ordering::Relaxed) {
        let s1 = seq_at(&map);
        if s1 == last || s1 % 2 == 1 {
            std::thread::sleep(Duration::from_millis(3));
            continue;
        }
        std::sync::atomic::fence(Ordering::Acquire);
        let px = map[HEADER..HEADER + len].to_vec();
        std::sync::atomic::fence(Ordering::Acquire);
        if seq_at(&map) != s1 {
            continue; // torn: Unity wrote while we copied
        }
        last = s1;
        *latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(InFrame {
            seq: s1,
            rgba: Arc::new(px),
            at: Instant::now(),
        });
    }
}
