//! MIDI mapping (the MIDI page): any pad, key, knob, fader or program
//! change mapped to anything the app can do.
//!
//! A binding is a [`Src`] (note, CC or program change, on a channel) and a
//! [`Target`]. Every target becomes a `remote::RemoteCmd`, the same
//! commands the LAN remote and OSC send, so anything they can reach MIDI
//! can too, through one code path (`App::remote_cmd`, which records into an
//! armed timeline as hotkeys do).
//!
//! - Buttons (actions, scenes, effects, palettes, routines, Looks) fire on a
//!   note-on, a program change, or a CC crossing upward through the middle
//!   (a controller's buttons often send CC 127/0).
//! - Controls (sliders) follow a CC's value across their range; a note
//!   sets them to full on press.
//!
//! The Keys page's per-action pad buttons (`Settings::midi_notes`, any
//! channel) keep working alongside; this table is `Settings::midi_map`.

use crate::config::{Action, Fx};
use crate::remote::{RemoteCmd, SceneSel, SetKey};
use serde::{Deserialize, Serialize};

/// Where a MIDI message comes from. Channels are 0..15.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Src {
    Note { ch: u8, num: u8 },
    Cc { ch: u8, num: u8 },
    Program { ch: u8, num: u8 },
}

impl Src {
    pub fn label(&self) -> String {
        match *self {
            Src::Note { ch, num } => format!("Note {} ({num}) · ch {}", crate::midi::note_name(num), ch + 1),
            Src::Cc { ch, num } => format!("CC {num} · ch {}", ch + 1),
            Src::Program { ch, num } => format!("Program {} · ch {}", num + 1, ch + 1),
        }
    }
}

/// A slider a knob or fader can move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Control {
    FxAmt,
    AccentAmt,
    EchoAmt,
    DancerSize,
    CameraAmt,
    CameraTol,
    RotoAmt,
    RotoDetail,
    RotoInk,
    RotoBands,
    RotoPal,
    BrandOpacity,
    NpSize,
    /// The k-th control (0-based) of the scene on air: a camera scene's,
    /// the rotoscope look's or a Unity show's (its "scene controls" card).
    SceneCtl(u8),
}

impl Control {
    pub const ALL: [Control; 18] = [
        Control::FxAmt,
        Control::AccentAmt,
        Control::EchoAmt,
        Control::DancerSize,
        Control::CameraAmt,
        Control::CameraTol,
        Control::RotoAmt,
        Control::RotoDetail,
        Control::RotoInk,
        Control::RotoBands,
        Control::RotoPal,
        Control::BrandOpacity,
        Control::NpSize,
        Control::SceneCtl(0),
        Control::SceneCtl(1),
        Control::SceneCtl(2),
        Control::SceneCtl(3),
        Control::SceneCtl(4),
    ];

    pub fn label(self) -> String {
        match self {
            Control::FxAmt => "Effect strength".into(),
            Control::AccentAmt => "Accent amount".into(),
            Control::EchoAmt => "Echo amount".into(),
            Control::DancerSize => "Dancer size".into(),
            Control::CameraAmt => "Camera layer amount".into(),
            Control::CameraTol => "Camera key tolerance".into(),
            Control::RotoAmt => "Rotoscope amount".into(),
            Control::RotoDetail => "Rotoscope detail".into(),
            Control::RotoInk => "Rotoscope ink".into(),
            Control::RotoBands => "Rotoscope bands".into(),
            Control::RotoPal => "Rotoscope palette".into(),
            Control::BrandOpacity => "Branding opacity".into(),
            Control::NpSize => "Now-playing card size".into(),
            Control::SceneCtl(k) => format!("Scene control {} (scene on air)", k + 1),
        }
    }

    /// The command that sets this control to `v` (0..1 across its range).
    pub fn cmd(self, v: f32) -> RemoteCmd {
        let v = v.clamp(0.0, 1.0);
        let lerp = |a: f32, b: f32| a + (b - a) * v;
        RemoteCmd::Set(match self {
            Control::FxAmt => SetKey::FxAmt(v),
            Control::AccentAmt => SetKey::AccentAmt(v),
            Control::EchoAmt => SetKey::EchoAmt(v),
            Control::DancerSize => SetKey::DancerSize(lerp(0.4, 1.0)),
            Control::CameraAmt => SetKey::CameraAmt(v),
            Control::CameraTol => SetKey::CameraTol(v),
            Control::RotoAmt => SetKey::RotoAmt(v),
            Control::RotoDetail => SetKey::RotoDetail(v),
            Control::RotoInk => SetKey::RotoInk(v),
            Control::RotoBands => SetKey::RotoBands(lerp(2.0, 8.0).round() as u32),
            Control::RotoPal => SetKey::RotoPal(v),
            Control::BrandOpacity => SetKey::BrandOpacity(lerp(0.1, 1.0)),
            Control::NpSize => SetKey::NpSize(lerp(0.5, 2.0)),
            Control::SceneCtl(k) => SetKey::SceneCtl(k, v),
        })
    }
}

