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

/// The running player. Global rather than in `Engine` because `Engine`
/// lives on the render thread, which isn't unwound on quit (macOS Cmd-Q
/// never returns from the event loop) — `shutdown` kills it from there.
static CHILD: Mutex<Option<Child>> = Mutex::new(None);
static QUITTING: AtomicBool = AtomicBool::new(false);

fn child() -> std::sync::MutexGuard<'static, Option<Child>> {
    CHILD.lock().unwrap_or_else(|p| p.into_inner())
}

/// Kill the player now. Called when the app exits; harmless if it isn't
/// running.
pub fn shutdown() {
    // Latched first, so the render thread's tick can't respawn it while the
    // app is going down.
    QUITTING.store(true, Ordering::Relaxed);
    kill_child();
}

fn kill_child() {
    if let Some(mut c) = child().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

/// Prebuilt players, downloaded on first use (like the Beat This! model) so
/// installers stay small and CI needs no Unity licence. Rebuild + upload a
/// new `unity-engine-vN` release when the Unity project changes, and bump
/// these. An empty checksum means no build for this platform yet.
const RELEASE_BASE: &str = "https://github.com/jimmyeao/Trippin/releases/download/unity-engine-v2";
#[cfg(target_os = "macos")]
const ASSET: (&str, &str) = (
    "TrippinEngine-macos-v2.zip",
    "3bcd269755768ac52522f4cfb8dacd0b78cf8ef5a787cdc09a491368c0af9a8a",
);
#[cfg(not(target_os = "macos"))]
const ASSET: (&str, &str) = (
    "TrippinEngine-windows-x64-v2.zip",
    "3d3c1a4bda0827ce998120b0647ec005488dc71c10955f9b4e312d7bc84bbbdb",
);
/// Written into the unpacked folder; a download whose stamp isn't the
/// current `ASSET` is stale and gets replaced.
const STAMP: &str = "ENGINE_VERSION";
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
    // TRIPPIN_ENGINE_FRESH=1: only the downloaded copy — tests the download.
    if std::env::var_os("TRIPPIN_ENGINE_FRESH").is_some() {
        c.retain(|p| p.starts_with(&data));
    }
    // An older download (or one without a stamp) counts as missing, so
    // tick() fetches the current release over it.
    let current = std::fs::read_to_string(data.join(STAMP)).is_ok_and(|s| s.trim() == ASSET.0);
    if !current {
        c.retain(|p| !p.starts_with(&data));
    }
    c.into_iter().find(|p| p.is_file())
}

/// Supervises the engine process and owns the frame file + reader thread.
pub struct Engine {
    started: Instant,
    restarts: u32,
    path: PathBuf,
    port: u16,
    pub latest: Arc<Mutex<Option<InFrame>>>,
    stop: Arc<AtomicBool>,
    /// Why the engine isn't running, for the Settings tab.
    pub status: String,
    /// Download progress / result while the player is being fetched.
    download: Option<Arc<Mutex<Result<String, String>>>>,
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
            started: Instant::now(),
            restarts: 0,
            path,
            port,
            latest,
            stop,
            status: "starting".into(),
            download: None,
        })
    }

    /// Keep the player running: spawn it, notice when it exits, restart it
    /// (backing off so a player that crashes on launch doesn't spin).
    pub fn tick(&mut self) {
        if QUITTING.load(Ordering::Relaxed) {
            return;
        }
        {
            let mut g = child();
            if let Some(c) = g.as_mut() {
                match c.try_wait() {
                    Ok(None) => return,
                    Ok(Some(st)) => {
                        eprintln!("unity engine exited ({st}); restarting");
                        *g = None;
                        self.restarts += 1;
                    }
                    Err(e) => {
                        self.status = format!("{e}");
                        *g = None;
                    }
                }
            }
        }
        // No player yet: fetch it once (background thread), then spawn.
        if player_path().is_none() {
            let d = self
                .download
                .get_or_insert_with(|| {
                    let st = Arc::new(Mutex::new(Ok("downloading…".to_string())));
                    let s2 = st.clone();
                    std::thread::spawn(move || {
                        let r = std::panic::catch_unwind(|| download(&s2))
                            .unwrap_or_else(|_| Err(anyhow!("download crashed")));
                        if let Err(e) = r {
                            *s2.lock().unwrap_or_else(|p| p.into_inner()) = Err(format!("{e:#}"));
                        }
                    });
                    st
                })
                .clone();
            let st = d.lock().unwrap_or_else(|p| p.into_inner()).clone();
            match st {
                Ok(s) => self.status = s,
                Err(e) => {
                    // Failed: retry after the backoff.
                    self.status = format!("download failed: {e}");
                    if self.started.elapsed() > Duration::from_secs(60) {
                        self.download = None;
                        self.started = Instant::now();
                    }
                }
            }
            return;
        }
        let backoff = Duration::from_secs((2u64 << self.restarts.min(5)).min(60));
        if self.restarts > 0 && self.started.elapsed() < backoff {
            return;
        }
        match self.spawn() {
            Ok(c) => {
                *child() = Some(c);
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
        // Absolute: data_dir() is "." for a local trippin.json, and the macOS
        // player resolves a relative -logFile beside its .app, not our cwd.
        let log = crate::config::data_dir().join("unity-engine.log");
        let log = std::path::absolute(&log).unwrap_or(log);
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
        kill_child(); // link switched off (not a quit: it may come back on)
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

/// Download this platform's player zip, verify it, unpack into
/// `<data dir>/unity/`. Progress goes into `st`.
fn download(st: &Mutex<Result<String, String>>) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    let (name, sha) = ASSET;
    if sha.is_empty() {
        return Err(anyhow!("no Unity engine build for this platform yet"));
    }
    let set = |s: String| *st.lock().unwrap_or_else(|p| p.into_inner()) = Ok(s);
    // Unpack beside the final folder and rename it into place at the end:
    // player_path() sees the exe the moment it exists, and launching a
    // half-unpacked player fails (sharing violation).
    let final_dir = crate::config::data_dir().join("unity");
    let dir = crate::config::data_dir().join("unity.part");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    let zip_path = crate::config::data_dir().join(format!("{name}.part"));
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(1800)))
        .build()
        .new_agent();
    let resp = agent
        .get(&format!("{RELEASE_BASE}/{name}"))
        .call()
        .with_context(|| format!("downloading {name}"))?;
    let total: u64 = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut reader = resp.into_body().into_reader();
    let mut file = std::fs::File::create(&zip_path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut got = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        hasher.update(&buf[..n]);
        got += n as u64;
        set(format!("downloading the Unity engine… {} / {} MB", got >> 20, total >> 20));
    }
    drop(file);
    if format!("{:x}", hasher.finalize()) != sha {
        let _ = std::fs::remove_file(&zip_path);
        return Err(anyhow!("{name}: checksum mismatch"));
    }
    set("unpacking the Unity engine…".into());
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&zip_path)?)?;
    for i in 0..zip.len() {
        let mut e = zip.by_index(i)?;
        let Some(rel) = e.enclosed_name() else { continue };
        let out = dir.join(rel);
        if e.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut f = std::fs::File::create(&out)?;
        std::io::copy(&mut e, &mut f)?;
        #[cfg(unix)]
        if let Some(mode) = e.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode))?;
        }
    }
    let _ = std::fs::remove_file(&zip_path);
    std::fs::write(dir.join(STAMP), name)?;
    let _ = std::fs::remove_dir_all(&final_dir);
    std::fs::rename(&dir, &final_dir).context("installing the Unity engine")?;
    set("starting".into());
    Ok(())
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
