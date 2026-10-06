//! Trippin — live music-reactive visuals for DJ sets.
//!
//! Usage: trippin [--list-devices] [--list-midi] [--device "<name part>"] [--mic] [--scene <name>]
//!                [--dancer [style]] [--no-dancer] [--canon] [--no-panel]
//!                [--gpu low] [--scale 0.75] [--fullscreen] [--vsync]
//!                [--song <audio file>] [--analyze <audio file>]
//!
//! `--song` (or dropping an audio file / timeline .json on the window) loads a
//! track for the Timeline tab: waveform + beat grid, draggable cues, record
//! mode (G) that writes hotkey/panel actions as cues while it plays (T).
//!
//! A control panel window opens alongside the visuals (F1 toggles it): modes,
//! scene playlist, dancer options, sync and rebindable hotkeys. Settings are
//! saved to trippin.json. Close the visuals window to quit; Esc only leaves
//! fullscreen, so a stray key can't end the show.
//!
//! Rendering runs on its own thread (`render_loop`): Windows parks the event
//! loop in a modal message pump while a native window is being dragged, so
//! doing GPU work in `RedrawRequested` froze the show whenever the panel was
//! moved. The event thread only handles input and the panel UI now; the
//! render thread free-runs, paced by vsync.

// Installer builds (`--features gui`) are a windowed app with no console;
// plain `cargo run` keeps the console for shader errors and logs.
#![cfg_attr(all(feature = "gui", windows), windows_subsystem = "windows")]
// The remote state frame is one big json! literal; its field count passed
// the default macro recursion limit (128).
#![recursion_limit = "256"]

mod ai;
mod audio;
mod beats;
mod engine;
mod link;
mod looks;
mod config;
mod dancer;
mod director;
mod editor;
mod gfx;
mod egui_win;
mod midi;
mod ndi;
mod nowplaying;
mod osc;
mod overlay;
mod remote;
mod rec;
#[cfg(windows)]
mod spout;
mod output;
mod palettes;
mod panel;
mod perf;
mod render;
mod snap;
mod song;
mod styles;
#[cfg(target_os = "macos")]
mod sysaudio;
mod tags;
mod text;
mod timeline;
mod ui_theme;

use std::sync::atomic::{AtomicBool, Ordering};

/// Strobe cue state (`CueKind::Strobe`) — set by timeline cues, read by the
/// render loop's master gate. A flag rather than another `&mut` threaded
/// through every cue path: it only lives inside a show, and
/// `apply_playhead` resets it on play / seek / stop.
static STROBE: AtomicBool = AtomicBool::new(false);

/// Whether the visuals window is fullscreen. The event thread owns the
/// window and publishes this every `about_to_wait`; the render thread copies
/// it into `Status` so the remote's state frame (and anything else reading
/// the shared status) sees it — the panel used to patch only its own copy.
static FULLSCREEN: AtomicBool = AtomicBool::new(false);

/// Frames are arriving from the external engine (Spout in) — the
/// `unity_stage` scene is only in rotation while this holds.
pub(crate) static EXT_LIVE: AtomicBool = AtomicBool::new(false);

/// The Unity engine's state for the Settings tab ("running", "not
/// installed", an error…).
pub(crate) static ENGINE_STATUS: Mutex<String> = Mutex::new(String::new());
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use anyhow::Result;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::Key;
use winit::window::{Fullscreen, Icon, Window, WindowId};

use audio::{AudioEngine, Command};
use config::{Action, Fx, Mode, Seasonal, Settings, Tristate, in_season, key_name, today};
use dancer::DancerLayer;
use director::Director;
use panel::{Panel, Status, UiCommand};
use render::{Gpu, Renderer, Uniforms};
use timeline::{Cue, CueKind, Matcher, PlayMode, SongCtl};

/// State shared between the event thread and the render thread.
struct Shared {
    /// Panel and hotkeys write; the render loop reads a copy each frame.
    settings: Mutex<Settings>,
    /// Live status for the panel, written by the render thread.
    status: Mutex<Status>,
    /// Timeline doc + transport/follow-live state.
    timeline: timeline::Shared,
    /// Raw onset envelope for the live-match correlator.
    env: audio::SharedEnv,
    /// Settings changed on the render thread — the event thread saves them.
    dirty: AtomicBool,
    /// What a timeline show borrowed, while it holds the rig. The saver
    /// writes these values instead of the show's, so a save during a show
    /// (a Look recall marks dirty) can't persist its Manual mode or palette.
    show_baseline: Mutex<Option<ShowBaseline>>,
    /// The visuals window is closing.
    quit: AtomicBool,
    /// Fixed after init; the panel lists them.
    scene_names: Vec<String>,
    /// `@heavy` flags parallel with `scene_names` (raymarched scenes).
    scene_heavy: Vec<bool>,
    /// The GPU tier can run `@heavy` scenes (panel greys them otherwise).
    heavy_ok: bool,
    clip_names: Vec<String>,
    /// Display titles parallel with `scene_names`/`clip_names` — pickers
    /// and the remote show these; the ids stay the keys everywhere.
    scene_titles: Arc<Vec<String>>,
    clip_titles: Arc<Vec<String>>,
    /// Last GPU baseline (`perf.json`): which scenes measured too slow —
    /// the picker flags them and the check deselects them.
    perf: Mutex<perf::Report>,
    /// Now playing: detector output, its live config, and a "show the card
    /// again" request from the hotkey.
    np: nowplaying::SharedNowPlaying,
    np_cfg: Arc<Mutex<nowplaying::NpConfig>>,
    /// Track pushed by a LAN agent over the remote protocol; the
    /// now-playing worker's `Remote` source polls it.
    np_remote: nowplaying::RemoteNp,
    np_replay: AtomicBool,
    /// Clip recorder requests from hotkeys/panel, handled on the render thread.
    rec_clip: AtomicBool,
    rec_set: AtomicBool,
    /// Finished scene thumbnails: key ("scene:<name>") → (w, h, RGBA8).
    /// Produced on the render thread. A store, not a queue — the panel and
    /// the editor each look up the keys they asked for, so the two windows
    /// can't eat each other's results.
    thumbs: Mutex<std::collections::HashMap<String, (u32, u32, Vec<u8>)>>,
    /// MIDI connection state for the panel's status dot: (connected, label).
    /// Written by the event thread where the connection lives.
    midi_status: Mutex<(bool, String)>,
    /// Remote-server state for the Settings card: (listening, status line).
    remote_status: Mutex<(bool, String)>,
    /// Saved Looks (files in `looks::looks_dir()`), mirrored here for the panel
    /// and the remote. Written only by the event thread.
    looks: Mutex<Vec<looks::SavedLook>>,
    /// A line the Looks row shows for a few seconds (a Look that skipped a
    /// scene this machine lacks). The GUI build has no console to log to.
    look_notice: Mutex<Option<(String, Instant)>>,
}

/// A pad press on the MIDI keyboard, posted to the event loop from midir's
/// callback thread. The raw note (not the mapped action) travels so the
/// Keys page can capture it for MIDI-learn. Remote commands (the iOS app's
/// WebSocket clients, OSC controllers) arrive the same way.
enum AppEvent {
    MidiNote(u8),
    Remote(remote::RemoteCmd),
}

/// Work the event thread hands to the render thread.
enum Msg {
    Resize(u32, u32),
    Act(Action),
    GoToScene(usize),
    /// Queue a scene as the next cut ("play next" — panel right-click).
    QueueNext(usize),
    ShowClip(usize),
    /// Decode an audio file and append it to the current timeline as a clip
    /// (a fresh doc is created when none is loaded).
    LoadSong(std::path::PathBuf),
    /// Open a saved timeline `.json` — decodes every clip's audio file.
    LoadTimeline(std::path::PathBuf),
    /// Fire a cue's effect immediately (editor preview).
    FireCue(CueKind),
    /// Transport control for the show player.
    Transport(SongCtl),
    /// Render a scene thumbnail for the editor's cue blocks.
    Thumb(String),
    /// Fade out every live text overlay (editor closed / show tidied up).
    FadeText,
    /// GPU baseline: step through @heavy + unity_* scenes, measure real
    /// frame cost, deselect anything under 30 fps.
    PerfCheck,
}

