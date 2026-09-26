//! Trippin — live music-reactive visuals for DJ sets.
//!
//! Usage: trippin [--list-devices] [--device "<name part>"] [--scene <name>]
//!                [--dancer [style]] [--no-dancer] [--canon] [--no-panel]
//!                [--gpu low] [--scale 0.75] [--fullscreen]
//!
//! A control panel window opens alongside the visuals (F1 toggles it): modes,
//! scene playlist, dancer options, sync and rebindable hotkeys. Settings are
//! saved to trippin.json. Close the visuals window to quit; Esc only leaves
//! fullscreen, so a stray key can't end the show.

// Installer builds (`--features gui`) are a windowed app with no console;
// plain `cargo run` keeps the console for shader errors and logs.
#![cfg_attr(feature = "gui", windows_subsystem = "windows")]

mod audio;
mod config;
mod dancer;
mod director;
mod panel;
mod render;

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::Key;
use winit::window::{Fullscreen, Window, WindowId};

use audio::{AudioEngine, Command, Features};
use config::{in_season, key_name, today, Action, Mode, Seasonal, Settings, Tristate};
use dancer::DancerLayer;
use director::Director;
use panel::{Panel, Status, UiCommand};
use render::{Renderer, Uniforms};

struct App {
    audio: AudioEngine,
    renderer: Option<Renderer>,
    panel: Option<Panel>,
    director: Director,
    dancer: DancerLayer,
    settings: Settings,
    /// When settings last changed (saved a moment later).
    dirty_since: Option<Instant>,
    /// Command-line overrides are for this run only: don't write them to disk.
    no_save: bool,
    start_fullscreen: bool,
    start_scene: Option<String>,
    no_panel: bool,
    low_power: bool,
    render_scale: Option<f32>,
    fps: f32,
    started: Instant,
    last_frame: Instant,
    last_reload_check: Instant,
    last_title: Instant,
    last_panel_draw: Instant,
    /// Smooth beat clock (see `Uniforms::flow`) and the tempo driving it.
    flow: f64,
    flow_bpm: f32,
    blackout: bool,
    master: f32,
    features: Features,
    beat_pos: f64,
}

impl App {
    fn mark_dirty(&mut self) {
        self.dirty_since = Some(Instant::now());
    }

    /// Scenes that compile, are ticked in the playlist and (for seasonal
    /// scenes) are in season.
    fn usable_scenes(&self) -> Vec<usize> {
        let Some(r) = self.renderer.as_ref() else { return Vec::new() };
        let names = r.scene_names();
        let all = r.usable_scenes();
        let date = today();
        let on: Vec<usize> = all
            .iter()
            .copied()
            .filter(|&i| !self.settings.disabled_scenes.contains(&names[i]))
            .filter(|&i| match (self.settings.seasonal, in_season(&names[i], date)) {
                (_, None) | (Seasonal::Always, _) => true,
                (Seasonal::Off, Some(_)) => false,
                (Seasonal::Auto, Some(in_now)) => in_now,
            })
            .collect();
        if on.is_empty() {
            all
        } else {
            on
        }
    }

