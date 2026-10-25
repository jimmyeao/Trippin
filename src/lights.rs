//! Lights that follow the show: WLED strips over DDP (Philips Hue next).
//!
//! The render loop hands a small `LightFrame` (palette, beat grid, audio,
//! director state, strobe gate) to `Lights::update` each frame; a worker
//! thread turns it into pixels for each enabled strip about 50 times a
//! second and sends them, so a slow or missing device never touches the
//! render thread.
//!
//! What a strip shows ("follow the show", `render_strip`):
//! - the palette across the strip, drifting with the smooth beat clock;
//! - a pulse on every beat and a ripple from the middle on each beat while
//!   the drums play, both timed from the *predicted* beat grid, `lead_ms`
//!   ahead, so they land on the kick despite network and LED latency;
//! - a bright segment chasing along the strip once a bar, turning round
//!   every phrase;
//! - a slow breathing wash through breakdowns;
//! - white flashes only while the strobe gate is live (drum fills, strobe
//!   cues), at most `MAX_FLASH_HZ`, and only with flashes allowed.
//!
//! WLED takes realtime data on DDP (UDP 4048) with no setup, and hands the
//! strip back to its own effects about 2.5 s after the packets stop.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::WledLight;

/// Photosensitivity: white flashes at most this often (WCAG 2.3.1).
const MAX_FLASH_HZ: f32 = 3.0;
const SEND_HZ: f32 = 50.0;
const DDP_PORT: u16 = 4048;
/// DDP payload per packet: 480 RGB pixels.
const DDP_CHUNK: usize = 1440;

/// What the lights need from one rendered frame.
#[derive(Clone)]
pub struct LightFrame {
    pub at: Instant,
    /// The active palette, 256 RGBA entries (palettes::lut).
    pub lut: Arc<[u8; crate::palettes::LUT_SIZE * 4]>,
    pub hue: f32,
    /// Beat position (beats, latency-compensated as the visuals use it).
    pub pos: f64,
    pub bpm: f32,
    /// Beat index within the bar, 0 = the one.
    pub beat_in_bar: u64,
    pub calm: f32,
    pub intensity: f32,
    pub energy: f32,
    pub hits4: [f32; 4],
    pub pres4: [f32; 4],
    /// Master fade incl. blackout and fade-out cues (0..1).
    pub master: f32,
    /// Strobe live (a strobe cue or an auto fill strobe), and the gate.
    pub strobe_live: bool,
    pub strobe_open: bool,
    /// Flight/tunnel scene on air: no flashes, softer pulses.
    pub smooth: bool,
}

/// What the Settings card shows per strip.
#[derive(Clone, Default)]
pub struct StripStatus {
    pub host: String,
    pub sent: u64,
    pub error: String,
}

#[derive(Clone)]
struct Conf {
    on: bool,
    level: f32,
    flashes: bool,
    strips: Vec<WledLight>,
    hue: crate::hue::HueBridge,
    hue_on: bool,
    hue_lead_ms: f32,
    hue_slow: Vec<String>,
    /// A test pattern runs on this strip (host) until then (Settings > Test).
    test: Option<(String, Instant)>,
}

struct Shared {
    frame: Option<LightFrame>,
    conf: Conf,
}

pub struct Lights {
    shared: Arc<Mutex<Shared>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// The latest status, for the panel.
pub static STATUS: Mutex<Vec<StripStatus>> = Mutex::new(Vec::new());
/// Hue stream status for the panel ("streaming to …", or the error).
pub static HUE_STATUS: Mutex<String> = Mutex::new(String::new());
/// Settings > Test: run the test pattern on this strip (host) for 3 s.
pub static TEST: Mutex<Option<String>> = Mutex::new(None);

impl Lights {
    pub fn start() -> Lights {
        let shared = Arc::new(Mutex::new(Shared {
            frame: None,
            conf: Conf {
                on: false,
                level: 0.8,
                flashes: false,
                strips: Vec::new(),
                hue: Default::default(),
                hue_on: false,
                hue_lead_ms: 120.0,
                hue_slow: Vec::new(),
                test: None,
            },
        }));
        let sh = shared.clone();
        std::thread::Builder::new()
            .name("lights".into())
            .spawn(move || worker(sh))
            .ok();
        // Bridge lights outside the stream: their own thread, as each
        // command is a blocking HTTPS call.
        let sh = shared.clone();
        std::thread::Builder::new()
            .name("hue-slow".into())
            .spawn(move || slow_worker(sh))
            .ok();
        Lights { shared }
    }

