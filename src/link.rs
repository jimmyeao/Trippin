//! Show-state feed for an external render engine (the Unity stage in
//! `unity/`): one small JSON datagram per rendered frame to
//! 127.0.0.1:<port>. It carries the same audio vocabulary the WGSL scenes
//! get (see `shaders/common.wgsl`), the beat/bar position, the palette as 8
//! colours, and a scene-cut flag, so the engine can move with the music the
//! way native scenes do. Fire-and-forget: nothing listening costs nothing.

use std::net::{SocketAddr, UdpSocket};

use serde_json::json;

use crate::render::Uniforms;

pub struct Link {
    sock: UdpSocket,
    addr: SocketAddr,
    palette_name: String,
    palette: Vec<[f32; 3]>,
    last_scene: String,
}

impl Link {
    pub fn new(port: u16) -> anyhow::Result<Self> {
        let sock = UdpSocket::bind("127.0.0.1:0")?;
        sock.set_nonblocking(true)?;
        Ok(Self {
            sock,
            addr: SocketAddr::from(([127, 0, 0, 1], port)),
            palette_name: String::new(),
            palette: Vec::new(),
            last_scene: String::new(),
        })
    }

    pub fn send(&mut self, u: &Uniforms, scene: &str, palette: &str, drums: bool) {
        if palette != self.palette_name {
            self.palette = palette_colours(palette);
            self.palette_name = palette.to_string();
        }
        let cut = scene != self.last_scene;
        if cut {
            self.last_scene = scene.to_string();
        }
        let msg = frame_json(u, scene, &self.palette, cut, drums);
        let _ = self.sock.send_to(msg.to_string().as_bytes(), self.addr);
    }
}

/// The palette as the 8 colours the engine gets (evenly spaced along the LUT).
pub fn palette_colours(name: &str) -> Vec<[f32; 3]> {
    let lut = crate::palettes::lut(name);
    let n = crate::palettes::LUT_SIZE;
    (0..8)
        .map(|i| {
            let p = &lut[(i * (n - 1) / 7) * 4..];
            [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0]
        })
        .collect()
}

/// One frame of the feed as JSON (the datagram body, and one line of a
/// `--dump-feed` file that the engine replays with `-replayFeed`).
pub fn frame_json(u: &Uniforms, scene: &str, palette: &[[f32; 3]], cut: bool, drums: bool) -> serde_json::Value {
    json!({
        "time": u.time, "dt": u.dt,
        "bpm": u.bpm, "beat": u.beat, "beat_phase": u.beat_phase, "bar_phase": u.bar_phase,
        "bass": u.bass, "mid": u.mid, "high": u.high, "energy": u.energy,
        "onset": u.onset, "kick": u.kick, "build": u.build,
        "intensity": u.intensity, "calm": u.calm, "flow": u.flow,
        "hue": u.hue, "flash": u.flash, "master": u.master,
        "lvl4": u.lvl4, "hits4": u.hits4, "pres4": u.pres4, "clock4": u.clock4,
        "spectrum": u.spectrum.to_vec(),
        // Flat r,g,b x 8 — Unity's JsonUtility can't read nested arrays.
        "palette": palette.iter().flatten().collect::<Vec<_>>(),
        "scene": scene, "cut": cut, "drums": drums,
    })
}
