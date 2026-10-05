//! LAN remote for the companion iOS app — and any other client that speaks
//! the protocol (a browser remote falls out for free).
//!
//! One WebSocket listener on `Settings::remote_port`, advertised over mDNS
//! as `_trippin._tcp`. Text frames are JSON both ways:
//!
//! ```text
//! C→S  {"cmd":"hello","v":1,"name":"iPad","pin":"1234"}   — must be first
//! C→S  {"cmd":"action","action":"NextScene"}              — any Action variant
//! C→S  {"cmd":"goto_scene"|"queue_next","scene":"laser_show"|12}
//! C→S  {"cmd":"show_clip","clip":"hiphop_01"}
//! C→S  {"cmd":"look","look":"club-red"|"3"}        — a saved Look by id or slot
//! C→S  {"cmd":"set","key":"palette","value":"sunset"}     — SetKey whitelist
//! C→S  {"cmd":"transport","op":"toggle"|"stop"|"seek","pos":12.0}
//! C→S  {"cmd":"now_playing","artist":"..","title":"..","source":"Serato"}
//!      — the companion agent's push; empty strings clear it. A client's
//!      pushed track is retracted when its socket drops.
//! C→S  {"cmd":"thumb","scene":"laser_show"}
//!
//! S→C  {"type":"hello","ok":true,...} — scene/clip/palette/action lists;
//!      `scene_titles`/`clip_titles` are the display names parallel with
//!      `scenes`/`clips` — the ids stay the keys for every command. `looks`
//!      lists the saved Looks as `{id,name,slot}` (absent from older servers).
//! S→C  {"type":"state",...}           — show state, ~10 Hz; `scene_name`
//!      etc are ids, `scene_title`/`clip_title` the display names.
//! S→C  {"type":"thumb","scene":...,"png_b64":...}
//! S→C  {"type":"err","msg":...}
//! ```
//!
//! Commands reach the event loop through `Hooks::cmd`, exactly like MIDI
//! pads, so remote presses record into an armed timeline like hotkeys do.
//!
//! Threads: an accept loop, one per client, and a state pusher. None holds
//! a lock across socket I/O — the pusher serialises once and hands each
//! client a copy through a bounded channel; a client whose queue stays full
//! (wedged reader) is disconnected.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

use crate::config::{Action, Fx};
use crate::nowplaying::Track;
use crate::timeline::SongCtl;

/// A scene by index or (case-insensitive) name.
#[derive(Debug)]
pub enum SceneSel {
    Index(usize),
    Name(String),
}

impl SceneSel {
    pub fn resolve(&self, names: &[String]) -> Option<usize> {
        match self {
            SceneSel::Index(i) => (*i < names.len()).then_some(*i),
            SceneSel::Name(n) => names
                .iter()
                .position(|s| s == n)
                .or_else(|| names.iter().position(|s| s.eq_ignore_ascii_case(n))),
        }
    }
}

/// The settings knobs a remote may turn — a whitelist, not open access.
/// Ranges match the panel sliders.
#[derive(Debug)]
pub enum SetKey {
    Palette(String),
    Fx(Fx),
    FxAmt(f32),
    FxAuto(bool),
    DancerSize(f32),
    DancerTrails(bool),
    PhraseBars(u32),
    CutOnDrops(bool),
    LatencyMs(f32),
    NpSize(f32),
    BrandOpacity(f32),
    TickerSpeed(f32),
    TickerText(String),
}

/// A command a remote client sent — arrives on the event thread as
/// `AppEvent::Remote`. Plain data so it stays `Send`.
#[derive(Debug)]
pub enum RemoteCmd {
    Act(Action),
    GoToScene(SceneSel),
    QueueNext(SceneSel),
    ShowClip(String),
    /// Recall a saved Look by id, or by slot number ("1".."8").
    Look(String),
    Set(SetKey),
    Transport(SongCtl),
    /// Now-playing push from a companion agent; `client` tags it so a
    /// disconnect retracts only that client's track.
    NowPlaying {
        client: u64,
        track: Option<Track>,
    },
    /// Render + push a scene thumbnail to the asking client.
    Thumb(String),
}