/// Lock even when poisoned: a panicking sibling thread shouldn't take the
/// visuals (or the panel) down with it mid-set.
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Scenes that compile, are ticked in the playlist and (for seasonal
/// scenes) are in season.
fn usable_scenes(r: &Renderer, s: &Settings) -> Vec<usize> {
    usable_scenes_mood(r, s, false)
}

/// `usable_scenes`, with the Style's calm pool in effect while `calm` (a
/// breakdown). Only the director's per-frame list wants `calm`; one-off picks
/// (next/prev, show playback) use the main pool.
fn usable_scenes_mood(r: &Renderer, s: &Settings, calm: bool) -> Vec<usize> {
    let names = r.scene_names();
    // The selected Style narrows what's left (never what the user turned off:
    // allowed_scenes removed those, so a Style can't bring them back).
    styles::narrow(&allowed_scenes(r, s), &|i| names[i].clone(), s, styles::catalog(), calm)
}

/// The director's per-frame list: `allowed` narrowed by the Style (its calm
/// pool while `calm`). A scene the DJ picked or queued by hand outside the pool
/// stays on air: the director cuts away at once from a scene that isn't usable
/// (meant for one switched off in the playlist) and drops a queued one at cut
/// time, so they are kept while the user allows them. The pickers skip the
/// live scene, so every automatic pick is still the Style's and the next phrase
/// cut goes back to the pool.
fn director_scenes(r: &Renderer, s: &Settings, allowed: &[usize], calm: bool, dir: &Director) -> Vec<usize> {
    let mut usable = styles::narrow(allowed, &|i| r.scene_name(i).to_string(), s, styles::catalog(), calm);
    for keep in [Some(dir.scene), dir.next.filter(|_| dir.next_queued)].into_iter().flatten() {
        if allowed.contains(&keep) && !usable.contains(&keep) {
            usable.push(keep);
        }
    }
    usable.sort_unstable();
    usable
}

/// The scenes the user allows (compiled, ticked, in season, GPU tier), before
/// the Style narrows them.
fn allowed_scenes(r: &Renderer, s: &Settings) -> Vec<usize> {
    let names = r.scene_names();
    let heavy = r.scene_heavy();
    let all = r.usable_scenes();
    let date = today();
    // `@heavy` (raymarched) scenes need a GPU that can keep up; the user can
    // force them on or off regardless of the detected tier.
    let heavy_on = match s.heavy_scenes {
        Tristate::Auto => r.heavy_ok(),
        Tristate::On => true,
        Tristate::Off => false,
    };
    let on: Vec<usize> = all
        .iter()
        .copied()
        // `void` is the timeline's "scenes off" baseline — reachable only
        // via an explicit cue, never by autopilot or next/prev.
        .filter(|&i| names[i] != "void")
        .filter(|&i| !names[i].starts_with("unity_") || EXT_LIVE.load(Ordering::Relaxed))
        .filter(|&i| !s.disabled_scenes.contains(&names[i]))
        .filter(|&i| heavy_on || !heavy[i])
        .filter(|&i| s.flat_scenes || heavy[i])
        .filter(|&i| match (s.seasonal, in_season(&names[i], date)) {
            (_, None) | (Seasonal::Always, _) => true,
            (Seasonal::Off, Some(_)) => false,
            (Seasonal::Auto, Some(in_now)) => in_now,
        })
        .collect();
    if on.is_empty() { all } else { on }
}

/// Stop the show player and put the live audio engine back.
fn stop_song(
    player: &mut Option<song::ShowPlayer>,
    audio: &mut AudioEngine,
    cfg: &(Option<String>, bool),
    shared: &Shared,
) {
    if player.take().is_some() {
        match AudioEngine::start(cfg.0.as_deref(), cfg.1, Some(shared.env.clone())) {
            Ok(eng) => *audio = eng,
            Err(e) => eprintln!("live audio restart failed: {e:#}"),
        }
    }
}

/// The live rig's settings the first time a timeline takes over — the show
/// borrows mode/dancer/fx while it plays, and they must be handed back (and
/// never persisted) when it stops, or the live show looks "stuck" afterwards.
#[derive(Clone)]
struct ShowBaseline {
    mode: Mode,
    dancer_enabled: bool,
    dancer_style: Option<usize>,
    dancer_trails: bool,
    canon: Tristate,
    fx: Fx,
    fx_auto: bool,
    palette: String,
    scene: usize,
    blackout: bool,
    dancer_showing: bool,
}

impl ShowBaseline {
    fn take(shared: &Shared, dir: &Director, dancer: &DancerLayer, blackout: bool) -> Self {
        let s = lock(&shared.settings);
        Self {
            mode: s.mode,
            dancer_enabled: s.dancer_enabled,
            dancer_style: s.dancer_style,
            dancer_trails: s.dancer_trails,
            canon: s.canon,
            fx: s.fx,
            fx_auto: s.fx_auto,
            palette: s.palette.clone(),
            scene: dir.scene,
            blackout,
            dancer_showing: dancer.showing,
        }
    }

    /// Write the borrowed settings back into `s`.
    fn restore_into(&self, s: &mut Settings) {
        s.mode = self.mode;
        s.dancer_enabled = self.dancer_enabled;
        s.dancer_style = self.dancer_style;
        s.dancer_trails = self.dancer_trails;
        s.canon = self.canon;
        s.fx = self.fx;
        s.fx_auto = self.fx_auto;
        s.palette = self.palette.clone();
    }
}

/// End the show: stop the player, swap audio back to the live input, hand the
/// borrowed settings back, and fade any text the show (or a palette preview)
/// left on screen.
fn end_show(
    player: &mut Option<song::ShowPlayer>,
    audio: &mut AudioEngine,
    cfg: &(Option<String>, bool),
    shared: &Shared,
    pre_show: &mut Option<ShowBaseline>,
    dir: &mut Director,
    dancer: &mut DancerLayer,
    text_slots: &mut [Option<text::TextState>; text::TEXT_SLOTS],
    blackout: &mut bool,
    started: Instant,
) {
    stop_song(player, audio, cfg, shared);
    if let Some(b) = pre_show.take() {
        b.restore_into(&mut lock(&shared.settings));
        *lock(&shared.show_baseline) = None;
        // A save during the show wrote the baseline (see the saver); save
        // again so anything else changed since lands too.
        shared.dirty.store(true, Ordering::Relaxed);
        *blackout = b.blackout;
        STROBE.store(false, Ordering::Relaxed);
        dancer.showing = b.dancer_showing;
        if dir.scene != b.scene {
            dir.cut_to(b.scene);
        }
    }
    let now_t = (Instant::now() - started).as_secs_f32();
    for ts in text_slots.iter_mut().flatten() {
        ts.out_at.get_or_insert(now_t);
    }
}

/// Fire one timeline cue — mirrors `apply_render` for the cue kinds.
/// `usable` is this frame's usable-scenes list. `persist` marks the settings
/// dirty for saving — only for explicit user previews; cues fired by timeline
/// playback are show state and must never reach trippin.json.
fn fire_cue(
    kind: &CueKind,
    r: &mut Renderer,
    dir: &mut Director,
    dancer: &mut DancerLayer,
    text_slots: &mut [Option<text::TextState>; text::TEXT_SLOTS],
    now_t: f32,
    blackout: &mut bool,
    shared: &Shared,
    usable: &[usize],
    persist: bool,
) {
    let mut s = lock(&shared.settings);
    match kind {
        CueKind::Scene(name) => {
            if let Some(i) = r.scene_names().iter().position(|n| n == name) {
                dir.cut_to(i);
            }
        }
        CueKind::NextScene => dir.next_scene(usable, s.random_order),
        CueKind::PrevScene => dir.prev_scene(usable),
        CueKind::Mode(m) => s.mode = *m,
        CueKind::Dancer(on) => {
            s.dancer_enabled = *on;
            if *on {
                dancer.showing = true;
            }
        }
        CueKind::Clip(name) => {
            if let Some(i) = shared.clip_names.iter().position(|n| n == name) {
                dancer.pin(i);
            }
            dancer.showing = true;
            s.dancer_enabled = true;
        }
        CueKind::NextClip => {
            dancer.next_clip();
            dancer.showing = true;
            s.dancer_enabled = true;
        }
        CueKind::NextLook => s.dancer_style = Some((dancer.style + 1) % dancer::STYLES.len()),
        CueKind::Look(l) => {
            s.dancer_style = l.map(|i| i.min(dancer::STYLES.len() - 1));
        }
        CueKind::Trails(b) => s.dancer_trails = *b,
        CueKind::Canon(t) => s.canon = *t,
        CueKind::Blackout(b) => *blackout = *b,
        CueKind::Strobe(b) => STROBE.store(*b, Ordering::Relaxed),
        CueKind::Fx(f) => {
            s.fx_auto = false;
            s.fx = *f;
        }
        CueKind::FxAuto(b) => s.fx_auto = *b,
        CueKind::Palette(n) => {
            if crate::palettes::is_valid(n) {
                s.palette = n.clone();
            }
        }
        CueKind::Text(spec) => {
            let lane = spec.lane as usize % text::TEXT_SLOTS;
            if spec.text.trim().is_empty() {
                return;
            }
            match text::rasterize(&spec.text, 96.0) {
                Some(bmp) => {
                    let aspect = bmp.width as f32 / bmp.height.max(1) as f32;
                    r.set_text_bitmap(lane, &bmp);
                    text_slots[lane] = Some(text::TextState {
                        spec: spec.clone(),
                        aspect,
                        born: now_t,
                        out_at: None,
                    });
                }
                None => {
                    lock(&shared.timeline).message =
                        "text: no usable font found — drop a .ttf in fonts/".into();
                }
            }
        }
        CueKind::TextOff(lane) => {
            let lane = *lane as usize % text::TEXT_SLOTS;
            if let Some(ts) = text_slots[lane].as_mut() {
                ts.out_at.get_or_insert(now_t);
            }
        }
    }
    if persist {
        shared.dirty.store(true, Ordering::Relaxed);
    }
}

/// Apply the timeline's implied state at the playhead (see
/// [`Timeline::state_at`]) — called when playback starts and after every
/// seek, so skipping around lands on the right scene/dancer/fx state and a
/// fresh song starts with everything off until a cue turns it on.
fn apply_playhead(
    st: &timeline::PlayheadState,
    r: &mut Renderer,
    dir: &mut Director,
    dancer: &mut DancerLayer,
    text_slots: &mut [Option<text::TextState>; text::TEXT_SLOTS],
    now_t: f32,
    blackout: &mut bool,
    shared: &Shared,
    usable: &[usize],
    pre_show: &mut Option<ShowBaseline>,
) {
    // First borrow of the rig: remember what the live show looked like so the
    // show's state never leaks past its end (see `end_show`).
    if pre_show.is_none() {
        let b = ShowBaseline::take(shared, dir, dancer, *blackout);
        *lock(&shared.show_baseline) = Some(b.clone());
        *pre_show = Some(b);
    }
    {
        let mut s = lock(&shared.settings);
        s.mode = st.mode;
        s.dancer_enabled = st.dancer;
        dancer.showing = st.dancer;
        if let Some(l) = st.look {
            s.dancer_style = l;
        }
        if st.look_steps != 0 {
            let n = dancer::STYLES.len() as i64;
            let base = s.dancer_style.unwrap_or(dancer.style) as i64;
            s.dancer_style = Some((base + st.look_steps).rem_euclid(n) as usize);
        }
        s.dancer_trails = st.trails;
        s.canon = st.canon;
        s.fx = st.fx;
        s.fx_auto = st.fx_auto;
        if let Some(p) = &st.palette {
            s.palette = p.clone();
        }
        *blackout = st.blackout;
        STROBE.store(st.strobe, Ordering::Relaxed);
    }

    // Scene: the last absolute cue wins; before any scene cue the baseline
    // is `void` (black) so a song starts with scenes "off". Net next/prev
    // steps then apply relative to whatever that leaves showing.
    let want = st.scene.as_deref().unwrap_or("void");
    if let Some(i) = r.scene_names().iter().position(|n| n == want) {
        if i != dir.scene {
            dir.cut_to(i);
        }
    }
    if st.scene_steps != 0 && !usable.is_empty() {
        let n = usable.len() as i64;
        let cur = usable.iter().position(|&x| x == dir.scene).unwrap_or(0) as i64;
        let target = usable[(cur + st.scene_steps).rem_euclid(n) as usize];
        if target != dir.scene {
            dir.cut_to(target);
        }
    }

    // Routine: request only on change — loading a PNG sequence isn't cheap.
    if let Some(name) = &st.clip {
        if let Some(i) = shared.clip_names.iter().position(|n| n == name) {
            let n = shared.clip_names.len().max(1) as i64;
            let idx = ((i as i64 + st.clip_steps).rem_euclid(n)) as usize;
            if dancer.current() != Some(idx) {
                dancer.pin(idx);
            }
        }
    }

    // Text lanes: diff against what's live so seeks don't re-rasterize a
    // card that's already up (but a faded-out one gets revived).
    for lane in 0..text::TEXT_SLOTS {
        let want = st.text[lane].as_ref();
        let have = text_slots[lane]
            .as_ref()
            .filter(|t| t.out_at.is_none())
            .map(|t| &t.spec);
        let kind = match (want, have) {
            (Some(w), Some(h)) if w == h => continue,
            (Some(w), _) => CueKind::Text(w.clone()),
            (None, Some(_)) => CueKind::TextOff(lane as u8),
            (None, None) => continue,
        };
        fire_cue(
            &kind, r, dir, dancer, text_slots, now_t, blackout, shared, usable, false,
        );
    }
}

/// Refit the director's queued next scene to the music's mood. Cuts can
/// land anywhere now, so the pick made at scene start may be stale by the
/// time an event fires: in a breakdown the queue should hold a calm scene,
/// in a hot section a hot one. Random order only — an ordered playlist is
/// a curated sequence, and a queued "play next" is the operator's call.
fn repick_for_mood(dir: &mut Director, r: &Renderer, usable: &[usize], f: &audio::Features, s: &Settings) {
    if s.mode != Mode::Auto || !s.random_order || usable.len() < 2 || dir.next_queued {
        return;
    }
    // Mid-moods get no opinion — any pick is honest.
    let target = if f.calm > 0.55 {
        0.25
    } else if f.calm < 0.3 && f.energy > 0.6 {
        0.75
    } else {
        return;
    };
    // Distance from the target mood. An unrated scene (the `unity_*` shows:
    // no energy measure exists for them) fits any mood rather than sitting
    // at the 0.5 default, which is too far from both targets — that kept
    // every Unity show out of hot and calm stretches.
    let dist = |n: usize| {
        let m = ai::scene_meta(r.scene_name(n));
        if m.rated { (m.energy - target).abs() } else { 0.0 }
    };
    let close = |n: usize| dist(n) <= 0.22;
    // In a breakdown a Style prefers its own calm scenes.
    let pool = if target < 0.5 {
        styles::calm_candidates(usable, &|i| r.scene_name(i).to_string(), s, styles::catalog())
    } else {
        usable.to_vec()
    };
    // Keep the queued pick only if it already fits: near the target energy
    // AND in the pool (a low-energy main-pool scene isn't a Style's calm pick).
    if dir.next.is_some_and(|n| close(n) && pool.contains(&n)) {
        return;
    }
    // Pick among every scene that fits the mood and hasn't played lately;
    // only when none fits, the nearest cluster. Taking just the nearest
    // cluster from a small Style pool repeated the same few scenes.
    let fresh: Vec<usize> = pool
        .iter()
        .copied()
        .filter(|&n| n != dir.scene && !dir.recent(n, pool.len()))
        .collect();
    let others: Vec<usize> = pool.iter().copied().filter(|&n| n != dir.scene).collect();
    let cands = if fresh.is_empty() { others } else { fresh };
    let rated = |n: usize| ai::scene_meta(r.scene_name(n)).rated;
    let mut top: Vec<usize> = cands.iter().copied().filter(|&n| rated(n) && close(n)).collect();
    let unrated: Vec<usize> = cands.iter().copied().filter(|&n| !rated(n)).collect();
    // Unrated scenes count as fitting, but only at the rate rated ones do:
    // all of them in, against half the rated pool in a hot stretch, made
    // Unity shows 21 of 49 cuts on Party (32% of its pool; median).
    let n_rated = cands.len() - unrated.len();
    let fit = if n_rated == 0 { 1.0 } else { top.len() as f32 / n_rated as f32 };
    // No rated scene fits: the nearest rated cluster, unrated at par.
    let w_unrated = if top.is_empty() { 1.0 } else { fit };
    if top.is_empty() && n_rated > 0 {
        let rd = |n: usize| if rated(n) { dist(n) } else { f32::INFINITY };
        let best = cands.iter().map(|&n| rd(n)).fold(f32::INFINITY, f32::min);
        top = cands.iter().copied().filter(|&n| rd(n) <= best + 0.1).collect();
    }
    // Weighted pick: each rated fit counts 1, each unrated scene `w_unrated`.
    let total = top.len() as f32 + unrated.len() as f32 * w_unrated;
    if total > 0.0 {
        let mut x = dir.rand() * total;
        let pick = if x < top.len() as f32 {
            top[(x as usize).min(top.len() - 1)]
        } else {
            x -= top.len() as f32;
            unrated[((x / w_unrated) as usize).min(unrated.len() - 1)]
        };
        dir.next = Some(pick);
    }
}

/// The whole show, free-running on its own thread at vsync pace.
fn render_loop(
    mut r: Renderer,
    mut dir: Director,
    mut dancer: DancerLayer,
    mut audio: AudioEngine,
    mut audio_cfg: (Option<String>, bool),
    shared: Arc<Shared>,
    rx: mpsc::Receiver<Msg>,
) {
    let started = Instant::now();
    let mut last_frame = started;
    // Some present modes self-pace (vsync, or drawable back-pressure); when
    // they don't — macOS Immediate — cap the loop at the display's refresh.
    let mut frame_interval = r.frame_interval();
    let mut next_frame = started;
    let mut pace_recheck = started;
    let mut last_reload_check = started;
    let mut last_status = Instant::now();
    let mut fps = 0.0f32;
    let mut flow = 0.0f64;
    // External engine link (Settings > unity_link).
    let mut link_out: Option<link::Link> = None;
    let mut engine: Option<engine::Engine> = None;
    let mut ext_seq = 0u64;
    let mut flow_bpm = 120.0f32;
    let groove_log = std::env::var_os("TRIPPIN_GROOVE_LOG").is_some();
    let mut last_groove_log = Instant::now();
    // Camera-clock speed: eases to ~0.55x in breakdowns (floaty), back to 1x
    // with the drums. Integrated into `flow`, so it never jumps.
    let mut flow_speed = 1.0f32;
    // Energy clocks (see Uniforms::clock4) and the smoothed levels driving
    // them — ~0.35 s smoothing, so a kick is a surge, never a jolt.
    let mut clock4 = [0.0f64; 4];
    let mut clock_lvl = [0.0f32; 4];
    let mut blackout = false;
    let mut master = 1.0f32;
    // `palette = "auto"` mood matcher (palettes::Auto).
    let mut auto_pal = palettes::Auto::default();
    // The selected Style's auto-palette families, cached by Style id.
    let mut style_pals: Option<(String, [Vec<&'static str>; 4])> = None;
    // The track is in a breakdown, as far as a Style's calm pool is concerned
    // (same 0.7 in / 0.3 out hysteresis as the director's own breakdown mode).
    let mut style_calm = false;
    // Audio watchdog: a stream that dies or goes silent gets rebuilt after
    // `audio_retry` of sustained silence, backing off when the source is
    // genuinely quiet so it isn't re-opened forever.
    let mut silent_since: Option<Instant> = None;
    let mut last_audio_attempt: Option<Instant> = None;
    let mut audio_retry = Duration::from_secs(15);
    // The post effect showing now — the auto-pilot's pick when `fx_auto` is on.
    let mut fx_current = lock(&shared.settings).fx;

    // --- Timeline state ---------------------------------------------------
    // Decodes run on throwaway threads; results land in `song_rx` tagged with
    // their path and the player swap happens here (cpal streams aren't Send).
    let (song_tx, song_rx) = mpsc::channel::<(std::path::PathBuf, Result<song::Song, String>)>();
    // Decoded audio, keyed by file path — clips reference songs by path.
    let mut songs: std::collections::HashMap<std::path::PathBuf, Arc<song::Song>> =
        Default::default();
    let mut player: Option<song::ShowPlayer> = None;
    // Decode jobs in flight, and which of them are "append as a new clip"
    // (vs filling a placeholder clip from a loaded .json).
    let mut pending_decodes = 0usize;
    let mut pending_adds: Vec<std::path::PathBuf> = Vec::new();
    let mut matcher = Matcher::new();
    // Live text overlays — one per text lane; `TextOff` starts the fade-out.
    let mut text_slots: [Option<text::TextState>; text::TEXT_SLOTS] = [None, None];
    let mut overlays = overlay::Overlays::default();
    // Clip recorder (replay buffer / whole-set), and whether a set is rolling.
    let mut recorder: Option<rec::Recorder> = None;
    let mut rec_err: Option<String> = None;
    let mut set_on = false;
    // Auto drop clips: when to save the clip for a drop (its tail has to
    // play out first), when the last one was saved, and how many this run.
    let mut drop_clip_at: Option<(Instant, u32)> = None;
    let mut last_auto_clip: Option<Instant> = None;
    let mut auto_clips = 0u32;
    // Live rig as it was before a timeline borrowed it — restored on show end.
    let mut pre_show: Option<ShowBaseline> = None;
    // GPU baseline (Settings > performance check): steps @heavy + unity_*
    // scenes, measures real frame cost, deselects <30 fps stragglers.
    let mut check: Option<perf::Check> = None;
    // Global seconds up to which cues were already dispatched — one-shot.
    let mut fired_past = f64::MIN;
    let mut was_locked = false;
    // The saved audio source the running engine corresponds to — a panel
    // change to `audio_in` restarts capture live (see the frame loop).
    let mut audio_sel = lock(&shared.settings).audio_in.clone();
    let spawn_load = |tx: &mpsc::Sender<(std::path::PathBuf, Result<song::Song, String>)>,
                      path: std::path::PathBuf| {
        let tx = tx.clone();
        std::thread::spawn(move || {
            // A panicking decode must still reply — otherwise pending_decodes
            // never drains and the editor's busy flag wedges on.
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                song::load(&path).map_err(|e| format!("{e:#}"))
            }))
            .unwrap_or_else(|_| Err("decoder crashed".into()));
            let _ = tx.send((path, res));
        });
    };

    while !shared.quit.load(Ordering::Relaxed) {
        // Resize events can get lost or arrive out of order during fullscreen
        // transitions — trust the window's real size, checked every frame.
        r.sync_size();

        // Anything the event thread asked for since last frame.
        while let Ok(msg) = rx.try_recv() {
            match msg {
                // Route through the settling window too — fullscreen
                // transitions emit a burst of sizes.
                Msg::Resize(w, h) => r.note_size(w, h),
                Msg::GoToScene(i) => dir.cut_to(i),
                Msg::QueueNext(i) => dir.queue_next(i),
                Msg::PerfCheck => {
                    if let Some(chk) = check.take() {
                        // Pressed again = cancel: hand the user's scene back.
                        dir.cut_to(chk.home);
                    } else {
                        check = Some(perf::Check::new(&r, dir.scene));
                    }
                }
                Msg::ShowClip(i) => {
                    dancer.pin(i);
                    dancer.showing = true;
                }
                Msg::Act(a) => apply_render(
                    &mut r,
                    &mut dir,
                    &mut dancer,
                    &mut blackout,
                    &audio,
                    &shared,
                    a,
                ),
                Msg::LoadSong(path) => {
                    eprintln!("Timeline: load song {}", path.display());
                    // Already decoded? Append the clip straight away.
                    if let Some(sg) = songs.get(&path) {
                        let mut tl = lock(&shared.timeline);
                        if tl.doc.is_none() {
                            tl.doc = Some(timeline::Timeline::default());
                        }
                        tl.doc.as_mut().unwrap().add_song(sg);
                        tl.dirty = true;
                        tl.message = format!("added {}", sg.name);
                    } else if !pending_adds.contains(&path) {
                        pending_adds.push(path.clone());
                        pending_decodes += 1;
                        let mut tl = lock(&shared.timeline);
                        tl.busy = true;
                        tl.message = format!("loading {}…", path.display());
                        drop(tl);
                        spawn_load(&song_tx, path);
                    }
                }
                Msg::LoadTimeline(path) => match timeline::Timeline::load(&path) {
                    Ok(doc) => {
                        let name = doc.name.clone();
                        // Decode every clip's audio that isn't already loaded.
                        let missing: Vec<std::path::PathBuf> = doc
                            .clips
                            .iter()
                            .map(|c| c.song.clone())
                            .filter(|p| !songs.contains_key(p) && p.exists())
                            .collect();
                        let missing_n = missing.len();
                        let lacked: Vec<String> = doc
                            .clips
                            .iter()
                            .filter(|c| !songs.contains_key(&c.song) && !c.song.exists())
                            .map(|c| c.name.clone())
                            .collect();
                        {
                            let mut tl = lock(&shared.timeline);
                            tl.doc = Some(doc);
                            tl.mode = PlayMode::Stopped;
                            tl.pos_s = 0.0;
                            tl.cursor_s = 0.0;
                            tl.dirty = false;
                            tl.busy = missing_n > 0;
                            tl.message = if lacked.is_empty() {
                                format!("timeline {name}")
                            } else {
                                format!("timeline {name} — missing audio: {}", lacked.join(", "))
                            };
                        }
                        matcher.reset();
                        was_locked = false;
                        fired_past = f64::MIN;
                        end_show(
                            &mut player,
                            &mut audio,
                            &audio_cfg,
                            &shared,
                            &mut pre_show,
                            &mut dir,
                            &mut dancer,
                            &mut text_slots,
                            &mut blackout,
                            started,
                        );
                        pending_decodes += missing_n;
                        for p in missing {
                            spawn_load(&song_tx, p);
                        }
                    }
                    Err(e) => lock(&shared.timeline).message = format!("open failed: {e:#}"),
                },
                Msg::FireCue(kind) => {
                    let usable_now = {
                        let s = lock(&shared.settings);
                        usable_scenes(&r, &s)
                    };
                    fire_cue(
                        &kind,
                        &mut r,
                        &mut dir,
                        &mut dancer,
                        &mut text_slots,
                        (Instant::now() - started).as_secs_f32(),
                        &mut blackout,
                        &shared,
                        &usable_now,
                        true,
                    );
                }
                Msg::FadeText => {
                    let now_t = (Instant::now() - started).as_secs_f32();
                    for ts in text_slots.iter_mut().flatten() {
                        ts.out_at.get_or_insert(now_t);
                    }
                }
                Msg::Thumb(key) => {
                    // Editor keys thumbs "scene:<name>"; render by bare name.
                    let name = key.strip_prefix("scene:").unwrap_or(&key);
                    match r.scene_names().iter().position(|n| *n == name) {
                        Some(i) => match r.thumbnail(i, 128, 72) {
                            Some(px) => {
                                lock(&shared.thumbs).insert(key.clone(), (128, 72, px));
                            }
                            None => eprintln!("Thumb: render failed for {name}"),
                        },
                        None => eprintln!("Thumb: no scene named {name}"),
                    }
                }
                Msg::Transport(ctl) => {
                    eprintln!(
                        "Timeline: transport {ctl:?} (mode {:?})",
                        lock(&shared.timeline).mode
                    );
                    let mut tl = lock(&shared.timeline);
                    // State the playhead implies after this transport op —
                    // applied once the timeline lock is released below.
                    let mut apply_st = None;
                    match ctl {
                        SongCtl::Toggle => match tl.mode {
                            PlayMode::Playing => {
                                if let Some(p) = &player {
                                    p.set_playing(false);
                                }
                                tl.mode = PlayMode::Paused;
                            }
                            PlayMode::Paused => {
                                if let Some(p) = &player {
                                    p.set_playing(true);
                                }
                                tl.mode = PlayMode::Playing;
                            }
                            PlayMode::Stopped => {
                                let regions = tl
                                    .doc
                                    .as_ref()
                                    .map(|d| d.regions(&songs))
                                    .unwrap_or_default();
                                if regions.is_empty() {
                                    tl.message = if tl.busy {
                                        "still decoding…".into()
                                    } else {
                                        "no decoded songs on the timeline".into()
                                    };
                                } else {
                                    match AudioEngine::start_show(
                                        regions,
                                        tl.cursor_s,
                                        Some(shared.env.clone()),
                                    ) {
                                        Ok((eng, pl)) => {
                                            fired_past = tl.cursor_s;
                                            apply_st =
                                                tl.doc.as_ref().map(|d| d.state_at(tl.cursor_s));
                                            audio = eng;
                                            player = Some(pl);
                                            tl.mode = PlayMode::Playing;
                                            tl.message.clear();
                                            println!("Timeline: playing");
                                        }
                                        Err(e) => {
                                            tl.message = format!("play failed: {e:#}");
                                            eprintln!("Timeline: {e:#}");
                                        }
                                    }
                                }
                            }
                        },
                        SongCtl::Stop => {
                            tl.mode = PlayMode::Stopped;
                            tl.pos_s = 0.0;
                            tl.live_locked = false;
                            drop(tl);
                            end_show(
                                &mut player,
                                &mut audio,
                                &audio_cfg,
                                &shared,
                                &mut pre_show,
                                &mut dir,
                                &mut dancer,
                                &mut text_slots,
                                &mut blackout,
                                started,
                            );
                            // A stale lock must not re-apply show state over
                            // the rig we just handed back.
                            matcher.reset();
                            was_locked = false;
                            fired_past = f64::MIN;
                            continue;
                        }
                        SongCtl::Seek(t) => {
                            if let Some(p) = &player {
                                p.seek(t);
                                tl.pos_s = t;
                                fired_past = t;
                            } else {
                                tl.cursor_s = t.max(0.0);
                                tl.pos_s = tl.cursor_s;
                            }
                            apply_st = tl.doc.as_ref().map(|d| d.state_at(t));
                        }
                    }
                    drop(tl);
                    if let Some(st) = apply_st {
                        let usable_now = {
                            let s = lock(&shared.settings);
                            usable_scenes(&r, &s)
                        };
                        apply_playhead(
                            &st,
                            &mut r,
                            &mut dir,
                            &mut dancer,
                            &mut text_slots,
                            (Instant::now() - started).as_secs_f32(),
                            &mut blackout,
                            &shared,
                            &usable_now,
                            &mut pre_show,
                        );
                    }
                }
            }
        }

        let now = Instant::now();
        let dt = (now - last_frame).as_secs_f32().min(0.1);
        last_frame = now;
        if dt > 0.0 {
            fps += (1.0 / dt - fps) * 0.05;
        }
        let mut s = lock(&shared.settings).clone();
        // The Style's pacing overlays this per-frame clone; nothing is saved.
        styles::overlay(&mut s, styles::catalog());
        // While the GPU check is on a `unity_*` scene the engine link is
        // borrowed so its shows can be measured — this is the per-frame
        // clone, so the saved setting is never touched.
        if check.as_ref().is_some_and(|c| c.cur_is_unity()) {
            s.unity_link = true;
        }
        let allowed = allowed_scenes(&r, &s);
        let mut usable = director_scenes(&r, &s, &allowed, style_calm, &dir);
        {
            let mut c = lock(&shared.np_cfg);
            if c.source != s.np_source || c.delay_s != s.np_delay_s || c.file != s.np_file {
                *c = nowplaying::NpConfig {
                    source: s.np_source,
                    delay_s: s.np_delay_s,
                    file: s.np_file.clone(),
                };
            }
        }
        // Audio-in picker: switching the saved source swaps the capture
        // live. While a show plays the engine belongs to it — the new
        // source takes over when the show ends (audio_cfg feeds stop_song).
        if s.audio_in != audio_sel {
            audio_sel = s.audio_in.clone();
            audio_cfg.0 = (!audio_sel.is_empty()).then(|| audio_sel.clone());
            if player.is_none() {
                match AudioEngine::start(
                    audio_cfg.0.as_deref(),
                    audio_cfg.1,
                    Some(shared.env.clone()),
                ) {
                    Ok(eng) => {
                        println!("Audio: {}", eng.device_name);
                        audio = eng;
                    }
                    Err(e) => eprintln!("audio input switch failed: {e:#}"),
                }
            }
        }
        // Global palette — a no-op while the name is unchanged. `auto` is
        // resolved against the live features below (they don't exist yet).
        if s.palette != palettes::AUTO {
            r.set_palette(&s.palette);
        }
        // NDI output — a conf change rebuilds it; otherwise a cheap no-op.
        r.transparent = s.out_transparent;
        // Clip recorder: runs while the buffer is armed or a set is rolling.
        {
            let want_set = shared.rec_set.load(Ordering::Relaxed);
            let want = s.rec_buffer || want_set || set_on;
            let conf = if want {
                match rec::find_ffmpeg(&s.ffmpeg_path) {
                    Some(ff) => {
                        let (w, h) = output::Conf {
                            name: String::new(),
                            ndi: false,
                            spout: false,
                            transparent: false,
                            record: true,
                            height: s.ndi_height,
                            fps: s.ndi_fps,
                        }
                        .size();
                        rec_err = None;
                        Some(rec::RecConf {
                            ffmpeg: ff,
                            width: w,
                            height: h,
                            fps: s.ndi_fps,
                            keep_s: s.rec_keep_s.clamp(10, 600),
                            out_dir: if s.rec_dir.trim().is_empty() {
                                rec::default_out_dir()
                            } else {
                                s.rec_dir.trim().into()
                            },
                        })
                    }
                    None => {
                        rec_err = Some(
                            "ffmpeg not found — install it (winget install ffmpeg / brew install ffmpeg) or pick ffmpeg in Stream → Recording".into(),
                        );
                        None
                    }
                }
            } else {
                None
            };
            if recorder.as_ref().map(|r| &r.conf) != conf.as_ref() {
                if let (Some(old), true) = (recorder.as_ref(), set_on) {
                    old.set_stop(s.rec_layout);
                    set_on = false;
                }
                // Drop first: the old one clears the global taps on drop, which
                // must happen before the new one installs its own.
                drop(recorder.take());
                recorder = conf.map(rec::Recorder::start);
            }
            if let Some(rc) = &recorder {
                if want_set != set_on {
                    if want_set {
                        rc.set_start();
                    } else {
                        rc.set_stop(s.rec_layout);
                    }
                    set_on = want_set;
                }
                if shared.rec_clip.swap(false, Ordering::Relaxed) {
                    rc.save_clip(s.rec_keep_s, s.rec_layout);
                }
                if let Some((_, secs)) = drop_clip_at.filter(|&(t, _)| now >= t) {
                    drop_clip_at = None;
                    rc.save_clip(secs, s.rec_layout);
                    last_auto_clip = Some(now);
                    auto_clips += 1;
                }
            } else {
                shared.rec_clip.store(false, Ordering::Relaxed);
                drop_clip_at = None;
                if !want {
                    set_on = false;
                }
            }
        }
        r.set_output((s.ndi_enabled || s.spout_enabled || recorder.is_some()).then(|| output::Conf {
            name: s.ndi_name.clone(),
            ndi: s.ndi_enabled,
            spout: s.spout_enabled,
            transparent: s.out_transparent,
            record: recorder.is_some(),
            height: s.ndi_height,
            fps: s.ndi_fps,
        }));

        // --- Timeline: loader results, transport state, cue dispatch -------
        // Lock order note: the panel draws under the settings lock and takes
        // the timeline lock inside it, so here the timeline lock must never
        // be held while locking settings — collect, drop, then act.
        while let Ok((path, res)) = song_rx.try_recv() {
            pending_decodes = pending_decodes.saturating_sub(1);
            match res {
                Ok(s2) => {
                    let sg = Arc::new(s2);
                    songs.insert(path.clone(), sg.clone());
                    println!(
                        "Song: {} — {:.1} BPM, {:.1}s, first beat {:.2}s",
                        sg.name, sg.bpm, sg.duration, sg.first_beat
                    );
                    let mut tl = lock(&shared.timeline);
                    tl.busy = pending_decodes > 0;
                    if pending_adds.iter().any(|p| p == &path) {
                        // A bare audio file — append it as a clip region.
                        pending_adds.retain(|p| p != &path);
                        let doc = tl.doc.get_or_insert_with(timeline::Timeline::default);
                        doc.add_song(&sg);
                        tl.dirty = true;
                        tl.message = format!(
                            "{} — {:.0} BPM, {:.0} bars",
                            sg.name,
                            sg.bpm,
                            sg.bpm * sg.duration / 240.0
                        );
                    } else if let Some(doc) = tl.doc.as_mut() {
                        // Fill placeholder clips loaded from a .json — keep
                        // the saved offset, take the fresh analysis.
                        let mut filled = false;
                        for c in doc
                            .clips
                            .iter_mut()
                            .filter(|c| c.song == path && c.overview.is_empty())
                        {
                            *c = timeline::Clip::from_song(&sg, c.offset_s);
                            filled = true;
                        }
                        if filled {
                            tl.message = format!("decoded {}", sg.name);
                        }
                    }
                }
                Err(e) => {
                    pending_adds.retain(|p| p != &path);
                    let mut tl = lock(&shared.timeline);
                    tl.busy = pending_decodes > 0;
                    tl.message = format!("load failed: {e}");
                }
            }
        }

        // Track end → stop cleanly and hand audio back to the live input.
        if player.as_ref().is_some_and(|p| p.finished()) {
            {
                let mut tl = lock(&shared.timeline);
                tl.mode = PlayMode::Stopped;
                tl.pos_s = 0.0;
                tl.live_locked = false;
            }
            end_show(
                &mut player,
                &mut audio,
                &audio_cfg,
                &shared,
                &mut pre_show,
                &mut dir,
                &mut dancer,
                &mut text_slots,
                &mut blackout,
                started,
            );
            matcher.reset();
            was_locked = false;
            fired_past = f64::MIN;
        }

        // Pick a playhead: the player's clock while the file plays, else the
        // live-match estimate while follow-live is on. Fire the cues due.
        {
            let mut guard = lock(&shared.timeline);
            let tl = &mut *guard;
            let mut due: Vec<CueKind> = Vec::new();
            let mut new_fired = fired_past;
            // A fresh live-lock lands mid-song — apply the playhead state
            // (like a seek) so cues already behind it still took effect.
            let mut locked_state = None;
            let mut lost_lock = false;
            if let Some(doc) = tl.doc.as_ref() {
                // The playhead in global timeline seconds.
                let pos_t: Option<f64> = match tl.mode {
                    PlayMode::Playing => {
                        let t = player.as_ref().map(|p| p.position_s()).unwrap_or(0.0);
                        tl.pos_s = t;
                        Some(t)
                    }
                    PlayMode::Stopped if tl.autosync => {
                        // Follow-live: correlate the room's onset envelope
                        // against every clip's (2×/sec), coast between evals.
                        let live_pos = if matcher.due() {
                            let (env, fps) = {
                                let e = lock(&shared.env);
                                (e.env.iter().copied().collect::<Vec<f32>>(), e.fps as f64)
                            };
                            if fps > 0.0 {
                                matcher.update(&env, fps, doc)
                            } else {
                                matcher.coast()
                            }
                        } else {
                            matcher.coast()
                        };
                        tl.live_locked = matcher.locked;
                        tl.live_score = matcher.score;
                        // Losing the lock ends the borrowed state — a live
                        // lock still counts as "the show owns the rig".
                        lost_lock = was_locked && !matcher.locked;
                        if matcher.locked && !was_locked {
                            // Fresh lock: don't dump the backlog mid-bar —
                            // apply the implied state at the lock point.
                            if let Some(p) = live_pos {
                                new_fired = p;
                                locked_state = Some(doc.state_at(p));
                            }
                        }
                        was_locked = matcher.locked;
                        if let Some(p) = live_pos {
                            tl.pos_s = p;
                            Some(p)
                        } else {
                            None
                        }
                    }
                    // Follow-live switched off while locked: the arm above
                    // no longer runs, so hand the borrowed rig back here
                    // (else Manual mode, a blackout or a cue's palette stick).
                    PlayMode::Stopped if was_locked => {
                        lost_lock = true;
                        was_locked = false;
                        matcher.reset();
                        tl.live_locked = false;
                        None
                    }
                    _ => None,
                };
                if let Some(t) = pos_t {
                    // Big jumps (lock acquire, seek) skip rather than dump.
                    if new_fired == f64::MIN || (t - new_fired).abs() > 4.0 {
                        new_fired = t;
                    }
                    for k in doc.edges_between_s(new_fired, t) {
                        due.push(k);
                    }
                    new_fired = t;
                }
            }
            drop(guard);
            fired_past = new_fired;
            if let Some(st) = locked_state {
                apply_playhead(
                    &st,
                    &mut r,
                    &mut dir,
                    &mut dancer,
                    &mut text_slots,
                    (now - started).as_secs_f32(),
                    &mut blackout,
                    &shared,
                    &usable,
                    &mut pre_show,
                );
            }
            for kind in &due {
                fire_cue(
                    kind,
                    &mut r,
                    &mut dir,
                    &mut dancer,
                    &mut text_slots,
                    (now - started).as_secs_f32(),
                    &mut blackout,
                    &shared,
                    &usable,
                    false,
                );
            }
            if lost_lock {
                end_show(
                    &mut player,
                    &mut audio,
                    &audio_cfg,
                    &shared,
                    &mut pre_show,
                    &mut dir,
                    &mut dancer,
                    &mut text_slots,
                    &mut blackout,
                    started,
                );
            }
        }

        if now - last_reload_check > Duration::from_millis(500) {
            last_reload_check = now;
            r.reload_shaders(false);
        }

        let mut f = audio.features.lock().map(|f| f.clone()).unwrap_or_default();
        // Audio watchdog: a stream can die outright (`dead`, or the
        // analyser starving — phase_at goes stale when the zero-feed is
        // the only thing keeping it warm) or keep delivering silence from
        // a stranded loopback (Windows doesn't move a live capture when
        // the default device changes). Only cure is a rebuild, which also
        // picks up the current default device. Shows own their engine —
        // a paused timeline's silence is legit and must not rebuild.
        if player.is_none() {
            let stream_dead = audio.dead.load(Ordering::Relaxed);
            let ana_dead = f.phase_at.elapsed() > Duration::from_secs(3);
            if f.silent || stream_dead || ana_dead {
                let since = *silent_since.get_or_insert(now);
                // First dead signal retries at once; after an attempt,
                // wait out the backoff so a rebuild that keeps failing
                // (device gone for good) doesn't spin every frame.
                let due = match last_audio_attempt {
                    Some(t) => now - t >= audio_retry,
                    None => stream_dead || ana_dead || now - since >= audio_retry,
                };
                if due {
                    audio.dead.store(false, Ordering::Relaxed);
                    last_audio_attempt = Some(now);
                    match AudioEngine::start(
                        audio_cfg.0.as_deref(),
                        audio_cfg.1,
                        Some(shared.env.clone()),
                    ) {
                        Ok(eng) => {
                            println!("Audio: {} (rebuilt after stream loss)", eng.device_name);
                            audio = eng;
                        }
                        Err(e) => eprintln!("audio rebuild failed: {e:#}"),
                    }
                    audio_retry = (audio_retry * 4).min(Duration::from_secs(300));
                }
            } else {
                silent_since = None;
                last_audio_attempt = None;
                audio_retry = Duration::from_secs(15);
            }
        }
        // TRIPPIN_GROOVE_LOG=1: print the beats/breakdown detector once a
        // second (tuning aid for live audio).
        if groove_log && now - last_groove_log > Duration::from_secs(1) {
            last_groove_log = now;
            println!(
                "groove {:.2} calm {:.2} kick {:.2} bass {:.2} bpm {:.1}",
                f.groove, f.calm, f.kick, f.bass, f.bpm
            );
        }
        if !s.breakdown_mode {
            f.calm = 0.0;
        }
        // The Style pool: the calm pool while the breakdown lasts. Same 0.7/0.3
        // edges as the director's breakdown events, so when it flips the list
        // is rebuilt before dir.update: the entry cut must already see the calm
        // pool and the drop cut the main one (a one-frame-old list sent them
        // the wrong way round on the i9 test).
        let was_calm = style_calm;
        style_calm = s.style.is_some() && (if f.calm > 0.7 { true } else if f.calm < 0.3 { false } else { style_calm });
        if style_calm != was_calm {
            usable = director_scenes(&r, &s, &allowed, style_calm, &dir);
        }
        // Positive latency shows the beat earlier (compensating capture delay).
        let pos = f.beat_position(now) + s.latency_ms as f64 / 1000.0 * f.bpm as f64 / 60.0;
        let ev = dir.update(&f, pos, dt, &usable, &s);
        // A drop schedules a clip once its payoff has played. Not while a
        // timeline show plays (a paused or scrubbed song fakes drops), nor
        // too soon after the last one, nor past the per-run cap.
        if ev.drop
            && s.auto_clip
            && s.rec_buffer
            && player.is_none()
            && drop_clip_at.is_none()
            && auto_clips < s.auto_clip_max
            && last_auto_clip.map_or(true, |t| now.duration_since(t).as_secs() >= 45)
        {
            // The buffer only keeps `rec_keep_s`. Trimming the clip's start to fit it cut
            // the drop itself out (10 s kept, 8 + 12 asked: the clip was all aftermath), so
            // shrink both sides instead: at most half the buffer before the drop.
            let keep = s.rec_keep_s.max(10);
            let before = s.auto_clip_before_s.min(keep / 2);
            let after = s.auto_clip_after_s.min(keep - before);
            drop_clip_at = Some((now + Duration::from_secs(after as u64), (before + after).max(5)));
        }
        // GPU baseline: pin the scene under test against the auto-pilot's
        // own cuts. The measurement runs below, once the engine's frame
        // counter for this frame is fresh.
        if let Some(chk) = check.as_mut() {
            if chk.cur.is_none() {
                chk.advance();
            }
            if let Some(i) = chk.cur {
                if dir.scene != i {
                    dir.cut_to(i);
                }
            }
        }
        // A Style change swaps the auto-palette families (and restarts the mood
        // picker so the new family shows within a couple of beats).
        {
            let want = s.style.as_deref();
            if style_pals.as_ref().map(|(id, _)| id.as_str()) != want {
                style_pals = want
                    .and_then(|id| styles::catalog().theme(id))
                    .map(|t| (t.id.clone(), styles::mood_palettes(t)));
                auto_pal = palettes::Auto::default();
                if let Some(first) = style_pals.as_ref().and_then(|(_, p)| p[1].first().copied()) {
                    auto_pal.start_with(first);
                }
            }
        }
        // `palette = "auto"`: pick the gradient to match the music's mood.
        let pal = if s.palette == palettes::AUTO {
            auto_pal.pick_in(&f, pos, style_pals.as_ref().map(|(_, p)| p))
        } else {
            s.palette.as_str()
        };
        r.set_palette(pal);
        // Free cuts land on musical events — the queued pick should fit
        // the mood too: calm sections want calm scenes, hot ones want hot.
        repick_for_mood(&mut dir, &r, &usable, &f, &s);

        // Dancer follows the settings; auto-pilot changes it on cuts and phrases.
        dancer.enabled = s.dancer_enabled;
        if let Some(style) = s.dancer_style {
            dancer.style = style;
        }
        match s.canon {
            Tristate::On => dancer.canon = true,
            Tristate::Off => dancer.canon = false,
            Tristate::Auto => {}
        }
        for (slot, clip) in dancer.poll_loaded() {
            r.set_dancer_clip(slot, &clip);
        }
        dancer.bpm = f.bpm;
        // Breakdowns and drops re-pick the dancer too (graceful <-> driving).
        if (ev.cut || ev.phrase || ev.mode_change) && s.mode != Mode::Manual && dancer.enabled {
            let intensity = dir.intensity;
            dancer.on_cut(
                intensity,
                || dir.rand(),
                s.dancer_style,
                s.canon,
                &s.disabled_clips,
            );
        }
        let dancer_u = dancer.uniforms(
            pos,
            f.downbeat,
            f.bpm,
            dir.intensity,
            dt,
            s.dancer_size,
            s.dancer_trails,
            &s.disabled_clips,
            s.mode != Mode::Manual,
        );
        if ev.cut && s.fx_auto {
            let seed = dir.rand();
            fx_current = Fx::random(seed, fx_current);
        }

        // Tempo changes ease in; position only ever moves forward smoothly.
        flow_bpm += (f.bpm - flow_bpm) * (dt * 1.5).min(1.0);
        let speed_target = 1.0 - 0.45 * f.calm;
        flow_speed += (speed_target - flow_speed) * (dt * 0.8).min(1.0);
        flow = (flow + dt as f64 * flow_bpm as f64 / 60.0 * flow_speed as f64) % 4096.0;
        // Energy clocks: whole mix, bass, mid, high (mid-high folds into
        // high). Rate in beats/s = tempo x (0.3 + 2.4 x level^1.6): about
        // 0.4x in a breakdown, ~1.1x on a drop (measured on real tracks).
        let whole = (f.lvl4[0] * 0.45 + f.lvl4[1] * 0.3 + f.lvl4[2] * 0.15 + f.lvl4[3] * 0.1).min(1.0);
        let src = [whole, f.lvl4[0], f.lvl4[1], f.lvl4[2].max(f.lvl4[3])];
        let k = (dt / 0.35).min(1.0);
        for i in 0..4 {
            let target = if f.silent { 0.0 } else { src[i] };
            clock_lvl[i] += (target - clock_lvl[i]) * k;
            let rate = 0.3 + 2.4 * clock_lvl[i].powf(1.6);
            clock4[i] = (clock4[i] + dt as f64 * flow_bpm as f64 / 60.0 * rate as f64) % 4096.0;
        }
        let target = if blackout { 0.0 } else { 1.0 };
        master += (target - master) * (dt * 3.0).min(1.0);
        // Strobe: black, with a hard cut to the picture on each drum hit the
        // analyser hears (`onset` jumps on a hit and decays over ~0.12 s, so
        // each flash lasts ~70 ms). It follows the actual fill — rolls that
        // start mid-bar or cross the bar line — not a grid, and no easing
        // (blackout's fade is far too slow for this). `dir.fill_strobe` is
        // the auto-pilot strobing a detected fill — suppressed on
        // tunnel/flight scenes, where it reads as jerky (AGENTS.md).
        let fill_strobe = dir.fill_strobe
            && !crate::ai::scene_meta(r.scene_name(dir.scene)).flight;
        let strobe_live = STROBE.load(Ordering::Relaxed) || fill_strobe;
        let strobe_gate = if strobe_live && f.onset < 0.45 {
            0.0
        } else {
            1.0
        };

        let (w, h) = r.size();
        let mut spectrum = [0.0; audio::SPECTRUM_BINS];
        spectrum.copy_from_slice(&f.spectrum);
        let mut waveform = [[0.0f32; 4]; 16];
        for (i, &v) in f.waveform.iter().enumerate() {
            waveform[i / 4][i % 4] = v;
        }
        let beat_in_bar = f.beat_in_bar(pos);
        let u = Uniforms {
            time: (now - started).as_secs_f32(),
            dt,
            res_x: w as f32,
            res_y: h as f32,
            bass: f.bass,
            mid: f.mid,
            high: f.high,
            energy: f.energy,
            // Melodic onsets in a breakdown (piano, plucks) shouldn't fire the
            // drum-style flashes scenes hang off `onset`.
            onset: f.onset * (1.0 - 0.6 * f.calm),
            kick: f.kick,
            beat: (pos % 4096.0) as f32,
            beat_phase: pos.fract() as f32,
            bar_phase: (beat_in_bar as f32 + pos.fract() as f32) / 4.0,
            bpm: f.bpm,
            build: f.build,
            scene_time: (now - dir.scene_started).as_secs_f32(),
            intensity: dir.intensity,
            hue: dir.hue,
            seed: dir.seed,
            flash: dir.flash,
            flow: flow as f32,
            master: master * strobe_gate,
            fx: if s.fx_auto { fx_current } else { s.fx }.index(),
            fx_amt: s.fx_amt,
            spectrum,
            waveform,
            // The renderer fills these from the scene header.
            bloom: 0.0,
            tonemap: 0.0,
            frame: 0.0,
            calm: f.calm,
            lvl4: f.lvl4,
            hits4: f.hits4,
            pres4: f.pres4,
            clock4: clock4.map(|c| c as f32),
            misc4: [0.0; 4],
        };
        // External engine link: Trippin runs the Unity engine itself (see
        // engine.rs) — show state out over UDP, frames back through shared
        // memory. Cross-platform; nothing for the user to start.
        if s.unity_link {
            if link_out.is_none() {
                link_out = link::Link::new(s.link_port).ok();
            }
            if let Some(l) = link_out.as_mut() {
                l.send(&u, r.scene_name(dir.scene), pal, f.calm < 0.5);
            }
            if engine.is_none() {
                match engine::Engine::new(s.link_port) {
                    Ok(e) => engine = Some(e),
                    Err(e) => *lock(&ENGINE_STATUS) = format!("{e:#}"),
                }
            }
            if let Some(e) = engine.as_mut() {
                e.tick();
                let latest = e.latest.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(fr) = latest.as_ref() {
                    if fr.seq != ext_seq {
                        ext_seq = fr.seq;
                        r.upload_external(&fr.rgba);
                    }
                }
                drop(latest);
                let live = e.live();
                EXT_LIVE.store(live, Ordering::Relaxed);
                let st = if live { "running".to_string() } else { e.status.clone() };
                let mut g = lock(&ENGINE_STATUS);
                if *g != st {
                    *g = st;
                }
            }
        } else if link_out.is_some() || engine.is_some() {
            link_out = None;
            engine = None; // Drop kills the player
            EXT_LIVE.store(false, Ordering::Relaxed);
            lock(&ENGINE_STATUS).clear();
        }
        // GPU baseline: sample the pinned scene (wgpu = per-frame ms,
        // unity_* = engine frames/sec), then step to the next or finish.
        if let Some(mut chk) = check.take() {
            if let Some(i) = chk.cur {
                let done = if chk.is_unity(i) {
                    chk.step_unity(ext_seq, EXT_LIVE.load(Ordering::Relaxed))
                        .map(|fps| (true, fps))
                } else {
                    chk.step_wgpu(dt * 1000.0).map(|ms| (false, ms))
                };
                if let Some((unity, v)) = done {
                    let name = chk.names[i].clone();
                    if unity {
                        chk.rep.fps.insert(name, v);
                    } else {
                        chk.rep.ms.insert(name, v);
                    }
                    chk.done += 1;
                    chk.cur = None;
                }
            }
            if chk.cur.is_none() && chk.queue.is_empty() {
                dir.cut_to(chk.home);
                finish_check(chk, r.size(), &shared);
            } else {
                check = Some(chk);
            }
        }
        // Text overlays: fade in over 0.35 s, out over 0.5 s; a faded-out
        // slot drops off (its texture stays bound but the shader skips it).
        let now_t = u.time;
        let mut text_u = text::TextUniforms::default();
        let mut any_text = false;
        for (slot, st) in text_slots.iter_mut().enumerate() {
            let Some(ts) = st else { continue };
            let mut opacity = ((now_t - ts.born) / 0.35).min(1.0);
            if let Some(o) = ts.out_at {
                opacity *= (1.0 - (now_t - o) / 0.5).max(0.0);
                if opacity <= 0.0 {
                    *st = None;
                    continue;
                }
            }
            // 11% of screen height; squeeze wider text to fit the screen.
            let screen_asp = w as f32 / h.max(1) as f32;
            let mut half_h = 0.11f32 * ts.spec.size.unwrap_or(1.0).clamp(0.4, 2.5);
            let mut half_w = half_h * ts.aspect;
            if half_w > screen_asp * 0.92 {
                half_w = screen_asp * 0.92;
                half_h = half_w / ts.aspect;
            }
            text_u.slots[slot] = text::TextSlotU {
                quad: [0.0, ts.spec.pos.y(), half_w, half_h],
                aspect: ts.aspect,
                style: ts.spec.style.index(),
                opacity,
                born: ts.born,
                life: 0.0,
                hue: (slot as f32) * 0.37 + ts.spec.style.index() * 0.11,
                anim: ts.spec.anim.index(),
                fx: ts.spec.fx.index(),
            };
            any_text = true;
        }
        let text_arg = any_text.then_some(&text_u);
        // Stream overlays: now-playing card, branding, ticker.
        if shared.np_replay.swap(false, Ordering::Relaxed) {
            overlays.replay_card(now_t);
        }
        let ov_u = {
            let np = lock(&shared.np);
            overlays.update(&s, &np, now_t, w as f32 / h.max(1) as f32, &mut |i, img| {
                r.set_overlay_image(i, img)
            })
        };
        if let Err(e) = r.render(dir.scene, &u, dancer_u.as_ref(), text_arg, ov_u.as_ref()) {
            eprintln!("render error: {e}");
        }

        if now - last_status > Duration::from_millis(50) {
            last_status = now;
            let (np_track, np_status) = {
                let np = lock(&shared.np);
                (np.track.as_ref().map(|t| t.line()), np.status.clone())
            };
            let perf_slow = lock(&shared.perf).slow();
            let clip_now = dancer.loaded_name();
            let clip_title = clip_now
                .as_ref()
                .and_then(|n| shared.clip_names.iter().position(|c| c == n))
                .and_then(|i| shared.clip_titles.get(i).cloned());
            *lock(&shared.status) = Status {
                looks: Vec::new(), // filled by the event thread when the panel draws
                look_notice: None,
                bpm: f.bpm,
                confidence: f.tempo_confidence,
                beat_in_bar,
                silent: f.silent,
                fps,
                device: audio.device_name.clone(),
                scene: dir.scene,
                // The director picks the next scene when the current one
                // starts — the pick is known in every mode that can cut.
                next_scene: dir.next,
                bar_in_scene: if s.mode == Mode::Auto {
                    dir.bars_progress(s.phrase_bars).0
                } else {
                    0
                },
                bars_total: if s.mode == Mode::Auto {
                    dir.bars_progress(s.phrase_bars).1
                } else {
                    0
                },
                clip: clip_now,
                clip_title,
                blackout,
                strobe: strobe_live,
                palette_now: pal.to_string(),
                // Published by the event thread — it owns the window state.
                fullscreen: FULLSCREEN.load(Ordering::Relaxed),
                fx: if s.fx_auto { fx_current } else { s.fx },
                output: r.output_status(),
                groove: f.groove,
                calm: f.calm,
                np_track,
                np_status,
                rec: recorder.as_ref().map(|r| lock(&r.status).clone()),
                rec_err: rec_err.clone(),
                scene_titles: shared.scene_titles.clone(),
                clip_titles: shared.clip_titles.clone(),
                perf_prog: check.as_ref().map(|c| c.prog()),
                perf_slow,
            };
        }

        if let Some(interval) = frame_interval {
            next_frame += interval;
            if next_frame < now {
                // Long stall (heavy scene, window drag): don't play catch-up.
                next_frame = now;
            }
            std::thread::sleep(next_frame - now);
            // The window may have moved to a display with another refresh rate.
            if now - pace_recheck > Duration::from_secs(2) {
                pace_recheck = now;
                frame_interval = r.frame_interval();
            }
        }
    }
}