    /// Called once a frame from the render loop: cheap (a clone and a lock).
    pub fn update(&self, frame: LightFrame, s: &crate::config::Settings) {
        let mut sh = lock(&self.shared);
        sh.frame = Some(frame);
        let test = lock(&TEST).take();
        let c = &mut sh.conf;
        c.on = s.lights_on;
        c.level = s.light_level.clamp(0.0, 1.0);
        c.flashes = s.light_flashes;
        if c.strips != s.wled {
            c.strips = s.wled.clone();
        }
        if c.hue != s.hue {
            c.hue = s.hue.clone();
        }
        c.hue_on = s.hue_on;
        c.hue_lead_ms = s.hue_lead_ms;
        if c.hue_slow != s.hue_slow {
            c.hue_slow = s.hue_slow.clone();
        }
        if let Some(host) = test {
            c.test = Some((host, Instant::now() + Duration::from_secs(3)));
        }
    }
}

struct Strip {
    host: String,
    addr: Option<SocketAddr>,
    resolved_at: Instant,
    seq: u8,
    status: StripStatus,
    /// Last white flash (rate cap).
    flash_at: Instant,
    gate_was: bool,
}

fn worker(sh: Arc<Mutex<Shared>>) {
    let sock = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("lights: {e}");
            return;
        }
    };
    let _ = sock.set_nonblocking(true);
    let mut strips: Vec<Strip> = Vec::new();
    let period = Duration::from_secs_f32(1.0 / SEND_HZ);
    let mut next = Instant::now();
    let mut px: Vec<u8> = Vec::new();
    // Hue: the open stream (and which bridge/area it's for), retry timer.
    let mut hue: Option<(crate::hue::Stream, crate::hue::HueBridge)> = None;
    let mut hue_retry = Instant::now();
    let mut hue_rgb: Vec<[f32; 3]> = Vec::new();
    let mut tick = 0u64;
    loop {
        tick += 1;
        next += period;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
        let (frame, mut conf) = {
            let s = lock(&sh);
            (s.frame.clone(), s.conf.clone())
        };
        let test_host = conf.test.as_ref().filter(|(_, until)| Instant::now() < *until).map(|(h, _)| h.clone());
        if test_host.is_none() && conf.test.is_some() {
            lock(&sh).conf.test = None;
        }
        let testing = test_host.is_some();
        // Hue: open the stream while wanted, close it (lights handed back)
        // when not, or when the bridge or area changed.
        let want_hue = conf.on && conf.hue_on && !conf.hue.area.is_empty();
        if hue.as_ref().is_some_and(|(_, b)| !want_hue || *b != conf.hue) {
            if let Some((s, b)) = hue.take() {
                s.close(&b);
            }
            lock(&HUE_STATUS).clear();
        }
        if want_hue && hue.is_none() && Instant::now() >= hue_retry {
            *lock(&HUE_STATUS) = "connecting…".into();
            match crate::hue::Stream::open(&conf.hue) {
                Ok(s) => {
                    *lock(&HUE_STATUS) = format!("streaming to {} lights", s.channels.len());
                    hue = Some((s, conf.hue.clone()));
                }
                Err(e) => {
                    *lock(&HUE_STATUS) = format!("{e:#}");
                    hue_retry = Instant::now() + Duration::from_secs(10);
                }
            }
        }
        // 25 Hz to the bridge: it can't take more, and keeps the stream up.
        if tick % 2 == 0 {
            if let Some((s, _)) = hue.as_mut() {
                match &frame {
                    Some(f) if f.at.elapsed() < Duration::from_millis(500) => {
                        render_hue(&mut hue_rgb, f, &s.channels, conf.hue_lead_ms, conf.level, Instant::now());
                    }
                    _ => {
                        hue_rgb.clear();
                        hue_rgb.resize(s.channels.len(), [0.0; 3]);
                    }
                }
                if let Err(e) = s.send(&hue_rgb) {
                    *lock(&HUE_STATUS) = format!("{e:#}");
                    if let Some((s, b)) = hue.take() {
                        s.close(&b);
                    }
                    hue_retry = Instant::now() + Duration::from_secs(5);
                }
            }
        }
        if !conf.on && !testing {
            // Off: send nothing, and WLED takes its strips back.
            strips.clear();
            *lock(&STATUS) = Vec::new();
            continue;
        }
        // A test lights only its strip, even with the lights off.
        match &test_host {
            Some(h) => conf.strips.retain(|s| &s.host == h && s.leds > 0),
            None => conf.strips.retain(|s| s.on && s.leds > 0),
        }
        strips.retain(|s| conf.strips.iter().any(|c| c.host == s.host));
        for c in &conf.strips {
            if !strips.iter().any(|s| s.host == c.host) {
                strips.push(Strip {
                    host: c.host.clone(),
                    addr: None,
                    resolved_at: Instant::now() - Duration::from_secs(60),
                    seq: 0,
                    status: StripStatus { host: c.host.clone(), ..Default::default() },
                    flash_at: Instant::now() - Duration::from_secs(1),
                    gate_was: false,
                });
            }
        }
        let t = Instant::now();
        for st in strips.iter_mut() {
            let Some(c) = conf.strips.iter().find(|c| c.host == st.host) else { continue };
            if st.addr.is_none() && st.resolved_at.elapsed() > Duration::from_secs(5) {
                st.resolved_at = Instant::now();
                st.addr = (st.host.as_str(), DDP_PORT).to_socket_addrs().ok().and_then(|mut a| a.next());
                if st.addr.is_none() {
                    st.status.error = format!("can't find {}", st.host);
                }
            }
            let Some(addr) = st.addr else { continue };
            px.clear();
            px.resize(c.leds as usize * 3, 0);
            if testing {
                render_test(&mut px);
            } else if let Some(f) = &frame {
                // A stale frame (render thread stalled): hold black rather
                // than freeze on a bright frame.
                if f.at.elapsed() < Duration::from_millis(500) {
                    let flash = flash_now(st, f, &conf);
                    render_strip(&mut px, f, c, conf.level, flash, t);
                }
            }
            if c.reverse {
                let n = px.len() / 3;
                for i in 0..n / 2 {
                    for k in 0..3 {
                        px.swap(i * 3 + k, (n - 1 - i) * 3 + k);
                    }
                }
            }
            match send_ddp(&sock, addr, &mut st.seq, &px) {
                Ok(()) => {
                    st.status.sent += 1;
                    st.status.error.clear();
                }
                Err(e) => st.status.error = e.to_string(),
            }
        }
        *lock(&STATUS) = strips.iter().map(|s| s.status.clone()).collect();
    }
}

