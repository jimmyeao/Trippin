//! The external Unity engine (unity/TrippinStage): Trippin starts it, keeps
//! it running and reads its frames — the user never launches anything.
//!
//! - **Process:** when `Settings::unity_link` is on, the player is spawned
//!   headless (`-batchmode`, renders offscreen) with the frame-file path and
//!   the feed port on its command line; it's restarted if it exits and
//!   killed when the link goes off or Trippin quits. The player quits by
//!   itself if Trippin's feed stops — but a relaunched Trippin feeds the same
//!   port, so a player left by a crash or force-quit kept running, writing to
//!   the dead instance's frame file, and the new player could not start. On
//!   launch [`sweep_stale_players`] stops such orphans first.
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

/// The frame-file name for the Trippin with process id `owner`. It goes on
/// the player's command line, so a running player names its owner.
fn frame_name(owner: u32) -> String {
    format!("trippin-engine-{owner}.frame")
}

/// Players in `listing` whose owning Trippin is gone. `listing` is one
/// process per line, `<pid> <command line>` (spaces or a tab after the pid).
/// A player is recognised by `-trippinFrame` and its owner by the pid in
/// `trippin-engine-<pid>.frame`; the owner counts as alive while a process
/// with that pid whose command line names trippin (and isn't itself a
/// player) is in the listing. `me` is never stopped, nor is a player whose
/// owner is alive — another running Trippin's player is left alone. A
/// reused owner pid can only make a player look owned, never the reverse.
fn stale_players(listing: &str, me: u32) -> Vec<u32> {
    let procs: Vec<(u32, &str)> = listing
        .lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let (pid, cmd) = l.split_once(|c: char| c.is_whitespace())?;
            Some((pid.parse().ok()?, cmd.trim()))
        })
        .collect();
    let is_player = |cmd: &str| cmd.contains("-trippinFrame");
    let trippin_alive = |pid: u32| {
        procs
            .iter()
            .any(|&(p, cmd)| p == pid && !is_player(cmd) && cmd.to_lowercase().contains("trippin"))
    };
    procs
        .iter()
        .filter(|&&(pid, cmd)| pid != me && is_player(cmd))
        .filter_map(|&(pid, cmd)| {
            let at = cmd.find("trippin-engine-")? + "trippin-engine-".len();
            let digits: String = cmd[at..].chars().take_while(|c| c.is_ascii_digit()).collect();
            let owner: u32 = digits.parse().ok()?;
            (!trippin_alive(owner)).then_some(pid)
        })
        .collect()
}