/// What the server needs from the app — closures instead of access to
/// `Shared`, so tests can drive the server with canned data.
pub struct Hooks {
    /// Post a command to the event loop (`proxy.send_event`).
    pub cmd: Box<dyn Fn(RemoteCmd) + Send + Sync>,
    /// The `hello` reply body: scene/clip/palette/action lists + version.
    pub meta: Box<dyn Fn() -> Value + Send + Sync>,
    /// The live show state, polled ~10 Hz.
    pub state: Box<dyn Fn() -> Value + Send + Sync>,
    /// A rendered thumbnail as PNG bytes ("scene:<name>" in `shared.thumbs`).
    pub thumb: Box<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SceneArg {
    Index(usize),
    Name(String),
}

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum In {
    Hello {
        pin: String,
        #[serde(default)]
        #[allow(dead_code)]
        name: String,
    },
    Action {
        action: Action,
    },
    #[serde(alias = "go_to_scene")]
    GotoScene {
        scene: SceneArg,
    },
    QueueNext {
        scene: SceneArg,
    },
    ShowClip {
        clip: String,
    },
    Look {
        look: String,
    },
    Set {
        key: String,
        value: Value,
    },
    Transport {
        op: String,
        #[serde(default)]
        pos: Option<f64>,
    },
    NowPlaying {
        #[serde(default)]
        artist: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        source: Option<String>,
    },
    Thumb {
        scene: String,
    },
}

/// A thumbnail a client asked for and the render thread hasn't produced yet.
struct Pending {
    client: u64,
    scene: String,
    since: Instant,
}

pub struct Server {
    /// The bound port (differs from the request when 0 was asked for).
    pub port: u16,
    /// The PIN clients must send in `hello` — checked so `about_to_wait`
    /// knows to respawn when it changes.
    pub pin: String,
    clients: Arc<Mutex<HashMap<u64, mpsc::SyncSender<String>>>>,
    stop: Arc<AtomicBool>,
    handles: Vec<thread::JoinHandle<()>>,
    /// Dropping unregisters the Bonjour service.
    _mdns: Option<mdns_sd::ServiceDaemon>,
}

impl Server {
    /// Number of clients currently connected (panel status line).
    pub fn client_count(&self) -> usize {
        self.clients.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Bind the listener on all interfaces, advertise `_trippin._tcp`,
    /// spawn the threads. `port` 0 picks a free port (tests).
    pub fn start(port: u16, pin: &str, hooks: Hooks) -> Result<Server> {
        Self::start_on("0.0.0.0", port, pin, hooks)
    }

    /// `start` with an explicit bind address — tests use loopback so the
    /// Windows firewall prompt isn't part of the test.
    pub fn start_on(addr: &str, port: u16, pin: &str, hooks: Hooks) -> Result<Server> {
        let listener = TcpListener::bind((addr, port))?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;

        let hooks = Arc::new(hooks);
        let clients: Arc<Mutex<HashMap<u64, mpsc::SyncSender<String>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending = Arc::new(Mutex::new(Vec::<Pending>::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let mut handles = Vec::new();

        {
            let (clients, pending, stop, hooks, pin) = (
                clients.clone(),
                pending.clone(),
                stop.clone(),
                hooks.clone(),
                pin.to_string(),
            );
            handles.push(thread::spawn(move || {
                let mut next_id = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let id = next_id;
                            next_id += 1;
                            let (tx, rx) = mpsc::sync_channel::<String>(64);
                            clients
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .insert(id, tx);
                            let (clients, pending, stop, hooks, pin) = (
                                clients.clone(),
                                pending.clone(),
                                stop.clone(),
                                hooks.clone(),
                                pin.clone(),
                            );
                            thread::spawn(move || {
                                // A wedged client mustn't take the accept
                                // loop down — and the entry below cleans up
                                // either way.
                                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                                    || client(id, stream, hooks, pin, rx, pending, stop),
                                ));
                                clients
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .remove(&id);
                            });
                        }
                        Err(e) if e.kind() == ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(25));
                        }
                        Err(_) => thread::sleep(Duration::from_millis(250)),
                    }
                }
            }));
        }

        {
            let (clients, pending, stop, hooks) =
                (clients.clone(), pending, stop.clone(), hooks);
            handles.push(thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(100));
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let state = (hooks.state)().to_string();
                    // Thumbnails land in shared.thumbs a frame or two after
                    // the request — keep them queued until they do.
                    {
                        let mut pending = pending.lock().unwrap_or_else(|e| e.into_inner());
                        let mut i = 0;
                        while i < pending.len() {
                            let p = &pending[i];
                            if let Some(png) = (hooks.thumb)(&p.scene) {
                                let msg = json!({
                                    "type": "thumb",
                                    "scene": p.scene,
                                    "png_b64": base64::engine::general_purpose::STANDARD.encode(png),
                                })
                                .to_string();
                                if let Some(tx) =
                                    clients.lock().unwrap_or_else(|e| e.into_inner()).get(&p.client)
                                {
                                    let _ = tx.try_send(msg);
                                }
                                pending.remove(i);
                            } else if p.since.elapsed() > Duration::from_secs(10) {
                                pending.remove(i);
                            } else {
                                i += 1;
                            }
                        }
                    }
                    // A full queue means the reader thread is wedged — drop
                    // it (its rx disconnect tears the socket down).
                    clients
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .retain(|_, tx| tx.try_send(state.clone()).is_ok());
                }
            }));
        }

        Ok(Server {
            port,
            pin: pin.to_string(),
            clients,
            stop,
            handles,
            // Advertising only makes sense on a reachable interface.
            _mdns: (addr != "127.0.0.1" && addr != "::1").then(|| advertise(port)).flatten(),
        })
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Accept poll ≤25 ms, pusher sleep ≤100 ms — join is cheap and
        // keeps toggle-off deterministic. Client threads see `stop` (or a
        // disconnected tx) and leave on their own.
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

