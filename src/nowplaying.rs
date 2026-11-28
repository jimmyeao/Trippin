//! "Now playing" track detection for the overlay (and `nowplaying.txt` for
//! OBS text sources).
//!
//! Modelled on how Now Playing / What's Now Playing do it: read the DJ
//! software running on this computer through the files it keeps up to date,
//! and fall back to the OS media session for players like Spotify.
//!
//! | Source            | How                                                        |
//! |-------------------|------------------------------------------------------------|
//! | Media session     | Windows SMTC (Spotify, Apple Music, browsers, djay, …);    |
//! |                   | macOS: AppleScript for Spotify / Music                     |
//! | Serato            | Serato 4+: `Library/master.sqlite` → `history_entry`;      |
//! |                   | ≤3.x: `_Serato_/History/Sessions/*.session` (binary)       |
//! | VirtualDJ         | `Documents/VirtualDJ/History/tracklist.txt`                |
//! | rekordbox         | encrypted `master.db` (SQLCipher) → `djmdSongHistory`      |
//! | Mixxx             | `mixxxdb.sqlite` → the hidden history ("set log") playlist |
//!
//! A worker thread polls every source once a second; only sources whose
//! files changed are re-read. In `Auto` mode the source that changed most
//! recently wins, so switching from Spotify to Serato just works.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default)]
pub struct Track {
    pub artist: String,
    pub title: String,
    /// Which source reported it ("Serato", "Spotify", …).
    pub source: String,
    /// Where its cover art can be found (`albumart.rs` resolves it). Not
    /// part of a track's identity: equality ignores it, so art that turns
    /// up a poll later doesn't count as a track change.
    pub art: ArtHint,
}

impl PartialEq for Track {
    fn eq(&self, o: &Self) -> bool {
        self.artist == o.artist && self.title == o.title && self.source == o.source
    }
}
impl Eq for Track {}

/// A cheap pointer to a track's cover art, filled in by the source that
/// reported the track. Resolving it (reading tags, fetching a URL, decoding)
/// happens off this thread, in `albumart.rs`.
#[derive(Clone, Default, PartialEq)]
pub enum ArtHint {
    #[default]
    None,
    /// Candidates in order: image files are read as they are, audio files
    /// for their embedded picture (then a cover image beside them).
    Files(Vec<PathBuf>),
    /// An image URL (Spotify on macOS reports one).
    #[allow(dead_code)] // built on macOS only
    Url(String),
    /// The encoded image itself (the Windows media session's thumbnail,
    /// a streamed Apple Music track's artwork on macOS).
    Bytes(Arc<Vec<u8>>),
}

impl ArtHint {
    pub fn is_none(&self) -> bool {
        matches!(self, ArtHint::None)
    }
    pub fn describe(&self) -> String {
        match self {
            ArtHint::None => "no art".into(),
            ArtHint::Files(f) => {
                let names: Vec<String> = f.iter().map(|p| p.display().to_string()).collect();
                format!("files {}", names.join(" | "))
            }
            ArtHint::Url(u) => format!("url {u}"),
            ArtHint::Bytes(b) => format!("image {} bytes", b.len()),
        }
    }
}

impl std::fmt::Debug for ArtHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.describe())
    }
}

