//! Trippin — live music-reactive visuals for DJ sets.
//!
//! Usage: trippin [--list-devices] [--device "<name part>"] [--mic] [--scene <name>]
//!                [--dancer [style]] [--no-dancer] [--canon] [--no-panel]
//!                [--gpu low] [--scale 0.75] [--fullscreen] [--vsync]
//!                [--song <audio file>]
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

mod audio;
mod config;
mod dancer;
mod director;
mod panel;
mod render;
mod song;
#[cfg(target_os = "macos")]
mod sysaudio;
mod timeline;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::Key;
use winit::window::{Fullscreen, Icon, Window, WindowId};

use audio::{AudioEngine, Command};
use config::{in_season, key_name, today, Action, Fx, Mode, Seasonal, Settings, Tristate};
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
    /// The visuals window is closing.
    quit: AtomicBool,
    /// Fixed after init; the panel lists them.
    scene_names: Vec<String>,
    clip_names: Vec<String>,
}

/// Work the event thread hands to the render thread.
enum Msg {
    Resize(u32, u32),
    Act(Action),
    GoToScene(usize),
    ShowClip(usize),
    /// Decode an audio file and start a fresh timeline from it.
    LoadSong(std::path::PathBuf),
    /// Open a saved timeline `.json`.
    LoadTimeline(std::path::PathBuf),
    /// Transport control for the song player.
    Transport(SongCtl),
}

/// Lock even when poisoned: a panicking sibling thread shouldn't take the
/// visuals (or the panel) down with it mid-set.
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Scenes that compile, are ticked in the playlist and (for seasonal
/// scenes) are in season.
fn usable_scenes(r: &Renderer, s: &Settings) -> Vec<usize> {
    let names = r.scene_names();
    let all = r.usable_scenes();
    let date = today();
    let on: Vec<usize> = all
        .iter()
        .copied()
        .filter(|&i| !s.disabled_scenes.contains(&names[i]))
        .filter(|&i| match (s.seasonal, in_season(&names[i], date)) {
            (_, None) | (Seasonal::Always, _) => true,
            (Seasonal::Off, Some(_)) => false,
            (Seasonal::Auto, Some(in_now)) => in_now,
        })
        .collect();
    if on.is_empty() { all } else { on }
}

/// Stop the song player and put the live audio engine back.
fn stop_song(
    player: &mut Option<song::SongPlayer>,
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

/// Fire one timeline cue — mirrors `apply_render` for the cue kinds.
/// `usable` is this frame's usable-scenes list.
fn fire_cue(
    kind: &CueKind,
    r: &mut Renderer,
    dir: &mut Director,
    dancer: &mut DancerLayer,
    blackout: &mut bool,
    shared: &Shared,
    usable: &[usize],
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
                dancer.request(i);
                dancer.showing = true;
            }
        }
        CueKind::NextClip => {
            dancer.next_clip();
            dancer.showing = true;
        }
        CueKind::NextLook => s.dancer_style = Some((dancer.style + 1) % dancer::STYLES.len()),
        CueKind::Look(l) => s.dancer_style = *l,
        CueKind::Canon(t) => s.canon = *t,
        CueKind::Blackout(b) => *blackout = *b,
        CueKind::Fx(f) => {
            s.fx_auto = false;
            s.fx = *f;
        }
        CueKind::FxAuto(b) => s.fx_auto = *b,
    }
    shared.dirty.store(true, Ordering::Relaxed);
}