/// Bonjour advert: "Trippin on <host>" as `_trippin._tcp` so NWBrowser
/// finds the rig without typing an IP. Failure is fine — the panel still
/// shows the address to type.
fn advertise(port: u16) -> Option<mdns_sd::ServiceDaemon> {
    let daemon = mdns_sd::ServiceDaemon::new().ok()?;
    let ip = local_ip()?;
    let host = host_name();
    let instance = format!("Trippin on {}", host.trim_end_matches(".local."));
    let props = std::collections::HashMap::from([(
        "version".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    )]);
    let info = mdns_sd::ServiceInfo::new(
        "_trippin._tcp.local.",
        &instance,
        &host,
        &*ip,
        port,
        props,
    )
    .ok()?;
    daemon.register(info).ok()?;
    Some(daemon)
}

/// The LAN address to advertise and show for pairing. Asking the routing
/// table which interface reaches the mDNS multicast group isn't enough:
/// on the owner's PC that answer was Tailscale's adapter with only a
/// self-assigned 169.254.x address, so phones were told to connect to an
/// unreachable IP. Instead, list the interfaces and prefer a private LAN
/// address (192.168/16, then 10/8 and 172.16/12), skipping loopback,
/// link-local and CGNAT (100.64/10, Tailscale); among equals, the one the
/// default route uses.
pub fn local_ip() -> Option<String> {
    use std::net::{IpAddr, Ipv4Addr};
    fn rank(ip: Ipv4Addr) -> Option<u8> {
        let o = ip.octets();
        if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() || ip.is_multicast() {
            return None;
        }
        Some(match o {
            [192, 168, ..] => 0,
            [10, ..] => 1,
            [172, b, ..] if (16..=31).contains(&b) => 1,
            [100, b, ..] if (64..=127).contains(&b) => 3,
            _ => 2,
        })
    }
    // The default route's source address: a UDP "connect" sends nothing.
    let routed = UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("8.8.8.8:80").map(|_| s))
        .and_then(|s| s.local_addr())
        .ok()
        .and_then(|a| match a.ip() {
            IpAddr::V4(v) => Some(v),
            _ => None,
        });
    let mut best: Option<(u8, Ipv4Addr)> = None;
    for i in if_addrs::get_if_addrs().unwrap_or_default() {
        let IpAddr::V4(v) = i.ip() else { continue };
        let Some(r) = rank(v) else { continue };
        let better = match best {
            None => true,
            Some((b, _)) => r < b || (r == b && Some(v) == routed),
        };
        if better {
            best = Some((r, v));
        }
    }
    best.map(|(_, ip)| ip.to_string())
}

