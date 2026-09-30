//! Clip recording: a replay buffer ("save the last 60 s") and whole-set
//! recording, both through ffmpeg.
//!
//! Video: the output tap's BGRA frames (overlays included) are piped into
//! one long-running ffmpeg that encodes (hardware H.264 when available) into
//! 2-second MPEG-TS segments, keyframe-aligned. Frames are paced to a
//! constant rate against the wall clock (duplicated/dropped), so segment k
//! covers exactly [t0 + 2k, t0 + 2k + 2) s.
//!
//! Audio: every capture source feeds `audio_in` (stereo where the device
//! gives it). Blocks are resampled to 48 kHz and written as raw f32 chunks
//! on the same 2-second grid, padded/trimmed against the wall clock so they
//! can't drift from the video.
//!
//! Saving a clip concatenates the wanted segments (`-c copy` for 16:9, a
//! re-encode for the 9:16 layouts) and muxes the matching audio chunks.
//! Segments older than the buffer length are deleted, unless a whole set is
//! being recorded.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Seconds per segment / audio chunk.
const SEG: u64 = 2;
const RATE: u32 = 48_000;
/// Allowed audio-vs-wall-clock slack before padding/trimming (samples).
const SLACK: u64 = (RATE as u64) * 6 / 100;

#[cfg(windows)]
fn no_window(c: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    c.creation_flags(0x0800_0000) // CREATE_NO_WINDOW
}
#[cfg(not(windows))]
fn no_window(c: &mut Command) -> &mut Command {
    c
}

// ---- Taps (called from the output worker and the audio threads) -----------

struct Frame {
    at: Instant,
    data: Vec<u8>,
}

struct AudioBlock {
    at: Instant,
    rate: u32,
    ch: u16,
    data: Vec<f32>,
}

static ON: AtomicBool = AtomicBool::new(false);
static FRAME_TX: Mutex<Option<SyncSender<Frame>>> = Mutex::new(None);
static AUDIO_TX: Mutex<Option<SyncSender<AudioBlock>>> = Mutex::new(None);

/// True while a recorder wants frames/audio — lets the taps skip all work.
pub fn active() -> bool {
    ON.load(Ordering::Relaxed)
}

/// One BGRA output frame (`stride` bytes per row). Copies only while
/// recording; drops the frame if the encoder is behind.
pub fn frame_in(w: u32, h: u32, stride: u32, data: &[u8]) {
    if !active() {
        return;
    }
    let Some(tx) = FRAME_TX.lock().ok().and_then(|g| g.clone()) else { return };
    let row = (w * 4) as usize;
    let mut v = Vec::with_capacity(row * h as usize);
    for y in 0..h as usize {
        let s = y * stride as usize;
        if s + row > data.len() {
            return;
        }
        v.extend_from_slice(&data[s..s + row]);
    }
    let _ = tx.try_send(Frame { at: Instant::now(), data: v });
}

/// Captured audio: `ch` interleaved channels at `rate`. `make` only runs
/// while recording, so the audio callbacks pay nothing otherwise.
pub fn audio_in(rate: u32, ch: u16, make: impl FnOnce() -> Vec<f32>) {
    if !active() {
        return;
    }
    let Some(tx) = AUDIO_TX.lock().ok().and_then(|g| g.clone()) else { return };
    let _ = tx.try_send(AudioBlock { at: Instant::now(), rate, ch, data: make() });
}

// ---- Public API --------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Layout {
    /// 16:9 as rendered (stream copy — instant).
    Wide,
    /// 9:16, centre crop — fills the phone screen.
    Crop,
    /// 9:16, the whole frame over a blurred zoom of itself — keeps overlays.
    Fit,
}

impl Layout {
    pub const ALL: [Layout; 3] = [Layout::Wide, Layout::Crop, Layout::Fit];

