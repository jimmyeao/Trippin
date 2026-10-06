//! OSC input (UDP) for TouchOSC/Lemur-style controllers — the same
//! `RemoteCmd`s the iOS app sends, mapped from a fixed address layout.
//! Everything lives under `/trippin`:
//!
//! ```text
//! /trippin/scene/next   /trippin/scene/prev     scene stepping
//! /trippin/scene/goto   /trippin/scene/queue    arg: scene index (int/float)
//!                                                 or name (string)
//! /trippin/mode                                 arg: "auto"/"static"/"manual"
//!                                                 or 0/1/2
//! /trippin/dancer       /trippin/dancer/clip    dancer toggle / next routine
//! /trippin/dancer/style /trippin/canon          next look / canon cycle
//! /trippin/blackout     /trippin/fx             blackout / next post effect
//! /trippin/downbeat     /trippin/phrase         grid marks
//! /trippin/overlay/logo /overlay/name           branding kills
//! /trippin/overlay/ticker /overlay/np           ticker / now-playing card
//! /trippin/rec/clip     /trippin/rec/set        clip save / set recording
//! /trippin/timeline/play                        transport toggle
//! /trippin/look                                  arg: Look slot 1-8 (int)
//!                                                 or Look id (string)
//! /trippin/style                                 arg: Style id (string, "off")
//!                                                 or position (0 = off, 1 = first)
//! /trippin/set/<key>                            a SetKey knob: fx_amt,
//!   dancer_size, phrase_bars, latency_ms, np_size, brand_opacity,
//!   ticker_speed, ticker_text, palette (string), fx (string)
//! ```
//!
//! Buttons fire on the press only: a falsy first arg (0.0 / 0 / false) is
//! the release, the MIDI note-off lesson. `/set/*` and `/scene/goto|queue`
//! read their arg directly — a fader at 0.0 is a real value, not a release.

use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use rosc::{decoder, OscMessage, OscPacket, OscType};
use serde_json::{json, Value};

use crate::config::Action;
use crate::remote::{self, RemoteCmd, SceneSel};

pub struct Osc {
    pub port: u16,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Osc {
    /// Bind the UDP socket on all interfaces and start dispatching.
    /// `port` 0 picks a free port (tests).
    pub fn start(port: u16, on_cmd: impl Fn(RemoteCmd) + Send + 'static) -> Result<Osc> {
        Self::start_on("0.0.0.0", port, on_cmd)
    }

    /// `start` with an explicit bind address — tests use loopback so the
    /// Windows firewall prompt isn't part of the test.
    pub fn start_on(
        addr: &str,
        port: u16,
        on_cmd: impl Fn(RemoteCmd) + Send + 'static,
    ) -> Result<Osc> {
        let sock = UdpSocket::bind((addr, port))?;
        let port = sock.local_addr()?.port();
        sock.set_read_timeout(Some(Duration::from_millis(200)))?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while !flag.load(Ordering::Relaxed) {
                match sock.recv_from(&mut buf) {
                    Ok((n, _)) => {
                        if let Ok((_, packet)) = decoder::decode_udp(&buf[..n]) {
                            walk(&packet, &on_cmd);
                        }
                    }
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(_) => thread::sleep(Duration::from_millis(250)),
                }
            }
        });
        Ok(Osc {
            port,
            stop,
            handle: Some(handle),
        })
    }
}

