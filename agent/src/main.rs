//! Trippin agent — a tray app that lives on the DJ machine. It runs the
//! same now-playing detection as the main app (Serato, rekordbox,
//! VirtualDJ, Mixxx, the OS media session) and pushes the track to a
//! Trippin rig over the LAN remote protocol, so the rig on another
//! machine can render + output while this one just plays music.
//!
//! Config: `<data dir>/Trippin/agent.json` — {"host": "ip:port" (empty =
//! first rig found over Bonjour), "pin": "1234", "file": ""}.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod link;
#[allow(dead_code)] // the shared module has more than the agent needs
#[path = "../../src/nowplaying.rs"]
mod np;

use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

/// Saved settings. `host` empty = auto-discover; `file` is the same
/// text-file source the app has, for DJ software not listed.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Cfg {
    pub host: String,
    pub pin: String,
    pub file: String,
}

#[cfg(not(windows))]
fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn data_dir() -> PathBuf {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    #[cfg(target_os = "macos")]
    let base = home()
        .map(|h| h.join("Library/Application Support"))
        .unwrap_or_else(|| PathBuf::from("."));
    #[cfg(not(any(windows, target_os = "macos")))]
    let base = home()
        .map(|h| h.join(".config"))
        .unwrap_or_else(|| PathBuf::from("."));
    let d = base.join("Trippin");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn load_cfg(path: &PathBuf) -> Cfg {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_cfg(path: &PathBuf, cfg: &Cfg) {
    if let Ok(s) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(path, s);
    }
}

fn main() {
    let dir = data_dir();
    let cfg_path = dir.join("agent.json");
    let fresh = !cfg_path.is_file();
    let cfg = Arc::new(Mutex::new(load_cfg(&cfg_path)));
    let cfg_serial = Arc::new(AtomicU64::new(0));

    // Now-playing detection on this machine — the same worker the app
    // runs, polling Serato/rekordbox/media session/etc.
    let np_cfg = Arc::new(Mutex::new(np::NpConfig {
        source: np::NpSource::Auto,
        delay_s: 0.0, // the receiving rig applies its own update delay
        file: cfg.lock().unwrap().file.clone(),
    }));
    let np_state = np::start(
        np_cfg.clone(),
        dir.join("nowplaying.txt"),
        np::RemoteNp::default(),
    );

    let link = link::SharedLink::default();
    link::spawn(cfg.clone(), cfg_serial.clone(), np_state.clone(), link.clone());

    ui::run(ui::Args {
        cfg,
        cfg_serial,
        cfg_path,
        np_cfg,
        np_state,
        link,
        show: fresh,
    });
}

mod ui {
    //! Tray icon + the small settings window (egui, like the app's panel).

    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    use eframe::egui;

    use super::{save_cfg, Cfg};
    use crate::link::SharedLink;
    use crate::np;

    pub struct Args {
        pub cfg: Arc<Mutex<Cfg>>,
        pub cfg_serial: Arc<AtomicU64>,
        pub cfg_path: PathBuf,
        pub np_cfg: Arc<Mutex<np::NpConfig>>,
        pub np_state: np::SharedNowPlaying,
        pub link: SharedLink,
        pub show: bool,
    }

    /// A simple glowing-disc tray glyph — no asset dependency.
    fn icon() -> tray_icon::Icon {
        const S: u32 = 64;
        let mut rgba = vec![0u8; (S * S * 4) as usize];
        for y in 0..S {
            for x in 0..S {
                let dx = x as f32 - S as f32 / 2.0;
                let dy = y as f32 - S as f32 / 2.0;
                let r = (dx * dx + dy * dy).sqrt();
                let px = ((y * S + x) * 4) as usize;
                if r < 30.0 {
                    let t = 1.0 - r / 30.0;
                    rgba[px] = (110.0 + 90.0 * t) as u8;
                    rgba[px + 1] = (40.0 + 60.0 * t) as u8;
                    rgba[px + 2] = (220.0 + 35.0 * t) as u8;
                    rgba[px + 3] = 255;
                }
                if r < 12.0 {
                    rgba[px..px + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
        tray_icon::Icon::from_rgba(rgba, S, S).expect("icon")
    }

    pub fn run(args: Args) {
        let start_visible = args.show;
        let opts = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Trippin agent")
                .with_inner_size([340.0, 200.0])
                .with_resizable(false)
                .with_visible(start_visible),
            ..Default::default()
        };
        let _ = eframe::run_native(
            "Trippin agent",
            opts,
            Box::new(move |cc| Ok(Box::new(Agent::new(cc, args)))),
        );
    }

    /// The tray icon and its menu ids. Built lazily on the first `logic`
    /// pass: on macOS `TrayIconBuilder` must run while the event loop is
    /// already pumping — inside `App::new` the status item never appears.
    struct Tray {
        _icon: tray_icon::TrayIcon,
        status_item: tray_icon::menu::MenuItem,
        open_id: tray_icon::menu::MenuId,
        quit_id: tray_icon::menu::MenuId,
    }

    impl Tray {
        fn new() -> tray_icon::Result<Self> {
            use tray_icon::menu::{Menu, MenuItem};
            let status_item = MenuItem::new("starting…", false, None);
            let open = MenuItem::new("Settings…", true, None);
            let quit = MenuItem::new("Quit", true, None);
            let (open_id, quit_id) = (open.id().clone(), quit.id().clone());
            let menu = Menu::new();
            let _ = menu.append(&MenuItem::new("Trippin agent", false, None));
            let _ = menu.append(&status_item);
            let _ = menu.append(&tray_icon::menu::PredefinedMenuItem::separator());
            let _ = menu.append(&open);
            let _ = menu.append(&tray_icon::menu::PredefinedMenuItem::separator());
            let _ = menu.append(&quit);
            let icon = tray_icon::TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip("Trippin agent — now playing relay")
                .with_icon(icon())
                .build()?;
            Ok(Self {
                _icon: icon,
                status_item,
                open_id,
                quit_id,
            })
        }
    }

    struct Agent {
        args: Args,
        edit: Cfg,
        tray: Option<Tray>,
        first: bool,
    }

    impl Agent {
        fn new(_cc: &eframe::CreationContext, args: Args) -> Self {
            let edit = args.cfg.lock().unwrap().clone();
            Self {
                args,
                edit,
                tray: None,
                first: true,
            }
        }
    }

    impl eframe::App for Agent {
        /// Runs before each `ui` — and keeps running while the window is
        /// hidden (each `request_repaint_after` re-arms the next pass), so
        /// this is where the tray icon is built and its menu polled.
        fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            if self.first {
                self.first = false;
                // First pass = event loop running — safe for TrayIcon on
                // every platform. If the tray can't be built the window
                // becomes the only UI, so always show it then.
                match Tray::new() {
                    Ok(t) => self.tray = Some(t),
                    Err(e) => {
                        eprintln!("tray icon: {e}");
                        self.args.show = true;
                    }
                }
                // Also nudge the window up on first run: an unbundled
                // macOS exe can open off the active space.
                if self.args.show {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
            if let Some(tray) = &self.tray {
                while let Ok(ev) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                    if ev.id == tray.open_id {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    } else if ev.id == tray.quit_id {
                        // The socket dies with the process — the rig retracts
                        // the pushed track on disconnect.
                        std::process::exit(0);
                    }
                }
                // Keep the tray status line current.
                let status = self.args.link.lock().unwrap().status.clone();
                tray.status_item.set_text(format!(
                    "Status: {}",
                    if status.is_empty() { "starting…" } else { &status }
                ));
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }

        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let ctx = ui.ctx().clone();
            // Closing the window hides it; Quit lives on the tray menu.
            if ctx.input(|i| i.viewport().close_requested()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }

            egui::CentralPanel::default().show(ui, |ui| {
                ui.heading("Trippin agent");
                let status = self.args.link.lock().unwrap().status.clone();
                ui.label(if status.is_empty() { "starting…".into() } else { status });
                let heard = self
                    .args
                    .np_state
                    .lock()
                    .unwrap()
                    .track
                    .as_ref()
                    .map(|t| t.line())
                    .unwrap_or_else(|| "nothing detected".into());
                ui.label(format!("Now playing here: {heard}"));
                ui.separator();

                // Rig picker: Auto, each discovered rig, or a typed
                // host:port. The edit buffer mirrors `cfg` between saves.
                let rigs = self.args.link.lock().unwrap().rigs.clone();
                egui::ComboBox::from_label("Trippin rig")
                    .selected_text(if self.edit.host.is_empty() {
                        "Auto-discover".to_string()
                    } else {
                        self.edit.host.clone()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.edit.host, String::new(), "Auto-discover");
                        for (label, addr) in &rigs {
                            ui.selectable_value(
                                &mut self.edit.host,
                                addr.clone(),
                                format!("{label} — {addr}"),
                            );
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("or address:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.edit.host)
                            .hint_text("192.168.x.x:9138"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("PIN:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.edit.pin)
                            .password(true)
                            .desired_width(80.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Track file:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.edit.file)
                            .hint_text("optional, for other DJ tools"),
                    );
                });

                if ui.button("Save").clicked() {
                    *self.args.cfg.lock().unwrap() = self.edit.clone();
                    self.args.np_cfg.lock().unwrap().file = self.edit.file.clone();
                    save_cfg(&self.args.cfg_path, &self.edit);
                    // Tell the connector to drop and re-dial.
                    self.args.cfg_serial.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    }
}