/// Commands a second to the bridge, all lights together (Philips asks for
/// about 10; leave room for the app and other apps).
const SLOW_HZ: f32 = 8.0;

/// Non-streaming bridge lights: on each bar (each two in a breakdown), the
/// next palette colour per light, sent one light at a time at `SLOW_HZ` so
/// the change sweeps across the room; dim and slow through breakdowns, off
/// on blackout. Each light's own state is saved first and put back when the
/// lights stop.
/// Saved states of the bulbs `slow_worker` has taken, and their bridge, so
/// quitting can put them back (`restore_on_quit`).
static TAKEN: Mutex<Option<(crate::hue::HueBridge, Vec<crate::hue::Saved>)>> = Mutex::new(None);
static QUITTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Held while a command to a bulb is in flight.
static SENDING: Mutex<()> = Mutex::new(());

/// Quit: put the bar-by-bar bulbs back as they were (the worker thread dies
/// with the process). Called from `ApplicationHandler::exiting`.
pub fn restore_on_quit() {
    // Stop the worker first, and wait out a command it has in flight: one
    // landing after the restore left a bulb on its show colour.
    QUITTING.store(true, std::sync::atomic::Ordering::Release);
    let _sending = lock(&SENDING);
    if let Some((b, saved)) = lock(&TAKEN).take() {
        for s in &saved {
            crate::hue::restore_light(&b, s);
        }
    }
}