/// What a binding does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Target {
    /// Any hotkey action (next scene, blackout, strobe, Look slots...).
    Action(Action),
    /// Cut to a scene (by id).
    Scene(String),
    /// Queue a scene to play next (by id).
    QueueScene(String),
    Fx(Fx),
    Palette(String),
    /// Show a dancer routine (by clip name).
    Routine(String),
    /// Recall a saved Look by id.
    Look(String),
    Control(Control),
}

impl Target {
    pub fn kind(&self) -> &'static str {
        match self {
            Target::Action(_) => "Action",
            Target::Scene(_) => "Scene",
            Target::QueueScene(_) => "Queue scene",
            Target::Fx(_) => "Effect",
            Target::Palette(_) => "Palette",
            Target::Routine(_) => "Routine",
            Target::Look(_) => "Look",
            Target::Control(_) => "Control",
        }
    }

    /// "Kind: item", with scene ids shown by their titles.
    pub fn label(&self, title: &dyn Fn(&str) -> String) -> String {
        let item = match self {
            Target::Action(a) => a.label().to_string(),
            Target::Scene(s) | Target::QueueScene(s) => title(s),
            Target::Fx(f) => f.label().to_string(),
            Target::Palette(p) | Target::Routine(p) | Target::Look(p) => p.clone(),
            Target::Control(c) => c.label(),
        };
        format!("{}: {item}", self.kind())
    }

    /// The command for a button press (None for a control: see `Control::cmd`).
    pub fn press(&self) -> Option<RemoteCmd> {
        Some(match self {
            Target::Action(a) => RemoteCmd::Act(*a),
            Target::Scene(s) => RemoteCmd::GoToScene(SceneSel::Name(s.clone())),
            Target::QueueScene(s) => RemoteCmd::QueueNext(SceneSel::Name(s.clone())),
            Target::Fx(f) => RemoteCmd::Set(SetKey::Fx(*f)),
            Target::Palette(p) => RemoteCmd::Set(SetKey::Palette(p.clone())),
            Target::Routine(r) => RemoteCmd::ShowClip(r.clone()),
            Target::Look(l) => RemoteCmd::Look(l.clone()),
            Target::Control(_) => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    pub src: Src,
    pub target: Target,
}

/// A MIDI message as the input callback reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    NoteOn { ch: u8, num: u8, vel: u8 },
    NoteOff { ch: u8, num: u8 },
    Cc { ch: u8, num: u8, val: u8 },
    Program { ch: u8, num: u8 },
}

impl Msg {
    /// Decode a raw message (running status isn't used by midir).
    pub fn parse(m: &[u8]) -> Option<Msg> {
        let (&st, rest) = m.split_first()?;
        let ch = st & 0x0F;
        Some(match (st & 0xF0, rest) {
            (0x90, [n, v, ..]) if *v > 0 => Msg::NoteOn { ch, num: n & 0x7F, vel: v & 0x7F },
            (0x90, [n, _, ..]) | (0x80, [n, ..]) => Msg::NoteOff { ch, num: n & 0x7F },
            (0xB0, [n, v, ..]) => Msg::Cc { ch, num: n & 0x7F, val: v & 0x7F },
            (0xC0, [n, ..]) => Msg::Program { ch, num: n & 0x7F },
            _ => return None,
        })
    }

    /// The source a learn binds this message to (none for a note-off).
    pub fn src(&self) -> Option<Src> {
        match *self {
            Msg::NoteOn { ch, num, .. } => Some(Src::Note { ch, num }),
            Msg::Cc { ch, num, .. } => Some(Src::Cc { ch, num }),
            Msg::Program { ch, num } => Some(Src::Program { ch, num }),
            Msg::NoteOff { .. } => None,
        }
    }

    pub fn label(&self) -> String {
        match *self {
            Msg::NoteOn { ch, num, vel } => format!("Note {} ({num}) on · vel {vel} · ch {}", crate::midi::note_name(num), ch + 1),
            Msg::NoteOff { ch, num } => format!("Note {} ({num}) off · ch {}", crate::midi::note_name(num), ch + 1),
            Msg::Cc { ch, num, val } => format!("CC {num} = {val} · ch {}", ch + 1),
            Msg::Program { ch, num } => format!("Program {} · ch {}", num + 1, ch + 1),
        }
    }
}

/// Turns messages into commands: remembers each CC's last value, so a
/// button on a CC fires once as it crosses upward through 64.
#[derive(Default)]
pub struct Router {
    last_cc: std::collections::HashMap<(u8, u8), u8>,
}

impl Router {
    /// The commands `msg` triggers under `map`.
    pub fn route(&mut self, map: &[Binding], msg: Msg) -> Vec<RemoteCmd> {
        let mut out = Vec::new();
        match msg {
            Msg::NoteOn { ch, num, .. } => {
                for b in map.iter().filter(|b| b.src == Src::Note { ch, num }) {
                    match &b.target {
                        Target::Control(c) => out.push(c.cmd(1.0)),
                        t => out.extend(t.press()),
                    }
                }
            }
            Msg::Cc { ch, num, val } => {
                let prev = self.last_cc.insert((ch, num), val);
                let rising = val >= 64 && prev.is_none_or(|p| p < 64);
                for b in map.iter().filter(|b| b.src == Src::Cc { ch, num }) {
                    match &b.target {
                        Target::Control(c) => out.push(c.cmd(val as f32 / 127.0)),
                        t if rising => out.extend(t.press()),
                        _ => {}
                    }
                }
            }
            Msg::Program { ch, num } => {
                for b in map.iter().filter(|b| b.src == Src::Program { ch, num }) {
                    match &b.target {
                        Target::Control(c) => out.push(c.cmd(1.0)),
                        t => out.extend(t.press()),
                    }
                }
            }
            Msg::NoteOff { .. } => {}
        }
        out
    }
}

/// Bind `src` to `target`: one source drives one target, so whatever the
/// source had is replaced.
pub fn learn(map: &mut Vec<Binding>, src: Src, target: Target) {
    map.retain(|b| b.src != src);
    map.push(Binding { src, target });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_notes_ccs_and_programs() {
        assert_eq!(Msg::parse(&[0x93, 60, 100]), Some(Msg::NoteOn { ch: 3, num: 60, vel: 100 }));
        assert_eq!(Msg::parse(&[0x93, 60, 0]), Some(Msg::NoteOff { ch: 3, num: 60 }));
        assert_eq!(Msg::parse(&[0x80, 60, 40]), Some(Msg::NoteOff { ch: 0, num: 60 }));
        assert_eq!(Msg::parse(&[0xB1, 21, 64]), Some(Msg::Cc { ch: 1, num: 21, val: 64 }));
        assert_eq!(Msg::parse(&[0xC0, 4]), Some(Msg::Program { ch: 0, num: 4 }));
        assert_eq!(Msg::parse(&[0xF8]), None);
    }

    #[test]
    fn a_knob_moves_a_slider_and_a_cc_button_fires_once() {
        let mut map = Vec::new();
        learn(&mut map, Src::Cc { ch: 0, num: 21 }, Target::Control(Control::FxAmt));
        learn(&mut map, Src::Cc { ch: 0, num: 30 }, Target::Action(Action::Blackout));
        let mut r = Router::default();
        let c = r.route(&map, Msg::Cc { ch: 0, num: 21, val: 127 });
        assert!(matches!(c.as_slice(), [RemoteCmd::Set(SetKey::FxAmt(v))] if (*v - 1.0).abs() < 1e-6));
        // The button fires going up through 64, not again while held high.
        assert_eq!(r.route(&map, Msg::Cc { ch: 0, num: 30, val: 127 }).len(), 1);
        assert_eq!(r.route(&map, Msg::Cc { ch: 0, num: 30, val: 120 }).len(), 0);
        assert_eq!(r.route(&map, Msg::Cc { ch: 0, num: 30, val: 0 }).len(), 0);
        assert_eq!(r.route(&map, Msg::Cc { ch: 0, num: 30, val: 127 }).len(), 1);
        // Other channels don't match.
        assert!(r.route(&map, Msg::Cc { ch: 1, num: 21, val: 127 }).is_empty());
    }

    #[test]
    fn learning_a_source_again_replaces_its_target() {
        let mut map = Vec::new();
        let src = Src::Note { ch: 9, num: 36 };
        learn(&mut map, src, Target::Fx(Fx::Melt));
        learn(&mut map, src, Target::Scene("aurora".into()));
        assert_eq!(map.len(), 1);
        let mut r = Router::default();
        assert!(matches!(
            r.route(&map, Msg::NoteOn { ch: 9, num: 36, vel: 90 }).as_slice(),
            [RemoteCmd::GoToScene(SceneSel::Name(n))] if n == "aurora"
        ));
    }

    #[test]
    fn controls_scale_into_their_ranges() {
        assert!(matches!(Control::RotoBands.cmd(0.0), RemoteCmd::Set(SetKey::RotoBands(2))));
        assert!(matches!(Control::RotoBands.cmd(1.0), RemoteCmd::Set(SetKey::RotoBands(8))));
        assert!(matches!(Control::DancerSize.cmd(0.5), RemoteCmd::Set(SetKey::DancerSize(v)) if (v - 0.7).abs() < 1e-6));
    }

    /// The table must survive a settings round trip.
    #[test]
    fn bindings_serialise() {
        let map = vec![
            Binding { src: Src::Program { ch: 0, num: 2 }, target: Target::Look("party".into()) },
            Binding { src: Src::Cc { ch: 2, num: 7 }, target: Target::Control(Control::SceneCtl(3)) },
        ];
        let j = serde_json::to_string(&map).unwrap();
        let back: Vec<Binding> = serde_json::from_str(&j).unwrap();
        assert_eq!(back, map);
    }
}