/// Fold a finished GPU baseline into the rig: persist `perf.json`, publish
/// it for the panel's "slow" flags, and deselect the sub-30 fps stragglers.
/// Deselection lands in `disabled_scenes`, so anything the user wants back
/// is one toggle away in the picker.
fn finish_check(chk: perf::Check, size: (u32, u32), shared: &Shared) {
    let mut rep = chk.rep;
    rep.checked = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    rep.size = size;
    if rep.drop_dead_engine() {
        println!("perf check: the Unity engine sent no frames; Unity shows not measured");
    }
    rep.save();
    let slow = rep.slow();
    *lock(&shared.perf) = rep;
    if !slow.is_empty() {
        {
            let mut s = lock(&shared.settings);
            for n in &slow {
                if !s.disabled_scenes.contains(n) {
                    s.disabled_scenes.push(n.clone());
                }
            }
        }
        shared.dirty.store(true, Ordering::Relaxed);
    }
    println!(
        "perf check: {} scenes measured, {} below 30 fps: {}",
        chk.total,
        slow.len(),
        slow.join(", ")
    );
}

/// An action that touches render-side state (scene, dancer, fx, sync).
/// Window/panel actions are handled on the event thread instead.
fn apply_render(
    r: &mut Renderer,
    dir: &mut Director,
    dancer: &mut DancerLayer,
    blackout: &mut bool,
    audio: &AudioEngine,
    shared: &Shared,
    action: Action,
) {
    let mut s = lock(&shared.settings);
    let usable = usable_scenes(r, &s);
    match action {
        Action::NextScene => dir.next_scene(&usable, s.random_order),
        Action::PrevScene => dir.prev_scene(&usable),
        Action::ModeAuto => s.mode = Mode::Auto,
        Action::ModeStatic => s.mode = Mode::Static,
        Action::ModeManual => s.mode = Mode::Manual,
        Action::ToggleRandom => s.random_order = !s.random_order,
        Action::ToggleDancer => {
            s.dancer_enabled = !s.dancer_enabled;
            dancer.showing = true;
        }
        Action::NextClip => {
            dancer.next_clip();
            dancer.showing = true;
        }
        Action::NextStyle => s.dancer_style = Some((dancer.style + 1) % dancer::STYLES.len()),
        Action::CycleCanon => {
            s.canon = match s.canon {
                Tristate::Auto => Tristate::On,
                Tristate::On => Tristate::Off,
                Tristate::Off => Tristate::Auto,
            }
        }
        Action::Blackout => *blackout = !*blackout,
        // The same gate as the timeline's strobe cue: flashes ride the
        // live drum onsets while it's on.
        Action::Strobe => {
            let on = !STROBE.load(Ordering::Relaxed);
            STROBE.store(on, Ordering::Relaxed);
        }
        Action::MarkDownbeat => {
            let _ = audio.commands.send(Command::MarkDownbeat);
        }
        // A segment starts here: re-anchor the bar grid AND restart the
        // phrase clock so the next auto cut lands `phrase_bars` from now.
        Action::MarkPhrase => {
            let _ = audio.commands.send(Command::MarkDownbeat);
            dir.mark_phrase();
        }
        Action::ToggleLogo => {
            s.brand_logo_on = !s.brand_logo_on;
            // Showing a piece also enables the block, so the pad always has
            // a visible effect; hiding leaves the master as it was. With no
            // logo set there's nothing to show, so the block stays as it is.
            s.brand_on |= s.brand_logo_on && !s.brand_logo.trim().is_empty();
        }
        Action::ToggleName => {
            s.brand_name_on = !s.brand_name_on;
            s.brand_on |= s.brand_name_on && !s.brand_name.trim().is_empty();
        }
        Action::ToggleTicker => s.ticker_on = !s.ticker_on,
        Action::LatencyDown => s.latency_ms -= 5.0,
        Action::LatencyUp => s.latency_ms += 5.0,
        // Cycling the effect by hand turns auto off: the key always shows what it does.
        Action::CycleFx => {
            s.fx_auto = false;
            s.fx = s.fx.next();
        }
        Action::ReloadShaders => r.reload_shaders(true),
        Action::ShowNowPlaying => shared.np_replay.store(true, Ordering::Relaxed),
        // Looks are recalled on the event thread (App::apply); never sent here.
        Action::Look1
        | Action::Look2
        | Action::Look3
        | Action::Look4
        | Action::Look5
        | Action::Look6
        | Action::Look7
        | Action::Look8
        | Action::NextTheme => {}
        Action::SaveClip => shared.rec_clip.store(true, Ordering::Relaxed),
        Action::RecordSet => {
            shared.rec_set.fetch_xor(true, Ordering::Relaxed);
        }
        // Handled on the event thread (windows / timeline transport).
        Action::Fullscreen
        | Action::LeaveFullscreen
        | Action::TogglePanel
        | Action::ToggleEditor
        | Action::TimelinePlay
        | Action::TimelineRecord => {}
    }
    shared.dirty.store(true, Ordering::Relaxed);
}