fn slow_worker(sh: Arc<Mutex<Shared>>) {
    let mut saved: Vec<crate::hue::Saved> = Vec::new();
    let mut bridge = crate::hue::HueBridge::default();
    let mut queue: std::collections::VecDeque<(String, [f32; 3], u32)> = Default::default();
    let mut last_bar: Option<i64> = None;
    let gap = Duration::from_secs_f32(1.0 / SLOW_HZ);
    loop {
        std::thread::sleep(Duration::from_millis(20));
        let (frame, conf) = {
            let s = lock(&sh);
            (s.frame.clone(), s.conf.clone())
        };
        let want = conf.on && !conf.hue.user.is_empty() && !conf.hue_slow.is_empty();
        // Stop (or the light list or bridge changed): hand back what we took.
        let keep = |id: &str| want && conf.hue == bridge && conf.hue_slow.iter().any(|x| x == id);
        if saved.iter().any(|s| !keep(&s.id)) {
            for s in saved.iter().filter(|s| !keep(&s.id)) {
                crate::hue::restore_light(&bridge, s);
            }
            saved.retain(|s| keep(&s.id));
            queue.retain(|(id, _, _)| keep(id));
        }
        if !want {
            last_bar = None;
            *lock(&TAKEN) = None;
            continue;
        }
        bridge = conf.hue.clone();
        for id in &conf.hue_slow {
            if !saved.iter().any(|s| &s.id == id) {
                if let Some(s) = crate::hue::save_light(&bridge, id) {
                    saved.push(s);
                }
            }
        }
        let Some(f) = frame.filter(|f| f.at.elapsed() < Duration::from_millis(500)) else { continue };
        // Zigbee bulbs lag: aim the change ahead of the bar line.
        let pos = f.pos + (f.at.elapsed().as_secs_f64() + 0.3) * f.bpm as f64 / 60.0;
        // Bars counted from the downbeat: the bar line the frame's beat is in,
        // then on from there.
        let start = f.pos.floor() - f.beat_in_bar as f64;
        let bar = (start / 4.0).round() as i64 + ((pos - start) / 4.0).floor() as i64;
        let every = if f.calm > 0.5 { 2 } else { 1 };
        if last_bar != Some(bar) && bar.rem_euclid(every) == 0 {
            last_bar = Some(bar);
            let n = conf.hue_slow.len().max(1) as f32;
            let lvl = (0.35 + 0.65 * f.intensity) * (1.0 - 0.6 * f.calm) * f.master.clamp(0.0, 1.0) * conf.level;
            let fade = if f.calm > 0.5 { 2000 } else { 400 };
            queue.clear();
            for (i, id) in conf.hue_slow.iter().enumerate() {
                let c = pal(&f.lut[..], f.hue + bar as f32 * 0.13 + i as f32 / n * 0.35);
                queue.push_back((id.clone(), c.map(|v| v * lvl), fade));
            }
        } else if last_bar.is_none() {
            last_bar = Some(bar);
        }
        *lock(&TAKEN) = Some((bridge.clone(), saved.clone()));
        if let Some((id, rgb, fade)) = queue.pop_front() {
            let sending = lock(&SENDING);
            if QUITTING.load(std::sync::atomic::Ordering::Acquire) {
                return;
            }
            if let Err(e) = crate::hue::set_light(&bridge, &id, rgb, fade) {
                *lock(&HUE_STATUS) = format!("{e:#}");
            }
            drop(sending);
            std::thread::sleep(gap);
        }
    }
}

/// White flash for this strip now? The strobe gate opens on each drum hit
/// while a strobe is live; flashes follow it, capped at `MAX_FLASH_HZ`.
fn flash_now(st: &mut Strip, f: &LightFrame, conf: &Conf) -> f32 {
    let open = f.strobe_live && f.strobe_open && conf.flashes && !f.smooth;
    let rising = open && !st.gate_was;
    st.gate_was = open;
    if rising && st.flash_at.elapsed().as_secs_f32() >= 1.0 / MAX_FLASH_HZ {
        st.flash_at = Instant::now();
    }
    // ~80 ms of white, fading.
    let age = st.flash_at.elapsed().as_secs_f32();
    if age < 0.08 { 1.0 - age / 0.08 } else { 0.0 }
}