impl Track {
    pub fn line(&self) -> String {
        match (self.artist.is_empty(), self.title.is_empty()) {
            (false, false) => format!("{} - {}", self.artist, self.title),
            (true, false) => self.title.clone(),
            (false, true) => self.artist.clone(),
            _ => String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NpSource {
    #[default]
    Auto,
    MediaSession,
    Serato,
    VirtualDj,
    Rekordbox,
    Mixxx,
    /// Any text file another tool keeps up to date ("Artist - Title").
    File,
    /// Pushed over the LAN remote by the companion agent (`agent/`), which
    /// runs this same detection on the DJ machine.
    Remote,
    Off,
}

impl NpSource {
    pub const ALL: [NpSource; 9] = [
        Self::Auto,
        Self::MediaSession,
        Self::Serato,
        Self::VirtualDj,
        Self::Rekordbox,
        Self::Mixxx,
        Self::File,
        Self::Remote,
        Self::Off,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::MediaSession => "Spotify / media",
            Self::Serato => "Serato",
            Self::VirtualDj => "VirtualDJ",
            Self::Rekordbox => "rekordbox",
            Self::Mixxx => "Mixxx",
            Self::File => "Text file",
            Self::Remote => "LAN agent",
            Self::Off => "Off",
        }
    }
}

/// Shared state read by the render loop and the panel.
#[derive(Clone, Default)]
pub struct NowPlayingState {
    pub track: Option<Track>,
    /// Bumped every time the displayed track changes (triggers the card).
    pub serial: u64,
    /// Bumped when the displayed track or its art hint changes (the art
    /// resolver in `albumart.rs` watches this; the card doesn't replay).
    pub art_serial: u64,
    /// Per-source status for the panel, e.g. "Serato: no history yet".
    pub status: Vec<(String, String)>,
}

pub type SharedNowPlaying = Arc<Mutex<NowPlayingState>>;

/// A track pushed over the LAN remote (`remote.rs` `now_playing`), tagged
/// with the WebSocket client's id so its disconnect retracts only its own
/// track. The `Remote` source below reads it like any other poll.
pub type RemoteNp = Arc<Mutex<Option<(u64, Track)>>>;

/// Start the detector thread. `mode` is re-read every poll, so the panel can
/// switch sources live.
/// Live settings for the detector (the panel edits these).
#[derive(Clone, Debug)]
pub struct NpConfig {
    pub source: NpSource,
    /// Seconds a new track must stay current before it's announced — the
    /// "Track Update Delay" idea from Now Playing: DJ software logs a track
    /// when it's *loaded*, not when the crowd hears it, so a track that's
    /// cued and then abandoned (or mixed out quickly) never gets announced.
    pub delay_s: f32,
    /// File-watcher source: path to a text file ("" = off).
    pub file: String,
}

pub fn start(
    mode: Arc<Mutex<NpConfig>>,
    txt_path: PathBuf,
    remote: RemoteNp,
) -> SharedNowPlaying {
    let shared: SharedNowPlaying = Arc::new(Mutex::new(NowPlayingState::default()));
    let out = shared.clone();
    std::thread::Builder::new()
        .name("nowplaying".into())
        .spawn(move || run(mode, out, txt_path, remote))
        .ok();
    shared
}

struct Latest {
    track: Option<Track>,
    changed: Instant,
}

fn run(
    mode: Arc<Mutex<NpConfig>>,
    shared: SharedNowPlaying,
    txt_path: PathBuf,
    remote: RemoteNp,
) {
    let mut sources: Vec<Box<dyn Source>> = vec![
        Box::new(media::MediaSession::new()),
        Box::new(Serato::default()),
        Box::new(VirtualDj::default()),
        Box::new(Rekordbox::default()),
        Box::new(Mixxx::default()),
        Box::new(FileWatch::default()),
        Box::new(Remote { inbox: remote }),
    ];
    let mut latest: Vec<Latest> = sources
        .iter()
        .map(|_| Latest {
            track: None,
            changed: Instant::now() - Duration::from_secs(3600),
        })
        .collect();
    let mut shown: Option<Track> = None;
    let mut first = vec![true; sources.len()];
    // Candidate waiting out the update delay, and since when.
    let mut pending: Option<(Option<Track>, Instant)> = None;
    loop {
        let cfg = mode.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let m = cfg.source;
        let mut status = Vec::new();
        for (i, s) in sources.iter_mut().enumerate() {
            if m != NpSource::Auto && m != s.kind() {
                continue;
            }
            s.configure(&cfg);
            match s.poll() {
                Ok(t) => {
                    // Same track, art found since (Spotify publishes the
                    // thumbnail a moment after the title): take the art
                    // without counting it as a change.
                    if let (Some(n), Some(l)) = (&t, latest[i].track.as_mut()) {
                        if n == &*l && n.art != l.art && !n.art.is_none() {
                            l.art = n.art.clone();
                        }
                    }
                    if t != latest[i].track {
                        // A source going quiet (None) isn't a "change" that
                        // should steal Auto from the others. And what a
                        // history file says at startup is old news — only a
                        // live source (media session) counts as current then.
                        if t.is_some() && (!first[i] || s.live_on_start()) {
                            latest[i].changed = Instant::now();
                        }
                        latest[i].track = t;
                    }
                    first[i] = false;
                    status.push((
                        s.kind().label().to_string(),
                        latest[i].track.as_ref().map_or("idle".into(), |t| {
                            if t.art.is_none() {
                                t.line()
                            } else if std::env::var_os("TRIPPIN_ART_DEBUG").is_some() {
                                format!("{} · {}", t.line(), t.art.describe())
                            } else {
                                format!("{} · cover", t.line())
                            }
                        }),
                    ));
                }
                Err(e) => status.push((s.kind().label().to_string(), e)),
            }
        }
        let pick = if m == NpSource::Off {
            None
        } else {
            latest
                .iter()
                .enumerate()
                .filter(|(i, l)| {
                    l.track.is_some() && (m == NpSource::Auto || sources[*i].kind() == m)
                })
                .max_by_key(|(_, l)| l.changed)
                .and_then(|(_, l)| l.track.clone())
        };
        {
            let mut st = shared.lock().unwrap_or_else(|e| e.into_inner());
            // Debounce: the candidate must hold for delay_s before it
            // replaces what's shown (clearing to nothing is immediate-ish
            // too, via the same delay, so a brief gap doesn't blank it).
            if pick != shown {
                if pending.as_ref().is_none_or(|(p, _)| *p != pick) {
                    pending = Some((pick.clone(), Instant::now()));
                }
            } else {
                pending = None;
                // The shown track's art turned up (or changed) after it
                // was announced.
                if let (Some(p), Some(cur)) = (&pick, st.track.as_mut()) {
                    if p.art != cur.art && !p.art.is_none() {
                        cur.art = p.art.clone();
                        shown = Some(cur.clone());
                        st.art_serial += 1;
                    }
                }
            }
            let due = pending
                .as_ref()
                .is_some_and(|(_, t)| t.elapsed().as_secs_f32() >= cfg.delay_s);
            if due {
                let pick = pending.take().unwrap().0;
                shown = pick.clone();
                st.track = pick.clone();
                st.serial += 1;
                st.art_serial += 1;
                let line = pick.as_ref().map(|t| t.line()).unwrap_or_default();
                let _ = std::fs::write(&txt_path, line);
            }
            st.status = status;
        }
        std::thread::sleep(Duration::from_millis(1000));
    }
}

trait Source: Send {
    fn kind(&self) -> NpSource;
    /// Ok(None) = source present but nothing playing; Err = not available
    /// (message for the panel).
    fn poll(&mut self) -> Result<Option<Track>, String>;
    /// Whether a track seen on the very first poll is playing *now* (true
    /// for the media session; history files report the past).
    fn live_on_start(&self) -> bool {
        false
    }
    fn configure(&mut self, _cfg: &NpConfig) {}
}

// ---- File watcher ----------------------------------------------------------

/// Reads the last non-empty line of any text file ("Artist - Title", or a
/// bare title) — plugs in other tools, scripts, or DJ software not listed.
#[derive(Default)]
struct FileWatch {
    path: String,
    seen: Option<SystemTime>,
    last: Option<Track>,
}

impl Source for FileWatch {
    fn kind(&self) -> NpSource {
        NpSource::File
    }
    fn configure(&mut self, cfg: &NpConfig) {
        if cfg.file != self.path {
            self.path = cfg.file.clone();
            self.seen = None;
        }
    }
    fn live_on_start(&self) -> bool {
        true
    }
    fn poll(&mut self) -> Result<Option<Track>, String> {
        if self.path.is_empty() {
            return Err("no file set".into());
        }
        let p = Path::new(&self.path);
        let m = mtime(p);
        if m.is_none() {
            return Err("file not found".into());
        }
        if m != self.seen {
            self.seen = m;
            let s = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
            let s = s.trim_start_matches('\u{feff}');
            self.last = s.lines().rev().find(|l| !l.trim().is_empty()).map(|l| {
                let (a, t) = l.trim().split_once(" - ").unwrap_or(("", l.trim()));
                Track { artist: a.trim().into(), title: t.trim().into(), source: "File".into(), ..Default::default() }
            });
        }
        Ok(self.last.clone())
    }
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

// ---- LAN agent ------------------------------------------------------------

/// A companion agent (the `agent/` crate) running this same detection on
/// the DJ machine pushes tracks over the WebSocket remote. The inbox is
/// written by `remote_cmd`; this just reads it — the tag is the client's
/// id, stripped here and used by the retract-on-disconnect logic.
struct Remote {
    inbox: RemoteNp,
}

impl Source for Remote {
    fn kind(&self) -> NpSource {
        NpSource::Remote
    }
    /// A pushed track is live now, same as the media session.
    fn live_on_start(&self) -> bool {
        true
    }
    fn poll(&mut self) -> Result<Option<Track>, String> {
        Ok(self
            .inbox
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .map(|(_, t)| t))
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

// ---- Serato --------------------------------------------------------------

/// Serato DJ 4+ keeps its history in a plain SQLite library,
/// `~/Library/Application Support/Serato/Library/master.sqlite` (Windows:
/// `%APPDATA%\Serato\Library\master.sqlite`): every track on a deck is a
/// `history_entry` row, with `played` flipped to 1 once it's audible —
/// cued-but-unplayed loads stay 0 and are skipped. It's in WAL mode and
/// written live, so watch the `-wal` file for changes.
///
/// Serato DJ Pro ≤3.x writes each session to
/// `_Serato_/History/Sessions/<n>.session`: chunks of (4-byte tag, u32 BE
/// length, payload). Each played track is an `oent` chunk wrapping an
/// `adat` chunk of fields (u32 BE id, u32 BE len, data). Field ids: 2 path,
/// 6 title, 7 artist (UTF-16BE), 28 start time, 29 end time (u32 BE unix
/// seconds), 31 deck.
#[derive(Default)]
struct Serato {
    file: Option<PathBuf>,
    db: Option<PathBuf>,
    seen: Option<SystemTime>,
    last: Option<Track>,
    scanned: Option<Instant>,
}

impl Serato {
    fn db_path() -> Option<PathBuf> {
        let mut c = Vec::new();
        if let Some(a) = std::env::var_os("APPDATA") {
            c.push(PathBuf::from(a).join("Serato/Library/master.sqlite"));
        }
        if let Some(a) = std::env::var_os("LOCALAPPDATA") {
            c.push(PathBuf::from(a).join("Serato/Library/master.sqlite"));
        }
        if let Some(h) = home() {
            c.push(h.join("Library/Application Support/Serato/Library/master.sqlite"));
        }
        c.into_iter().find(|p| p.is_file())
    }

    fn session_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(h) = home() {
            dirs.push(h.join("Music/_Serato_/History/Sessions"));
        }
        #[cfg(windows)]
        for d in b'C'..=b'Z' {
            dirs.push(PathBuf::from(format!("{}:\\_Serato_\\History\\Sessions", d as char)));
        }
        #[cfg(target_os = "macos")]
        if let Ok(rd) = std::fs::read_dir("/Volumes") {
            for e in rd.flatten() {
                dirs.push(e.path().join("_Serato_/History/Sessions"));
            }
        }
        dirs.into_iter().filter(|d| d.is_dir()).collect()
    }

    fn newest_session() -> Option<PathBuf> {
        Self::session_dirs()
            .iter()
            .filter_map(|d| std::fs::read_dir(d).ok())
            .flat_map(|rd| rd.flatten())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "session"))
            .max_by_key(|p| mtime(p))
    }
}

pub fn parse_serato_session(data: &[u8]) -> Option<Track> {
    fn chunks(mut b: &[u8]) -> Vec<(&[u8], &[u8])> {
        let mut out = Vec::new();
        while b.len() >= 8 {
            let tag = &b[..4];
            let len = u32::from_be_bytes([b[4], b[5], b[6], b[7]]) as usize;
            if 8 + len > b.len() {
                break;
            }
            out.push((tag, &b[8..8 + len]));
            b = &b[8 + len..];
        }
        out
    }
    fn utf16(b: &[u8]) -> String {
        let u: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&u).trim_end_matches('\0').trim().to_string()
    }
    let mut best: Option<(u32, Track)> = None;
    for (tag, body) in chunks(data) {
        if tag != b"oent" {
            continue;
        }
        for (t2, adat) in chunks(body) {
            if t2 != b"adat" {
                continue;
            }
            let (mut title, mut artist, mut path, mut start) = (String::new(), String::new(), String::new(), 0u32);
            for (id, val) in chunks(adat) {
                let id = u32::from_be_bytes([id[0], id[1], id[2], id[3]]);
                match id {
                    2 => path = utf16(val),
                    6 => title = utf16(val),
                    7 => artist = utf16(val),
                    28 if val.len() >= 4 => start = u32::from_be_bytes([val[0], val[1], val[2], val[3]]),
                    _ => {}
                }
            }
            if title.is_empty() && !path.is_empty() {
                title = Path::new(&path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
            }
            if (!title.is_empty() || !artist.is_empty()) && best.as_ref().is_none_or(|(s, _)| start >= *s) {
                let art = if path.is_empty() { ArtHint::None } else { ArtHint::Files(vec![serato_path(&path)]) };
                best = Some((start, Track { artist, title, source: "Serato".into(), art }));
            }
        }
    }
    best.map(|(_, t)| t)
}

/// Serato 4+: the newest track the crowd actually heard (`played = 1`),
/// across all sessions in `master.sqlite`.
fn serato_db_track(db: &Path) -> Result<Option<Track>, String> {
    let c = rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    // The file's path, for its cover art. Which column holds it isn't
    // documented, so take the first that looks like one.
    let path_col = c
        .prepare("SELECT name FROM pragma_table_info('history_entry')")
        .and_then(|mut st| st.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>())
        .unwrap_or_default()
        .into_iter()
        .find(|n| {
            let n = n.to_ascii_lowercase();
            ["file_name", "file_path", "filepath", "location", "path", "portable_id"].contains(&n.as_str())
        });
    let sql = format!(
        "SELECT IFNULL(artist, ''), IFNULL(name, ''), {} FROM history_entry \
         WHERE played = 1 AND (name <> '' OR artist <> '') \
         ORDER BY start_time DESC, id DESC LIMIT 1",
        path_col.map_or("''".to_string(), |n| format!("IFNULL(CAST(\"{n}\" AS TEXT), '')"))
    );
    let r = c.query_row(&sql, [], |r| {
        let path: String = r.get(2)?;
        Ok(Track {
            artist: r.get(0)?,
            title: r.get(1)?,
            source: "Serato".into(),
            art: if path.is_empty() { ArtHint::None } else { ArtHint::Files(vec![serato_path(&path)]) },
        })
    });
    match r {
        Ok(t) => Ok(Some(t)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Serato stores paths without the drive on Windows (`Users/…/x.mp3`, the
/// "portable id") and without the leading `/` on macOS: put it back when
/// the path isn't absolute.
fn serato_path(p: &str) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() {
        return pb;
    }
    if cfg!(windows) {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        PathBuf::from(format!("{drive}\\")).join(pb)
    } else {
        PathBuf::from("/").join(pb)
    }
}

impl Source for Serato {
    fn kind(&self) -> NpSource {
        NpSource::Serato
    }
    fn poll(&mut self) -> Result<Option<Track>, String> {
        // Re-scan every 10 s (new set = new file; Serato 4 may appear later).
        if self.scanned.is_none_or(|t| t.elapsed() > Duration::from_secs(10)) {
            self.scanned = Some(Instant::now());
            self.db = Self::db_path();
            self.file = Self::newest_session();
        }
        if let Some(db) = self.db.clone() {
            // WAL mode: the live rows are in the -wal file between
            // checkpoints, so watch it too.
            let m = mtime(&db.with_file_name("master.sqlite-wal")).max(mtime(&db));
            if m != self.seen {
                self.seen = m;
                self.last = serato_db_track(&db)?;
            }
            return Ok(self.last.clone());
        }
        let Some(f) = self.file.clone() else {
            return Err("no Serato history found".into());
        };
        let m = mtime(&f);
        if m != self.seen {
            self.seen = m;
            let data = std::fs::read(&f).map_err(|e| e.to_string())?;
            self.last = parse_serato_session(&data);
        }
        Ok(self.last.clone())
    }
}

// ---- VirtualDJ -------------------------------------------------------------

/// VirtualDJ appends "HH:MM : Artist - Title" to History/tracklist.txt as
/// each track goes live.
#[derive(Default)]
struct VirtualDj {
    seen: Option<SystemTime>,
    last: Option<Track>,
}

impl VirtualDj {
    fn path() -> Option<PathBuf> {
        let h = home()?;
        [
            h.join("Documents/VirtualDJ/History/tracklist.txt"),
            h.join("Library/Application Support/VirtualDJ/History/tracklist.txt"),
        ]
        .into_iter()
        .find(|p| p.is_file())
    }
}

pub fn parse_vdj_line(line: &str) -> Option<Track> {
    let rest = line.split_once(" : ").map_or(line, |(_, r)| r).trim();
    if rest.is_empty() {
        return None;
    }
    let (artist, title) = rest.split_once(" - ").unwrap_or(("", rest));
    Some(Track {
        artist: artist.trim().into(),
        title: title.trim().into(),
        source: "VirtualDJ".into(),
        ..Default::default()
    })
}

/// The newest of VirtualDJ's history playlists (`History/*.m3u`, also in
/// year/month subfolders): its last entry whose file name contains `title`.
fn vdj_history_file(dir: &Path, title: &str) -> Option<PathBuf> {
    let mut lists = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), 0)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && depth < 3 {
                stack.push((p, depth + 1));
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("m3u")) {
                lists.push(p);
            }
        }
    }
    let newest = lists.into_iter().max_by_key(|p| mtime(p))?;
    let text = std::fs::read_to_string(newest).ok()?;
    let want = title.to_lowercase();
    text.lines()
        .rev()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(PathBuf::from)
        .find(|f| f.file_stem().is_some_and(|s| s.to_string_lossy().to_lowercase().contains(&want)))
}

impl Source for VirtualDj {
    fn kind(&self) -> NpSource {
        NpSource::VirtualDj
    }
    fn poll(&mut self) -> Result<Option<Track>, String> {
        let p = Self::path().ok_or("no VirtualDJ tracklist")?;
        let m = mtime(&p);
        if m != self.seen {
            self.seen = m;
            let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
            self.last = s.lines().rev().find(|l| !l.trim().is_empty()).and_then(parse_vdj_line);
            // tracklist.txt has no paths; the history playlists beside it do.
            if let Some(t) = self.last.as_mut() {
                if let Some(f) = p.parent().and_then(|d| vdj_history_file(d, &t.title)) {
                    t.art = ArtHint::Files(vec![f]);
                }
            }
        }
        Ok(self.last.clone())
    }
}

// ---- rekordbox -------------------------------------------------------------

/// rekordbox 6/7 keeps its history in an SQLCipher-encrypted `master.db`
/// (fixed key — same approach as BeatDis / pyrekordbox). On change we
/// decrypt a copy in memory and read the newest `djmdSongHistory` row.
/// Pages rekordbox hasn't checkpointed from `master.db-wal` yet aren't seen,
/// so a new track can show up with a short delay.
#[derive(Default)]
struct Rekordbox {
    seen: Option<(Option<SystemTime>, Option<SystemTime>)>,
    last: Option<Track>,
    checked: Option<Instant>,
    keys: Option<([u8; 16], rbcrypt::Keys)>,
}

impl Rekordbox {
    fn path() -> Option<PathBuf> {
        #[cfg(windows)]
        let p = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Pioneer/rekordbox/master.db"));
        #[cfg(not(windows))]
        let p = home().map(|h| h.join("Library/Pioneer/rekordbox/master.db"));
        p.filter(|p| p.is_file())
    }
}

impl Source for Rekordbox {
    fn kind(&self) -> NpSource {
        NpSource::Rekordbox
    }
    fn poll(&mut self) -> Result<Option<Track>, String> {
        let p = Self::path().ok_or("rekordbox not installed")?;
        // Decrypting is ~0.3 s of CPU: at most every 3 s, and only on change.
        if self.checked.is_some_and(|t| t.elapsed() < Duration::from_secs(3)) {
            return Ok(self.last.clone());
        }
        self.checked = Some(Instant::now());
        let wal = p.with_file_name("master.db-wal");
        let sig = (mtime(&p), mtime(&wal));
        if Some(sig) == self.seen {
            return Ok(self.last.clone());
        }
        self.seen = Some(sig);
        let data = std::fs::read(&p).map_err(|e| e.to_string())?;
        let plain = rbcrypt::decrypt(&data, &mut self.keys).map_err(|e| e.to_string())?;
        self.last = rbcrypt::latest_history(&plain).map_err(|e| e.to_string())?;
        Ok(self.last.clone())
    }
}

mod rbcrypt {
    use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::NoPadding};
    use anyhow::{Result, anyhow, bail};