/// "<host>.local." for the mDNS SRV target — stable enough for Bonjour.
fn host_name() -> String {
    let raw = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "trippin".into())
        .to_lowercase();
    let clean: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("{}.local.", clean.trim_matches('-'))
}

/// A fresh 4-digit pairing PIN.
pub fn new_pin() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
        .unwrap_or(0);
    format!("{:04}", n % 9000 + 1000)
}

/// One client: blocking WS handshake + hello gate (5 s budget), then a
/// loop that drains its outbound queue and dispatches incoming commands.
fn client(
    id: u64,
    stream: TcpStream,
    hooks: Arc<Hooks>,
    pin: String,
    out_rx: mpsc::Receiver<String>,
    pending: Arc<Mutex<Vec<Pending>>>,
    stop: Arc<AtomicBool>,
) {
    // Windows hands accepted sockets the listener's non-blocking flag —
    // the timeouts below are dead letters until this is switched back.
    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    // So a client that stops reading can't wedge this thread in send().
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    // A plain browser GET (no WS upgrade) gets the test-remote page —
    // browsing to http://<ip>:<port> is the no-app way to drive the rig.
    if serve_page(&stream) {
        return;
    }
    let mut ws = match tungstenite::accept(stream) {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("remote: handshake failed: {e}");
            return;
        }
    };
    // First frame must be hello with the PIN — everything else is refused.
    let ok = match read_in(&mut ws) {
        Some(In::Hello { pin: p, .. }) if p == pin => {
            ws.send(Message::Text((hooks.meta)().to_string().into()))
                .is_ok()
        }
        Some(In::Hello { .. }) => {
            let _ = ws.send(Message::Text(
                json!({"type": "err", "msg": "bad pin"})
                    .to_string()
                    .into(),
            ));
            // Cheap rate-limit: a brute-force PIN guess costs half a second.
            thread::sleep(Duration::from_millis(500));
            false
        }
        _ => false,
    };
    if !ok {
        let _ = ws.close(None);
        return;
    }
    let _ = ws.get_mut().set_read_timeout(Some(Duration::from_millis(20)));
    'read: loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        loop {
            match out_rx.try_recv() {
                Ok(m) => {
                    if ws.send(Message::Text(m.into())).is_err() {
                        break 'read;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break 'read,
            }
        }
        match ws.read() {
            Ok(Message::Text(t)) => {
                let _ = ws.flush();
                if let Some(err) = handle(&t, id, &hooks, &pending) {
                    if ws.send(Message::Text(err.into())).is_err() {
                        break 'read;
                    }
                }
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                let _ = ws.flush();
            }
            Ok(Message::Close(_)) => break,
            // WouldBlock/TimedOut: the 20 ms read budget elapsed — loop and
            // drain the outbound queue.
            Err(tungstenite::Error::Io(e))
                if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {}
            Err(_) => break,
            _ => {}
        }
    }
    let _ = ws.close(None);
    // Retract this client's now-playing push, if it made one — a dead
    // agent shouldn't leave a stale track on the overlay. The main loop
    // ignores this when the inbox holds another client's track.
    (hooks.cmd)(RemoteCmd::NowPlaying {
        client: id,
        track: None,
    });
}