impl Drop for Osc {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn walk(packet: &OscPacket, on_cmd: &dyn Fn(RemoteCmd)) {
    match packet {
        OscPacket::Message(OscMessage { addr, args }) => {
            if let Some(cmd) = map(addr, args) {
                on_cmd(cmd);
            }
        }
        OscPacket::Bundle(b) => b.content.iter().for_each(|p| walk(p, on_cmd)),
    }
}

/// A button press? TouchOSC sends 1.0 on press and 0.0 on release — like a
/// MIDI note-off, firing on the release doubles every toggle. No arg at
/// all counts as a press.
fn pressed(args: &[OscType]) -> bool {
    args.first().is_none_or(truthy)
}

fn truthy(t: &OscType) -> bool {
    match t {
        OscType::Float(f) => *f > 0.5,
        OscType::Double(f) => *f > 0.5,
        OscType::Int(i) => *i > 0,
        OscType::Long(i) => *i > 0,
        OscType::Bool(b) => *b,
        OscType::String(s) => matches!(s.as_str(), "1" | "true" | "on" | "press"),
        _ => true,
    }
}

/// The first arg as a scene selector: int/float = index, string = name.
fn scene_sel(args: &[OscType]) -> Option<SceneSel> {
    match args.first() {
        Some(OscType::Int(i)) => Some(SceneSel::Index(*i as usize)),
        Some(OscType::Long(i)) => Some(SceneSel::Index(*i as usize)),
        Some(OscType::Float(f)) => Some(SceneSel::Index(*f as usize)),
        Some(OscType::Double(f)) => Some(SceneSel::Index(*f as usize)),
        Some(OscType::String(s)) => Some(SceneSel::Name(s.clone())),
        _ => None,
    }
}

/// The first arg as a Look selector: an int is a slot (1-8; a button's 0
/// release matches no slot and is ignored), a string an id.
fn look_sel(args: &[OscType]) -> Option<String> {
    match args.first() {
        Some(OscType::Int(i)) => Some(i.to_string()),
        Some(OscType::Long(i)) => Some(i.to_string()),
        Some(OscType::Float(f)) => Some((*f as i64).to_string()),
        Some(OscType::Double(f)) => Some((*f as i64).to_string()),
        Some(OscType::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// The first arg as a Style selector: a string is an id ("off" clears), an int
/// is the position in the Style list (0 = off, 1 = the first Style, ...).
fn style_sel(args: &[OscType]) -> Option<String> {
    let n = match args.first()? {
        OscType::String(s) => return Some(s.clone()),
        OscType::Int(i) => *i as i64,
        OscType::Long(i) => *i,
        OscType::Float(f) => *f as i64,
        OscType::Double(f) => *f as i64,
        _ => return None,
    };
    if n <= 0 {
        return Some("off".into());
    }
    crate::styles::catalog().themes.get(n as usize - 1).map(|t| t.id.clone())
}

/// The first arg as JSON so `remote::parse_set` can be shared verbatim.
fn arg_value(args: &[OscType]) -> Option<Value> {
    match args.first() {
        Some(OscType::Int(i)) => Some(json!(i)),
        Some(OscType::Long(i)) => Some(json!(i)),
        Some(OscType::Float(f)) => Some(json!(f)),
        Some(OscType::Double(f)) => Some(json!(f)),
        Some(OscType::String(s)) => Some(Value::String(s.clone())),
        Some(OscType::Bool(b)) => Some(Value::Bool(*b)),
        _ => None,
    }
}

fn map(addr: &str, args: &[OscType]) -> Option<RemoteCmd> {
    let press = |a: Action| pressed(args).then_some(RemoteCmd::Act(a));
    Some(match addr {
        "/trippin/scene/next" => press(Action::NextScene)?,
        "/trippin/scene/prev" => press(Action::PrevScene)?,
        "/trippin/scene/goto" => RemoteCmd::GoToScene(scene_sel(args)?),
        "/trippin/scene/queue" => RemoteCmd::QueueNext(scene_sel(args)?),
        "/trippin/mode" => RemoteCmd::Act(mode(args)?),
        "/trippin/dancer" => press(Action::ToggleDancer)?,
        "/trippin/dancer/clip" => press(Action::NextClip)?,
        "/trippin/dancer/style" => press(Action::NextStyle)?,
        "/trippin/canon" => press(Action::CycleCanon)?,
        "/trippin/blackout" => press(Action::Blackout)?,
        "/trippin/fx" => press(Action::CycleFx)?,
        "/trippin/downbeat" => press(Action::MarkDownbeat)?,
        "/trippin/phrase" => press(Action::MarkPhrase)?,
        "/trippin/overlay/logo" => press(Action::ToggleLogo)?,
        "/trippin/overlay/name" => press(Action::ToggleName)?,
        "/trippin/overlay/ticker" => press(Action::ToggleTicker)?,
        "/trippin/overlay/np" => press(Action::ShowNowPlaying)?,
        "/trippin/look" => RemoteCmd::Look(look_sel(args)?),
        "/trippin/style" => RemoteCmd::Style(style_sel(args)?),
        "/trippin/rec/clip" => press(Action::SaveClip)?,
        "/trippin/rec/set" => press(Action::RecordSet)?,
        "/trippin/timeline/play" => press(Action::TimelinePlay)?,
        _ if addr.starts_with("/trippin/set/") => {
            RemoteCmd::Set(remote::parse_set(&addr["/trippin/set/".len()..], arg_value(args)?).ok()?)
        }
        _ => return None,
    })
}

/// `/trippin/mode`'s arg: a name ("auto"/"static"/"manual") or 0/1/2.
fn mode(args: &[OscType]) -> Option<Action> {
    let n = match args.first()? {
        OscType::String(s) => {
            return match s.as_str() {
                "auto" => Some(Action::ModeAuto),
                "static" | "hold" => Some(Action::ModeStatic),
                "manual" => Some(Action::ModeManual),
                _ => None,
            }
        }
        OscType::Int(i) => *i as i64,
        OscType::Long(i) => *i,
        OscType::Float(f) => *f as i64,
        OscType::Double(f) => *f as i64,
        _ => return None,
    };
    Some(match n {
        0 => Action::ModeAuto,
        1 => Action::ModeStatic,
        2 => Action::ModeManual,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn buttons_fire_on_press_only() {
        let press = vec![OscType::Float(1.0)];
        let release = vec![OscType::Float(0.0)];
        assert!(matches!(
            map("/trippin/blackout", &press),
            Some(RemoteCmd::Act(Action::Blackout))
        ));
        assert!(map("/trippin/blackout", &release).is_none());
        // No arg = a press (some controllers send bare triggers).
        assert!(map("/trippin/fx", &[]).is_some());
    }

    #[test]
    fn set_and_goto_take_args_verbatim() {
        // A fader at 0.0 is a value, not a release.
        assert!(matches!(
            map("/trippin/set/fx_amt", &[OscType::Float(0.0)]),
            Some(RemoteCmd::Set(remote::SetKey::FxAmt(v))) if v == 0.0
        ));
        assert!(matches!(
            map("/trippin/scene/goto", &[OscType::String("comets".into())]),
            Some(RemoteCmd::GoToScene(SceneSel::Name(n))) if n == "comets"
        ));
        assert!(matches!(
            map("/trippin/mode", &[OscType::Int(2)]),
            Some(RemoteCmd::Act(Action::ModeManual))
        ));
    }

    #[test]
    fn look_takes_a_slot_or_an_id() {
        assert!(matches!(
            map("/trippin/look", &[OscType::Int(3)]),
            Some(RemoteCmd::Look(s)) if s == "3"
        ));
        assert!(matches!(
            map("/trippin/look", &[OscType::String("club-red".into())]),
            Some(RemoteCmd::Look(s)) if s == "club-red"
        ));
        // A button release sends 0: that is "slot 0", which no Look has.
        assert!(matches!(map("/trippin/look", &[OscType::Float(0.0)]), Some(RemoteCmd::Look(s)) if s == "0"));
        assert!(map("/trippin/look", &[]).is_none());
    }

    #[test]
    fn style_takes_an_id_or_a_position() {
        assert!(matches!(
            map("/trippin/style", &[OscType::String("rock".into())]),
            Some(RemoteCmd::Style(s)) if s == "rock"
        ));
        assert!(matches!(map("/trippin/style", &[OscType::Int(0)]), Some(RemoteCmd::Style(s)) if s == "off"));
        let first = crate::styles::catalog().themes.first().map(|t| t.id.clone());
        assert!(matches!(map("/trippin/style", &[OscType::Int(1)]), Some(RemoteCmd::Style(s)) if Some(&s) == first.as_ref()));
        assert!(map("/trippin/style", &[OscType::Int(999)]).is_none(), "past the end is ignored, not 'off'");
        assert!(map("/trippin/style", &[]).is_none());
    }

    #[test]
    fn udp_packet_dispatches() {
        let (tx, rx) = mpsc::channel();
        let osc = Osc::start_on("127.0.0.1", 0, move |c| {
            let _ = tx.send(c);
        })
        .unwrap();
        let packet = rosc::encoder::encode(&OscPacket::Message(OscMessage {
            addr: "/trippin/scene/next".into(),
            args: vec![],
        }))
        .unwrap();
        UdpSocket::bind("0.0.0.0:0")
            .unwrap()
            .send_to(&packet, ("127.0.0.1", osc.port))
            .unwrap();
        let cmd = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(cmd, RemoteCmd::Act(Action::NextScene)));
    }
}