    fn frame(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        if dt > 0.0 {
            self.fps += (1.0 / dt - self.fps) * 0.05;
        }
        let usable = self.usable_scenes();
        let Some(r) = self.renderer.as_mut() else { return };

        if now - self.last_reload_check > Duration::from_millis(500) {
            self.last_reload_check = now;
            r.reload_shaders(false);
        }

        let f = self.audio.features.lock().map(|f| f.clone()).unwrap_or_default();
        // Positive latency shows the beat earlier (compensating capture delay).
        let pos = f.beat_position(now) + self.settings.latency_ms as f64 / 1000.0 * f.bpm as f64 / 60.0;
        let ev = self.director.update(&f, pos, dt, &usable, &self.settings);

        // Dancer follows the settings; auto-pilot changes it on cuts and phrases.
        let s = &self.settings;
        self.dancer.enabled = s.dancer_enabled;
        if let Some(style) = s.dancer_style {
            self.dancer.style = style;
        }
        match s.canon {
            Tristate::On => self.dancer.canon = true,
            Tristate::Off => self.dancer.canon = false,
            Tristate::Auto => {}
        }
        for (slot, clip) in self.dancer.poll_loaded() {
            r.set_dancer_clip(slot, &clip);
        }
        if (ev.cut || ev.phrase) && s.mode != Mode::Manual && self.dancer.enabled {
            let intensity = self.director.intensity;
            let director = &mut self.director;
            self.dancer.on_cut(intensity, || director.rand(), s.dancer_style, s.canon, &s.disabled_clips);
        }
        let dancer_u = self.dancer.uniforms(pos, f.downbeat, f.bpm, dt, s.dancer_size, &s.disabled_clips);

        // Tempo changes ease in; position only ever moves forward smoothly.
        self.flow_bpm += (f.bpm - self.flow_bpm) * (dt * 1.5).min(1.0);
        self.flow = (self.flow + dt as f64 * self.flow_bpm as f64 / 60.0) % 4096.0;
        let target = if self.blackout { 0.0 } else { 1.0 };
        self.master += (target - self.master) * (dt * 3.0).min(1.0);

        let (w, h) = r.size();
        let mut spectrum = [0.0; audio::SPECTRUM_BINS];
        spectrum.copy_from_slice(&f.spectrum);
        let beat_in_bar = f.beat_in_bar(pos);
        let d = &self.director;
        let u = Uniforms {
            time: (now - self.started).as_secs_f32(),
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
            scene_time: (now - d.scene_started).as_secs_f32(),
            intensity: d.intensity,
            hue: d.hue,
            seed: d.seed,
            flash: d.flash,
            flow: self.flow as f32,
            master: self.master,
            _pad: [0.0; 2],
            spectrum,
        };
        if let Err(e) = r.render(d.scene, &u, dancer_u.as_ref()) {
            eprintln!("render error: {e}");
        }

        if now - self.last_title > Duration::from_millis(500) {
            self.last_title = now;
            let names = r.scene_names();
            r.window.set_title(&format!(
                "Trippin — {} — {:.1} BPM — {:?}",
                names.get(d.scene).map(String::as_str).unwrap_or("?"),
                f.bpm,
                self.settings.mode
            ));
        }
        if let Some(t) = self.dirty_since {
            if now - t > Duration::from_secs(1) {
                self.dirty_since = None;
                if !self.no_save {
                    self.settings.save();
                }
            }
        }
        self.features = f;
        self.beat_pos = pos;
    }

    fn draw_panel(&mut self, event_loop: &ActiveEventLoop) {
        let Some(r) = self.renderer.as_ref() else { return };
        let scenes = r.scene_names();
        let clips = self.dancer.clip_names();
        let status = Status {
            bpm: self.features.bpm,
            confidence: self.features.tempo_confidence,
            beat_in_bar: self.features.beat_in_bar(self.beat_pos),
            silent: self.features.silent,
            fps: self.fps,
            device: self.audio.device_name.clone(),
            scene: self.director.scene,
            clip: self.dancer.loaded_name(),
            blackout: self.blackout,
            fullscreen: r.window.fullscreen().is_some(),
        };
        let Some(p) = self.panel.as_mut() else { return };
        self.last_panel_draw = Instant::now();
        let (commands, changed) = p.redraw(&mut self.settings, &status, &scenes, &clips);
        if changed {
            self.mark_dirty();
        }
        for c in commands {
            match c {
                UiCommand::Do(a) => self.apply(a, event_loop),
                UiCommand::GoToScene(i) => self.director.cut_to(i),
                UiCommand::ShowClip(i) => {
                    self.dancer.request(i);
                    self.dancer.showing = true;
                }
            }
        }
    }

    fn apply(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        let usable = self.usable_scenes();
        let s = &mut self.settings;
        match action {
            Action::NextScene => self.director.next_scene(&usable, s.random_order),
            Action::PrevScene => self.director.prev_scene(&usable),
            Action::ModeAuto => s.mode = Mode::Auto,
            Action::ModeStatic => s.mode = Mode::Static,
            Action::ModeManual => s.mode = Mode::Manual,
            Action::ToggleRandom => s.random_order = !s.random_order,
            Action::ToggleDancer => {
                s.dancer_enabled = !s.dancer_enabled;
                self.dancer.showing = true;
            }
            Action::NextClip => {
                self.dancer.next_clip();
                self.dancer.showing = true;
            }
            Action::NextStyle => s.dancer_style = Some((self.dancer.style + 1) % dancer::STYLES.len()),
            Action::CycleCanon => {
                s.canon = match s.canon {
                    Tristate::Auto => Tristate::On,
                    Tristate::On => Tristate::Off,
                    Tristate::Off => Tristate::Auto,
                }
            }
            Action::Blackout => self.blackout = !self.blackout,
            Action::Fullscreen => {
                if let Some(r) = &self.renderer {
                    toggle_fullscreen(&r.window);
                }
            }
            Action::LeaveFullscreen => {
                if let Some(r) = &self.renderer {
                    r.window.set_fullscreen(None);
                }
            }
            Action::MarkDownbeat => {
                let _ = self.audio.commands.send(Command::MarkDownbeat);
            }
            Action::LatencyDown => s.latency_ms -= 5.0,
            Action::LatencyUp => s.latency_ms += 5.0,
            Action::ReloadShaders => {
                if let Some(r) = self.renderer.as_mut() {
                    r.reload_shaders(true);
                }
            }
            Action::TogglePanel => {
                if self.panel.is_some() {
                    self.panel = None;
                    self.settings.show_panel = false;
                } else {
                    self.open_panel(event_loop);
                    self.settings.show_panel = true;
                }
            }
        }
        self.mark_dirty();
    }