/// Peek at the request: if it's an HTTP GET that isn't a WebSocket
/// upgrade, answer with the single-file remote and report handled.
/// Anything else is left for `tungstenite::accept` (we never consumed
/// bytes — `peek` only looks).
fn serve_page(stream: &TcpStream) -> bool {
    use std::io::Write;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut buf = [0u8; 4096];
    loop {
        let n = match stream.peek(&mut buf) {
            Ok(0) => return false,
            Ok(n) => n,
            // Timeout / WouldBlock: nothing arrived yet — a slow client
            // gets the WS path anyway, where the same deadline applies.
            Err(_) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(_) => return false,
        };
        let head = &buf[..n];
        if !head.starts_with(b"GET") {
            return false;
        }
        // Wait for the whole header block before judging upgrade-ness.
        if !head.windows(4).any(|w| w == b"\r\n\r\n") && n < buf.len() {
            if Instant::now() > deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        let text = String::from_utf8_lossy(head).to_lowercase();
        if text.contains("sec-websocket-key") {
            return false;
        }
        // Drain the request — `peek` never consumed it, and closing with
        // unread inbound data RSTs the socket (the page could be lost).
        use std::io::Read;
        let mut r = stream;
        let _ = r.read_exact(&mut vec![0u8; n]);
        let page = include_str!("../tools/remote_test.html");
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{}",
            page.len(),
            page
        );
        let mut w = stream;
        let _ = w.write_all(resp.as_bytes());
        let _ = w.flush();
        let _ = stream.shutdown(std::net::Shutdown::Both);
        return true;
    }
}

/// Blocking read of one text frame, parsed as `In`. Returns None on
/// timeout, close, error or an unrecognised shape.
fn read_in(ws: &mut WebSocket<TcpStream>) -> Option<In> {
    match ws.read() {
        Ok(Message::Text(t)) => serde_json::from_str(&t).ok(),
        _ => None,
    }
}

/// A post-hello command frame. Returns an `err` reply to send back when
/// the frame didn't parse or named something it shouldn't.
fn handle(text: &str, client: u64, hooks: &Hooks, pending: &Mutex<Vec<Pending>>) -> Option<String> {
    let err = |msg: String| Some(json!({"type": "err", "msg": msg}).to_string());
    let msg = match serde_json::from_str::<In>(text) {
        Ok(m) => m,
        Err(e) => return err(format!("bad frame: {e}")),
    };
    match parse(msg, client) {
        Ok(Some(RemoteCmd::Thumb(scene))) => {
            pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(Pending {
                    client,
                    scene: scene.clone(),
                    since: Instant::now(),
                });
            (hooks.cmd)(RemoteCmd::Thumb(scene));
            None
        }
        Ok(Some(c)) => {
            (hooks.cmd)(c);
            None
        }
        Ok(None) => None,
        Err(e) => err(format!("{e:#}")),
    }
}

fn parse(msg: In, client: u64) -> Result<Option<RemoteCmd>> {
    Ok(match msg {
        In::Hello { .. } => None, // a second hello is a no-op
        In::Action { action } => Some(RemoteCmd::Act(action)),
        In::GotoScene { scene } => Some(RemoteCmd::GoToScene(match scene {
            SceneArg::Index(i) => SceneSel::Index(i),
            SceneArg::Name(n) => SceneSel::Name(n),
        })),
        In::QueueNext { scene } => Some(RemoteCmd::QueueNext(match scene {
            SceneArg::Index(i) => SceneSel::Index(i),
            SceneArg::Name(n) => SceneSel::Name(n),
        })),
        In::ShowClip { clip } => Some(RemoteCmd::ShowClip(clip)),
        In::Look { look } => Some(RemoteCmd::Look(look)),
        In::Set { key, value } => Some(RemoteCmd::Set(parse_set(&key, value)?)),
        In::Transport { op, pos } => Some(RemoteCmd::Transport(match op.as_str() {
            "toggle" | "play" | "pause" => SongCtl::Toggle,
            "stop" => SongCtl::Stop,
            "seek" => SongCtl::Seek(pos.ok_or_else(|| anyhow!("seek needs pos"))?),
            other => return Err(anyhow!("unknown transport op {other:?}")),
        })),
        In::NowPlaying {
            artist,
            title,
            source,
        } => {
            let (artist, title) = (
                artist.unwrap_or_default().trim().to_string(),
                title.unwrap_or_default().trim().to_string(),
            );
            // Both empty = a retract; otherwise it's the pushed track.
            let track = (!artist.is_empty() || !title.is_empty()).then(|| Track {
                artist,
                title,
                source: source.unwrap_or_default(),
            });
            Some(RemoteCmd::NowPlaying { client, track })
        }
        In::Thumb { scene } => Some(RemoteCmd::Thumb(scene)),
    })
}