/// The whole show, free-running on its own thread at vsync pace.
fn render_loop(
    mut r: Renderer,
    mut dir: Director,
    mut dancer: DancerLayer,
    mut audio: AudioEngine,
    audio_cfg: (Option<String>, bool),
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
    let mut flow_bpm = 120.0f32;
    let mut blackout = false;
    let mut master = 1.0f32;
    // The post effect showing now — the auto-pilot's pick when `fx_auto` is on.
    let mut fx_current = lock(&shared.settings).fx;

    // --- Timeline state ---------------------------------------------------
    // Decodes run on a throwaway thread; results land in `song_rx` and the
    // engine/player swap happens here (cpal streams aren't Send anyway).
    let (song_tx, song_rx) = mpsc::channel::<Result<song::Song, String>>();
    let mut song: Option<Arc<song::Song>> = None;
    let mut player: Option<song::SongPlayer> = None;
    // True while a decode belongs to a loaded .json (keep its cues) rather
    // than a bare audio file (start a fresh doc from it).
    let mut loading_for_doc = false;
    let mut matcher = Matcher::new();
    // Highest beat already dispatched — cues are one-shot events.
    let mut fired_past = f64::MIN;
    let mut was_locked = false;
    let spawn_load = |tx: &mpsc::Sender<Result<song::Song, String>>, path: std::path::PathBuf| {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(song::load(&path).map_err(|e| format!("{e:#}")));
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
                Msg::ShowClip(i) => {
                    dancer.request(i);
                    dancer.showing = true;
                }
                Msg::Act(a) => apply_render(&mut r, &mut dir, &mut dancer, &mut blackout, &audio, &shared, a),
                Msg::LoadSong(path) => {
                    let mut tl = lock(&shared.timeline);
                    if !tl.busy {
                        tl.busy = true;
                        tl.message = format!("loading {}…", path.display());
                        loading_for_doc = false;
                        spawn_load(&song_tx, path);
                    }
                }
                Msg::LoadTimeline(path) => match timeline::Timeline::load(&path) {
                    Ok(doc) => {
                        let song_path = doc.song.clone();
                        let name = doc.name.clone();
                        {
                            let mut tl = lock(&shared.timeline);
                            tl.doc = Some(doc);
                            tl.mode = PlayMode::Stopped;
                            tl.pos_s = 0.0;
                            tl.cursor_s = 0.0;
                            tl.dirty = false;
                            tl.message = format!("timeline {name}");
                        }
                        matcher.reset();
                        was_locked = false;
                        fired_past = f64::MIN;
                        stop_song(&mut player, &mut audio, &audio_cfg, &shared);
                        if song_path.exists() {
                            let mut tl = lock(&shared.timeline);
                            if !tl.busy {
                                tl.busy = true;
                                loading_for_doc = true;
                                spawn_load(&song_tx, song_path);
                            }
                        } else {
                            lock(&shared.timeline).message =
                                "song file missing — strip + live match only".into();
                        }
                    }
                    Err(e) => lock(&shared.timeline).message = format!("{e:#}"),
                },
                Msg::Transport(ctl) => {
                    eprintln!("Timeline: transport {ctl:?} (mode {:?})", lock(&shared.timeline).mode);
                    let mut tl = lock(&shared.timeline);
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
                                if let Some(sg) = &song {
                                    match AudioEngine::start_song(sg.clone(), Some(shared.env.clone()))
                                    {
                                        Ok((eng, pl)) => {
                                            if tl.cursor_s > 0.05 {
                                                pl.seek_s(tl.cursor_s);
                                                fired_past = tl
                                                    .doc
                                                    .as_ref()
                                                    .map(|d| d.beat_at(tl.cursor_s))
                                                    .unwrap_or(f64::MIN);
                                            }
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
                                } else {
                                    tl.message = "no song loaded".into();
                                }
                            }
                        },
                        SongCtl::Stop => {
                            tl.mode = PlayMode::Stopped;
                            tl.pos_s = 0.0;
                            drop(tl);
                            stop_song(&mut player, &mut audio, &audio_cfg, &shared);
                            fired_past = f64::MIN;
                            continue;
                        }
                        SongCtl::Seek(t) => {
                            if let Some(p) = &player {
                                p.seek_s(t);
                                tl.pos_s = t;
                                fired_past = tl
                                    .doc
                                    .as_ref()
                                    .map(|d| d.beat_at(t))
                                    .unwrap_or(f64::MIN);
                            } else {
                                tl.cursor_s = t.max(0.0);
                                tl.pos_s = tl.cursor_s;
                            }
                        }
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
        let s = lock(&shared.settings).clone();
        let usable = usable_scenes(&r, &s);

        // --- Timeline: loader results, transport state, cue dispatch -------
        // Lock order note: the panel draws under the settings lock and takes
        // the timeline lock inside it, so here the timeline lock must never
        // be held while locking settings — collect, drop, then act.
        while let Ok(res) = song_rx.try_recv() {
            match res {
                Ok(s2) => {
                    let song_arc = Arc::new(s2);
                    {
                        let mut tl = lock(&shared.timeline);
                        tl.busy = false;
                        if loading_for_doc {
                            tl.message = format!("song ready: {}", song_arc.name);
                        } else {
                            println!(
                                "Song: {} — {:.1} BPM, {:.1}s, first beat {:.2}s",
                                song_arc.name, song_arc.bpm, song_arc.duration,
                                song_arc.first_beat
                            );
                            tl.doc = Some(timeline::Timeline::from_song(&song_arc));
                            tl.mode = PlayMode::Stopped;
                            tl.pos_s = 0.0;
                            tl.cursor_s = 0.0;
                            tl.dirty = false;
                            tl.message = format!(
                                "{} — {:.0} BPM, {:.0} bars",
                                song_arc.name,
                                song_arc.bpm,
                                song_arc.bpm * song_arc.duration / 240.0
                            );
                            matcher.reset();
                            was_locked = false;
                            fired_past = f64::MIN;
                        }
                    }
                    song = Some(song_arc);
                }
                Err(e) => {
                    let mut tl = lock(&shared.timeline);
                    tl.busy = false;
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
            }
            stop_song(&mut player, &mut audio, &audio_cfg, &shared);
            fired_past = f64::MIN;
        }

        // Pick a playhead: the player's clock while the file plays, else the
        // live-match estimate while follow-live is on. Fire the cues due.
        {
            let mut guard = lock(&shared.timeline);
            let tl = &mut *guard;
            let mut due: Vec<CueKind> = Vec::new();
            let mut new_fired = fired_past;
            if let Some(doc) = tl.doc.as_ref() {
                let pos_b: Option<f64> = match tl.mode {
                    PlayMode::Playing => {
                        let t = player.as_ref().map(|p| p.position_s()).unwrap_or(0.0);
                        tl.pos_s = t;
                        Some(doc.beat_at(t))
                    }
                    _ if tl.autosync => {
                        // Follow-live: correlate the room's onset envelope
                        // against the song's (2×/sec), coast between evals.
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
                        if matcher.locked && !was_locked {
                            // Fresh lock: don't dump the backlog mid-bar.
                            new_fired = live_pos.map(|p| doc.beat_at(p)).unwrap_or(new_fired);
                        }
                        was_locked = matcher.locked;
                        if let Some(p) = live_pos {
                            tl.pos_s = p;
                            Some(doc.beat_at(p))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(b) = pos_b {
                    // Big jumps (lock acquire, seek) skip rather than dump.
                    if new_fired == f64::MIN || (b - new_fired).abs() > 8.0 {
                        new_fired = b;
                    }
                    for c in doc.cues_between(new_fired, b) {
                        due.push(c.kind.clone());
                    }
                    new_fired = b;
                }
            }
            drop(guard);
            fired_past = new_fired;
            for kind in &due {
                fire_cue(kind, &mut r, &mut dir, &mut dancer, &mut blackout, &shared, &usable);
            }
        }

        if now - last_reload_check > Duration::from_millis(500) {
            last_reload_check = now;
            r.reload_shaders(false);
        }

        let f = audio.features.lock().map(|f| f.clone()).unwrap_or_default();
        // Positive latency shows the beat earlier (compensating capture delay).
        let pos = f.beat_position(now) + s.latency_ms as f64 / 1000.0 * f.bpm as f64 / 60.0;
        let ev = dir.update(&f, pos, dt, &usable, &s);

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
        if (ev.cut || ev.phrase) && s.mode != Mode::Manual && dancer.enabled {
            let intensity = dir.intensity;
            dancer.on_cut(intensity, || dir.rand(), s.dancer_style, s.canon, &s.disabled_clips);
        }
        let dancer_u = dancer.uniforms(pos, f.downbeat, f.bpm, dt, s.dancer_size, &s.disabled_clips);
        if ev.cut && s.fx_auto {
            let seed = dir.rand();
            fx_current = Fx::random(seed, fx_current);
        }

        // Tempo changes ease in; position only ever moves forward smoothly.
        flow_bpm += (f.bpm - flow_bpm) * (dt * 1.5).min(1.0);
        flow = (flow + dt as f64 * flow_bpm as f64 / 60.0) % 4096.0;
        let target = if blackout { 0.0 } else { 1.0 };
        master += (target - master) * (dt * 3.0).min(1.0);

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
            onset: f.onset,
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
            master,
            fx: if s.fx_auto { fx_current } else { s.fx }.index(),
            fx_amt: s.fx_amt,
            spectrum,
            waveform,
        };
        if let Err(e) = r.render(dir.scene, &u, dancer_u.as_ref()) {
            eprintln!("render error: {e}");
        }

        if now - last_status > Duration::from_millis(50) {
            last_status = now;
            *lock(&shared.status) = Status {
                bpm: f.bpm,
                confidence: f.tempo_confidence,
                beat_in_bar,
                silent: f.silent,
                fps,
                device: audio.device_name.clone(),
                scene: dir.scene,
                clip: dancer.loaded_name(),
                blackout,
                // Filled in by the event thread — it owns the window state.
                fullscreen: false,
                fx: if s.fx_auto { fx_current } else { s.fx },
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
        Action::MarkDownbeat => {
            let _ = audio.commands.send(Command::MarkDownbeat);
        }
        Action::LatencyDown => s.latency_ms -= 5.0,
        Action::LatencyUp => s.latency_ms += 5.0,
        // Cycling the effect by hand turns auto off: the key always shows what it does.
        Action::CycleFx => {
            s.fx_auto = false;
            s.fx = s.fx.next();
        }
        Action::ReloadShaders => r.reload_shaders(true),
        // Handled on the event thread (windows / timeline transport).
        Action::Fullscreen
        | Action::LeaveFullscreen
        | Action::TogglePanel
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
    last_title: Instant,
    /// Debounce for the panel toggle — autorepeat and focus churn can both
    /// re-fire it within the same press.
    last_panel_toggle: Instant,
    /// Title-bar/taskbar icon, decoded once from the bundled PNG.
    icon: Option<Icon>,
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
        let mut beat = doc.beat_at(tl.pos_s).max(0.0);
        if tl.snap {
            beat = (beat * 4.0).round() / 4.0;
        }
        doc.cues.push(Cue { beat, kind });
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
            Action::TimelinePlay => self.send(Msg::Transport(SongCtl::Toggle)),
            Action::TimelineRecord => {
                if let Some(sh) = &self.shared {
                    let mut tl = lock(&sh.timeline);
                    tl.recording = !tl.recording;
                }
            }
            _ => self.send(Msg::Act(action)),
        }
        self.record_action(action);
        self.mark_dirty();
    }

    /// The live settings: the shared copy once rendering is running.
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

    fn key(&mut self, event_loop: &ActiveEventLoop, key: &Key) {
        let Some(name) = key_name(key) else { return };
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
            return;
        }
        let action = self.shared.as_ref().and_then(|sh| lock(&sh.settings).action_for(&name));
        if let Some(action) = action {
            self.apply(action, event_loop);
        }
    }

    fn draw_panel(&mut self, event_loop: &ActiveEventLoop) {
        let Some(shared) = self.shared.clone() else { return };
        let mut status = lock(&shared.status).clone();
        if let Some(w) = &self.window {
            status.fullscreen = w.fullscreen().is_some();
        }
        // The UI runs under the settings lock; the GPU acquire/present must
        // not — a blocked surface acquire would freeze the render thread
        // through the lock.
        let (commands, changed, frame) = {
            let mut s = lock(&shared.settings);
            let Some(p) = self.panel.as_mut() else { return };
            if p.window.is_visible() == Some(false) {
                return;
            }
            p.run_ui(&mut s, &status, &shared.scene_names, &shared.clip_names, &shared.timeline)
        };
        if let Some(p) = self.panel.as_mut() {
            p.present(frame);
        }
        if changed {
            self.mark_dirty();
        }
        for c in commands {
            match c {
                UiCommand::Do(a) => self.apply(a, event_loop),
                UiCommand::GoToScene(i) => {
                    if let Some(name) = shared.scene_names.get(i) {
                        self.record_cue(CueKind::Scene(name.clone()));
                    }
                    self.send(Msg::GoToScene(i));
                }
                UiCommand::ShowClip(i) => {
                    if let Some(name) = shared.clip_names.get(i) {
                        self.record_cue(CueKind::Clip(name.clone()));
                    }
                    self.send(Msg::ShowClip(i));
                }
                UiCommand::LoadSong(p) => self.send(Msg::LoadSong(p)),
                UiCommand::LoadTimeline(p) => self.send(Msg::LoadTimeline(p)),
                UiCommand::Song(ctl) => self.send(Msg::Transport(ctl)),
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
    }
}

fn toggle_fullscreen(w: &Window) {
    if w.fullscreen().is_some() {
        w.set_fullscreen(None);
    } else {
        w.set_fullscreen(Some(Fullscreen::Borderless(w.current_monitor())));
    }
}

impl ApplicationHandler for App {
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
        let shared = Arc::new(Shared {
            settings: Mutex::new(self.settings.clone()),
            status: Mutex::new(Status::default()),
            timeline: timeline::TimelineState::new(),
            env: self.env.clone(),
            dirty: AtomicBool::new(false),
            quit: AtomicBool::new(false),
            scene_names: r.scene_names(),
            clip_names: self.dancer.as_ref().map(|d| d.clip_names()).unwrap_or_default(),
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
        if let Some(p) = self.start_song.take() {
            self.send(Msg::LoadSong(p));
        }
        if self.settings.show_panel && !self.no_panel {
            self.open_panel(event_loop);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let is_panel = self.panel.as_ref().is_some_and(|p| p.window.id() == id);
        if is_panel {
            match &event {
                WindowEvent::CloseRequested => {
                    // Hide rather than destroy — reopening is then instant and
                    // can't fail partway through.
                    if let Some(p) = &self.panel {
                        p.window.set_visible(false);
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
                        // Only text-producing keys belong to a focused text
                        // field; function/arrow keys stay global hotkeys, so
                        // F1 toggles even while typing in the scene filter.
                        let text_like = matches!(event.logical_key, Key::Character(_));
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
                    self.key(event_loop, &event.logical_key)
                }
            }
            // The render thread draws continuously; nothing to do here.
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // If the render thread died, don't sit here ignoring input — exit.
        if self.shared.as_ref().is_some_and(|sh| sh.quit.load(Ordering::Relaxed)) {
            event_loop.exit();
            return;
        }
        // Title refresh — Win32 window calls stay on the thread that owns the
        // window; the render thread only writes status.
        if self.last_title.elapsed() > Duration::from_millis(500) {
            self.last_title = Instant::now();
            if let (Some(w), Some(sh)) = (&self.window, &self.shared) {
                let st = lock(&sh.status);
                let name = sh.scene_names.get(st.scene).map(String::as_str).unwrap_or("?");
                let mode = lock(&sh.settings).mode;
                w.set_title(&format!("Trippin — {name} — {:.1} BPM — {mode:?}", st.bpm));
            }
        }
        // The visuals render on their own thread; the panel needs ~10 fps.
        if let Some(p) = &self.panel {
            if p.window.is_visible() != Some(false) && self.last_panel_draw.elapsed() > Duration::from_millis(100) {
                p.window.request_redraw();
            }
        }
        // Settings changed on the render thread also need flushing to disk.
        if let Some(sh) = &self.shared {
            if sh.dirty.load(Ordering::Relaxed) && self.dirty_since.is_none() {
                self.dirty_since = Some(Instant::now());
            }
        }
        if let Some(t) = self.dirty_since {
            if t.elapsed() > Duration::from_secs(1) {
                self.dirty_since = None;
                if !self.no_save {
                    if let Some(sh) = &self.shared {
                        lock(&sh.settings).save();
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
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

/// Window/taskbar icon, decoded once from the PNG embedded in the exe.
fn load_icon() -> Option<Icon> {
    let img = image::load_from_memory(include_bytes!("../logo.png")).ok()?.into_rgba8();
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

fn main() -> Result<()> {
    // Panics on the render thread must be visible — a silent death leaves the
    // app looking alive while ignoring every command.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        eprintln!("panic on thread {:?}: {info}", thread.name());
        hook(info);
    }));
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--list-devices") {
        return audio::list_devices();
    }
    if args.iter().any(|a| a == "--check-shaders") {
        return check_shaders();
    }
    let device = arg_value(&args, "--device");
    let mic = args.iter().any(|a| a == "--mic");
    let env = audio::EnvLog::new();
    let audio = AudioEngine::start(device, mic, Some(env.clone()))?;
    println!("Audio: {}", audio.device_name);
    let start_song = arg_value(&args, "--song").map(std::path::PathBuf::from);

    let mut settings = Settings::load();
    let mut no_save = false;
    if args.iter().any(|a| a == "--no-dancer") {
        settings.dancer_enabled = false;
        no_save = true;
    }
    if let Some(i) = args.iter().position(|a| a == "--dancer") {
        settings.dancer_enabled = true;
        if let Some(style) = args.get(i + 1).and_then(|s| dancer::STYLES.iter().position(|n| n == s)) {
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
        let first = dancer.clips.iter().position(|c| !settings.disabled_clips.contains(&c.name)).unwrap_or(0);
        dancer.request(first);
    }

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        audio: Some(audio),
        window: None,
        gpu: None,
        panel: None,
        director: Some(Director::new()),
        dancer: Some(dancer),
        settings,
        shared: None,
        render_tx: None,
        audio_cfg: (device.map(str::to_string), mic),
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
        last_title: Instant::now(),
        last_panel_toggle: Instant::now() - Duration::from_secs(1),
        icon: load_icon(),
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}
