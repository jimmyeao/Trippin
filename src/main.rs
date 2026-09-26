//! Trippin — live music-reactive visuals for DJ sets.
//!
//! Usage: trippin [--list-devices] [--device "<name part>"] [--mic] [--scene <name>]
//!                [--dancer [style]] [--no-dancer] [--canon] [--no-panel]
//!                [--gpu low] [--scale 0.75] [--fullscreen] [--vsync]
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
#[cfg(target_os = "macos")]
mod sysaudio;

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

/// State shared between the event thread and the render thread.
struct Shared {
    /// Panel and hotkeys write; the render loop reads a copy each frame.
    settings: Mutex<Settings>,
    /// Live status for the panel, written by the render thread.
    status: Mutex<Status>,
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

/// The whole show, free-running on its own thread at vsync pace.
fn render_loop(
    mut r: Renderer,
    mut dir: Director,
    mut dancer: DancerLayer,
    audio: AudioEngine,
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
        // Handled on the event thread (it owns the windows).
        Action::Fullscreen | Action::LeaveFullscreen | Action::TogglePanel => {}
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
            _ => self.send(Msg::Act(action)),
        }
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
            p.run_ui(&mut s, &status, &shared.scene_names, &shared.clip_names)
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
                UiCommand::GoToScene(i) => self.send(Msg::GoToScene(i)),
                UiCommand::ShowClip(i) => self.send(Msg::ShowClip(i)),
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
        let sh = shared.clone();
        std::thread::Builder::new()
            .name("render".into())
            .spawn(move || {
                // A render panic must not leave a zombie: dead visuals with a
                // live UI is the worst failure mode mid-set. Log it (the panic
                // hook prints details) and tell the event loop to exit.
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    render_loop(r, dir, dancer, audio, sh.clone(), rx)
                }))
                .is_err()
                {
                    sh.quit.store(true, Ordering::Relaxed);
                }
            })
            .expect("spawn render thread");

        self.window = Some(window);
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
    let audio = AudioEngine::start(device, mic)?;
    println!("Audio: {}", audio.device_name);

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