    pub fn label(self) -> &'static str {
        match self {
            Layout::Wide => "16:9",
            Layout::Crop => "9:16 crop",
            Layout::Fit => "9:16 fit",
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct RecConf {
    pub ffmpeg: PathBuf,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Replay buffer length (seconds kept when no set is recording).
    pub keep_s: u32,
    pub out_dir: PathBuf,
}

#[derive(Clone, Default)]
pub struct Status {
    pub encoder: String,
    /// Set recording started (for the panel's timer).
    pub set_since: Option<Instant>,
    /// Seconds buffered so far.
    pub buffered_s: u64,
    pub saving: bool,
    pub last: Option<String>,
    pub err: Option<String>,
}

enum Cmd {
    Clip(u32, Layout),
    SetStart,
    SetStop(Layout),
}

pub struct Recorder {
    pub conf: RecConf,
    cmd: Sender<Cmd>,
    pub status: Arc<Mutex<Status>>,
    worker: Option<JoinHandle<()>>,
}

impl Recorder {
    pub fn start(conf: RecConf) -> Recorder {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (ftx, frx) = mpsc::sync_channel::<Frame>(4);
        let (atx, arx) = mpsc::sync_channel::<AudioBlock>(256);
        *FRAME_TX.lock().unwrap_or_else(|e| e.into_inner()) = Some(ftx);
        *AUDIO_TX.lock().unwrap_or_else(|e| e.into_inner()) = Some(atx);
        let status = Arc::new(Mutex::new(Status::default()));
        let st = status.clone();
        let c = conf.clone();
        let worker = std::thread::Builder::new()
            .name("recorder".into())
            .spawn(move || {
                if let Err(e) = run(c, frx, arx, cmd_rx, st.clone()) {
                    eprintln!("recorder: {e:#}");
                    st.lock().unwrap_or_else(|e| e.into_inner()).err = Some(format!("{e:#}"));
                }
                ON.store(false, Ordering::Relaxed);
            })
            .ok();
        ON.store(true, Ordering::Relaxed);
        Recorder { conf, cmd: cmd_tx, status, worker }
    }

    pub fn save_clip(&self, secs: u32, layout: Layout) {
        let _ = self.cmd.send(Cmd::Clip(secs, layout));
    }

    pub fn set_start(&self) {
        let _ = self.cmd.send(Cmd::SetStart);
    }

    pub fn set_stop(&self, layout: Layout) {
        let _ = self.cmd.send(Cmd::SetStop(layout));
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        ON.store(false, Ordering::Relaxed);
        *FRAME_TX.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *AUDIO_TX.lock().unwrap_or_else(|e| e.into_inner()) = None;
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

/// ffmpeg: an explicit path, else next to the exe, else PATH, else the
/// usual install spots (winget / Homebrew).
pub fn find_ffmpeg(user: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let user = user.trim();
    if !user.is_empty() {
        let p = PathBuf::from(user);
        return p.is_file().then_some(p);
    }
    let mut c: Vec<PathBuf> = Vec::new();
    if let Ok(me) = std::env::current_exe() {
        if let Some(d) = me.parent() {
            c.push(d.join(exe));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        c.extend(std::env::split_paths(&path).map(|d| d.join(exe)));
    }
    if cfg!(windows) {
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            c.push(PathBuf::from(l).join("Microsoft/WinGet/Links").join(exe));
        }
        c.push(PathBuf::from("C:/ffmpeg/bin").join(exe));
    } else {
        c.push(PathBuf::from("/opt/homebrew/bin/ffmpeg"));
        c.push(PathBuf::from("/usr/local/bin/ffmpeg"));
    }
    c.into_iter().find(|p| p.is_file())
}

/// Default clip folder: Videos/Trippin (Movies on macOS).
pub fn default_out_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    match home {
        Some(h) => PathBuf::from(h)
            .join(if cfg!(target_os = "macos") { "Movies" } else { "Videos" })
            .join("Trippin"),
        None => crate::config::data_dir().join("clips"),
    }
}

// ---- Worker ------------------------------------------------------------------

fn probe(ffmpeg: &Path, enc: &str) -> bool {
    no_window(&mut Command::new(ffmpeg))
        .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "color=black:s=320x240:r=30", "-frames:v", "5", "-c:v", enc, "-f", "null", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Encoder + its quality args, fastest available first.
fn pick_encoder(ffmpeg: &Path) -> (String, Vec<String>) {
    let cands: &[(&str, &[&str])] = &[
        ("h264_nvenc", &["-preset", "p5", "-rc", "vbr", "-cq", "21", "-b:v", "0"]),
        ("h264_videotoolbox", &["-b:v", "14M", "-allow_sw", "1"]),
        ("h264_qsv", &["-global_quality", "23"]),
        ("h264_amf", &["-quality", "balanced", "-rc", "cqp", "-qp_i", "21", "-qp_p", "23"]),
    ];
    for (enc, args) in cands {
        if probe(ffmpeg, enc) {
            return (enc.to_string(), args.iter().map(|s| s.to_string()).collect());
        }
    }
    ("libx264".into(), ["-preset", "veryfast", "-crf", "21"].iter().map(|s| s.to_string()).collect())
}

struct Buf {
    dir: PathBuf,
}

impl Buf {
    fn video(&self, k: u64) -> PathBuf {
        self.dir.join(format!("v{k:08}.ts"))
    }
    fn audio(&self, k: u64) -> PathBuf {
        self.dir.join(format!("a{k:08}.f32"))
    }
}

impl Drop for Buf {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Wall-clock-locked 48 kHz stereo writer into per-segment chunk files.
struct AudioSink {
    written: u64,
    file: Option<(u64, std::fs::File)>,
    // Stateful linear resampler.
    src_rate: u32,
    pos: f64,
    last: [f32; 2],
}

impl AudioSink {
    fn push(&mut self, buf: &Buf, t0: Instant, b: AudioBlock) {
        if b.at < t0 || b.ch == 0 {
            return;
        }
        if b.rate != self.src_rate {
            self.src_rate = b.rate;
            self.pos = 0.0;
        }
        // To stereo frames at the source rate.
        let ch = b.ch as usize;
        let frames: Vec<[f32; 2]> = b
            .data
            .chunks_exact(ch)
            .map(|f| if ch == 1 { [f[0], f[0]] } else { [f[0], f[1]] })
            .collect();
        // Resample to RATE (linear, phase carried across blocks).
        let step = b.rate as f64 / RATE as f64;
        let mut out: Vec<[f32; 2]> = Vec::with_capacity((frames.len() as f64 / step) as usize + 2);
        let at = |i: isize| if i < 0 { self.last } else { frames[i as usize] };
        while self.pos < frames.len() as f64 - 1.0 {
            let i = self.pos.floor() as isize;
            let f = (self.pos - i as f64) as f32;
            let (a, c) = (at(i), at(i + 1));
            out.push([a[0] + (c[0] - a[0]) * f, a[1] + (c[1] - a[1]) * f]);
            self.pos += step;
        }
        self.pos -= frames.len() as f64;
        if let Some(l) = frames.last() {
            self.last = *l;
        }
        // Lock to the wall clock: the block ends at `at`.
        let expected = ((b.at - t0).as_secs_f64() * RATE as f64) as u64;
        let n = out.len() as u64;
        let mut skip = 0usize;
        if self.written + n + SLACK < expected {
            let pad = expected - n - self.written;
            self.write(buf, &vec![[0.0; 2]; pad as usize]);
        } else if self.written + n > expected + SLACK {
            skip = (self.written + n - expected) as usize;
        }
        if skip < out.len() {
            self.write(buf, &out[skip..]);
        }
    }

    fn write(&mut self, buf: &Buf, mut s: &[[f32; 2]]) {
        let per = RATE as u64 * SEG;
        while !s.is_empty() {
            let k = self.written / per;
            if self.file.as_ref().is_none_or(|(fk, _)| *fk != k) {
                self.file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(buf.audio(k))
                    .ok()
                    .map(|f| (k, f));
            }
            let room = ((k + 1) * per - self.written) as usize;
            let take = room.min(s.len());
            if let Some((_, f)) = self.file.as_mut() {
                let bytes: &[u8] = bytemuck::cast_slice(&s[..take]);
                let _ = f.write_all(bytes);
            }
            self.written += take as u64;
            s = &s[take..];
        }
    }

    /// Chunks strictly below this index are complete.
    fn done_before(&self) -> u64 {
        self.written / (RATE as u64 * SEG)
    }
}

fn stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let t = secs.rem_euclid(86_400);
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", t / 3600, t / 60 % 60, t % 60)
}

fn run(
    conf: RecConf,
    frames: Receiver<Frame>,
    audio: Receiver<AudioBlock>,
    cmds: Receiver<Cmd>,
    status: Arc<Mutex<Status>>,
) -> anyhow::Result<()> {
    // Absolute: the concat lists resolve relative paths against themselves.
    let dir = std::path::absolute(crate::config::data_dir())?.join(format!("recbuf-{}", std::process::id()));
    // Buffers left by a crashed/killed run: stale for 10+ minutes (a live
    // one is written every 2 s).
    if let Some(Ok(rd)) = dir.parent().map(std::fs::read_dir) {
        for e in rd.flatten() {
            let stale = e.metadata().and_then(|m| m.modified()).is_ok_and(|t| {
                t.elapsed().unwrap_or_default() > Duration::from_secs(600)
            });
            if stale && e.file_name().to_string_lossy().starts_with("recbuf-") {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    let buf = Arc::new(Buf { dir });
    let (enc, enc_args) = pick_encoder(&conf.ffmpeg);
    println!("recorder: {enc} via {}", conf.ffmpeg.display());
    status.lock().unwrap_or_else(|e| e.into_inner()).encoder = enc.clone();

    let fps = conf.fps.max(1) as u64;
    let gop = (fps * SEG).to_string();
    let mut args: Vec<String> = [
        "-hide_banner", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "bgra",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.extend([
        "-s".into(),
        format!("{}x{}", conf.width, conf.height),
        "-framerate".into(),
        fps.to_string(),
        "-i".into(),
        "pipe:0".into(),
    ]);
    // 4K taps record at 1080p — plenty for clips, and keeps the encoder easy.
    if conf.height > 1080 {
        args.extend(["-vf".into(), "scale=-2:1080".into()]);
    }
    args.extend(["-c:v".into(), enc.clone()]);
    args.extend(enc_args.iter().cloned());
    args.extend([
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-g".into(),
        gop.clone(),
        "-force_key_frames".into(),
        format!("expr:gte(t,n_forced*{SEG})"),
        "-f".into(),
        "segment".into(),
        "-segment_time".into(),
        SEG.to_string(),
        "-segment_format".into(),
        "mpegts".into(),
        "-reset_timestamps".into(),
        "1".into(),
        buf.dir.join("v%08d.ts").to_string_lossy().into_owned(),
    ]);
    let log = std::fs::File::create(buf.dir.join("ffmpeg.log"))?;
    let mut child: Child = no_window(&mut Command::new(&conf.ffmpeg))
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::from(log))
        .spawn()?;
    let mut stdin = child.stdin.take();

    let mut t0: Option<Instant> = None;
    let mut sent = 0u64; // frames written
    let mut last: Option<Vec<u8>> = None;
    let mut sink = AudioSink { written: 0, file: None, src_rate: 0, pos: 0.0, last: [0.0; 2] };
    let mut set_from: Option<u64> = None;
    // A stopped set waits for the segment it stopped in to close: (from,
    // until, layout).
    let mut set_pending: Option<(u64, u64, Layout)> = None;
    let mut last_gc = Instant::now();
    let per_seg = fps * SEG;
    let mut saves: Vec<JoinHandle<()>> = Vec::new();

    loop {
        match frames.recv_timeout(Duration::from_millis(20)) {
            Ok(f) => {
                let t = *t0.get_or_insert(f.at);
                // Constant frame rate against the wall clock.
                let want = ((f.at - t).as_secs_f64() * fps as f64) as u64 + 1;
                if want > sent {
                    // Behind: repeat the previous frame into the gap (at
                    // most a second of them), then this one.
                    let gap = (want - sent - 1).min(fps);
                    if let (Some(prev), Some(w)) = (&last, stdin.as_mut()) {
                        for _ in 0..gap {
                            if w.write_all(prev).is_err() {
                                break;
                            }
                            sent += 1;
                        }
                    }
                    sent = sent.max(want - 1);
                    if let Some(w) = stdin.as_mut() {
                        if w.write_all(&f.data).is_err() {
                            let e = child.try_wait().ok().flatten();
                            anyhow::bail!("ffmpeg stopped ({e:?}) — see {}", buf.dir.join("ffmpeg.log").display());
                        }
                    }
                    sent += 1;
                    last = Some(f.data);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if let Some(t) = t0 {
            while let Ok(b) = audio.try_recv() {
                sink.push(&buf, t, b);
            }
        } else {
            while audio.try_recv().is_ok() {}
        }

        // Segments strictly below `done` are complete on disk (video and
        // audio): the next video segment exists and audio has moved past.
        let cur_v = sent / per_seg;
        let done = {
            let mut k = cur_v.saturating_sub(1);
            while k > 0 && !buf.video(k).exists() {
                k -= 1;
            }
            k.min(sink.done_before())
        };

        while let Ok(c) = cmds.try_recv() {
            match c {
                Cmd::Clip(secs, layout) => {
                    let n = (secs as u64).div_ceil(SEG).max(1);
                    let from = done.saturating_sub(n);
                    saves.push(spawn_save(&conf, &buf, &status, from, done, layout, "clip"));
                }
                Cmd::SetStart => {
                    set_from = Some(cur_v);
                    status.lock().unwrap_or_else(|e| e.into_inner()).set_since = Some(Instant::now());
                }
                Cmd::SetStop(layout) => {
                    if let Some(from) = set_from.take() {
                        set_pending = Some((from, cur_v + 1, layout));
                    }
                    status.lock().unwrap_or_else(|e| e.into_inner()).set_since = None;
                }
            }
        }
        if let Some((from, until, layout)) = set_pending {
            if done >= until {
                set_pending = None;
                saves.push(spawn_save(&conf, &buf, &status, from, until, layout, "set"));
            }
        }

        if last_gc.elapsed() > Duration::from_secs(1) {
            last_gc = Instant::now();
            status.lock().unwrap_or_else(|e| e.into_inner()).buffered_s =
                (done * SEG).min(conf.keep_s as u64);
            // Drop what's older than the buffer (plus slack for saves in
            // flight) — unless a set is recording, or a save is running.
            saves.retain(|h| !h.is_finished());
            if set_from.is_none() && set_pending.is_none() && saves.is_empty() {
                let keep = (conf.keep_s as u64).div_ceil(SEG) + 3;
                if done > keep {
                    for k in done.saturating_sub(keep + 30)..done - keep {
                        let _ = std::fs::remove_file(buf.video(k));
                        let _ = std::fs::remove_file(buf.audio(k));
                    }
                }
            }
        }
    }
    drop(stdin.take());
    let _ = child.wait();
    // Shutting down mid-set: keep what was recorded.
    if let Some(from) = set_from.take() {
        set_pending = Some((from, u64::MAX, Layout::Wide));
    }
    if let Some((from, until, layout)) = set_pending {
        let mut end = 0;
        while buf.video(end).exists() || end < from {
            end += 1;
        }
        saves.push(spawn_save(&conf, &buf, &status, from, until.min(end), layout, "set"));
    }
    for h in saves {
        let _ = h.join();
    }
    Ok(())
}

/// Assemble segments [from, to) into an MP4 on a helper thread.
fn spawn_save(
    conf: &RecConf,
    buf: &Arc<Buf>,
    status: &Arc<Mutex<Status>>,
    from: u64,
    to: u64,
    layout: Layout,
    kind: &str,
) -> JoinHandle<()> {
    let conf = conf.clone();
    let buf = buf.clone();
    let status = status.clone();
    let kind = kind.to_string();
    status.lock().unwrap_or_else(|e| e.into_inner()).saving = true;
    std::thread::spawn(move || {
        let r = save(&conf, &buf, from, to, layout, &kind);
        let mut st = status.lock().unwrap_or_else(|e| e.into_inner());
        st.saving = false;
        match r {
            Ok(p) => {
                println!("recorder: saved {}", p.display());
                st.last = Some(p.display().to_string());
                st.err = None;
            }
            Err(e) => {
                eprintln!("recorder: save failed: {e:#}");
                st.err = Some(format!("save failed: {e:#}"));
            }
        }
    })
}

fn save(conf: &RecConf, buf: &Buf, from: u64, to: u64, layout: Layout, kind: &str) -> anyhow::Result<PathBuf> {
    let ks: Vec<u64> = (from..to).filter(|&k| buf.video(k).exists()).collect();
    anyhow::ensure!(!ks.is_empty(), "nothing buffered yet");
    let tag = format!("{}-{}", stamp(), kind);
    let list = buf.dir.join(format!("{tag}.txt"));
    let mut l = String::new();
    for &k in &ks {
        let p = buf.video(k).to_string_lossy().replace('\\', "/").replace('\'', "'\\''");
        l.push_str(&format!("file '{p}'\n"));
    }
    std::fs::write(&list, l)?;
    // Matching audio (missing chunks become silence).
    let wav = buf.dir.join(format!("{tag}.f32"));
    {
        let mut out = std::io::BufWriter::new(std::fs::File::create(&wav)?);
        let chunk_bytes = (RATE as u64 * SEG * 8) as usize;
        for &k in &ks {
            let mut data = std::fs::read(buf.audio(k)).unwrap_or_default();
            data.resize(chunk_bytes, 0);
            out.write_all(&data)?;
        }
    }
    std::fs::create_dir_all(&conf.out_dir)?;
    let suffix = match layout {
        Layout::Wide => "",
        Layout::Crop | Layout::Fit => " 9x16",
    };
    let out = conf.out_dir.join(format!("Trippin {tag}{suffix}.mp4"));
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    a.push(list.to_string_lossy().into_owned());
    a.extend(["-f", "f32le", "-ar", "48000", "-ac", "2", "-i"].iter().map(|s| s.to_string()));
    a.push(wav.to_string_lossy().into_owned());
    match layout {
        Layout::Wide => a.extend(["-map", "0:v", "-c:v", "copy"].iter().map(|s| s.to_string())),
        Layout::Crop | Layout::Fit => {
            let f = if layout == Layout::Crop {
                "[0:v]crop=ih*9/16:ih,scale=1080:1920:flags=lanczos,setsar=1[v]".to_string()
            } else {
                "[0:v]split[a][b];[a]scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920,boxblur=24:2,eq=brightness=-0.08[bg];[b]scale=1080:-2:flags=lanczos[fg];[bg][fg]overlay=(W-w)/2:(H-h)/2,setsar=1[v]".to_string()
            };
            let (enc, eargs) = pick_encoder(&conf.ffmpeg);
            a.extend(["-filter_complex".to_string(), f, "-map".into(), "[v]".into(), "-c:v".into(), enc]);
            a.extend(eargs);
            a.extend(["-pix_fmt", "yuv420p"].iter().map(|s| s.to_string()));
        }
    }
    a.extend(["-map", "1:a", "-c:a", "aac", "-b:a", "192k", "-shortest", "-movflags", "+faststart"].iter().map(|s| s.to_string()));
    a.push(out.to_string_lossy().into_owned());
    let o = no_window(&mut Command::new(&conf.ffmpeg))
        .args(&a)
        .stdin(Stdio::null())
        .output()?;
    let _ = std::fs::remove_file(&list);
    let _ = std::fs::remove_file(&wav);
    anyhow::ensure!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr).trim());
    Ok(out)
}