    const DB_KEY: &[u8] = b"402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497";
    const PAGE: usize = 4096;
    const RESERVE: usize = 16 + 64;
    type Dec = cbc::Decryptor<aes::Aes256>;

    pub struct Keys {
        enc: [u8; 32],
    }

    pub fn decrypt(data: &[u8], cache: &mut Option<([u8; 16], Keys)>) -> Result<Vec<u8>> {
        if data.len() < PAGE || data.len() % PAGE != 0 {
            bail!("master.db is not a valid SQLCipher database");
        }
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&data[..16]);
        if cache.as_ref().is_none_or(|(s, _)| *s != salt) {
            let mut enc = [0u8; 32];
            pbkdf2::pbkdf2_hmac::<sha2::Sha512>(DB_KEY, &salt, 256_000, &mut enc);
            *cache = Some((salt, Keys { enc }));
        }
        let keys = &cache.as_ref().unwrap().1;
        let mut out = Vec::with_capacity(data.len());
        for i in 0..data.len() / PAGE {
            let pg = &data[i * PAGE..(i + 1) * PAGE];
            let off = if i == 0 { 16 } else { 0 };
            let mut iv = [0u8; 16];
            iv.copy_from_slice(&pg[PAGE - RESERVE..PAGE - RESERVE + 16]);
            let mut buf = pg[off..PAGE - RESERVE].to_vec();
            let pt = Dec::new((&keys.enc).into(), (&iv).into())
                .decrypt_padded_mut::<NoPadding>(&mut buf)
                .map_err(|e| anyhow!("decrypt page {i}: {e}"))?;
            if i == 0 {
                out.extend_from_slice(b"SQLite format 3\x00");
            }
            out.extend_from_slice(pt);
            out.extend(std::iter::repeat_n(0u8, RESERVE));
        }
        Ok(out)
    }

    pub fn latest_history(plain: &[u8]) -> Result<Option<super::Track>> {
        let tmp = std::env::temp_dir().join(format!("trippin-rb-{}.db", std::process::id()));
        std::fs::write(&tmp, plain)?;
        let res = (|| -> Result<Option<super::Track>> {
            let c = rusqlite::Connection::open_with_flags(&tmp, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let mut st = c.prepare(
                "SELECT c.Title, IFNULL(a.Name, ''), IFNULL(c.ImagePath, ''), IFNULL(c.FolderPath, '') \
                 FROM djmdSongHistory h \
                 JOIN djmdContent c ON c.ID = h.ContentID \
                 LEFT JOIN djmdArtist a ON a.ID = c.ArtistID \
                 WHERE IFNULL(h.rb_local_deleted, 0) = 0 \
                 ORDER BY h.created_at DESC, h.TrackNo DESC LIMIT 1",
            )?;
            let mut rows = st.query([])?;
            Ok(match rows.next()? {
                Some(r) => {
                    // ImagePath is rekordbox's own thumbnail, relative to
                    // its `share` folder; FolderPath is the audio file.
                    let (img, file): (String, String) = (r.get(2)?, r.get(3)?);
                    let mut art = Vec::new();
                    if !img.is_empty() {
                        if let Some(share) = super::Rekordbox::path().and_then(|p| p.parent().map(|d| d.join("share"))) {
                            art.push(share.join(img.trim_start_matches(['/', '\\'])));
                        }
                    }
                    if !file.is_empty() {
                        art.push(file.into());
                    }
                    Some(super::Track {
                        title: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                        artist: r.get(1)?,
                        source: "rekordbox".into(),
                        art: if art.is_empty() { super::ArtHint::None } else { super::ArtHint::Files(art) },
                    })
                }
                None => None,
            })
        })();
        let _ = std::fs::remove_file(&tmp);
        res
    }
}