struct App {
    /// Moved into the render thread once the window exists.
    audio: Option<AudioEngine>,
    window: Option<Arc<Window>>,
    /// GPU handles for the panel, cloned out of the renderer before it moves.
    gpu: Option<Gpu>,
    panel: Option<Panel>,
    /// Dedicated timeline-editor window (separate from the small panel).
    editor: Option<editor::Editor>,
    /// Moved into the render thread once the window exists.
    director: Option<Director>,
    dancer: Option<DancerLayer>,
    /// The authoritative settings once `shared` exists; before that this copy
    /// holds the CLI-adjusted values the render thread starts from.
    settings: Settings,
    shared: Option<Arc<Shared>>,
    render_tx: Option<mpsc::Sender<Msg>>,
    /// Live-input args — kept so stopping song playback restores the engine.
    audio_cfg: (Option<String>, bool),
    /// `--song <path>` — a timeline for this file is opened on startup.
    start_song: Option<std::path::PathBuf>,
    /// Shared onset-envelope log — the same one the live audio engine taps.
    env: audio::SharedEnv,
    /// When settings last changed (saved a moment later).
    dirty_since: Option<Instant>,
    /// Command-line overrides are for this run only: don't write them to disk.
    no_save: bool,
    start_fullscreen: bool,
    start_scene: Option<String>,
    no_panel: bool,
    low_power: bool,
    render_scale: Option<f32>,
    vsync: bool,
    last_panel_draw: Instant,
    last_editor_draw: Instant,
    last_title: Instant,
    /// Debounce for the panel toggle — autorepeat and focus churn can both
    /// re-fire it within the same press.
    last_panel_toggle: Instant,
    /// Title-bar/taskbar icon, decoded once from the bundled PNG.
    icon: Option<Icon>,
    /// Posts MIDI notes from midir's callback thread into `user_event`.
    midi_proxy: EventLoopProxy<AppEvent>,
    /// Pointer over the visuals is hidden while fullscreen — synced to the
    /// window's real fullscreen state in `about_to_wait`.
    cursor_hidden: bool,
    /// The open MIDI input, if any — its port name tells when `midi_in`
    /// points somewhere new.
    midi: Option<midi::Midi>,
    /// Throttle for a failing/absent device retry, and for the port-list
    /// scan that notices an unplugged device (WinMM has no disconnect event).
    midi_retry: Instant,
    midi_scan: Instant,
    /// The LAN remote (WebSocket server for the iOS app) and the OSC
    /// listener — respawned when their settings change, like `midi`.
    remote: Option<remote::Server>,
    osc: Option<osc::Osc>,
    /// Throttle for bind-failure retries (port already in use).
    remote_retry: Instant,
    /// Last client count shown in the remote status line.
    remote_clients: usize,
}