pub(crate) fn parse_set(key: &str, value: Value) -> Result<SetKey> {
    let f = |r: std::ops::RangeInclusive<f32>| -> Result<f32> {
        let v = value
            .as_f64()
            .ok_or_else(|| anyhow!("{key}: expected a number"))? as f32;
        Ok(v.clamp(*r.start(), *r.end()))
    };
    let flag = || {
        value
            .as_bool()
            .ok_or_else(|| anyhow!("{key}: expected true/false"))
    };
    Ok(match key {
        "palette" => SetKey::Palette(
            value
                .as_str()
                .ok_or_else(|| anyhow!("palette: expected a name"))?
                .to_string(),
        ),
        "fx" => SetKey::Fx(serde_json::from_value(value)?),
        "fx_amt" => SetKey::FxAmt(f(0.0..=1.0)?),
        "fx_auto" => SetKey::FxAuto(flag()?),
        "dancer_size" => SetKey::DancerSize(f(0.4..=1.0)?),
        "dancer_trails" => SetKey::DancerTrails(flag()?),
        "phrase_bars" => SetKey::PhraseBars(
            value
                .as_u64()
                .ok_or_else(|| anyhow!("phrase_bars: expected a count"))?
                .clamp(1, 128) as u32,
        ),
        "cut_on_drops" => SetKey::CutOnDrops(flag()?),
        "latency_ms" => SetKey::LatencyMs(f(0.0..=200.0)?),
        "np_size" => SetKey::NpSize(f(0.5..=2.0)?),
        "brand_opacity" => SetKey::BrandOpacity(f(0.1..=1.0)?),
        "ticker_speed" => SetKey::TickerSpeed(f(0.3..=3.0)?),
        "ticker_text" => SetKey::TickerText(
            value
                .as_str()
                .ok_or_else(|| anyhow!("ticker_text: expected a string"))?
                .to_string(),
        ),
        other => return Err(anyhow!("{other:?} isn't remotely settable")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn hooks(cmd: mpsc::Sender<RemoteCmd>) -> Hooks {
        Hooks {
            cmd: Box::new(move |c| {
                let _ = cmd.send(c);
            }),
            meta: Box::new(|| {
                json!({"type": "hello", "ok": true, "scenes": ["void", "comets"]})
            }),
            state: Box::new(|| json!({"type": "state", "bpm": 126.0})),
            thumb: Box::new(|name| (name == "comets").then(|| vec![1u8, 2, 3, 4])),
        }
    }

    fn connect(port: u16) -> WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>> {
        tungstenite::connect(format!("ws://127.0.0.1:{port}/"))
            .expect("connect")
            .0
    }

    fn read_text(ws: &mut WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>) -> Value {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match ws.read() {
                Ok(Message::Text(t)) => return serde_json::from_str(&t).unwrap(),
                Ok(_) => {}
                Err(_) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => panic!("read failed: {e}"),
            }
            assert!(Instant::now() < deadline, "timed out waiting for a frame");
        }
    }

    #[test]
    fn hello_state_commands_thumb() {
        let (tx, rx) = mpsc::channel();
        let server = Server::start_on("127.0.0.1", 0, "1234", hooks(tx)).unwrap();
        let mut ws = connect(server.port);
        ws.send(Message::Text(
            r#"{"cmd":"hello","pin":"1234","name":"test"}"#.into(),
        ))
        .unwrap();
        let hello = read_text(&mut ws);
        assert_eq!(hello["ok"], true);
        assert_eq!(hello["scenes"], json!(["void", "comets"]));
        let state = read_text(&mut ws);
        assert_eq!(state["type"], "state");

        ws.send(Message::Text(r#"{"cmd":"action","action":"NextScene"}"#.into()))
            .unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            RemoteCmd::Act(Action::NextScene)
        ));

        ws.send(Message::Text(
            r#"{"cmd":"goto_scene","scene":"comets"}"#.into(),
        ))
        .unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            RemoteCmd::GoToScene(SceneSel::Name(n)) if n == "comets"
        ));

        // A saved Look by id, and a Look pad as an ordinary action.
        ws.send(Message::Text(r#"{"cmd":"look","look":"club-red"}"#.into())).unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            RemoteCmd::Look(id) if id == "club-red"
        ));
        ws.send(Message::Text(r#"{"cmd":"action","action":"Look3"}"#.into())).unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            RemoteCmd::Act(Action::Look3)
        ));

        // Thumbnail: the hook answers immediately; the client thread must
        // have queued the render command AND the PNG push must arrive.
        ws.send(Message::Text(r#"{"cmd":"thumb","scene":"comets"}"#.into()))
            .unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            RemoteCmd::Thumb(n) if n == "comets"
        ));
        let thumb = read_text(&mut ws);
        assert_eq!(thumb["type"], "thumb");
        assert_eq!(thumb["scene"], "comets");
        assert_eq!(thumb["png_b64"], "AQIDBA==");
    }

    #[test]
    fn now_playing_push_and_disconnect_retract() {
        let (tx, rx) = mpsc::channel();
        let server = Server::start_on("127.0.0.1", 0, "1234", hooks(tx)).unwrap();
        let mut ws = connect(server.port);
        ws.send(Message::Text(r#"{"cmd":"hello","pin":"1234","name":"agent"}"#.into()))
            .unwrap();
        let _ = read_text(&mut ws); // hello reply

        ws.send(Message::Text(
            r#"{"cmd":"now_playing","artist":"A","title":"T","source":"Serato"}"#.into(),
        ))
        .unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            RemoteCmd::NowPlaying { track: Some(t), .. }
                if t.artist == "A" && t.title == "T" && t.source == "Serato"
        ));

        // Dropping the socket retracts the pushed track (client-tagged —
        // the app only clears it if the inbox still holds this client's).
        ws.close(None).unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            RemoteCmd::NowPlaying { track: None, .. }
        ));
    }

    #[test]
    fn bad_pin_is_refused() {
        let (tx, _rx) = mpsc::channel();
        let server = Server::start_on("127.0.0.1", 0, "1234", hooks(tx)).unwrap();
        let mut ws = connect(server.port);
        ws.send(Message::Text(r#"{"cmd":"hello","pin":"9999"}"#.into()))
            .unwrap();
        let msg = read_text(&mut ws);
        assert_eq!(msg["type"], "err");
        // And the socket closes after the error.
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match ws.read() {
                Err(_) | Ok(Message::Close(_)) => break,
                Ok(_) => {}
            }
            assert!(Instant::now() < deadline, "socket never closed");
        }
    }

    #[test]
    fn frames_must_start_with_hello() {
        let (tx, rx) = mpsc::channel();
        let server = Server::start_on("127.0.0.1", 0, "1234", hooks(tx)).unwrap();
        let mut ws = connect(server.port);
        ws.send(Message::Text(r#"{"cmd":"action","action":"NextScene"}"#.into()))
            .unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
    }

    #[test]
    fn browser_get_gets_the_page() {
        let (tx, _rx) = mpsc::channel();
        let server = Server::start_on("127.0.0.1", 0, "1234", hooks(tx)).unwrap();
        let mut s = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
        use std::io::{Read, Write};
        s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut body = String::new();
        s.read_to_string(&mut body).unwrap();
        assert!(body.starts_with("HTTP/1.1 200"));
        assert!(body.contains("Trippin remote"));
    }

    #[test]
    fn set_keys_parse_and_clamp() {
        assert!(matches!(
            parse_set("fx_amt", json!(7.0)).unwrap(),
            SetKey::FxAmt(v) if v == 1.0
        ));
        assert!(matches!(
            parse_set("palette", json!("sunset")).unwrap(),
            SetKey::Palette(p) if p == "sunset"
        ));
        assert!(parse_set("midi_in", json!("x")).is_err());
    }
}