// ---- Mixxx -----------------------------------------------------------------

/// Mixxx logs every played track to a hidden "set log" playlist
/// (`Playlists.hidden = 2`) in its plain SQLite library.
#[derive(Default)]
struct Mixxx {
    seen: Option<SystemTime>,
    last: Option<Track>,
}

impl Mixxx {
    fn path() -> Option<PathBuf> {
        let mut c = Vec::new();
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            c.push(PathBuf::from(l).join("Mixxx/mixxxdb.sqlite"));
        }
        if let Some(h) = home() {
            c.push(h.join("Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/mixxxdb.sqlite"));
            c.push(h.join("Library/Application Support/Mixxx/mixxxdb.sqlite"));
            c.push(h.join(".mixxx/mixxxdb.sqlite"));
        }
        c.into_iter().find(|p| p.is_file())
    }
}

impl Source for Mixxx {
    fn kind(&self) -> NpSource {
        NpSource::Mixxx
    }
    fn poll(&mut self) -> Result<Option<Track>, String> {
        let p = Self::path().ok_or("Mixxx not installed")?;
        let wal = p.with_file_name("mixxxdb.sqlite-wal");
        let m = mtime(&wal).max(mtime(&p));
        if m == self.seen {
            return Ok(self.last.clone());
        }
        self.seen = m;
        let c = rusqlite::Connection::open_with_flags(&p, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| e.to_string())?;
        let r = c.query_row(
            "SELECT IFNULL(l.artist, ''), IFNULL(l.title, ''), IFNULL(tl.location, ''), \
             IFNULL(l.coverart_type, 0), IFNULL(l.coverart_location, '') FROM PlaylistTracks pt \
             JOIN Playlists p ON p.id = pt.playlist_id JOIN library l ON l.id = pt.track_id \
             LEFT JOIN track_locations tl ON tl.id = l.location \
             WHERE p.hidden = 2 ORDER BY pt.pl_datetime_added DESC LIMIT 1",
            [],
            |r| {
                let (file, kind, cover): (String, i64, String) = (r.get(2)?, r.get(3)?, r.get(4)?);
                Ok(Track { artist: r.get(0)?, title: r.get(1)?, source: "Mixxx".into(), art: mixxx_art(&file, kind, &cover) })
            },
        );
        self.last = r.ok();
        Ok(self.last.clone())
    }
}