impl App {
    fn mark_dirty(&mut self) {
        if let Some(sh) = &self.shared {
            sh.dirty.store(true, Ordering::Relaxed);
        }
        self.dirty_since = Some(Instant::now());
    }

    fn send(&self, msg: Msg) {
        if let Some(tx) = &self.render_tx {
            let _ = tx.send(msg);
        }
    }

    /// Write an action into the timeline if recording is armed — the cue
    /// lands on the (snapped) beat at the current playhead.
    fn record_cue(&mut self, kind: CueKind) {
        let Some(sh) = &self.shared else { return };
        let mut guard = lock(&sh.timeline);
        let tl = &mut *guard;
        if !tl.recording || !(tl.mode == PlayMode::Playing || tl.live_locked) {
            return;
        }
        let Some(doc) = tl.doc.as_mut() else { return };
        // The cue pins to whichever clip the playhead is inside, on that
        // clip's beat grid — actions in a gap between songs aren't recorded.
        let pos = tl.pos_s;
        let Some((ci, clip)) = doc.clip_at(pos) else {
            return;
        };
        let mut beat = clip.beat_at(pos - clip.offset_s).max(0.0);
        if tl.snap {
            beat = (beat * 4.0).round() / 4.0;
        }
        doc.cues.push(Cue {
            clip: ci,
            beat,
            beats: 4.0,
            kind,
        });
        doc.sort_cues();
        tl.dirty = true;
    }

    fn record_action(&mut self, action: Action) {
        let Some(sh) = &self.shared else { return };
        // Order: settings/status cloned first, timeline lock after — same
        // order as the panel, never nested.
        let s = lock(&sh.settings).clone();
        let st = lock(&sh.status).clone();
        if let Some(kind) = timeline::cue_for_action(action, &s, &st) {
            self.record_cue(kind);
        }
    }

    fn apply(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        match action {
            Action::Fullscreen => {
                if let Some(w) = &self.window {
                    toggle_fullscreen(w);
                }
            }
            Action::LeaveFullscreen => {
                if let Some(w) = &self.window {
                    w.set_fullscreen(None);
                }
            }
            Action::TogglePanel => {
                // Debounce: whatever re-fires the key (autorepeat slips, a
                // focus bounce) can't toggle more than once per quarter
                // second — that's faster than any intentional double-tap.
                if self.last_panel_toggle.elapsed() < Duration::from_millis(250) {
                    return;
                }
                self.last_panel_toggle = Instant::now();
                match &mut self.panel {
                    // The panel is created once and hidden/shown — no window or
                    // surface churn per press, nothing can half-open.
                    Some(p) => {
                        let show = p.window.is_visible() == Some(false);
                        p.window.set_visible(show);
                        if !show {
                            // A pending learn behind a hidden window would
                            // eat the next pad press.
                            p.midi_learn = None;
                        }
                        if show {
                            p.window.request_redraw();
                        } else if let Some(w) = &self.window {
                            // Hand focus back to the visuals so hotkeys keep
                            // working after the panel disappears.
                            w.focus_window();
                        }
                        self.settings_mut().show_panel = show;
                    }
                    None => {
                        self.open_panel(event_loop);
                        self.settings_mut().show_panel = true;
                    }
                }
            }
            Action::ToggleEditor => self.open_editor(event_loop),
            Action::TimelinePlay => self.send(Msg::Transport(SongCtl::Toggle)),
            Action::TimelineRecord => {
                if let Some(sh) = &self.shared {
                    let mut tl = lock(&sh.timeline);
                    tl.recording = !tl.recording;
                }
            }
            Action::NextTheme => {
                let cat = styles::catalog();
                let next = {
                    let s = self.settings_mut();
                    styles::next_id(s.style.as_deref(), cat)
                };
                styles::select(&mut self.settings_mut(), next);
                self.mark_dirty();
            }
            a if a.look_slot().is_some() => {
                let slot = a.look_slot().unwrap_or(0);
                let look = self
                    .shared
                    .as_ref()
                    .and_then(|sh| looks::by_slot(&lock(&sh.looks), slot).cloned());
                if let Some(l) = look {
                    self.apply_look(&l);
                }
            }
            _ => self.send(Msg::Act(action)),
        }
        self.record_action(action);
        self.mark_dirty();
    }

    /// The live settings: the shared copy once rendering is running.
    /// Recall a Look: the settings half is written here; the scene and routine
    /// go through the render thread like any other pick, and are recorded as
    /// cues when a timeline is armed. A scene this machine lacks is skipped
    /// with a log line and the rest of the Look still applies.
    fn apply_look(&mut self, look: &looks::SavedLook) {
        let Some(sh) = self.shared.clone() else { return };
        let applied = {
            let mut s = self.settings_mut();
            looks::apply(look, &mut s, &sh.scene_names, &sh.clip_names)
        };
        // The settings guard is gone before record_cue takes the timeline lock.
        if applied.changed {
            self.mark_dirty();
        }
        // Colours and effect record too, so a recorded Look replays whole.
        if let Some(p) = applied.palette {
            self.record_cue(CueKind::Palette(p));
        }
        if let Some((mode, auto)) = applied.fx {
            self.record_cue(CueKind::Fx(mode));
            self.record_cue(CueKind::FxAuto(auto));
        }
        if let Some(name) = applied.scene {
            if let Some(i) = sh.scene_names.iter().position(|n| *n == name) {
                self.record_cue(CueKind::Scene(name));
                self.send(Msg::GoToScene(i));
            }
        }
        if let Some(name) = applied.clip {
            if let Some(i) = sh.clip_names.iter().position(|c| *c == name) {
                self.record_cue(CueKind::Clip(name));
                self.send(Msg::ShowClip(i));
            }
        }
        if let Some(missing) = applied.skipped_scene {
            let need = if look.requires.is_empty() {
                String::new()
            } else {
                format!(" (needs {})", look.requires.join(", "))
            };
            let msg = format!("Look \"{}\": scene {missing} isn't available here{need}; applied the rest", look.name);
            eprintln!("{msg}");
            *lock(&sh.look_notice) = Some((msg, Instant::now()));
        }
    }