fn pal(lut: &[u8], x: f32) -> [f32; 3] {
    let n = crate::palettes::LUT_SIZE;
    let i = ((x.rem_euclid(1.0) * (n - 1) as f32) as usize).min(n - 1);
    [lut[i * 4] as f32 / 255.0, lut[i * 4 + 1] as f32 / 255.0, lut[i * 4 + 2] as f32 / 255.0]
}

/// One strip of "follow the show" pixels into `px` (RGB bytes).
/// The beat-grid state every pixel of a frame shares (`look`), so strips
/// and Hue lights shade the same show from a position across the room.
struct Look {
    beat: f32,
    front: f32,
    chase: f32,
    base: f32,
    pulse: f32,
    drums: f32,
    soft: f32,
    drift: f32,
}

fn look(f: &LightFrame, lead_ms: f32, now: Instant) -> Look {
    // Where the beat will be when these lights change: the frame's
    // position moved on to now, plus the device's lead.
    let ahead = now.saturating_duration_since(f.at).as_secs_f64() + lead_ms as f64 / 1000.0;
    let pos = f.pos + ahead * f.bpm as f64 / 60.0;
    let beat = pos.fract() as f32;
    // `beat_in_bar` is the beat `f.pos` is in; count on from there.
    let bar_t = ((f.beat_in_bar as f64 + pos - f.pos.floor()).rem_euclid(4.0) / 4.0) as f32;
    let drums = (1.0 - f.calm).clamp(0.0, 1.0);
    let soft = if f.smooth { 0.4 } else { 1.0 };
    // Bar chase: once across per bar, turning round each phrase.
    let phrase = ((pos / 16.0).floor() as i64).rem_euclid(2) == 1;
    // Breakdown: a slow breath over two bars.
    let breath = 0.5 + 0.5 * (pos as f32 / 8.0 * std::f32::consts::TAU).sin();
    Look {
        beat,
        // Kick ripple from the middle, out over half a beat.
        front: beat * 2.0,
        chase: if phrase { 1.0 - bar_t } else { bar_t },
        base: (0.45 + 0.3 * f.intensity) * (0.65 + 0.35 * drums) + 0.35 * f.calm * breath,
        // Beat pulse: sharp attack on the beat, decays through it.
        pulse: (-beat * 7.0).exp() * drums * soft * (0.35 + 0.65 * f.hits4[0].max(f.pres4[0]).min(1.0)),
        drums,
        soft,
        drift: (pos as f32 / 64.0).fract(),
    }
}

/// Colour at `x` (0..1 across the strip or the room), linear, before the
/// master level, flashes and gamma.
fn shade(l: &Look, f: &LightFrame, x: f32) -> [f32; 3] {
    // The palette as is: squaring it (as the HDR scenes do) on top of the
    // LED gamma left mid colours nearly black.
    let col = pal(&f.lut[..], f.hue + l.drift + x * 0.45);
    let d = (x - 0.5).abs() * 2.0;
    let ripple = (-((d - l.front) * 9.0).powi(2)).exp() * (1.0 - l.beat) * l.drums * l.soft * 0.8;
    let cd = (x - l.chase).abs().min(1.0 - (x - l.chase).abs());
    let chase = (-(cd * 14.0).powi(2)).exp() * (0.25 + 0.5 * l.drums) * f.energy.clamp(0.3, 1.0);
    let v = (l.base + l.pulse * 0.9 + ripple + chase).min(1.4);
    [col[0] * v, col[1] * v, col[2] * v]
}

/// One strip of "follow the show" pixels into `px` (RGB bytes).
pub fn render_strip(px: &mut [u8], f: &LightFrame, c: &WledLight, level: f32, flash: f32, now: Instant) {
    let n = px.len() / 3;
    if n == 0 {
        return;
    }
    let l = look(f, c.lead_ms, now);
    let master = f.master.clamp(0.0, 1.0) * level;
    for i in 0..n {
        let col = shade(&l, f, (i as f32 + 0.5) / n as f32);
        for k in 0..3 {
            // Gamma: perceptual brightness to LED duty (WLED sends realtime
            // data uncorrected).
            let o = ((col[k] + flash) * master).clamp(0.0, 1.0).powf(1.6);
            px[i * 3 + k] = (o * 255.0 + 0.5) as u8;
        }
    }
}