/// Mixxx's cover: `coverart_type` 2 is an image file (`coverart_location`,
/// relative to the track's folder unless absolute), otherwise the picture
/// embedded in the track.
fn mixxx_art(file: &str, kind: i64, cover: &str) -> ArtHint {
    let mut v = Vec::new();
    if kind == 2 && !cover.is_empty() {
        let c = PathBuf::from(cover);
        v.push(if c.is_absolute() { c } else { Path::new(file).parent().map_or(c.clone(), |d| d.join(&c)) });
    }
    if !file.is_empty() {
        v.push(file.into());
    }
    if v.is_empty() { ArtHint::None } else { ArtHint::Files(v) }
}

// ---- OS media session ------------------------------------------------------

#[cfg(windows)]
mod media {
    use super::*;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };

    /// Windows' System Media Transport Controls — the same info the volume
    /// flyout shows. Covers Spotify, Apple Music, Tidal, browsers, djay, etc.
    pub struct MediaSession {
        mgr: Option<Manager>,
        /// The track the thumbnail below belongs to, the thumbnail, and
        /// how many more polls re-read it (players publish the new cover a
        /// moment after the new title, so the first read can be the old one).
        art_for: (String, String),
        art: ArtHint,
        art_reads: u8,
    }

    impl MediaSession {
        pub fn new() -> Self {
            Self { mgr: None, art_for: Default::default(), art: ArtHint::None, art_reads: 0 }
        }
    }

    /// The session's thumbnail as encoded image bytes (PNG or JPEG).
    fn thumbnail(props: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties) -> Option<Vec<u8>> {
        use windows::Storage::Streams::DataReader;
        let stream = props.Thumbnail().ok()?.OpenReadAsync().ok()?.join().ok()?;
        let size = stream.Size().ok()?;
        if size == 0 || size > 16 << 20 {
            return None;
        }
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0).ok()?).ok()?;
        let got = reader.LoadAsync(size as u32).ok()?.join().ok()?;
        let mut buf = vec![0u8; got as usize];
        reader.ReadBytes(&mut buf).ok()?;
        (!buf.is_empty()).then_some(buf)
    }

    impl Source for MediaSession {
        fn kind(&self) -> NpSource {
            NpSource::MediaSession
        }
        fn live_on_start(&self) -> bool {
            true
        }
        fn poll(&mut self) -> Result<Option<Track>, String> {
            if self.mgr.is_none() {
                self.mgr = Manager::RequestAsync()
                    .and_then(|op| op.join())
                    .map_err(|e| format!("media session: {e}"))
                    .ok();
            }
            let Some(mgr) = &self.mgr else {
                return Err("media session unavailable".into());
            };
            let Ok(sess) = mgr.GetCurrentSession() else {
                return Ok(None);
            };
            let playing = sess
                .GetPlaybackInfo()
                .and_then(|i| i.PlaybackStatus())
                .is_ok_and(|s| s == Status::Playing);
            if !playing {
                return Ok(None);
            }
            let app = sess.SourceAppUserModelId().map(|s| s.to_string()).unwrap_or_default();
            let props = sess
                .TryGetMediaPropertiesAsync()
                .and_then(|op| op.join())
                .map_err(|e| e.to_string())?;
            let title = props.Title().map(|s| s.to_string()).unwrap_or_default();
            let artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
            if title.is_empty() {
                return Ok(None);
            }
            let source = if app.to_lowercase().contains("spotify") { "Spotify" } else { "Media" };
            let key = (artist.clone(), title.clone());
            if key != self.art_for {
                self.art_for = key;
                self.art = ArtHint::None;
                self.art_reads = 4;
            }
            if self.art_reads > 0 {
                self.art_reads -= 1;
                if let Some(b) = thumbnail(&props) {
                    if !matches!(&self.art, ArtHint::Bytes(old) if **old == b) {
                        self.art = ArtHint::Bytes(Arc::new(b));
                    }
                }
            }
            Ok(Some(Track { artist, title, source: source.into(), art: self.art.clone() }))
        }
    }
}