    /// Save the live state as a new Look called `name` (never overwrites: a
    /// taken name gets `-2`, `-3`...).
    fn save_look(&mut self, name: &str, shared: &Shared) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let (scene, clip, palette_now) = {
            let st = lock(&shared.status);
            (shared.scene_names.get(st.scene).cloned(), st.clip.clone(), st.palette_now.clone())
        };
        let mut look = {
            let s = lock(&shared.settings);
            looks::capture(name, &s, scene.as_deref(), clip.as_deref(), Some(palette_now.as_str()))
        };
        let mut all = lock(&shared.looks);
        look.id = looks::unique_id(&all, name);
        look.name = looks::unique_name(name, &look.id);
        if looks::save(&looks::looks_dir(), &look).is_ok() {
            all.push(look);
            all.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
        }
    }

    fn settings_mut(&mut self) -> std::sync::MutexGuard<'_, Settings> {
        lock(&self.shared.as_ref().unwrap().settings)
    }

    fn open_panel(&mut self, event_loop: &ActiveEventLoop) {
        let Some(gpu) = self.gpu.clone() else { return };
        match Panel::new(event_loop, gpu, self.icon.clone(), self.window.as_deref()) {
            Ok(p) => self.panel = Some(p),
            Err(e) => eprintln!("control panel failed to open: {e:#}"),
        }
    }

    /// Show the timeline editor, creating it on first use.
    fn open_editor(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(e) = &self.editor {
            e.window.set_visible(true);
            e.window.request_redraw();
            e.window.focus_window();
            return;
        }
        let Some(gpu) = self.gpu.clone() else { return };
        match editor::Editor::new(event_loop, gpu, self.icon.clone(), self.window.as_deref()) {
            Ok(e) => self.editor = Some(e),
            Err(e) => eprintln!("timeline editor failed to open: {e:#}"),
        }
    }

    /// A MIDI note-on from `midi_proxy`. While the Keys page is in learn mode
    /// the note becomes the binding instead of firing.
    fn midi_note(&mut self, event_loop: &ActiveEventLoop, note: u8) {
        if self.shared.is_none() {
            return;
        }
        if let Some(a) = self.panel.as_mut().and_then(|p| p.midi_learn.take()) {
            {
                let mut s = self.settings_mut();
                // One note, one action — steal it from whatever had it.
                s.midi_notes.retain(|_, n| *n != note);
                s.midi_notes.insert(a, note);
            }
            self.mark_dirty();
            if let Some(p) = &self.panel {
                p.window.request_redraw();
            }
            return;
        }
        let action = self
            .shared
            .as_ref()
            .and_then(|sh| lock(&sh.settings).midi_action_for(note));
        if let Some(a) = action {
            self.apply(a, event_loop);
        }
    }

    /// A command from the LAN remote (iOS app) or an OSC controller — same
    /// dispatch as a hotkey, so presses record into an armed timeline too.
    fn remote_cmd(&mut self, event_loop: &ActiveEventLoop, cmd: remote::RemoteCmd) {
        use remote::{RemoteCmd as R, SetKey};
        let Some(sh) = self.shared.clone() else { return };
        match cmd {
            R::Act(a) => self.apply(a, event_loop),
            R::GoToScene(sel) => {
                if let Some(i) = sel.resolve(&sh.scene_names) {
                    self.record_cue(CueKind::Scene(sh.scene_names[i].clone()));
                    self.send(Msg::GoToScene(i));
                }
            }
            R::QueueNext(sel) => {
                if let Some(i) = sel.resolve(&sh.scene_names) {
                    self.send(Msg::QueueNext(i));
                }
            }
            R::ShowClip(name) => {
                let i = sh
                    .clip_names
                    .iter()
                    .position(|c| *c == name)
                    .or_else(|| {
                        sh.clip_names
                            .iter()
                            .position(|c| c.eq_ignore_ascii_case(&name))
                    });
                if let Some(i) = i {
                    self.record_cue(CueKind::Clip(sh.clip_names[i].clone()));
                    self.send(Msg::ShowClip(i));
                }
            }
            R::Look(sel) => {
                // An id wins; else digits are a slot (ids like "2" stay reachable).
                let look = {
                    let all = lock(&sh.looks);
                    all.iter()
                        .find(|l| l.id == sel)
                        .or_else(|| sel.parse::<u8>().ok().and_then(|n| looks::by_slot(&all, n)))
                        .cloned()
                };
                if let Some(l) = look {
                    self.apply_look(&l);
                }
            }
            R::Style(sel) => {
                // "", "off" or "none" clears; an unknown id is ignored (a typo must
                // not silently turn the current Style off).
                let cat = styles::catalog();
                let next = match sel.trim().to_lowercase().as_str() {
                    "" | "off" | "none" => Some(None),
                    id => cat.theme(id).map(|t| Some(t.id.clone())),
                };
                if let Some(n) = next {
                    styles::select(&mut self.settings_mut(), n);
                    self.mark_dirty();
                }
            }
            R::Transport(ctl) => self.send(Msg::Transport(ctl)),
            // A LAN agent's now-playing push; `None` retracts, but only
            // the pushing client's own track.
            R::NowPlaying { client, track } => {
                let mut inbox = lock(&sh.np_remote);
                match track {
                    Some(t) => *inbox = Some((client, t)),
                    None if inbox.as_ref().is_some_and(|(id, _)| *id == client) => {
                        *inbox = None;
                    }
                    None => {}
                }
            }
            // The remote's pusher thread ships the PNG once the render
            // thread produces it ("scene:<name>" in shared.thumbs).
            R::Thumb(name) => self.send(Msg::Thumb(format!("scene:{name}"))),
            R::Set(k) => {
                {
                    let mut s = self.settings_mut();
                    match k {
                        // Unknown palettes fall back inside palettes::lut;
                        // validate anyway so a typo can't save a dead name.
                        SetKey::Palette(p) if palettes::is_valid(&p) => {
                            s.palette = p;
                        }
                        SetKey::Palette(_) => return,
                        SetKey::Fx(fx) => {
                            s.fx_auto = false;
                            s.fx = fx;
                        }
                        SetKey::FxAmt(v) => s.fx_amt = v,
                        SetKey::FxAuto(b) => s.fx_auto = b,
                        SetKey::DancerSize(v) => s.dancer_size = v,
                        SetKey::DancerTrails(b) => s.dancer_trails = b,
                        SetKey::PhraseBars(n) => s.phrase_bars = n,
                        SetKey::CutOnDrops(b) => s.cut_on_drops = b,
                        SetKey::LatencyMs(v) => s.latency_ms = v,
                        SetKey::NpSize(v) => s.np_size = v,
                        SetKey::BrandOpacity(v) => s.brand_opacity = v,
                        SetKey::TickerSpeed(v) => s.ticker_speed = v,
                        SetKey::TickerText(t) => s.ticker_text = t,
                    }
                }
                self.mark_dirty();
            }
        }
    }

    /// The closures the remote server drives: commands go to the event
    /// loop via the proxy, state/meta/thumbs read the shared state.
    fn remote_hooks(&self) -> Option<remote::Hooks> {
        let sh = self.shared.clone()?;
        let proxy = self.midi_proxy.clone();
        let (meta_sh, state_sh, thumb_sh) = (sh.clone(), sh.clone(), sh);
        Some(remote::Hooks {
            cmd: Box::new(move |c| {
                let _ = proxy.send_event(AppEvent::Remote(c));
            }),
            meta: Box::new(move || {
                serde_json::json!({
                    "type": "hello",
                    "ok": true,
                    "version": env!("CARGO_PKG_VERSION"),
                    "scenes": meta_sh.scene_names,
                    "scene_titles": meta_sh.scene_titles.as_slice(),
                    "heavy": meta_sh.scene_heavy,
                    "clips": meta_sh.clip_names,
                    "styles": styles::catalog()
                        .themes
                        .iter()
                        .map(|t| serde_json::json!({ "id": t.id, "name": t.name, "about": t.about }))
                        .collect::<Vec<_>>(),
                    "looks": lock(&meta_sh.looks)
                        .iter()
                        .map(|l| serde_json::json!({ "id": l.id, "name": l.name, "slot": l.slot }))
                        .collect::<Vec<_>>(),
                    "clip_titles": meta_sh.clip_titles.as_slice(),
                    "palettes": palettes::all_names().collect::<Vec<_>>(),
                    "actions": Action::ALL
                        .iter()
                        .map(|a| serde_json::json!({
                            "id": serde_json::to_value(a).unwrap_or_default(),
                            "label": a.label(),
                        }))
                        .collect::<Vec<_>>(),
                    "addr": remote::local_ip(),
                })
            }),
            state: Box::new(move || {
                // Timeline first and released before settings is taken (the
                // lock-order rule), then status, then settings.
                let (song, tl_recording) = {
                    let tl = lock(&state_sh.timeline);
                    let song = tl.doc.as_ref().map(|d| {
                        serde_json::json!({
                            "playing": tl.mode == PlayMode::Playing,
                            "pos": tl.pos_s,
                            "len": d.end_s(),
                            "name": d.name,
                        })
                    });
                    (song, tl.recording)
                };
                // Status copy first, settings second — never nested, same
                // order the panel uses.
                let st = lock(&state_sh.status).clone();
                let s = lock(&state_sh.settings);
                serde_json::json!({
                    "type": "state",
                    "bpm": st.bpm,
                    "conf": st.confidence,
                    "beat_in_bar": st.beat_in_bar,
                    "fps": st.fps,
                    "silent": st.silent,
                    "device": st.device,
                    "scene": st.scene,
                    "scene_name": state_sh.scene_names.get(st.scene),
                    "scene_title": st.scene_titles.get(st.scene),
                    "next_scene": st.next_scene,
                    "next_scene_name": st.next_scene.and_then(|i| state_sh.scene_names.get(i)),
                    "next_scene_title": st.next_scene.and_then(|i| st.scene_titles.get(i)),
                    "bar_in_scene": st.bar_in_scene,
                    "song": song,
                    "timeline_recording": tl_recording,
                    "strobe": STROBE.load(Ordering::Relaxed),
                    "cut_on_drops": s.cut_on_drops,
                    "bars_total": st.bars_total,
                    "clip": st.clip,
                    "clip_title": st.clip_title,
                    "blackout": st.blackout,
                    "fullscreen": st.fullscreen,
                    "fx": st.fx,
                    "groove": st.groove,
                    "calm": st.calm,
                    "np": st.np_track,
                    "rec_on": st.rec.is_some(),
                    "mode": s.mode,
                    "dancer": s.dancer_enabled,
                    "dancer_style": s.dancer_style,
                    "palette": s.palette,
                    "style": s.style,
                    "palette_now": st.palette_now,
                    "random_order": s.random_order,
                    "phrase_bars": s.phrase_bars,
                    "fx_amt": s.fx_amt,
                    "fx_auto": s.fx_auto,
                    "dancer_size": s.dancer_size,
                    "dancer_trails": s.dancer_trails,
                    "latency_ms": s.latency_ms,
                    "np_size": s.np_size,
                    "canon": s.canon,
                    "brand_on": s.brand_on,
                    // The pieces and whether there's anything to show: the
                    // remote lit Logo from brand_on alone, which only ever
                    // goes on, so the pad stuck lit.
                    "brand_logo_on": s.brand_logo_on,
                    "brand_name_on": s.brand_name_on,
                    "has_logo": !s.brand_logo.trim().is_empty(),
                    "has_name": !s.brand_name.trim().is_empty(),
                    "brand_opacity": s.brand_opacity,
                    "ticker_on": s.ticker_on,
                    "ticker_speed": s.ticker_speed,
                    "ticker_text": s.ticker_text,
                })
            }),
            thumb: Box::new(move |name| {
                let got = lock(&thumb_sh.thumbs)
                    .get(&format!("scene:{name}"))
                    .cloned();
                let (w, h, px) = got?;
                use image::ImageEncoder;
                let mut out = Vec::new();
                image::codecs::png::PngEncoder::new(&mut out)
                    .write_image(&px, w, h, image::ExtendedColorType::Rgba8)
                    .ok()?;
                Some(out)
            }),
        })
    }

    /// Dispatch a key through the binding map. Returns false when no action
    /// is bound, letting callers offer the key a fallback meaning (the
    /// editor's unbound-Space transport toggle).
    fn key(&mut self, event_loop: &ActiveEventLoop, key: &Key) -> bool {
        let Some(name) = key_name(key) else { return false };
        // Esc also cancels a pending MIDI-learn — the cancel button isn't the
        // only way out.
        if name == "Escape" && self.panel.as_mut().is_some_and(|p| p.midi_learn.is_some()) {
            if let Some(p) = &mut self.panel {
                p.midi_learn = None;
                p.window.request_redraw();
            }
            return true;
        }
        // Rebinding: the next key press becomes the action's key (Esc cancels).
        if let Some(action) = self.panel.as_mut().and_then(|p| p.rebinding.take()) {
            if name != "Escape" {
                let mut s = self.settings_mut();
                for (_, k) in s.keys.iter_mut().filter(|(_, k)| **k == name) {
                    k.clear(); // one key, one action
                }
                s.keys.insert(action, name);
                drop(s);
                self.mark_dirty();
            }
            if let Some(p) = &self.panel {
                p.window.request_redraw();
            }
            return true;
        }
        let action = self
            .shared
            .as_ref()
            .and_then(|sh| lock(&sh.settings).action_for(&name));
        let Some(action) = action else { return false };
        self.apply(action, event_loop);
        true
    }

    fn draw_panel(&mut self, event_loop: &ActiveEventLoop) {
        let Some(shared) = self.shared.clone() else {
            return;
        };
        let mut status = lock(&shared.status).clone();
        if let Some(w) = &self.window {
            status.fullscreen = w.fullscreen().is_some();
        }
        status.look_notice = lock(&shared.look_notice)
            .as_ref()
            .filter(|(_, at)| at.elapsed() < Duration::from_secs(8))
            .map(|(m, _)| m.clone());
        status.looks = lock(&shared.looks)
            .iter()
            .map(|l| panel::LookRow { id: l.id.clone(), name: l.name.clone(), slot: l.slot })
            .collect();
        let midi_status = lock(&shared.midi_status).clone();
        let remote_status = lock(&shared.remote_status).clone();
        // The UI runs under the settings lock; the GPU acquire/present must
        // not — a blocked surface acquire would freeze the render thread
        // through the lock.
        let (commands, changed, frame) = {
            let mut s = lock(&shared.settings);
            let Some(p) = self.panel.as_mut() else { return };
            if p.window.is_visible() == Some(false) {
                return;
            }
            p.run_ui(
                &mut s,
                &status,
                &shared.scene_names,
                &shared.scene_heavy,
                shared.heavy_ok,
                &shared.clip_names,
                &shared.timeline,
                &shared.thumbs,
                &midi_status,
                &remote_status,
            )
        };
        if let Some(p) = self.panel.as_mut() {
            p.present(frame);
        }
        if changed {
            self.mark_dirty();
        }
        for c in commands {
            self.ui_command(c, &shared, event_loop);
        }
    }

    /// A command from the panel or the editor window.
    fn ui_command(&mut self, c: UiCommand, shared: &Shared, event_loop: &ActiveEventLoop) {
        match c {
            UiCommand::Do(a) => self.apply(a, event_loop),
            UiCommand::GoToScene(i) => {
                if let Some(name) = shared.scene_names.get(i) {
                    self.record_cue(CueKind::Scene(name.clone()));
                }
                self.send(Msg::GoToScene(i));
            }
            UiCommand::QueueNext(i) => self.send(Msg::QueueNext(i)),
            UiCommand::ShowClip(i) => {
                if let Some(name) = shared.clip_names.get(i) {
                    self.record_cue(CueKind::Clip(name.clone()));
                }
                self.send(Msg::ShowClip(i));
            }
            UiCommand::AddSong(p) => self.send(Msg::LoadSong(p)),
            UiCommand::LoadTimeline(p) => self.send(Msg::LoadTimeline(p)),
            UiCommand::OpenEditor => self.open_editor(event_loop),
            // A preview that's also a live action gets recorded like a hotkey.
            UiCommand::FireCue(kind) => {
                self.record_cue(kind.clone());
                self.send(Msg::FireCue(kind));
            }
            UiCommand::Song(ctl) => self.send(Msg::Transport(ctl)),
            UiCommand::Thumb(name) => self.send(Msg::Thumb(name)),
            UiCommand::ApplyLook(id) => {
                let look = lock(&shared.looks).iter().find(|l| l.id == id).cloned();
                if let Some(l) = look {
                    self.apply_look(&l);
                }
            }
            UiCommand::SaveLook(name) => self.save_look(&name, shared),
            UiCommand::DeleteLook(id) => {
                let _ = looks::delete(&looks::looks_dir(), &id);
                lock(&shared.looks).retain(|l| l.id != id);
            }
            UiCommand::LookSlot(id, slot) => {
                let mut all = lock(&shared.looks);
                looks::assign_slot(&mut all, &id, slot);
                // Persist every Look whose slot changed (the old owner loses it too).
                for l in all.iter() {
                    let _ = looks::save(&looks::looks_dir(), l);
                }
            }
            UiCommand::PerfCheck => self.send(Msg::PerfCheck),
            UiCommand::SaveTimeline => {
                let mut tl = lock(&shared.timeline);
                if let Some(doc) = tl.doc.as_mut() {
                    doc.sort_cues();
                    tl.message = match doc.save(&config::timelines_dir()) {
                        Ok(f) => {
                            tl.dirty = false;
                            format!("saved {}", f.display())
                        }
                        Err(e) => format!("save failed: {e:#}"),
                    };
                }
            }
        }
    }

    /// Draw one editor frame — the timeline doc is read under its own lock;
    /// the GPU present happens after it drops, like the panel.
    fn draw_editor(&mut self, event_loop: &ActiveEventLoop) {
        let Some(shared) = self.shared.clone() else {
            return;
        };
        let (commands, frame) = {
            let Some(e) = self.editor.as_mut() else {
                return;
            };
            if e.window.is_visible() == Some(false) {
                return;
            }
            e.run_ui(
                &shared.scene_names,
                shared.scene_titles.as_slice(),
                &shared.clip_names,
                shared.clip_titles.as_slice(),
                &shared.timeline,
                &shared.settings,
                &shared.thumbs,
            )
        };
        if let Some(e) = self.editor.as_mut() {
            e.present(frame);
        }
        for c in commands {
            self.ui_command(c, &shared, event_loop);
        }
    }
}