/// Hue lights: the same show at each light's place across the room (its
/// x, -1..1). No flashes ever, and a gentler pulse: bulbs are slow and the
/// bridge isn't for strobing.
pub fn render_hue(out: &mut Vec<[f32; 3]>, f: &LightFrame, chans: &[crate::hue::Channel], lead_ms: f32, level: f32, now: Instant) {
    let mut l = look(f, lead_ms, now);
    l.pulse *= 0.7;
    let master = f.master.clamp(0.0, 1.0) * level;
    out.clear();
    for c in chans {
        let col = shade(&l, f, ((c.x + 1.0) * 0.5).clamp(0.0, 1.0));
        out.push(col.map(|v| (v * master).clamp(0.0, 1.0)));
    }
}

/// Settings > Test: a rainbow running along the strip.
fn render_test(px: &mut [u8]) {
    let n = px.len() / 3;
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_millis() % 1_000_000) as f32 / 1000.0)
        .unwrap_or(0.0);
    for i in 0..n {
        let h = (i as f32 / n.max(1) as f32 + s * 0.5).fract();
        let c = hsv(h);
        for k in 0..3 {
            px[i * 3 + k] = (c[k] * 0.6 * 255.0) as u8;
        }
    }
}

fn hsv(h: f32) -> [f32; 3] {
    let f = |n: f32| {
        let k = (n + h * 6.0) % 6.0;
        1.0 - (k.min(4.0 - k).clamp(0.0, 1.0))
    };
    [f(5.0), f(3.0), f(1.0)]
}

/// DDP: 10-byte header (version 1, push on the last packet, RGB 8-bit,
/// display 1, byte offset, length), then the pixels.
fn send_ddp(sock: &UdpSocket, addr: SocketAddr, seq: &mut u8, px: &[u8]) -> std::io::Result<()> {
    *seq = (*seq % 15) + 1;
    let mut off = 0usize;
    let mut buf = Vec::with_capacity(10 + DDP_CHUNK);
    while off < px.len() || px.is_empty() {
        let len = (px.len() - off).min(DDP_CHUNK);
        let last = off + len >= px.len();
        buf.clear();
        buf.push(0x40 | if last { 0x01 } else { 0x00 });
        buf.push(*seq);
        buf.push(0x0B); // RGB, 8 bits a channel
        buf.push(0x01);
        buf.extend_from_slice(&(off as u32).to_be_bytes());
        buf.extend_from_slice(&(len as u16).to_be_bytes());
        buf.extend_from_slice(&px[off..off + len]);
        match sock.send_to(&buf, addr) {
            Ok(_) => {}
            // A full send buffer drops this frame; the next one follows.
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) => return Err(e),
        }
        off += len;
        if px.is_empty() {
            break;
        }
    }
    Ok(())
}

// ---- finding WLED ----------------------------------------------------------

/// A WLED found on the network (Settings > Find).
#[derive(Clone, Debug)]
pub struct Found {
    pub name: String,
    pub host: String,
    pub leds: u32,
}