#[cfg(target_os = "macos")]
mod media {
    use super::*;

    /// Spotify / Apple Music via AppleScript (no private frameworks needed).
    pub struct MediaSession {
        /// The Music track whose artwork was last exported ("artist\ttitle"),
        /// and what came of it, so a streamed track's cover is fetched once.
        art_key: String,
        art: ArtHint,
    }

    impl MediaSession {
        pub fn new() -> Self {
            Self { art_key: String::new(), art: ArtHint::None }
        }
    }

    /// The current Music track's artwork as encoded image bytes. Streamed
    /// (Apple Music) tracks have no file to read a cover from, but Music
    /// hands over the artwork itself; AppleScript can only write it to a
    /// file, so it goes through a temp file that is read back and removed.
    fn music_artwork() -> Option<Vec<u8>> {
        let path = std::env::temp_dir().join(format!("trippin-music-art-{}", std::process::id()));
        let script = "on run argv\n\
             tell application \"Music\"\n\
             if (count of artworks of current track) is 0 then return \"\"\n\
             set d to raw data of artwork 1 of current track\n\
             end tell\n\
             set fh to open for access POSIX file (item 1 of argv) with write permission\n\
             try\n\
             set eof fh to 0\n\
             write d to fh\n\
             end try\n\
             close access fh\n\
             return \"ok\"\n\
             end run";
        let out = std::process::Command::new("osascript")
            .args(["-e", script])
            .arg(&path)
            .output()
            .ok()?;
        let ok = String::from_utf8_lossy(&out.stdout).trim() == "ok";
        let bytes = if ok { std::fs::read(&path).ok() } else { None };
        let _ = std::fs::remove_file(&path);
        bytes.filter(|b| b.len() > 64)
    }