fn toggle_fullscreen(w: &Window) {
    if w.fullscreen().is_some() {
        w.set_fullscreen(None);
    } else {
        w.set_fullscreen(Some(Fullscreen::Borderless(w.current_monitor())));
    }
}

/// The Settings card status line: "192.168.1.20:9138 · 2 clients".
fn remote_line(srv: &remote::Server) -> String {
    let ip = remote::local_ip().unwrap_or_else(|| "?".into());
    let n = srv.client_count();
    format!(
        "{ip}:{} · {n} client{}",
        srv.port,
        if n == 1 { "" } else { "s" }
    )
}

impl ApplicationHandler<AppEvent> for App {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::MidiNote(note) => self.midi_note(event_loop, note),
            AppEvent::Remote(cmd) => self.remote_cmd(event_loop, cmd),
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // The render thread (which owns the Engine) isn't unwound on quit —
        // macOS Cmd-Q never returns from run_app — so stop the player here.
        engine::shutdown();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.shared.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Trippin")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
            .with_window_icon(self.icon.clone());
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        if self.start_fullscreen {
            toggle_fullscreen(&window);
        }
        let r = match pollster::block_on(Renderer::new(
            window.clone(),
            self.low_power,
            self.render_scale,
            self.vsync,
        )) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("renderer init failed: {e:#}");
                event_loop.exit();
                return;
            }
        };
        println!("Scenes: {}", r.scene_names().join(", "));
        if let Some(name) = &self.start_scene {
            match r.scene_names().iter().position(|n| n == name) {
                Some(i) => {
                    self.director.as_mut().unwrap().cut_to(i);
                    self.settings.mode = Mode::Manual;
                }
                None => eprintln!("no scene named {name:?}"),
            }
        }

        self.gpu = Some(r.gpu());
        let np_cfg = Arc::new(Mutex::new(nowplaying::NpConfig {
            source: self.settings.np_source,
            delay_s: self.settings.np_delay_s,
            file: self.settings.np_file.clone(),
        }));
        let np_remote: nowplaying::RemoteNp = Default::default();
        let np = nowplaying::start(
            np_cfg.clone(),
            config::data_dir().join("nowplaying.txt"),
            np_remote.clone(),
        );
        let shared = Arc::new(Shared {
            np,
            np_cfg,
            np_remote,
            np_replay: AtomicBool::new(false),
            rec_clip: AtomicBool::new(false),
            rec_set: AtomicBool::new(false),
            settings: Mutex::new(self.settings.clone()),
            show_baseline: Mutex::new(None),
            status: Mutex::new(Status::default()),
            timeline: timeline::TimelineState::new(),
            env: self.env.clone(),
            dirty: AtomicBool::new(false),
            quit: AtomicBool::new(false),
            scene_names: r.scene_names(),
            scene_heavy: r.scene_heavy(),
            heavy_ok: r.heavy_ok(),
            clip_names: self
                .dancer
                .as_ref()
                .map(|d| d.clip_names())
                .unwrap_or_default(),
            scene_titles: Arc::new(r.scene_titles()),
            clip_titles: Arc::new(
                self.dancer
                    .as_ref()
                    .map(|d| d.clip_titles())
                    .unwrap_or_default(),
            ),
            perf: Mutex::new(perf::Report::load()),
            thumbs: Mutex::new(std::collections::HashMap::new()),
            midi_status: Mutex::new((false, "off".into())),
            remote_status: Mutex::new((false, "off".into())),
            looks: Mutex::new(looks::load_all(&looks::looks_dir())),
            look_notice: Mutex::new(None),
        });
        self.shared = Some(shared.clone());
        let (tx, rx) = mpsc::channel();
        self.render_tx = Some(tx);
        let audio = self.audio.take().unwrap();
        let dir = self.director.take().unwrap();
        let dancer = self.dancer.take().unwrap();
        let audio_cfg = self.audio_cfg.clone();
        let sh = shared.clone();
        std::thread::Builder::new()
            .name("render".into())
            .spawn(move || {
                // A render panic must not leave a zombie: dead visuals with a
                // live UI is the worst failure mode mid-set. Log it (the panic
                // hook prints details) and tell the event loop to exit.
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    render_loop(r, dir, dancer, audio, audio_cfg, sh.clone(), rx)
                }))
                .is_err()
                {
                    sh.quit.store(true, Ordering::Relaxed);
                }
            })
            .expect("spawn render thread");

        self.window = Some(window);
        if let Ok(v) = std::env::var("TRIPPIN_TEXT_TEST") {
            let (w, fx) = v.split_once('|').unwrap_or((v.as_str(), "none"));
            let spec: text::TextSpec = serde_json::from_value(serde_json::json!({
                "text": w, "style": "chrome", "anim": "zoom", "fx": fx, "size": 1.6
            }))
            .unwrap();
            self.send(Msg::FireCue(timeline::CueKind::Text(spec)));
        }
        // Dev aid: fire any cue ~3 s after startup (once the render loop has
        // settled), e.g. TRIPPIN_CUE_TEST='{"Strobe":true}'.
        if let Ok(v) = std::env::var("TRIPPIN_CUE_TEST") {
            match (serde_json::from_str::<timeline::CueKind>(&v), self.render_tx.clone()) {
                (Ok(k), Some(tx)) => {
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        let _ = tx.send(Msg::FireCue(k));
                    });
                }
                (Err(e), _) => eprintln!("TRIPPIN_CUE_TEST: {e}"),
                _ => {}
            }
        }
        if let Some(p) = self.start_song.take() {
            if p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("json"))
            {
                self.send(Msg::LoadTimeline(p));
            } else {
                self.send(Msg::LoadSong(p));
            }
        }
        if self.settings.show_panel && !self.no_panel {
            self.open_panel(event_loop);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let is_editor = self.editor.as_ref().is_some_and(|e| e.window.id() == id);
        if is_editor {
            match &event {
                WindowEvent::CloseRequested => {
                    if let Some(e) = &self.editor {
                        e.window.set_visible(false);
                    }
                    if let Some(w) = &self.window {
                        w.focus_window();
                    }
                    if let Some(sh) = &self.shared {
                        // Exiting the editor ends the show. A hidden player
                        // would keep the audio engine swapped (features
                        // frozen → every scene looks stuck), borrowed show
                        // state would stay latched, and preview text never
                        // fades on its own. Stop is cheap even while Stopped —
                        // it also restores state a plain scrub borrowed.
                        lock(&sh.timeline).autosync = false;
                        self.send(Msg::Transport(SongCtl::Stop));
                    }
                    self.send(Msg::FadeText);
                    return;
                }
                WindowEvent::RedrawRequested => {
                    self.draw_editor(event_loop);
                    return;
                }
                WindowEvent::DroppedFile(path) => {
                    if song::is_audio_file(path) {
                        self.send(Msg::LoadSong(path.clone()));
                    } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
                        self.send(Msg::LoadTimeline(path.clone()));
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if event.state == ElementState::Pressed && !event.repeat {
                        let typing = self.editor.as_ref().is_some_and(|e| e.wants_keyboard());
                        // Text-producing keys plus the editing keys belong to a
                        // focused field; arrows stay global (scene next/prev).
                        let text_like = matches!(event.logical_key, Key::Character(_))
                            || matches!(
                                event.logical_key,
                                Key::Named(
                                    winit::keyboard::NamedKey::Space
                                        | winit::keyboard::NamedKey::Backspace
                                        | winit::keyboard::NamedKey::Delete
                                        | winit::keyboard::NamedKey::Home
                                        | winit::keyboard::NamedKey::End
                                        | winit::keyboard::NamedKey::Tab
                                )
                            );
                        if typing && text_like {
                            // fall through to egui
                        } else if matches!(
                            event.logical_key,
                            Key::Named(winit::keyboard::NamedKey::Space)
                        ) {
                            // An explicit Space binding wins; only an
                            // unbound Space toggles transport here.
                            let key = event.logical_key.clone();
                            if !self.key(event_loop, &key) {
                                self.send(Msg::Transport(SongCtl::Toggle));
                            }
                            return;
                        } else {
                            let key = event.logical_key.clone();
                            self.key(event_loop, &key);
                            return;
                        }
                    }
                }
                _ => {}
            }
            if let Some(e) = self.editor.as_mut() {
                e.on_event(&event);
            }
            return;
        }
        let is_panel = self.panel.as_ref().is_some_and(|p| p.window.id() == id);
        if is_panel {
            match &event {
                WindowEvent::CloseRequested => {
                    // Hide rather than destroy — reopening is then instant and
                    // can't fail partway through.
                    if let Some(p) = &mut self.panel {
                        p.window.set_visible(false);
                        p.midi_learn = None;
                    }
                    if let Some(w) = &self.window {
                        w.focus_window();
                    }
                    self.settings_mut().show_panel = false;
                    self.mark_dirty();
                    return;
                }
                WindowEvent::RedrawRequested => {
                    self.draw_panel(event_loop);
                    return;
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    // Ignore autorepeat — a still-held key would re-fire the
                    // action (F1 opens the panel, focus moves to it, the next
                    // repeat instantly toggles it closed again).
                    if event.state == ElementState::Pressed && !event.repeat {
                        let rebinding = self.panel.as_ref().is_some_and(|p| p.rebinding.is_some());
                        let typing = self.panel.as_ref().is_some_and(|p| p.wants_keyboard());
                        // Only text-producing + editing keys belong to a
                        // focused text field; function/arrow keys stay global
                        // hotkeys, so F1 toggles even while typing in the
                        // scene filter. Space counts as text too — names
                        // need it — and Enter submits a field (Save look).
                        let text_like = matches!(event.logical_key, Key::Character(_))
                            || matches!(
                                event.logical_key,
                                Key::Named(
                                    winit::keyboard::NamedKey::Space
                                        | winit::keyboard::NamedKey::Enter
                                        | winit::keyboard::NamedKey::Backspace
                                        | winit::keyboard::NamedKey::Delete
                                        | winit::keyboard::NamedKey::Home
                                        | winit::keyboard::NamedKey::End
                                        | winit::keyboard::NamedKey::Tab
                                )
                            );
                        if rebinding || !(typing && text_like) {
                            let key = event.logical_key.clone();
                            self.key(event_loop, &key);
                            return;
                        }
                    }
                }
                _ => {}
            }
            if let Some(p) = self.panel.as_mut() {
                p.on_event(&event);
            }
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                if let Some(sh) = &self.shared {
                    sh.quit.store(true, Ordering::Relaxed);
                }
                event_loop.exit();
            }
            WindowEvent::Resized(size) => self.send(Msg::Resize(size.width, size.height)),
            WindowEvent::DroppedFile(path) => {
                // Drop an audio file or a timeline .json anywhere on the
                // visuals to load it.
                if song::is_audio_file(&path) {
                    self.send(Msg::LoadSong(path));
                } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    self.send(Msg::LoadTimeline(path));
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed && !event.repeat {
                    self.key(event_loop, &event.logical_key);
                }
            }
            // The render thread draws continuously; nothing to do here.
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // If the render thread died, don't sit here ignoring input — exit.
        if self
            .shared
            .as_ref()
            .is_some_and(|sh| sh.quit.load(Ordering::Relaxed))
        {
            event_loop.exit();
            return;
        }
        // Title refresh — Win32 window calls stay on the thread that owns the
        // window; the render thread only writes status.
        if self.last_title.elapsed() > Duration::from_millis(500) {
            self.last_title = Instant::now();
            if let (Some(w), Some(sh)) = (&self.window, &self.shared) {
                let st = lock(&sh.status);
                let name = sh
                    .scene_names
                    .get(st.scene)
                    .map(String::as_str)
                    .unwrap_or("?");
                let mode = lock(&sh.settings).mode;
                w.set_title(&format!("Trippin — {name} — {:.1} BPM — {mode:?}", st.bpm));
            }
        }
        // The visuals render on their own thread; the panel needs ~10 fps,
        // the editor ~30 fps so its playhead and meters move smoothly.
        if let Some(p) = &self.panel {
            if p.window.is_visible() != Some(false)
                && self.last_panel_draw.elapsed() > Duration::from_millis(100)
            {
                self.last_panel_draw = Instant::now();
                p.window.request_redraw();
            }
        }
        if let Some(e) = &self.editor {
            if e.window.is_visible() != Some(false)
                && self.last_editor_draw.elapsed() > Duration::from_millis(33)
            {
                self.last_editor_draw = Instant::now();
                e.window.request_redraw();
            }
        }
        // MIDI input: (re)connect when the chosen port changes. WinMM gives
        // no unplug callback, so every few seconds the port list is scanned —
        // a vanished port means a dead connection; drop it and the retry
        // below picks the device back up the moment it's plugged in again.
        if let Some(sh) = &self.shared {
            let want = lock(&sh.settings).midi_in.clone();
            if self.midi_scan.elapsed() > Duration::from_secs(3) {
                self.midi_scan = Instant::now();
                if self.midi.is_some() && !midi::ports().iter().any(|p| *p == want) {
                    self.midi = None;
                }
            }
            let have = self.midi.as_ref().map(|m| m.name.as_str()).unwrap_or("");
            if want != have && (want.is_empty() || self.midi_retry.elapsed() > Duration::from_secs(3))
            {
                // Drop the old connection before opening a new one — a port
                // can't be held twice.
                self.midi = None;
                self.midi_retry = Instant::now();
                let status = if want.is_empty() {
                    (false, "off".to_string())
                } else {
                    let proxy = self.midi_proxy.clone();
                    match midi::connect(&want, move |note| {
                        let _ = proxy.send_event(AppEvent::MidiNote(note));
                    }) {
                        Ok(m) => {
                            let status = (true, m.name.clone());
                            self.midi = Some(m);
                            status
                        }
                        Err(e) => (false, format!("{want}: {e:#}")),
                    }
                };
                *lock(&sh.midi_status) = status;
            }
        }
        // LAN remote + OSC: (re)start when the toggles/ports/PIN change —
        // same lifecycle shape as MIDI above, retries throttled on a busy
        // port.
        if let Some(sh) = &self.shared {
            let (want_on, want_port, want_pin, want_osc, want_osc_port) = {
                let s = lock(&sh.settings);
                (
                    s.remote_on,
                    s.remote_port,
                    s.remote_pin.clone(),
                    s.osc_on,
                    s.osc_port,
                )
            };
            let want = want_on.then_some((want_port, want_pin));
            let have = self.remote.as_ref().map(|r| (r.port, r.pin.clone()));
            if want != have
                && (want.is_none() || self.remote_retry.elapsed() > Duration::from_secs(2))
            {
                self.remote_retry = Instant::now();
                self.remote = None;
                let status = match want.and_then(|(port, pin)| {
                    self.remote_hooks()
                        .map(|h| (port, remote::Server::start(port, &pin, h)))
                }) {
                    Some((_, Ok(srv))) => {
                        let line = remote_line(&srv);
                        self.remote = Some(srv);
                        self.remote_clients = 0;
                        (true, line)
                    }
                    Some((port, Err(e))) => (false, format!("port {port}: {e:#}")),
                    None => (false, "off".into()),
                };
                *lock(&sh.remote_status) = status;
            }
            // Refresh the status line when the client count moves.
            if let Some(srv) = &self.remote {
                let n = srv.client_count();
                if n != self.remote_clients {
                    self.remote_clients = n;
                    *lock(&sh.remote_status) = (true, remote_line(srv));
                }
            }
            // OSC: keyed on (on, port). Shares the remote command path —
            // OSC presses act exactly like remote-app presses.
            let want_osc_p = want_osc.then_some(want_osc_port);
            let have_osc = self.osc.as_ref().map(|o| o.port);
            if want_osc_p != have_osc
                && (want_osc_p.is_none()
                    || self.remote_retry.elapsed() > Duration::from_secs(2))
            {
                self.osc = None;
                if let Some(port) = want_osc_p {
                    let proxy = self.midi_proxy.clone();
                    self.osc = osc::Osc::start(port, move |c| {
                        let _ = proxy.send_event(AppEvent::Remote(c));
                    })
                    .map_err(|e| eprintln!("OSC listener: {e:#}"))
                    .ok();
                }
            }
        }
        // Settings changed on the render thread also need flushing to disk.
        if let Some(sh) = &self.shared {
            if sh.dirty.load(Ordering::Relaxed) && self.dirty_since.is_none() {
                self.dirty_since = Some(Instant::now());
            }
        }
        // Fullscreen visuals get no pointer; the panel/editor keep theirs
        // (cursor visibility is per hovered window).
        let fs = self
            .window
            .as_ref()
            .is_some_and(|w| w.fullscreen().is_some());
        FULLSCREEN.store(fs, Ordering::Relaxed);
        if fs != self.cursor_hidden {
            self.cursor_hidden = fs;
            if let Some(w) = &self.window {
                w.set_cursor_visible(!fs);
            }
        }
        if let Some(t) = self.dirty_since {
            if t.elapsed() > Duration::from_secs(1) {
                self.dirty_since = None;
                if !self.no_save {
                    if let Some(sh) = &self.shared {
                        let mut s = lock(&sh.settings).clone();
                        if let Some(b) = lock(&sh.show_baseline).as_ref() {
                            b.restore_into(&mut s);
                        }
                        s.save();
                    }
                }
                if let Some(sh) = &self.shared {
                    sh.dirty.store(false, Ordering::Relaxed);
                }
            }
        }
    }
}

fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// Window/taskbar icon, decoded once from the PNG embedded in the exe.
fn load_icon() -> Option<Icon> {
    let img = image::load_from_memory(include_bytes!("../logo.png"))
        .ok()?
        .into_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).ok()
}

/// Validate every shader with naga and exit — no window or GPU needed.
fn check_shaders() -> Result<()> {
    let dir = render::find_shader_dir()?;
    let common = std::fs::read_to_string(dir.join("common.wgsl"))?;
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir.join("scenes"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    paths.retain(|p| p.extension().is_some_and(|e| e == "wgsl"));
    paths.sort();
    paths.push(dir.join("present.wgsl"));
    paths.push(dir.join("dancer.wgsl"));
    paths.push(dir.join("text.wgsl"));
    paths.push(dir.join("overlay.wgsl"));
    let mut bad = 0;
    for p in &paths {
        let body = std::fs::read_to_string(p)?;
        let src = format!("{common}\n{body}");
        let name = p.file_name().unwrap().to_string_lossy();
        let valid = wgpu::naga::front::wgsl::parse_str(&src)
            .map_err(|e| e.emit_to_string(&src))
            .and_then(|m| {
                wgpu::naga::valid::Validator::new(
                    wgpu::naga::valid::ValidationFlags::all(),
                    wgpu::naga::valid::Capabilities::all(),
                )
                .validate(&m)
                .map_err(|e| e.emit_to_string(&src))
            });
        match valid {
            Ok(_) => println!("ok   {name}"),
            Err(e) => {
                bad += 1;
                eprintln!("FAIL {name}:\n{e}");
            }
        }
    }
    if bad > 0 {
        anyhow::bail!("{bad} shader(s) failed validation");
    }
    println!("All shaders valid.");
    Ok(())
}

/// `trippin --ai-build track.mp3`: analyse + ask the configured provider for
/// a block plan, expand it, and print every cue. The editor equivalent of
/// "Build cues" without opening a window.
fn ai_build(path: &std::path::Path) -> Result<()> {
    let song = song::load(path)?;
    let clip = timeline::Clip::from_song(&song, 0.0);
    let mut scenes: Vec<String> = std::fs::read_dir(render::find_shader_dir()?.join("scenes"))?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            (p.extension().is_some_and(|x| x == "wgsl"))
                .then(|| p.file_stem().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    scenes.sort();
    let routines: Vec<String> = dancer::find_dancer_dir()
        .map(|d| dancer::list_clips(&d).into_iter().map(|c| c.name).collect())
        .unwrap_or_default();
    let conf = ai::AiConf::from_settings(&Settings::load());
    println!(
        "{} ({}) — {} at {:.1} BPM, {:.0} beats",
        conf.provider.label(),
        conf.model,
        song.name,
        song.bpm,
        (song.duration - song.first_beat) * song.bpm / 60.0
    );
    let ai::ShowBuild { cues, note, .. } = ai::build_show(&[clip], &scenes, &routines, &conf)?;
    println!("{note}");
    for cue in &cues {
        println!("  {:>6.1} bt  clip {}  {:?}", cue.beat, cue.clip, cue.kind);
    }
    Ok(())
}

fn main() -> Result<()> {
    // Panics on the render thread must be visible — a silent death leaves the
    // app looking alive while ignoring every command.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        eprintln!("panic on thread {:?}: {info}", thread.name());
        hook(info);
    }));
    // Beat This! inference (rten) defaults to every physical core; two keep
    // the live downbeat check (~0.8 s per 5 s window) clear of the render
    // and audio threads. Beyond 4 threads it barely speeds up anyway.
    if std::env::var_os("RTEN_NUM_THREADS").is_none() {
        // SAFETY: first thing in main, before any other thread exists.
        unsafe { std::env::set_var("RTEN_NUM_THREADS", "2") };
    }
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--list-devices") {
        return audio::list_devices();
    }
    // `--probe-audio [name]` captures ~6 s on a device (or the default tap)
    // and prints the analysed level — verifies a source carries signal
    // without opening the window.
    if args.iter().any(|a| a == "--probe-audio") {
        let name = arg_value(&args, "--probe-audio").filter(|s| !s.is_empty());
        let eng = AudioEngine::start(name, false, None)?;
        println!("Listening on {} …", eng.device_name);
        let mut peak = [0f32; 4];
        let mut silent = true;
        let end = Instant::now() + Duration::from_secs(6);
        while Instant::now() < end {
            std::thread::sleep(Duration::from_millis(200));
            let f = eng.features.lock().unwrap_or_else(|e| e.into_inner()).clone();
            for (i, v) in f.lvl4.iter().enumerate() {
                peak[i] = peak[i].max(*v);
            }
            silent &= f.silent;
        }
        let f = eng.features.lock().unwrap_or_else(|e| e.into_inner()).clone();
        println!(
            "signal: {}  peak bands [{:.2} {:.2} {:.2} {:.2}]  bpm {:.1}",
            if silent { "none (silent)" } else { "OK" },
            peak[0], peak[1], peak[2], peak[3], f.bpm,
        );
        return if silent {
            Err(anyhow::anyhow!("no signal on {}", eng.device_name))
        } else {
            Ok(())
        };
    }
    // `--list-midi`: print MIDI input port names (what the Keys page lists).
    if args.iter().any(|a| a == "--list-midi") {
        let ports = midi::ports();
        if ports.is_empty() {
            println!("No MIDI inputs found.");
        }
        for p in ports {
            println!("{p}");
        }
        return Ok(());
    }
    // `--snap <scenes|all>` renders scenes headless to PNG and times them.
    if args.iter().any(|a| a == "--snap") {
        return snap::run(&args);
    }
    // `--nowplaying`: watch what each track source (Spotify, Serato, …) sees.
    #[cfg(windows)]
    if let Some(name) = arg_value(&args, "--spout-grab") {
        let (w, h, px) = spout::grab(name)?;
        let out = args.last().filter(|a| a.ends_with(".png")).cloned().unwrap_or("spout.png".into());
        image::save_buffer(&out, &px, w, h, image::ColorType::Rgba8)?;
        println!("{w}x{h} → {out}");
        return Ok(());
    }
    #[cfg(windows)]
    if args.iter().any(|a| a == "--spout-test") {
        return spout::test();
    }
    if args.iter().any(|a| a == "--nowplaying") {
        return nowplaying::monitor();
    }
    // `--dump-feed track.flac out.jsonl [palette]`: the Unity feed a track
    // would produce (see audio::dump_feed); the engine replays it.
    if let Some(i) = args.iter().position(|a| a == "--dump-feed") {
        let (Some(track), Some(out)) = (args.get(i + 1), args.get(i + 2)) else {
            anyhow::bail!("usage: --dump-feed track.flac out.jsonl [palette]");
        };
        let pal = args.get(i + 3).filter(|p| !p.starts_with("--")).map(String::as_str).unwrap_or("sunset");
        return audio::dump_feed(std::path::Path::new(track), std::path::Path::new(out), pal);
    }
    if let Some(p) = arg_value(&args, "--groove-test") {
        return audio::groove_test(std::path::Path::new(&p));
    }
    if args.iter().any(|a| a == "--check-shaders") {
        return check_shaders();
    }
    // `--analyze track.mp3` prints the per-bar feature summary the AI show
    // builder sends to the model — the prompt-tuning / sanity-check path.
    if let Some(p) = arg_value(&args, "--analyze") {
        println!("{}", ai::analyze_file(std::path::Path::new(&p))?);
        return Ok(());
    }
    // `--beats track.mp3`: compare the autocorrelation grid with Beat This!
    // (downloads the model on first use) and time the inference.
    if let Some(p) = arg_value(&args, "--beats") {
        return beats::beat_test(std::path::Path::new(&p));
    }
    // `--ai-build track.mp3` runs the whole pipeline end-to-end (analysis,
    // provider call, cue expansion) and prints the cue list — a preview of
    // what "Build cues" in the editor would generate.
    if let Some(p) = arg_value(&args, "--ai-build") {
        return ai_build(std::path::Path::new(&p));
    }
    // `--ndi-monitor [name]` lists discoverable NDI sources and counts frames
    // from the first match — checks the send path end-to-end with no NDI
    // Tools install needed.
    if args.iter().any(|a| a == "--ndi-monitor") {
        let name = arg_value(&args, "--ndi-monitor");
        return ndi::Ndi::load().and_then(|n| n.monitor(name.as_deref(), 12));
    }
    let cli_device = arg_value(&args, "--device");
    let mic = args.iter().any(|a| a == "--mic");
    let env = audio::EnvLog::new();

    let mut settings = Settings::load();
    // Neural beat tracking: fetch the model in the background on first run.
    beats::set_enabled(settings.beat_model);
    if settings.beat_model {
        beats::ensure_models();
    }
    // Audio source: --device/--mic win for this run; otherwise the saved
    // panel choice ("" = the platform default tap).
    let device: Option<String> = cli_device.map(str::to_string).or_else(|| {
        (!mic && !settings.audio_in.is_empty()).then(|| settings.audio_in.clone())
    });
    let audio = match AudioEngine::start(device.as_deref(), mic, Some(env.clone())) {
        Ok(a) => a,
        // A saved device that's gone (controller unplugged) falls back to
        // the default tap rather than blocking startup.
        Err(e) if cli_device.is_none() && device.is_some() => {
            eprintln!("saved audio input {device:?} unavailable: {e:#} — using the default");
            AudioEngine::start(None, mic, Some(env.clone()))?
        }
        Err(e) => return Err(e),
    };
    println!("Audio: {}", audio.device_name);
    let start_song = arg_value(&args, "--song").map(std::path::PathBuf::from);

    let mut no_save = false;
    if args.iter().any(|a| a == "--no-dancer") {
        settings.dancer_enabled = false;
        no_save = true;
    }
    if let Some(i) = args.iter().position(|a| a == "--dancer") {
        settings.dancer_enabled = true;
        if let Some(style) = args
            .get(i + 1)
            .and_then(|s| dancer::STYLES.iter().position(|n| n == s))
        {
            settings.dancer_style = Some(style);
        }
        no_save = true;
    }
    if args.iter().any(|a| a == "--canon") {
        settings.canon = Tristate::On;
        no_save = true;
    }
    let start_scene = arg_value(&args, "--scene").map(str::to_string);
    if start_scene.is_some() {
        no_save = true;
    }
    if no_save {
        println!("Command-line overrides active: settings won't be saved this run.");
    }

    let mut dancer = DancerLayer::new();
    if dancer.clips.is_empty() {
        println!("Dancers: none found (run tools/build_dancer_library.py)");
    } else {
        let first = dancer
            .clips
            .iter()
            .position(|c| !settings.disabled_clips.contains(&c.name))
            .unwrap_or(0);
        dancer.request(first);
    }

    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let midi_proxy = event_loop.create_proxy();
    let mut app = App {
        audio: Some(audio),
        window: None,
        gpu: None,
        panel: None,
        editor: None,
        director: Some(Director::new()),
        dancer: Some(dancer),
        settings,
        shared: None,
        render_tx: None,
        audio_cfg: (device, mic),
        start_song,
        env,
        dirty_since: None,
        no_save,
        start_fullscreen: args.iter().any(|a| a == "--fullscreen"),
        start_scene,
        no_panel: args.iter().any(|a| a == "--no-panel"),
        low_power: arg_value(&args, "--gpu") == Some("low"),
        render_scale: arg_value(&args, "--scale").and_then(|s| s.parse().ok()),
        vsync: args.iter().any(|a| a == "--vsync"),
        last_panel_draw: Instant::now(),
        last_editor_draw: Instant::now(),
        last_title: Instant::now(),
        last_panel_toggle: Instant::now() - Duration::from_secs(1),
        icon: load_icon(),
        cursor_hidden: false,
        midi_proxy,
        midi: None,
        // Backdated so a configured device connects immediately at startup
        // rather than after one retry interval.
        midi_retry: Instant::now()
            .checked_sub(Duration::from_secs(4))
            .unwrap_or_else(Instant::now),
        midi_scan: Instant::now(),
        remote: None,
        osc: None,
        // Backdated like midi_retry so a saved `remote_on` starts the
        // server immediately at launch.
        remote_retry: Instant::now()
            .checked_sub(Duration::from_secs(4))
            .unwrap_or_else(Instant::now),
        remote_clients: 0,
    };
    event_loop.run_app(&mut app)?;
    engine::shutdown();
    Ok(())
}
