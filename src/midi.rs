//! MIDI input: pad/note presses become `Action`s. One input port at a time,
//! picked on the Keys page; bindings live in `Settings::midi_notes`.

use anyhow::Result;

/// A live input connection — the port stays open while this is held.
pub struct Midi {
    /// The port name this connection is bound to.
    pub name: String,
    /// Field kept alive, not read — dropping it closes the port.
    #[allow(dead_code)]
    conn: midir::MidiInputConnection<()>,
}

/// Names of the input ports currently available.
pub fn ports() -> Vec<String> {
    let Ok(input) = midir::MidiInput::new("Trippin") else {
        return Vec::new();
    };
    input
        .ports()
        .iter()
        .filter_map(|p| input.port_name(p).ok())
        .collect()
}

/// Connect to the port called `name`. `on_msg` runs on midir's callback
/// thread for every note on/off, CC and program change — keep it cheap (it
/// just posts to the event loop). Only hold actions act on a note-off.
pub fn connect(name: &str, on_msg: impl Fn(crate::midimap::Msg) + Send + 'static) -> Result<Midi> {
    let mut input = midir::MidiInput::new("Trippin")?;
    input.ignore(midir::Ignore::None);
    let port = input
        .ports()
        .into_iter()
        .find(|p| input.port_name(p).as_deref() == Ok(name))
        .ok_or_else(|| anyhow::anyhow!("not found"))?;
    let conn = input.connect(
        &port,
        "trippin",
        move |_ts, msg, _| {
            // Note-off (0x8n, or 0x9n with velocity 0) is the pad release:
            // only hold actions use it — firing a toggle on it would
            // double every press.
            if let Some(m) = crate::midimap::Msg::parse(msg) {
                on_msg(m);
            }
        },
        (),
    )?;
    Ok(Midi {
        name: name.to_string(),
        conn,
    })
}

/// "C2" / "F#4" style name for a note number (middle C = 60 = C4).
pub fn note_name(n: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[(n % 12) as usize], n as i32 / 12 - 1)
}