/// Every process as `<pid> <command line>` lines (see [`stale_players`]).
#[cfg(unix)]
fn process_listing() -> Option<String> {
    let out = Command::new("ps").args(["-axww", "-o", "pid=,command="]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Not compiled on Windows yet. Win32_Process has the command line (tasklist
/// doesn't); a null one prints as an empty field and is skipped as no player.
#[cfg(windows)]
fn process_listing() -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r#"Get-CimInstance Win32_Process | ForEach-Object { "$($_.ProcessId)`t$($_.CommandLine)" }"#,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn kill_pid(pid: u32) {
    #[cfg(unix)]
    let r = Command::new("kill").args(["-9", &pid.to_string()]).status();
    #[cfg(windows)]
    let r = {
        use std::os::windows::process::CommandExt;
        Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .creation_flags(0x0800_0000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    };
    match r {
        Ok(st) if st.success() => eprintln!("unity engine: stopped player {pid} left by a Trippin that is gone"),
        Ok(st) => eprintln!("unity engine: could not stop stale player {pid} ({st})"),
        Err(e) => eprintln!("unity engine: could not stop stale player {pid}: {e}"),
    }
}

/// Stop players left running by a Trippin that crashed or was force-quit
/// (its exit path, [`shutdown`], never ran). Slow on Windows (PowerShell), so
/// [`Engine::new`] runs it on a thread and the first spawn waits for it.
pub fn sweep_stale_players() {
    let Some(listing) = process_listing() else { return };
    for pid in stale_players(&listing, std::process::id()) {
        kill_pid(pid);
    }
}

/// Prebuilt players, downloaded on first use (like the Beat This! model) so
/// installers stay small and CI needs no Unity licence. Rebuild + upload a
/// new `unity-engine-vN` release when the Unity project changes, and bump
/// these. An empty checksum means no build for this platform yet.
const RELEASE_BASE: &str = "https://github.com/jimmyeao/Trippin/releases/download/unity-engine-v15";
#[cfg(target_os = "macos")]
const ASSET: (&str, &str) = (
    "TrippinEngine-macos-v15.zip",
    "6b91e0dad38ff954dae4e0fe4ab7dd5ef455002a38b4b30259a765d339caa745",
);
#[cfg(not(target_os = "macos"))]
const ASSET: (&str, &str) = (
    "TrippinEngine-windows-x64-v15.zip",
    "17be255939951923fc6b6a8552f1b9e75a09562998e778b206a912fac6619dc8",
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
    /// The stale-player sweep; nothing is spawned until it finishes.
    sweep: Option<(std::thread::JoinHandle<()>, Instant)>,
}

impl Engine {
    /// Create the frame file and start the reader thread; the process is
    /// spawned by `tick`.
    pub fn new(port: u16) -> Result<Engine> {
        let path = std::env::temp_dir().join(frame_name(std::process::id()));
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
            sweep: std::thread::Builder::new()
                .name("engine-sweep".into())
                .spawn(sweep_stale_players)
                .ok()
                .map(|h| (h, Instant::now())),
        })
    }

    /// Keep the player running: spawn it, notice when it exits, restart it
    /// (backing off so a player that crashes on launch doesn't spin).
    pub fn tick(&mut self) {
        if QUITTING.load(Ordering::Relaxed) {
            return;
        }
        // A stale player must be gone before ours starts (it holds the feed
        // port's traffic and blocks a second instance). Bounded: a hung
        // listing never keeps the engine off.
        if let Some((h, t)) = &self.sweep {
            if !h.is_finished() && t.elapsed() < Duration::from_secs(10) {
                return;
            }
            self.sweep = None;
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
        let mut cmd = Command::new(&exe);
        // TRIPPIN_ENGINE_READBACK=blit|direct|sync picks FrameExporter's
        // readback path (blit by default) — for chasing GPU-specific faults.
        if let Some(m) = std::env::var_os("TRIPPIN_ENGINE_READBACK") {
            cmd.arg("-trippinReadback").arg(m);
        }
        cmd.arg("-batchmode")
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
        .tls_config(crate::config::tls())
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
    // TRIPPIN_ENGINE_DUMP=<dir>: save a few frames exactly as read from the
    // file (engine-0.png…) and log accept/torn counts — tells a transport
    // fault from a render fault.
    let dump = std::env::var_os("TRIPPIN_ENGINE_DUMP").map(PathBuf::from);
    let (mut ok, mut torn, mut dumped) = (0u64, 0u64, 0u32);
    let mut stat_at = Instant::now();
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
            torn += 1;
            continue; // torn: Unity wrote while we copied
        }
        last = s1;
        ok += 1;
        if let Some(d) = &dump {
            if ok % 60 == 30 && dumped < 5 {
                let _ = std::fs::create_dir_all(d);
                let f = d.join(format!("engine-{dumped}.png"));
                match image::save_buffer(&f, &px, EXT_W, EXT_H, image::ColorType::Rgba8) {
                    Ok(()) => eprintln!("engine dump: {} (seq {s1})", f.display()),
                    Err(e) => eprintln!("engine dump: {e}"),
                }
                dumped += 1;
            }
            if stat_at.elapsed() > Duration::from_secs(5) {
                eprintln!("engine frames: {ok} ok, {torn} torn retries");
                stat_at = Instant::now();
            }
        }
        *latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(InFrame {
            seq: s1,
            rgba: Arc::new(px),
            at: Instant::now(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_player_whose_trippin_is_gone_is_stale() {
        // macOS `ps` lines: the dead Trippin 4100's player, the live 4200's
        // player and the live Trippin 4200 itself (paths with spaces).
        let listing = "\
 4101 ./unity/TrippinStage.app/Contents/MacOS/Trippin Stage -batchmode -trippinFrame /var/folders/x/T/trippin-engine-4100.frame -trippinPort 9137
 4201 ./unity/TrippinStage.app/Contents/MacOS/Trippin Stage -batchmode -trippinFrame /var/folders/x/T/trippin-engine-4200.frame -trippinPort 9137
 4200 /Applications/Trippin.app/Contents/MacOS/trippin
  300 /usr/sbin/cfprefsd agent
";
        assert_eq!(stale_players(listing, 4200), vec![4101]);
    }

    #[test]
    fn another_running_trippins_player_and_our_own_are_kept() {
        let players = "\
 4201 Trippin Stage -batchmode -trippinFrame /tmp/trippin-engine-4200.frame
 4200 /Users/dj/Trippin/target/release/trippin
 5001 Trippin Stage -batchmode -trippinFrame /tmp/trippin-engine-5000.frame
";
        let listing = format!("{players} 5000 /Applications/Trippin.app/Contents/MacOS/trippin\n");
        assert!(stale_players(&listing, 4200).is_empty(), "both owners are alive");
        // The owner pid reused by something that isn't Trippin: stale.
        let reused = format!("{players} 5000 /usr/bin/vim notes.txt\n");
        assert_eq!(stale_players(&reused, 4200), vec![5001]);
        // A player never counts its own pid as stale.
        assert!(stale_players(&reused, 5001).is_empty());
    }

    #[test]
    fn windows_lines_parse_too() {
        // Win32_Process via PowerShell: pid, tab, command line (may be empty).
        let listing = "4\t\n812\t\"C:\\Program Files\\Trippin\\unity\\TrippinStage.exe\" -batchmode -trippinFrame C:\\Users\\dj\\AppData\\Local\\Temp\\trippin-engine-777.frame -trippinPort 9137\n900\t\"C:\\Program Files\\Trippin\\trippin.exe\"\n";
        assert_eq!(stale_players(listing, 900), vec![812]);
        assert_eq!(frame_name(777), "trippin-engine-777.frame");
    }
}