pub static FOUND: Mutex<Vec<Found>> = Mutex::new(Vec::new());
pub static SEARCHING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Browse `_wled._tcp` for a few seconds and read each one's name and LED
/// count from its JSON API, in the background.
pub fn find_wled() {
    use std::sync::atomic::Ordering;
    if SEARCHING.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::Builder::new()
        .name("wled-find".into())
        .spawn(|| {
            let mut hosts: Vec<String> = Vec::new();
            if let Ok(d) = mdns_sd::ServiceDaemon::new() {
                if let Ok(rx) = d.browse("_wled._tcp.local.") {
                    let end = Instant::now() + Duration::from_secs(4);
                    while let Some(left) = end.checked_duration_since(Instant::now()) {
                        match rx.recv_timeout(left) {
                            Ok(mdns_sd::ServiceEvent::ServiceResolved(info)) => {
                                if let Some(a) = info.get_addresses_v4().iter().next() {
                                    let h = a.to_string();
                                    if !hosts.contains(&h) {
                                        hosts.push(h);
                                    }
                                }
                            }
                            Ok(_) => {}
                            Err(_) => break,
                        }
                    }
                }
                let _ = d.shutdown();
            }
            let agent = ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(3)))
                .build()
                .new_agent();
            let mut found: Vec<Found> = hosts
                .iter()
                .filter_map(|h| {
                    let v: serde_json::Value =
                        agent.get(&format!("http://{h}/json/info")).call().ok()?.body_mut().read_json().ok()?;
                    Some(Found {
                        name: v["name"].as_str().unwrap_or(h).to_string(),
                        host: h.clone(),
                        leds: v["leds"]["count"].as_u64().unwrap_or(0) as u32,
                    })
                })
                .collect();
            found.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            *lock(&FOUND) = found;
            SEARCHING.store(false, Ordering::Release);
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(pos: f64, calm: f32) -> LightFrame {
        LightFrame {
            at: Instant::now(),
            lut: Arc::new(crate::palettes::lut("sunset")),
            hue: 0.0,
            pos,
            bpm: 120.0,
            beat_in_bar: 0,
            calm,
            intensity: 0.7,
            energy: 0.7,
            hits4: [0.8, 0.5, 0.3, 0.2],
            pres4: [0.6, 0.5, 0.3, 0.2],
            master: 1.0,
            strobe_live: false,
            strobe_open: false,
            smooth: false,
        }
    }

    fn strip() -> WledLight {
        WledLight { name: "t".into(), host: "127.0.0.1".into(), leds: 60, on: true, lead_ms: 0.0, reverse: false }
    }

    fn total(px: &[u8]) -> u32 {
        px.iter().map(|&v| v as u32).sum()
    }

    /// The strip is brightest on the beat and dimmer before the next one.
    #[test]
    fn pulses_on_the_beat() {
        let mut a = vec![0u8; 180];
        let mut b = vec![0u8; 180];
        let now = Instant::now();
        let mut f = frame(8.0, 0.0);
        f.at = now;
        render_strip(&mut a, &f, &strip(), 1.0, 0.0, now);
        f.pos = 8.7;
        render_strip(&mut b, &f, &strip(), 1.0, 0.0, now);
        assert!(total(&a) > total(&b) * 13 / 10, "on beat {} vs late {}", total(&a), total(&b));
    }

    /// Lead time: a strip with 100 ms lead at 120 BPM shows the beat 0.2
    /// beats early.
    #[test]
    fn lead_moves_the_pulse_earlier() {
        let now = Instant::now();
        let mut f = frame(7.8, 0.0);
        f.at = now;
        f.beat_in_bar = 3;
        let mut early = vec![0u8; 180];
        let mut c = strip();
        c.lead_ms = 200.0;
        render_strip(&mut early, &f, &c, 1.0, 0.0, now);
        let mut on = vec![0u8; 180];
        f.pos = 8.2;
        f.beat_in_bar = 0;
        render_strip(&mut on, &f, &strip(), 1.0, 0.0, now);
        // 7.8 + 0.4 beats of lead lands on 8.2.
        assert_eq!(early, on);
    }

    /// Blackout means black.
    #[test]
    fn blackout_is_black() {
        let mut px = vec![1u8; 180];
        let mut f = frame(8.0, 0.0);
        f.master = 0.0;
        render_strip(&mut px, &f, &strip(), 1.0, 0.0, f.at);
        assert_eq!(total(&px), 0);
    }

    /// Flashes never come faster than the cap, however fast the gate runs.
    #[test]
    fn flashes_are_rate_capped() {
        let mut st = Strip {
            host: String::new(),
            addr: None,
            resolved_at: Instant::now(),
            seq: 0,
            status: Default::default(),
            flash_at: Instant::now() - Duration::from_secs(1),
            gate_was: false,
        };
        let conf = Conf {
            on: true,
            level: 1.0,
            flashes: true,
            strips: vec![],
            hue: Default::default(),
            hue_on: false,
            hue_lead_ms: 0.0,
            hue_slow: vec![],
            test: None,
        };
        let mut f = frame(8.0, 0.0);
        f.strobe_live = true;
        let mut starts = 0;
        let t0 = Instant::now();
        let mut was = 0.0;
        while t0.elapsed() < Duration::from_millis(1000) {
            f.strobe_open = !f.strobe_open; // a 50 Hz gate: far too fast
            let v = flash_now(&mut st, &f, &conf);
            if v > 0.9 && was <= 0.9 {
                starts += 1;
            }
            was = v;
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(starts <= MAX_FLASH_HZ.ceil() as i32, "{starts} flashes in a second");
    }

    /// Brightness levels (mean byte over the strip) through a beat and a
    /// breakdown: lit, not near-black (the first build sat at 2-10 of 255
    /// on the strip).
    #[test]
    fn levels_are_visible() {
        let now = Instant::now();
        let mut px = vec![0u8; 157 * 3];
        // How lit a pixel looks: its brightest channel.
        let mean = |px: &[u8]| px.chunks(3).map(|c| *c.iter().max().unwrap() as f32).sum::<f32>() / (px.len() / 3) as f32;
        let mut lv = Vec::new();
        for (pos, calm) in [(8.0, 0.0), (8.25, 0.0), (8.5, 0.0), (8.9, 0.0), (8.0, 1.0), (10.0, 1.0)] {
            let mut f = frame(pos, calm);
            f.at = now;
            render_strip(&mut px, &f, &strip(), 0.8, 0.0, now);
            lv.push(mean(&px));
        }
        eprintln!("levels {lv:?}");
        assert!(lv[0] > 140.0, "on the beat {lv:?}");
        assert!(lv[3] > 45.0 && lv[3] < lv[0] * 0.7, "between beats {lv:?}");
        assert!(lv[4].min(lv[5]) > 30.0, "breakdown {lv:?}");
    }

    #[test]
    fn ddp_splits_long_strips() {
        let rx = UdpSocket::bind("127.0.0.1:0").unwrap();
        rx.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let tx = UdpSocket::bind("127.0.0.1:0").unwrap();
        let px = vec![7u8; 600 * 3];
        let mut seq = 0;
        send_ddp(&tx, rx.local_addr().unwrap(), &mut seq, &px).unwrap();
        let mut buf = [0u8; 2000];
        let n1 = rx.recv(&mut buf).unwrap();
        assert_eq!(n1, 10 + 1440);
        assert_eq!(buf[0], 0x40, "no push on the first packet");
        let n2 = rx.recv(&mut buf).unwrap();
        assert_eq!(n2, 10 + 360);
        assert_eq!(buf[0], 0x41, "push on the last");
        assert_eq!(u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]), 1440);
    }
}