    fn open_panel(&mut self, event_loop: &ActiveEventLoop) {
        let Some(r) = self.renderer.as_ref() else { return };
        match Panel::new(event_loop, r.gpu()) {
            Ok(p) => self.panel = Some(p),
            Err(e) => eprintln!("control panel failed to open: {e:#}"),
        }
    }

    fn key(&mut self, event_loop: &ActiveEventLoop, key: &Key) {
        let Some(name) = key_name(key) else { return };
        // Rebinding: the next key press becomes the action's key (Esc cancels).
        if let Some(action) = self.panel.as_mut().and_then(|p| p.rebinding.take()) {
            if name != "Escape" {
                for (_, k) in self.settings.keys.iter_mut().filter(|(_, k)| **k == name) {
                    k.clear(); // one key, one action
                }
                self.settings.keys.insert(action, name);
                self.mark_dirty();
            }
            if let Some(p) = &self.panel {
                p.window.request_redraw();
            }
            return;
        }
        if let Some(action) = self.settings.action_for(&name) {
            self.apply(action, event_loop);
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
        if self.renderer.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Trippin")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        if self.start_fullscreen {
            toggle_fullscreen(&window);
        }
        match pollster::block_on(Renderer::new(window, self.low_power, self.render_scale)) {
            Ok(r) => {
                println!("Scenes: {}", r.scene_names().join(", "));
                if let Some(name) = &self.start_scene {
                    match r.scene_names().iter().position(|n| n == name) {
                        Some(i) => {
                            self.director.cut_to(i);
                            self.settings.mode = Mode::Manual;
                        }
                        None => eprintln!("no scene named {name:?}"),
                    }
                }
                self.renderer = Some(r);
            }
            Err(e) => {
                eprintln!("renderer init failed: {e:#}");
                event_loop.exit();
                return;
            }
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
                    self.panel = None;
                    self.settings.show_panel = false;
                    self.mark_dirty();
                    return;
                }
                WindowEvent::RedrawRequested => {
                    self.draw_panel(event_loop);
                    return;
                }
                WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state: ElementState::Pressed, .. }, .. } => {
                    let rebinding = self.panel.as_ref().is_some_and(|p| p.rebinding.is_some());
                    let typing = self.panel.as_ref().is_some_and(|p| p.wants_keyboard());
                    if rebinding || !typing {
                        let key = logical_key.clone();
                        self.key(event_loop, &key);
                        return;
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
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state: ElementState::Pressed, .. }, .. } => {
                self.key(event_loop, &logical_key)
            }
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(r) = &self.renderer {
            r.window.request_redraw();
        }
        // The panel only needs ~10 fps for its live status.
        if let Some(p) = &self.panel {
            if self.last_panel_draw.elapsed() > Duration::from_millis(100) {
                p.window.request_redraw();
            }
        }
    }
}

fn arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--list-devices") {
        return audio::list_devices();
    }
    let device = arg_value(&args, "--device");
    let audio = AudioEngine::start(device)?;
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
    let now = Instant::now();
    let mut app = App {
        audio,
        renderer: None,
        panel: None,
        director: Director::new(),
        dancer,
        settings,
        dirty_since: None,
        no_save,
        start_fullscreen: args.iter().any(|a| a == "--fullscreen"),
        start_scene,
        no_panel: args.iter().any(|a| a == "--no-panel"),
        low_power: arg_value(&args, "--gpu") == Some("low"),
        render_scale: arg_value(&args, "--scale").and_then(|s| s.parse().ok()),
        fps: 0.0,
        started: now,
        last_frame: now,
        last_reload_check: now,
        last_title: now,
        last_panel_draw: now,
        flow: 0.0,
        flow_bpm: 120.0,
        blackout: false,
        master: 1.0,
        features: Features::default(),
        beat_pos: 0.0,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}