    /// Per-app script. Only run for an app that's running: a script naming
    /// an app that isn't installed fails to compile (and asks "Where is…?").
    /// The third field is the cover: Spotify's artwork URL, or the file of
    /// a Music library track (its art is embedded; streamed tracks have
    /// none, and the `try` leaves the field empty).
    fn script(app: &str) -> String {
        let art = if app == "Spotify" {
            "set a to \"\"\n try\n set a to artwork url of current track\n end try\n"
        } else {
            "set a to \"\"\n try\n set a to POSIX path of ((location of current track) as alias)\n end try\n"
        };
        format!(
            "tell application \"{app}\"\n\
             if player state is playing then\n\
             {art}\
             return (artist of current track) & tab & (name of current track) & tab & a\n\
             end if\n\
             end tell\n\
             return \"\""
        )
    }

    fn running(proc_name: &str) -> bool {
        std::process::Command::new("pgrep")
            .args(["-xq", proc_name])
            .status()
            .is_ok_and(|s| s.success())
    }

    impl Source for MediaSession {
        fn kind(&self) -> NpSource {
            NpSource::MediaSession
        }
        fn live_on_start(&self) -> bool {
            true
        }
        fn poll(&mut self) -> Result<Option<Track>, String> {
            for app in ["Spotify", "Music"] {
                if !running(app) {
                    continue;
                }
                let out = std::process::Command::new("osascript")
                    .args(["-e", &script(app)])
                    .output()
                    .map_err(|e| e.to_string())?;
                let s = String::from_utf8_lossy(&out.stdout).trim_end_matches(['\n', '\r']).to_string();
                let mut f = s.splitn(3, '\t');
                let (artist, title, art) = (f.next().unwrap_or(""), f.next().unwrap_or(""), f.next().unwrap_or("").trim());
                if !title.is_empty() {
                    let art = if art.starts_with("http") {
                        ArtHint::Url(art.into())
                    } else if art.starts_with('/') {
                        ArtHint::Files(vec![art.into()])
                    } else if app == "Music" {
                        // Streamed: ask Music for the artwork, once a track.
                        let key = format!("{artist}\t{title}");
                        if key != self.art_key {
                            self.art_key = key;
                            self.art = music_artwork().map_or(ArtHint::None, |b| ArtHint::Bytes(Arc::new(b)));
                        }
                        self.art.clone()
                    } else {
                        ArtHint::None
                    };
                    return Ok(Some(Track { artist: artist.trim().into(), title: title.into(), source: app.into(), art }));
                }
            }
            Ok(None)
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod media {
    use super::*;
    pub struct MediaSession;
    impl MediaSession {
        pub fn new() -> Self {
            Self
        }
    }
    impl Source for MediaSession {
        fn kind(&self) -> NpSource {
            NpSource::MediaSession
        }
        fn live_on_start(&self) -> bool {
            true
        }
        fn poll(&mut self) -> Result<Option<Track>, String> {
            Err("not supported on this OS".into())
        }
    }
}

/// `trippin --nowplaying`: print what every source sees, once a second.
pub fn monitor() -> anyhow::Result<()> {
    let mode = Arc::new(Mutex::new(NpConfig { source: NpSource::Auto, delay_s: 0.0, file: String::new() }));
    let txt = std::env::temp_dir().join("trippin-nowplaying.txt");
    let np = start(mode, txt, RemoteNp::default());
    loop {
        std::thread::sleep(Duration::from_millis(1500));
        let st = np.lock().unwrap_or_else(|e| e.into_inner()).clone();
        println!(
            "NOW: {}",
            st.track.as_ref().map_or("—".into(), |t| format!("{}  [{}]  art: {}", t.line(), t.source, t.art.describe()))
        );
        for (k, v) in &st.status {
            println!("    {k:<16} {v}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(tag: &[u8], body: &[u8]) -> Vec<u8> {
        let mut v = tag.to_vec();
        v.extend_from_slice(&(body.len() as u32).to_be_bytes());
        v.extend_from_slice(body);
        v
    }
    fn field_str(id: u32, s: &str) -> Vec<u8> {
        let b: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
        let mut v = id.to_be_bytes().to_vec();
        v.extend_from_slice(&(b.len() as u32).to_be_bytes());
        v.extend_from_slice(&b);
        v
    }
    fn field_u32(id: u32, n: u32) -> Vec<u8> {
        let mut v = id.to_be_bytes().to_vec();
        v.extend_from_slice(&4u32.to_be_bytes());
        v.extend_from_slice(&n.to_be_bytes());
        v
    }

    #[test]
    fn serato_session_newest_start_wins() {
        let entry = |title: &str, artist: &str, start: u32| {
            let mut adat = field_str(6, title);
            adat.extend(field_str(7, artist));
            adat.extend(field_u32(28, start));
            chunk(b"oent", &chunk(b"adat", &adat))
        };
        let mut f = chunk(b"vrsn", &"1.0/Serato Scratch LIVE Review".encode_utf16().flat_map(|u| u.to_be_bytes()).collect::<Vec<u8>>());
        f.extend(entry("First", "A", 100));
        f.extend(entry("Third", "C", 300));
        f.extend(entry("Second", "B", 200));
        let t = parse_serato_session(&f).unwrap();
        assert_eq!(t.title, "Third");
        assert_eq!(t.artist, "C");
    }

    #[test]
    fn serato_db_latest_played_wins() {
        let dir = std::env::temp_dir();
        let db = dir.join(format!("trippin-serato-test-{}.db", std::process::id()));
        {
            let c = rusqlite::Connection::open(&db).unwrap();
            c.execute_batch(
                "CREATE TABLE history_entry (id INTEGER PRIMARY KEY AUTOINCREMENT, \
                 artist TEXT NOT NULL DEFAULT '', name TEXT NOT NULL DEFAULT '', \
                 played INTEGER NOT NULL DEFAULT 0, start_time INTEGER NOT NULL DEFAULT 0); \
                 INSERT INTO history_entry (artist, name, played, start_time) VALUES \
                 ('A', 'First', 1, 100), ('C', 'Newest played', 1, 300), \
                 ('B', 'Older', 1, 200), ('Cued', 'Not yet played', 0, 400);",
            )
            .unwrap();
        }
        let t = serato_db_track(&db).unwrap().unwrap();
        assert_eq!((t.artist.as_str(), t.title.as_str()), ("C", "Newest played"));
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn vdj_line() {
        let t = parse_vdj_line("21:05 : Calvin Harris - Free").unwrap();
        assert_eq!((t.artist.as_str(), t.title.as_str()), ("Calvin Harris", "Free"));
    }
}