/// `trippin --lights-demo <host> [seconds]`: drive one WLED strip with a
/// synthetic 126 BPM groove (beats, a breakdown in the middle), no audio or
/// window needed. Reads the LED count from the strip's JSON API.
pub fn demo(host: &str, secs: f32) -> anyhow::Result<()> {
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(3))).build().new_agent();
    let v: serde_json::Value = agent.get(&format!("http://{host}/json/info")).call()?.body_mut().read_json()?;
    let leds = v["leds"]["count"].as_u64().unwrap_or(0) as u32;
    println!("{}: {leds} LEDs", v["name"].as_str().unwrap_or(host));
    let lights = Lights::start();
    let mut s = crate::config::Settings::default();
    s.lights_on = true;
    s.light_level = 0.8;
    s.wled = vec![WledLight { name: host.into(), host: host.into(), leds, on: true, lead_ms: 0.0, reverse: false }];
    let lut = Arc::new(crate::palettes::lut("sunset"));
    let t0 = Instant::now();
    while t0.elapsed().as_secs_f32() < secs {
        let t = t0.elapsed().as_secs_f32();
        let pos = t as f64 * 126.0 / 60.0;
        let beat = pos.fract() as f32;
        // A breakdown through the middle third.
        let calm = if t > secs / 3.0 && t < secs * 2.0 / 3.0 { 1.0 } else { 0.0 };
        let hit = (-beat * 10.0).exp() * (1.0 - calm);
        lights.update(
            LightFrame {
                at: Instant::now(),
                lut: lut.clone(),
                hue: t * 0.01,
                pos,
                bpm: 126.0,
                beat_in_bar: (pos.floor() as u64) % 4,
                calm,
                intensity: 0.7,
                energy: 0.7,
                hits4: [hit, hit * 0.5, 0.3, 0.2],
                pres4: [0.6 * (1.0 - calm), 0.5, 0.3, 0.2],
                master: 1.0,
                strobe_live: false,
                strobe_open: false,
                smooth: false,
            },
            &s,
        );
        std::thread::sleep(Duration::from_millis(16));
    }
    println!("done; the strip goes back to its own effect in a few seconds");
    Ok(())
}
