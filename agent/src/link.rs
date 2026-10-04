//! The network half: discover Trippin rigs over Bonjour, keep one
//! WebSocket up, and push now-playing changes through it (the
//! `{"cmd":"now_playing", …}` frame from `src/remote.rs`).

use std::io::ErrorKind;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde_json::json;
use tungstenite::{Message, WebSocket};

use crate::np;
use crate::Cfg;

/// What the connector thread is doing, for the tray menu and window.
#[derive(Default)]
pub struct Link {
    /// One-line status ("connected to …", "searching for a rig", "bad PIN").
    pub status: String,
    /// mDNS-resolved rigs: (label, "host:port").
    pub rigs: Vec<(String, String)>,
}

pub type SharedLink = Arc<Mutex<Link>>;

pub fn spawn(
    cfg: Arc<Mutex<Cfg>>,
    cfg_serial: Arc<AtomicU64>,
    np: np::SharedNowPlaying,
    link: SharedLink,
) {
    discover(link.clone());
    std::thread::Builder::new()
        .name("link".into())
        .spawn(move || connect_loop(cfg, cfg_serial, np, link))
        .ok();
}

fn set_status(link: &SharedLink, s: impl Into<String>) {
    link.lock().unwrap_or_else(|e| e.into_inner()).status = s.into();
}

/// Bonjour browse for `_trippin._tcp`; keeps `link.rigs` current.
/// Runs for the life of the process on its own thread.
fn discover(link: SharedLink) {
    std::thread::Builder::new()
        .name("discover".into())
        .spawn(move || {
            let Ok(daemon) = mdns_sd::ServiceDaemon::new() else { return };
            let Ok(rx) = daemon.browse("_trippin._tcp.local.") else {
                return;
            };
            let mut rigs: Vec<(String, String)> = Vec::new();
            while let Ok(ev) = rx.recv() {
                match ev {
                    mdns_sd::ServiceEvent::ServiceResolved(info) => {
                        let label = info
                            .get_fullname()
                            .split("._trippin._tcp")
                            .next()
                            .unwrap_or("Trippin")
                            .to_string();
                        // Prefer a plain IPv4 address for the ws:// target.
                        let addr = info
                            .get_addresses()
                            .iter()
                            .find(|a| a.is_ipv4())
                            .or_else(|| info.get_addresses().iter().next())
                            .map(|a| format!("{a}:{}", info.get_port()));
                        if let Some(addr) = addr {
                            rigs.retain(|(_, a)| *a != addr);
                            rigs.push((label, addr));
                        }
                    }
                    mdns_sd::ServiceEvent::ServiceRemoved(_, fullname) => {
                        let label = fullname.split("._trippin._tcp").next().unwrap_or("");
                        rigs.retain(|(l, _)| l != label);
                    }
                    _ => {}
                }
                link.lock().unwrap_or_else(|e| e.into_inner()).rigs = rigs.clone();
            }
        })
        .ok();
}

/// Forever: pick a rig (manual `host`, else first discovered), connect,
/// run the session until it drops or the config changes, repeat.
fn connect_loop(
    cfg: Arc<Mutex<Cfg>>,
    cfg_serial: Arc<AtomicU64>,
    np: np::SharedNowPlaying,
    link: SharedLink,
) {
    loop {
        let (host, pin) = {
            let c = cfg.lock().unwrap_or_else(|e| e.into_inner());
            (c.host.trim().to_string(), c.pin.trim().to_string())
        };
        let addr = if !host.is_empty() {
            host
        } else {
            // The guard must drop before the match: scrutinee temporaries
            // live to the end of the match, so `set_status` in the None
            // arm would re-lock `link` on the same thread — a deadlock
            // that also froze the first eframe pass (no window/tray).
            let first_rig = link
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .rigs
                .first()
                .map(|(_, a)| a.clone());
            match first_rig {
                Some(a) => a,
                None => {
                    set_status(&link, "searching for a Trippin rig…");
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
            }
        };
        set_status(&link, format!("connecting to {addr}"));
        let seen_serial = cfg_serial.load(Ordering::Relaxed);
        match session(&addr, &pin, &cfg_serial, seen_serial, &np, &link) {
            Ok(()) => {} // config changed — reconnect straight away
            Err(e) => {
                set_status(&link, format!("{addr}: {e:#}"));
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    }
}

/// One connection: hello + PIN gate, then a loop that polls the detector
/// and drains the server's state pushes (a client that stops reading gets
/// dropped by the server's bounded queue).
fn session(
    addr: &str,
    pin: &str,
    cfg_serial: &AtomicU64,
    seen_serial: u64,
    np: &np::SharedNowPlaying,
    link: &SharedLink,
) -> Result<()> {
    let (mut ws, _) = tungstenite::connect(format!("ws://{addr}/"))
        .map_err(|e| anyhow!("connect: {e}"))?;
    if let tungstenite::stream::MaybeTlsStream::Plain(s) = ws.get_mut() {
        s.set_read_timeout(Some(Duration::from_secs(5)))?;
    }
    ws.send(Message::Text(
        json!({"cmd": "hello", "v": 1, "name": "Trippin agent", "pin": pin}).to_string().into(),
    ))?;
    match ws.read() {
        Ok(Message::Text(t)) => {
            let v: serde_json::Value = serde_json::from_str(&t)?;
            if v["type"] == "err" {
                return Err(anyhow!("{}", v["msg"].as_str().unwrap_or("refused")));
            }
        }
        _ => return Err(anyhow!("no hello reply")),
    }
    // From here: poll the socket rather than block, so outbound frames
    // get sent promptly between state pushes.
    if let tungstenite::stream::MaybeTlsStream::Plain(s) = ws.get_mut() {
        let _ = s.set_read_timeout(Some(Duration::from_millis(150)));
    }
    set_status(link, format!("connected to {addr}"));

    let mut sent: Option<np::Track> = None;
    let mut sent_stale = true; // push the current track on (re)connect
    let mut next_poll = Instant::now();
    loop {
        if cfg_serial.load(Ordering::Relaxed) != seen_serial {
            return Ok(());
        }
        if Instant::now() >= next_poll {
            next_poll = Instant::now() + Duration::from_millis(300);
            let heard = np.lock().unwrap_or_else(|e| e.into_inner()).track.clone();
            if heard != sent || sent_stale {
                push(&mut ws, heard.as_ref())?;
                sent = heard;
                sent_stale = false;
            }
        }
        match ws.read() {
            Ok(Message::Text(_)) | Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => return Err(anyhow!("closed")),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {}
            Err(e) => return Err(anyhow!("{e}")),
        }
    }
}

/// One `now_playing` frame. `None` / empty retracts the track.
fn push(
    ws: &mut WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    track: Option<&np::Track>,
) -> Result<()> {
    let msg = match track {
        Some(t) => json!({
            "cmd": "now_playing",
            "artist": t.artist,
            "title": t.title,
            "source": t.source,
        }),
        None => json!({"cmd": "now_playing"}),
    };
    ws.send(Message::Text(msg.to_string().into()))?;
    Ok(())
}
