//! The timeline editor — a dedicated, resizable Clipchamp-style window:
//! a palette of scenes / routines / effects to drag onto the strip, song
//! clip regions on a shared ruler, a cue lane, and a live playhead.
//! Clicks on palette items preview them on the visuals window; the doc
//! itself lives in `timeline::Shared`, so edits show up instantly.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Icon, Window};

use crate::config::Fx;
use crate::dancer::STYLES;
use crate::egui_win::{EguiWin, Frame};
use crate::panel::{UiCommand, cue_color, cue_param_ui, fmt_time};
use crate::render::Gpu;
use crate::timeline::{Cue, CueKind, PlayMode, SongCtl, Timeline, TimelineState};

pub struct Editor {
    /// Clone of `win.window` for event matching.
    pub window: Arc<Window>,
    win: EguiWin,
    /// Zoom — pixels per second on the strip.
    px_per_s: f64,
    /// Left edge of the strip, in timeline seconds.
    scroll_s: f64,
    /// Selected cue index into `doc.cues`.
    sel_cue: Option<usize>,
    /// Selected clip index into `doc.clips`.
    sel_clip: Option<usize>,
    /// Cue being dragged/trimmed: (index, mode, grab offset).
    cue_drag: Option<(usize, CueDrag, f64)>,
    /// Clip being dragged: (index, grab offset in global seconds).
    clip_drag: Option<(usize, f64)>,
    /// File-picked audio or timeline path, written by a dialog thread.
    file_pick: Arc<Mutex<Option<PathBuf>>>,
    /// Set by the toolbar's "fit" button; consumed by the canvas.
    zoom_fit: bool,
    /// Scrubbing the playhead by dragging on the ruler.
    scrub: bool,
    /// egui textures for cue-block thumbnails, keyed by scene/clip name.
    thumbs: HashMap<String, egui::TextureHandle>,
    /// Names already asked of the render thread or the disk loader.
    want_thumbs: HashSet<String>,
    /// Grab offset inside the scrollbar thumb while it is being dragged.
    sb_grab: Option<f32>,
    /// The text being composed in the palette — dragged on as a Text cue.
    text_draft: crate::text::TextSpec,
    /// AI show-builder dialog open.
    ai_open: bool,
    /// Clear existing cues before applying the generated show.
    ai_replace: bool,
    /// Shared state with the AI worker thread (analysis + API call).
    ai_job: Arc<Mutex<AiJob>>,
}

/// AI show-builder state shared with its worker thread.
struct AiJob {
    busy: bool,
    status: String,
    /// Finished build: cues + a summary note, or the error message.
    result: Option<Result<(Vec<Cue>, String), String>>,
}

/// What part of a cue block is being dragged.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CueDrag {
    Move,
    TrimStart,
    TrimEnd,
}

impl Editor {
    pub fn new(
        event_loop: &ActiveEventLoop,
        gpu: Gpu,
        icon: Option<Icon>,
        anchor: Option<&Window>,
    ) -> anyhow::Result<Self> {
        let win = EguiWin::new(
            event_loop,
            gpu,
            icon,
            "Trippin — timeline editor",
            PhysicalSize::new(1180, 640),
            anchor,
            false,
        )?;
        let window = win.window.clone();
        Ok(Self {
            window,
            win,
            px_per_s: 60.0,
            scroll_s: 0.0,
            sel_cue: None,
            sel_clip: None,
            cue_drag: None,
            clip_drag: None,
            file_pick: Arc::new(Mutex::new(None)),
            zoom_fit: false,
            scrub: false,
            thumbs: HashMap::new(),
            want_thumbs: HashSet::new(),
            sb_grab: None,
            text_draft: crate::text::TextSpec {
                text: "TRIPPIN".into(),
                ..Default::default()
            },
            ai_open: false,
            ai_replace: true,
            ai_job: Arc::new(Mutex::new(AiJob {
                busy: false,
                status: String::new(),
                result: None,
            })),
        })
    }

    pub fn on_event(&mut self, event: &WindowEvent) -> bool {
        self.win.on_event(event)
    }

    pub fn wants_keyboard(&self) -> bool {
        self.win.wants_keyboard()
    }

    pub fn present(&mut self, frame: Frame) {
        self.win.present(frame);
    }

    /// Build the editor UI. Mutates the timeline doc directly under its lock;
    /// anything that must happen on the render/event thread comes back as a
    /// `UiCommand`.
    pub fn run_ui(
        &mut self,
        scenes: &[String],
        routines: &[String],
        tl_shared: &crate::timeline::Shared,
        settings: &Mutex<crate::config::Settings>,
        settings_dirty: &std::sync::atomic::AtomicBool,
        new_thumbs: &[(String, u32, u32, Vec<u8>)],
    ) -> (Vec<UiCommand>, Frame) {
        let mut cmd = Vec::new();

        // Scene thumbnails rendered on the render thread land here.
        for (name, w, h, px) in new_thumbs {
            let img = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], px);
            let tex = self.win.ctx.load_texture(
                format!("thumb:{name}"),
                img,
                egui::TextureOptions::LINEAR,
            );
            self.thumbs.insert(name.clone(), tex);
        }

        // A file picked in the native dialog lands here.
        if let Some(p) = self
            .file_pick
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            eprintln!("Editor: picked {}", p.display());
            if crate::song::is_audio_file(&p) {
                cmd.push(UiCommand::AddSong(p));
            } else if p.extension().and_then(|e| e.to_str()) == Some("json") {
                cmd.push(UiCommand::LoadTimeline(p));
            }
        }

        let mut guard = tl_shared.lock().unwrap_or_else(|e| e.into_inner());
        let TimelineState {
            doc: doc_opt,
            mode,
            pos_s,
            cursor_s,
            recording,
            autosync,
            live_locked,
            live_score: _,
            dirty,
            snap,
            message,
            busy,
        } = &mut *guard;

        let (sel_cue, sel_clip, cue_drag, clip_drag) = (
            &mut self.sel_cue,
            &mut self.sel_clip,
            &mut self.cue_drag,
            &mut self.clip_drag,
        );
        let (pps, scroll) = (&mut self.px_per_s, &mut self.scroll_s);
        let (zoom_fit, scrub, sb_grab) = (&mut self.zoom_fit, &mut self.scrub, &mut self.sb_grab);
        let (thumbs, want_thumbs) = (&mut self.thumbs, &mut self.want_thumbs);
        let text_draft = &mut self.text_draft;
        let (ai_open, ai_replace, ai_job) = (&mut self.ai_open, &mut self.ai_replace, &self.ai_job);
        let file_pick = &self.file_pick;

        let frame = self.win.frame(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);

            // --- Toolbar ----------------------------------------------------
            egui::Panel::top("ed_tool").show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button("Open…")
                        .on_hover_text("audio or a saved timeline .json")
                        .clicked()
                    {
                        let slot = file_pick.clone();
                        std::thread::spawn(move || {
                            let p = rfd::FileDialog::new()
                                .add_filter(
                                    "audio / timeline",
                                    &[
                                        "mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "aiff",
                                        "json",
                                    ],
                                )
                                .pick_file();
                            eprintln!("Open dialog -> {p:?}");
                            *slot.lock().unwrap_or_else(|e| e.into_inner()) = p;
                        });
                    }
                    if ui
                        .add_enabled(!*busy, egui::Button::new("Add song…"))
                        .on_hover_text("append another song to this timeline")
                        .clicked()
                    {
                        let slot = file_pick.clone();
                        std::thread::spawn(move || {
                            let p = rfd::FileDialog::new()
                                .add_filter(
                                    "audio",
                                    &["mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "aiff"],
                                )
                                .pick_file();
                            eprintln!("Add-song dialog -> {p:?}");
                            *slot.lock().unwrap_or_else(|e| e.into_inner()) = p;
                        });
                    }
                    ui.separator();
                    if let Some(doc) = doc_opt.as_mut() {
                        ui.label("name");
                        if ui
                            .add(egui::TextEdit::singleline(&mut doc.name).desired_width(110.0))
                            .changed()
                        {
                            *dirty = true;
                        }
                        if ui
                            .button("Save")
                            .on_hover_text("timelines/<name>.json")
                            .clicked()
                        {
                            cmd.push(UiCommand::SaveTimeline);
                        }
                        if ui
                            .button("✦ AI show…")
                            .on_hover_text("analyse the tracks and have an LLM write the cue list")
                            .clicked()
                        {
                            *ai_open = !*ai_open;
                        }
                        ui.separator();
                    }
                    let play = if *mode == PlayMode::Playing {
                        "⏸ Pause"
                    } else {
                        "▶ Play"
                    };
                    if ui
                        .add_enabled(doc_opt.is_some() && !*busy, egui::Button::new(play))
                        .clicked()
                    {
                        cmd.push(UiCommand::Song(SongCtl::Toggle));
                    }
                    if ui
                        .add_enabled(*mode != PlayMode::Stopped, egui::Button::new("⏹"))
                        .clicked()
                    {
                        cmd.push(UiCommand::Song(SongCtl::Stop));
                    }
                    ui.label(fmt_time(*pos_s));
                    ui.separator();
                    let rec = ui.selectable_label(*recording, "● REC");
                    if rec.clicked() {
                        *recording = !*recording;
                    }
                    ui.checkbox(snap, "snap ¼");
                    ui.checkbox(autosync, "follow live");
                    if *autosync {
                        ui.colored_label(
                            if *live_locked {
                                egui::Color32::from_rgb(90, 220, 120)
                            } else {
                                egui::Color32::GRAY
                            },
                            if *live_locked { "locked" } else { "listening" },
                        );
                    }
                    ui.separator();
                    // Zoom: buttons + slider + fit (Ctrl+scroll zooms too).
                    if ui.small_button("−").clicked() {
                        *pps = (*pps / 1.4).clamp(4.0, 600.0);
                    }
                    let mut z = *pps;
                    if ui
                        .add_sized(
                            egui::vec2(70.0, 18.0),
                            egui::Slider::new(&mut z, 4.0..=600.0)
                                .logarithmic(true)
                                .show_value(false),
                        )
                        .changed()
                    {
                        *pps = z;
                    }
                    if ui.small_button("+").clicked() {
                        *pps = (*pps * 1.4).clamp(4.0, 600.0);
                    }
                    if ui
                        .small_button("fit")
                        .on_hover_text("zoom to fit the whole timeline")
                        .clicked()
                    {
                        *zoom_fit = true;
                    }
                    if *busy {
                        ui.spinner();
                        ui.small("decoding…");
                    }
                    if !message.is_empty() {
                        let failed = message.starts_with("load failed")
                            || message.starts_with("save failed")
                            || message.starts_with("open failed");
                        let col = if failed {
                            egui::Color32::from_rgb(255, 110, 110)
                        } else {
                            ui.visuals().weak_text_color()
                        };
                        ui.small(egui::RichText::new(message.as_str()).color(col));
                    }
                });
            });

            // --- AI show builder ------------------------------------------
            // Poll the worker: apply finished cues into the doc.
            if let Some(res) = ai_job
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .result
                .take()
            {
                match res {
                    Ok((cues, note)) => {
                        if let Some(doc) = doc_opt.as_mut() {
                            if *ai_replace {
                                doc.cues.clear();
                            }
                            doc.cues.extend(cues);
                            doc.cues.sort_by(|a, b| {
                                a.clip.cmp(&b.clip).then_with(|| a.beat.total_cmp(&b.beat))
                            });
                            *dirty = true;
                        }
                        *message = format!("AI: {note}");
                    }
                    Err(e) => *message = format!("AI failed: {e}"),
                }
            }
            if *ai_open {
                let mut open = true;
                let mut s_dirty = false;
                egui::Window::new("✦ AI show builder")
                    .open(&mut open)
                    .collapsible(false)
                    .default_width(430.0)
                    .show(ui, |ui| {
                        let mut s = settings.lock().unwrap_or_else(|e| e.into_inner());
                        ui.horizontal(|ui| {
                            ui.label("provider");
                            egui::ComboBox::from_id_salt("ai_prov")
                                .selected_text(s.ai_provider.label())
                                .show_ui(ui, |ui| {
                                    for p in crate::ai::AiProvider::ALL {
                                        if ui
                                            .selectable_label(s.ai_provider == p, p.label())
                                            .clicked()
                                        {
                                            s_dirty = true;
                                            // Reset fields that still hold another
                                            // provider's defaults.
                                            if crate::ai::AiProvider::ALL
                                                .iter()
                                                .any(|o| s.ai_endpoint == o.default_endpoint())
                                            {
                                                s.ai_endpoint.clear();
                                            }
                                            if crate::ai::AiProvider::ALL.iter().any(|o| {
                                                !o.default_model().is_empty()
                                                    && s.ai_model == o.default_model()
                                            }) {
                                                s.ai_model.clear();
                                            }
                                            s.ai_provider = p;
                                        }
                                    }
                                });
                        });
                        let def_ep = s.ai_provider.default_endpoint();
                        ui.horizontal(|ui| {
                            ui.label("endpoint ");
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut s.ai_endpoint)
                                        .hint_text(def_ep)
                                        .desired_width(330.0),
                                )
                                .changed()
                            {
                                s_dirty = true;
                            }
                        });
                        let def_model = s.ai_provider.default_model();
                        ui.horizontal(|ui| {
                            ui.label("model    ");
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut s.ai_model)
                                        .hint_text(def_model)
                                        .desired_width(200.0),
                                )
                                .changed()
                            {
                                s_dirty = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("API key  ");
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut s.ai_key)
                                        .password(true)
                                        .hint_text("or env var")
                                        .desired_width(240.0),
                                )
                                .changed()
                            {
                                s_dirty = true;
                            }
                        });
                        ui.small(format!(
                            "blank key tries {} — saved to trippin.json (gitignored)",
                            s.ai_provider.env_keys().join(" / ")
                        ));
                        ui.checkbox(ai_replace, "replace existing cues");
                        let busy = ai_job.lock().unwrap_or_else(|e| e.into_inner()).busy;
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    !busy && doc_opt.is_some(),
                                    egui::Button::new("Build cues"),
                                )
                                .on_hover_text(
                                    "analyse the tracks locally, then the model\n\
                                     designs scene/dancer/fx/text cues per section",
                                )
                                .clicked()
                            {
                                let clips = doc_opt.as_ref().unwrap().clips.clone();
                                let scenes = scenes.to_vec();
                                let routines = routines.to_vec();
                                let conf = crate::ai::AiConf::from_settings(&s);
                                let job = ai_job.clone();
                                {
                                    let mut j = job.lock().unwrap_or_else(|e| e.into_inner());
                                    j.busy = true;
                                    j.status = "analysing…".into();
                                    j.result = None;
                                }
                                std::thread::spawn(move || {
                                    let r =
                                        crate::ai::build_show(&clips, &scenes, &routines, &conf)
                                            .map_err(|e| format!("{e:#}"));
                                    let mut j = job.lock().unwrap_or_else(|e| e.into_inner());
                                    j.busy = false;
                                    j.status = match &r {
                                        Ok((_, n)) => n.clone(),
                                        Err(e) => e.clone(),
                                    };
                                    j.result = Some(r);
                                });
                            }
                            if busy {
                                ui.spinner();
                            }
                            let st = ai_job
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .status
                                .clone();
                            if !st.is_empty() {
                                ui.small(&st);
                            }
                        });
                    });
                *ai_open = open;
                if s_dirty {
                    settings_dirty.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }

            // --- Palette ----------------------------------------------------
            egui::Panel::left("ed_pal")
                .resizable(false)
                .exact_size(172.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Drag onto timeline");
                        ui.small("click = preview");
                        ui.separator();
                        ui.label(egui::RichText::new("Scenes").strong());
                        for name in scenes {
                            let kind = CueKind::Scene(name.clone());
                            if palette_item(ui, name, &kind) {
                                cmd.push(UiCommand::FireCue(kind));
                            }
                        }
                        ui.separator();
                        ui.label(egui::RichText::new("Dancer").strong());
                        for (lbl, kind) in [
                            ("dancer on", CueKind::Dancer(true)),
                            ("dancer off", CueKind::Dancer(false)),
                            ("next look", CueKind::NextLook),
                            ("auto look", CueKind::Look(None)),
                            ("canon", CueKind::Canon(crate::config::Tristate::On)),
                            ("canon auto", CueKind::Canon(crate::config::Tristate::Auto)),
                            ("canon off", CueKind::Canon(crate::config::Tristate::Off)),
                            ("trails on", CueKind::Trails(true)),
                            ("trails off", CueKind::Trails(false)),
                        ] {
                            if palette_item(ui, lbl, &kind) {
                                cmd.push(UiCommand::FireCue(kind));
                            }
                        }
                        for (i, n) in STYLES.iter().enumerate() {
                            let kind = CueKind::Look(Some(i));
                            if palette_item(ui, n, &kind) {
                                cmd.push(UiCommand::FireCue(kind));
                            }
                        }
                        for name in routines {
                            let kind = CueKind::Clip(name.clone());
                            if palette_item(ui, name, &kind) {
                                cmd.push(UiCommand::FireCue(kind));
                            }
                        }
                        ui.separator();
                        ui.label(egui::RichText::new("Effects").strong());
                        for f in Fx::ALL {
                            let kind = CueKind::Fx(f);
                            if palette_item(ui, f.label(), &kind) {
                                cmd.push(UiCommand::FireCue(kind));
                            }
                        }
                        let kind = CueKind::FxAuto(true);
                        if palette_item(ui, "auto FX", &kind) {
                            cmd.push(UiCommand::FireCue(kind));
                        }
                        let kind = CueKind::FxAuto(false);
                        if palette_item(ui, "auto FX off", &kind) {
                            cmd.push(UiCommand::FireCue(kind));
                        }
                        ui.separator();
                        ui.label(egui::RichText::new("Actions").strong());
                        for (lbl, kind) in [
                            ("next scene", CueKind::NextScene),
                            ("prev scene", CueKind::PrevScene),
                            ("auto mode", CueKind::Mode(crate::config::Mode::Auto)),
                            ("static mode", CueKind::Mode(crate::config::Mode::Static)),
                            ("manual mode", CueKind::Mode(crate::config::Mode::Manual)),
                            ("blackout", CueKind::Blackout(true)),
                        ] {
                            if palette_item(ui, lbl, &kind) {
                                cmd.push(UiCommand::FireCue(kind));
                            }
                        }
                        ui.separator();
                        ui.label(egui::RichText::new("Text").strong());
                        ui.add(
                            egui::TextEdit::singleline(&mut text_draft.text)
                                .desired_width(ui.available_width() - 8.0)
                                .hint_text("say something…"),
                        );
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("text_style")
                                .width(76.0)
                                .selected_text(text_draft.style.label())
                                .show_ui(ui, |ui| {
                                    for v in crate::text::TextStyle::ALL {
                                        ui.selectable_value(&mut text_draft.style, v, v.label());
                                    }
                                });
                            for v in crate::text::TextPos::ALL {
                                ui.selectable_value(&mut text_draft.pos, v, v.label());
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.small("lane");
                            ui.selectable_value(&mut text_draft.lane, 0, "1");
                            ui.selectable_value(&mut text_draft.lane, 1, "2");
                        });
                        if !text_draft.text.trim().is_empty() {
                            let kind = CueKind::Text(text_draft.clone());
                            if palette_item(ui, &format!("“{}”", text_draft.text), &kind) {
                                cmd.push(UiCommand::FireCue(kind));
                            }
                        }
                    });
                });

            // --- Inspector ----------------------------------------------------
            egui::Panel::bottom("ed_insp").show(ui, |ui| {
                inspector(
                    ui, doc_opt, scenes, routines, *sel_cue, *sel_clip, *cursor_s, *snap, dirty,
                    &mut cmd,
                );
            });

            // --- Timeline canvas ----------------------------------------------
            egui::CentralPanel::default().show(ui, |ui| {
                canvas(
                    ui,
                    doc_opt,
                    *mode,
                    *pos_s,
                    cursor_s,
                    *recording,
                    *snap,
                    pps,
                    scroll,
                    sel_cue,
                    sel_clip,
                    cue_drag,
                    clip_drag,
                    zoom_fit,
                    scrub,
                    sb_grab,
                    dirty,
                    thumbs,
                    want_thumbs,
                    &mut cmd,
                );
            });
        });
        (cmd, frame)
    }
}

/// A palette entry: drag onto the cue lane (drag ghost + payload via
/// `dnd_drag_source`), click = preview. The drag source only senses drags,
/// so the click is detected manually: press started inside, released inside,
/// and the drag never engaged.
fn palette_item(ui: &mut egui::Ui, label: &str, kind: &CueKind) -> bool {
    let id = egui::Id::new(("pal", label));
    let resp = ui
        .dnd_drag_source(id, kind.clone(), |ui| {
            ui.colored_label(cue_color(kind), label);
        })
        .response
        .on_hover_text("click to preview — drag onto the cue lane");
    let ctx = ui.ctx();
    let (released, origin) =
        ctx.input(|i| (i.pointer.primary_released(), i.pointer.press_origin()));
    released
        && resp.contains_pointer()
        && origin.is_some_and(|o| resp.rect.contains(o))
        && ctx.dragged_id().is_none()
        && ctx.drag_stopped_id() != Some(id)
}

// ---------------------------------------------------------------------------
// The strip: ruler + clip regions + cue lane + playhead.
// ---------------------------------------------------------------------------

const RULER_H: f32 = 20.0;
const CLIP_H: f32 = 56.0;
const TRACK_H: f32 = 22.0;
/// Resolve-style track header column at the left of the strip.
const GUTTER: f32 = 56.0;
const SCROLL_H: f32 = 12.0;
/// Cue lanes, in `CueKind::track()` order.
const TRACK_NAMES: [&str; 6] = ["scenes", "dancer", "fx", "show", "text 1", "text 2"];
const CUE_H: f32 = TRACK_H * TRACK_NAMES.len() as f32;
/// Trim zone: this many px *inside* a block's edge plus `EDGE_OUT` px of
/// overshoot past it, so grabbing an edge doesn't take pixel aim.
const EDGE: f32 = 8.0;
const EDGE_OUT: f32 = 5.0;

/// Edge-trim vs move for a press at pointer-x `px` on a block spanning
/// `x0..=x1`: the nearer edge wins inside the trim zone (which overshoots
/// the drawn edge by `EDGE_OUT`), so any block can be trimmed however
/// small it renders.
fn cue_drag_mode(px: f32, x0: f32, x1: f32) -> CueDrag {
    let dl = (px - x0).abs();
    let dr = (px - x1).abs();
    let zone = (EDGE + EDGE_OUT).min((x1 - x0) * 0.45);
    if dl <= zone && dl <= dr {
        CueDrag::TrimStart
    } else if dr <= zone {
        CueDrag::TrimEnd
    } else {
        CueDrag::Move
    }
}

#[allow(clippy::too_many_arguments)]
fn canvas(
    ui: &mut egui::Ui,
    doc_opt: &mut Option<Timeline>,
    mode: PlayMode,
    pos_s: f64,
    cursor_s: &mut f64,
    recording: bool,
    snap: bool,
    pps: &mut f64,
    scroll: &mut f64,
    sel_cue: &mut Option<usize>,
    sel_clip: &mut Option<usize>,
    cue_drag: &mut Option<(usize, CueDrag, f64)>,
    clip_drag: &mut Option<(usize, f64)>,
    zoom_fit: &mut bool,
    scrub: &mut bool,
    sb_grab: &mut Option<f32>,
    dirty: &mut bool,
    thumbs: &mut HashMap<String, egui::TextureHandle>,
    want_thumbs: &mut HashSet<String>,
    cmd: &mut Vec<UiCommand>,
) {
    use egui::{Align2, Color32, FontId, Rect, Sense, Shape, Stroke, pos2, vec2};

    let strip_h = ui
        .available_height()
        .max(RULER_H + CLIP_H + CUE_H + SCROLL_H + 8.0);
    let (rect, resp) =
        ui.allocate_exact_size(vec2(ui.available_width(), strip_h), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    // Time space starts after the track-header gutter; the horizontal
    // scrollbar takes the bottom edge.
    let lane_x = rect.left() + GUTTER;
    let bar = Rect::from_min_max(
        pos2(rect.left(), rect.bottom() - SCROLL_H - 2.0),
        pos2(rect.right(), rect.bottom() - 2.0),
    );
    let content = Rect::from_min_max(pos2(lane_x, rect.top()), pos2(rect.right(), bar.top()));
    let ruler = Rect::from_min_size(rect.min, vec2(rect.width(), RULER_H));
    let clip_lane = Rect::from_min_max(
        pos2(lane_x, ruler.bottom()),
        pos2(rect.right(), ruler.bottom() + CLIP_H),
    );
    let cue_lane = Rect::from_min_max(
        pos2(lane_x, clip_lane.bottom()),
        pos2(rect.right(), clip_lane.bottom() + CUE_H),
    );
    let gutter = Rect::from_min_max(pos2(rect.left(), ruler.bottom()), pos2(lane_x, bar.top()));
    // Register the cue lane as its own interactive region up front — inside
    // its rect it wins clicks/drags over the strip's response.
    let cue_resp = ui.interact(cue_lane, egui::Id::new("cue_lane"), Sense::click_and_drag());

    // Zoom on Ctrl+wheel around the cursor; wheel (or Shift+wheel) pans.
    // Gate on rect-contains-pointer: `resp.hovered()` is false during drags
    // and under some overlap cases, which reads as "scroll is broken".
    let pointer = || ui.ctx().input(|i| i.pointer.latest_pos());
    if pointer().is_some_and(|p| rect.contains(p)) {
        let zoom = ui.ctx().input(|i| i.zoom_delta());
        if (zoom - 1.0).abs() > 1e-3 {
            if let Some(hp) = pointer() {
                let anchor = *scroll + (hp.x - lane_x) as f64 / *pps;
                *pps = (*pps * zoom as f64).clamp(4.0, 600.0);
                *scroll = (anchor - (hp.x - lane_x) as f64 / *pps).max(0.0);
            }
        }
        let (dx, dy) = ui.ctx().input(|i| {
            let d = i.smooth_scroll_delta;
            (d.x, d.y)
        });
        let ds = if dy.abs() > 0.1 { dy } else { dx };
        if ds.abs() > 0.1 {
            *scroll = (*scroll - ds as f64 / *pps).max(0.0);
        }
    }
    // Middle-drag pans the timeline.
    if resp.dragged_by(egui::PointerButton::Middle) {
        let d = resp.drag_delta();
        *scroll = (*scroll - d.x as f64 / *pps).max(0.0);
    }
    if *zoom_fit {
        *zoom_fit = false;
        let dur = doc_opt.as_ref().map(|d| d.end_s()).unwrap_or(60.0).max(1.0);
        *pps = ((content.width() - 16.0) as f64 / dur).clamp(4.0, 600.0);
        *scroll = 0.0;
    }

    // Clamp the view to the doc (plus a short tail) — wheel/pan can't run
    // off into empty space, and the scrollbar range uses the same bound.
    let total_s = doc_opt.as_ref().map(|d| d.end_s() + 8.0).unwrap_or(120.0);
    let max_scroll = (total_s - content.width() as f64 / *pps).max(0.0);
    *scroll = scroll.clamp(0.0, max_scroll);

    // Bound once so the closures don't fight writes to the refs.
    let pps_v = *pps;
    let scroll_v = *scroll;
    let x_at = |t: f64| lane_x + ((t - scroll_v) * pps_v) as f32;
    let t_at = |x: f32| (scroll_v + (x - lane_x) as f64 / pps_v).max(0.0);
    let white = Color32::from_gray(210);
    let faint = Color32::from_gray(80);

    // --- Ruler ---------------------------------------------------------------
    painter.rect_filled(ruler, 0.0, Color32::from_gray(28));
    let steps = [0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0];
    let step = steps
        .iter()
        .copied()
        .find(|s| s * pps_v >= 80.0)
        .unwrap_or(600.0);
    let mut t = (scroll_v / step).floor() * step;
    while x_at(t) < rect.right() {
        let x = x_at(t);
        if x >= lane_x {
            painter.vline(x, ruler.y_range(), Stroke::new(1.0, Color32::from_gray(60)));
            painter.text(
                pos2(x + 3.0, ruler.top() + 3.0),
                Align2::LEFT_TOP,
                fmt_time(t),
                FontId::proportional(10.0),
                faint,
            );
        }
        t += step;
    }

    painter.rect_filled(clip_lane, 0.0, Color32::from_gray(22));
    painter.rect_filled(cue_lane, 0.0, Color32::from_gray(16));

    // Track header gutter + lane separators.
    painter.rect_filled(gutter, 0.0, Color32::from_gray(26));
    painter.vline(
        lane_x,
        egui::Rangef::new(rect.top(), bar.top()),
        Stroke::new(1.0, Color32::from_gray(70)),
    );
    painter.text(
        pos2(gutter.left() + 5.0, clip_lane.center().y),
        Align2::LEFT_CENTER,
        "audio",
        FontId::proportional(9.5),
        faint,
    );
    for (i, name) in TRACK_NAMES.iter().enumerate() {
        let ty = cue_lane.top() + i as f32 * TRACK_H;
        painter.text(
            pos2(gutter.left() + 5.0, ty + TRACK_H * 0.5),
            Align2::LEFT_CENTER,
            *name,
            FontId::proportional(9.5),
            faint,
        );
        if i > 0 {
            painter.hline(
                egui::Rangef::new(lane_x, rect.right()),
                ty,
                Stroke::new(1.0, Color32::from_gray(44)),
            );
        }
    }

    let Some(doc) = doc_opt.as_mut() else {
        painter.text(
            clip_lane.center(),
            Align2::CENTER_CENTER,
            "open a song or drop one here to start a timeline",
            FontId::proportional(13.0),
            faint,
        );
        return;
    };

    // --- Clip regions ---------------------------------------------------------
    let mut clip_del: Option<usize> = None;
    for (i, c) in doc.clips.iter_mut().enumerate() {
        let r = Rect::from_min_max(
            pos2(x_at(c.offset_s), clip_lane.top() + 3.0),
            pos2(x_at(c.end_s()), clip_lane.bottom() - 3.0),
        );
        if r.right() < rect.left() || r.left() > rect.right() {
            continue;
        }
        let hovered_clip = resp.hover_pos().is_some_and(|p| r.contains(p));
        let fill = if *sel_clip == Some(i) {
            Color32::from_rgb(46, 74, 96)
        } else {
            Color32::from_rgb(36, 52, 68)
        };
        painter.rect_filled(r, 3.0, fill);
        painter.rect_stroke(
            r,
            3.0,
            Stroke::new(
                1.0,
                if hovered_clip {
                    white
                } else {
                    Color32::from_gray(110)
                },
            ),
            egui::StrokeKind::Inside,
        );

        // Waveform inside the block.
        let n = c.overview.len();
        if n > 1 && c.duration_s > 0.0 {
            let mid = r.center().y;
            let h = r.height() * 0.42;
            let px_per_pt = r.width() / n as f32;
            if px_per_pt > 0.4 {
                for (j, &v) in c.overview.iter().enumerate() {
                    let x = r.left() + j as f32 * px_per_pt;
                    if x < rect.left() || x > rect.right() || x > r.right() {
                        continue;
                    }
                    let hh = (v * h).max(1.0);
                    painter.vline(
                        x,
                        egui::Rangef::new(mid - hh, mid + hh),
                        Stroke::new(1.0, Color32::from_rgb(110, 170, 210)),
                    );
                }
            }
        }
        painter.text(
            pos2(r.left() + 5.0, r.top() + 4.0),
            Align2::LEFT_TOP,
            format!("{} · {:.0} BPM", c.name, c.bpm),
            FontId::proportional(10.5),
            white,
        );
    }

    // --- Beat grid ------------------------------------------------------------
    // Full-height beat lines per clip, across the clip and cue lanes; bar
    // lines brighter with bar numbers on the clip block's top row.
    let grid_top = clip_lane.top();
    let grid_bot = cue_lane.bottom();
    for c in doc.clips.iter() {
        if c.bpm <= 0.0 {
            continue;
        }
        let spb = 60.0 / c.bpm;
        if spb * pps_v < 3.0 {
            continue; // too dense to be useful at this zoom
        }
        let r_left = x_at(c.offset_s).max(rect.left());
        let r_right = x_at(c.end_s()).min(rect.right());
        if r_right <= r_left {
            continue;
        }
        let mut b = 0u32;
        loop {
            let lt = c.first_beat + b as f64 * spb;
            if lt >= c.duration_s {
                break;
            }
            let x = x_at(c.offset_s + lt);
            if x > r_right {
                break;
            }
            if x >= r_left {
                let bar = b % 4 == 0;
                painter.vline(
                    x,
                    egui::Rangef::new(grid_top, grid_bot),
                    Stroke::new(
                        1.0,
                        if bar {
                            Color32::from_gray(85)
                        } else {
                            Color32::from_gray(48)
                        },
                    ),
                );
                if bar {
                    painter.text(
                        pos2(x + 3.0, clip_lane.top() + 4.0),
                        Align2::LEFT_TOP,
                        format!("{}", b / 4 + 1),
                        FontId::proportional(9.5),
                        faint,
                    );
                }
            }
            b += 1;
        }
    }

    // --- Clip interactions ----------------------------------------------------
    // Click selects, drag moves, right-click removes — all via `resp`, which
    // owns the clip lane and ruler (the cue lane belongs to cue_resp).
    let mut clicked_thing = false;
    if let Some(p) = resp.interact_pointer_pos() {
        if cue_drag.is_none() && clip_lane.contains(p) {
            let hit = doc.clips.iter().enumerate().find(|(_, c)| {
                let r = Rect::from_min_max(
                    pos2(x_at(c.offset_s), clip_lane.top()),
                    pos2(x_at(c.end_s()), clip_lane.bottom()),
                );
                r.contains(p)
            });
            if let Some((i, _)) = hit {
                if resp.secondary_clicked() {
                    clip_del = Some(i);
                } else if resp.clicked() {
                    *sel_clip = Some(i);
                    *sel_cue = None;
                    clicked_thing = true;
                }
            }
        }
    }
    // Clip drags classify by press position too (see the cue code below).
    if resp.drag_started() {
        if let Some(p) = ui.ctx().input(|i| i.pointer.press_origin()) {
            if cue_drag.is_none() && clip_lane.contains(p) {
                let hit = doc.clips.iter().enumerate().find(|(_, c)| {
                    let r = Rect::from_min_max(
                        pos2(x_at(c.offset_s), clip_lane.top()),
                        pos2(x_at(c.end_s()), clip_lane.bottom()),
                    );
                    r.contains(p)
                });
                if let Some((i, c)) = hit {
                    *clip_drag = Some((i, t_at(p.x) - c.offset_s));
                }
            }
        }
    }
    if let Some((i, grab)) = *clip_drag {
        if ui.ctx().input(|i| i.pointer.any_down()) {
            if let Some(p) = pointer() {
                let dur = doc.clips.get(i).map(|c| c.duration_s).unwrap_or(0.0);
                let mut new_off = (t_at(p.x) - grab).max(0.0);
                if snap {
                    // Snap to half-seconds and to neighbouring clip edges.
                    new_off = (new_off * 2.0).round() / 2.0;
                    for (j, o) in doc.clips.iter().enumerate() {
                        if j == i {
                            continue;
                        }
                        for edge in [o.offset_s, o.end_s()] {
                            for mine in [new_off, new_off + dur] {
                                if (mine - edge).abs() < 0.15 {
                                    new_off += edge - mine;
                                }
                            }
                        }
                    }
                }
                if let Some(c) = doc.clips.get_mut(i) {
                    if (c.offset_s - new_off).abs() > 1e-6 {
                        c.offset_s = new_off;
                        *dirty = true;
                    }
                }
            }
        } else {
            *clip_drag = None;
        }
    }
    if let Some(i) = clip_del {
        doc.remove_clip(i);
        *dirty = true;
        *sel_clip = None;
    }

    // --- Cue lane -------------------------------------------------------------
    // Palette drops land anywhere on the strip (the cue lane gets first pick).
    let drop = cue_resp
        .dnd_release_payload::<CueKind>()
        .or_else(|| resp.dnd_release_payload::<CueKind>());
    if let Some(payload) = drop {
        if let Some(pos) = pointer().filter(|p| content.contains(*p)) {
            let t = t_at(pos.x);
            if let Some((ci, c)) = doc.clip_at(t) {
                let mut beat = c.beat_at(t - c.offset_s);
                if snap {
                    beat = (beat * 4.0).round() / 4.0;
                }
                doc.cues.push(Cue {
                    clip: ci,
                    beat: beat.max(0.0),
                    beats: 4.0,
                    kind: (*payload).clone(),
                });
                doc.sort_cues();
                *dirty = true;
            }
        }
    }
    // Drop highlight across both lanes.
    if cue_resp.dnd_hover_payload::<CueKind>().is_some()
        || resp.dnd_hover_payload::<CueKind>().is_some()
    {
        painter.rect_stroke(
            Rect::from_min_max(pos2(lane_x, clip_lane.top()), cue_lane.max),
            0.0,
            Stroke::new(1.5, Color32::WHITE),
            egui::StrokeKind::Inside,
        );
    }

    let mut cue_del: Option<usize> = None;
    // Clip timing constants, copied out so cue iteration can be mutable.
    let clip_ts: Vec<(f64, f64, f64)> = doc
        .clips
        .iter()
        .map(|c| (c.offset_s, c.first_beat, c.bpm))
        .collect();
    let cue_gt = |c: &Cue| -> f64 {
        clip_ts
            .get(c.clip)
            .map(|&(o, fb, bpm)| o + fb + c.beat * 60.0 / bpm)
            .unwrap_or(f64::MAX)
    };
    let cue_ge = |c: &Cue| -> f64 {
        clip_ts
            .get(c.clip)
            .map(|&(o, fb, bpm)| o + fb + (c.beat + c.beats.max(0.0)) * 60.0 / bpm)
            .unwrap_or(f64::MAX)
    };

    // --- Cue blocks (Resolve-style: label + thumbnails + trim handles) ---------
    let mut want: Vec<String> = Vec::new();
    for (i, cue) in doc.cues.iter().enumerate() {
        let gt = cue_gt(cue);
        let ge = cue_ge(cue);
        if !gt.is_finite() || !ge.is_finite() {
            continue;
        }
        let x0 = x_at(gt).max(lane_x);
        let x1 = (x_at(ge) - x_at(gt)).max(8.0) + x_at(gt);
        let y = cue_lane.top() + cue.kind.track() as f32 * TRACK_H;
        let r = Rect::from_min_max(pos2(x0, y + 1.5), pos2(x1.max(x0 + 4.0), y + TRACK_H - 1.5));
        if r.right() < lane_x || r.left() > rect.right() {
            continue;
        }
        let col = cue_color(&cue.kind);
        let sel = *sel_cue == Some(i);
        painter.rect_filled(r, 3.0, col.gamma_multiply(0.28));
        painter.rect_stroke(
            r,
            3.0,
            Stroke::new(if sel { 1.6 } else { 1.0 }, col),
            egui::StrokeKind::Inside,
        );

        // Thumbnail strip: rendered scene frames or dancer clip frames.
        let mut text_x = r.left() + 4.0;
        let thumb_key: Option<String> = match &cue.kind {
            CueKind::Scene(n) => Some(format!("scene:{n}")),
            CueKind::Clip(n) => Some(format!("clip:{n}")),
            CueKind::Text(s) => Some(format!("text:{}:{}", s.style.index(), s.text)),
            _ => None,
        };
        if let Some(key) = thumb_key {
            if let Some(tex) = thumbs.get(&key) {
                let th = r.height() - 4.0;
                let asp = tex.size()[0] as f32 / (tex.size()[1] as f32).max(1.0);
                let tw = (th * asp).min(r.width() - 6.0);
                painter.image(
                    tex.id(),
                    Rect::from_min_size(pos2(r.left() + 2.0, r.top() + 2.0), vec2(tw, th)),
                    egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                text_x += tw + 3.0;
            } else if let CueKind::Clip(name) = &cue.kind {
                // Dancer clips: pull a mid-frame PNG straight off disk.
                if want_thumbs.insert(key.clone()) {
                    match load_clip_thumb(ui.ctx(), name) {
                        Some(tex) => {
                            thumbs.insert(key, tex);
                        }
                        None => eprintln!("Editor: no clip thumbnail for {name}"),
                    }
                }
            } else if let CueKind::Text(spec) = &cue.kind {
                // Text: rasterise the same mask the GPU gets — the block's
                // thumbnail shows the real lettering.
                if want_thumbs.insert(key.clone()) {
                    if let Some((w, h, px)) = crate::text::rasterize_rgba(&spec.text, 28.0) {
                        let img =
                            egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &px);
                        thumbs.insert(
                            key,
                            ui.ctx()
                                .load_texture("text-thumb", img, egui::TextureOptions::LINEAR),
                        );
                    }
                }
            } else if want_thumbs.insert(key.clone()) {
                // Scenes: ask the render thread for a one-shot offscreen frame.
                if let CueKind::Scene(name) = &cue.kind {
                    want.push(format!("scene:{name}"));
                }
            }
        }
        if r.width() > 24.0 {
            painter.text(
                pos2(text_x, r.center().y),
                Align2::LEFT_CENTER,
                cue.kind.text(),
                FontId::proportional(10.5),
                Color32::WHITE,
            );
        }
        // Trim grips: always drawn once the block is wide enough to see
        // them; the grip under the pointer (or being dragged) lights up.
        let (mut hot_l, mut hot_r) = (false, false);
        if let Some(p) = pointer() {
            if p.y >= y && p.y <= y + TRACK_H {
                match cue_drag_mode(p.x, r.left(), r.right()) {
                    CueDrag::TrimStart => hot_l = true,
                    CueDrag::TrimEnd => hot_r = true,
                    CueDrag::Move => {}
                }
            }
        }
        match cue_drag.as_ref() {
            Some((di, CueDrag::TrimStart, _)) if *di == i => hot_l = true,
            Some((di, CueDrag::TrimEnd, _)) if *di == i => hot_r = true,
            _ => {}
        }
        if r.width() > 8.0 {
            for (left_edge, hot) in [(true, hot_l), (false, hot_r)] {
                let x = if left_edge {
                    r.left() + 1.0
                } else {
                    r.right() - 4.0
                };
                painter.rect_filled(
                    Rect::from_min_size(pos2(x, r.top() + 1.0), vec2(3.0, r.height() - 2.0)),
                    0.0,
                    if hot {
                        Color32::WHITE.gamma_multiply(0.85)
                    } else {
                        col.gamma_multiply(0.9)
                    },
                );
            }
        }
    }
    for n in want {
        cmd.push(UiCommand::Thumb(n));
    }

    // Cue hit-testing — blocks on cue_resp. The hit row covers the whole
    // track height and overshoots the block's edges by EDGE_OUT, so aiming
    // at the visible edge line (or a hair outside it) still trims.
    let cue_at = |p: egui::Pos2| -> Option<(usize, CueDrag, f64)> {
        if !cue_lane.contains(p) {
            return None;
        }
        let mut best: Option<(usize, CueDrag, f64, f32)> = None;
        for (i, c) in doc.cues.iter().enumerate() {
            let gt = cue_gt(c);
            let ge = cue_ge(c);
            if !gt.is_finite() || !ge.is_finite() {
                continue;
            }
            let x0 = x_at(gt).max(lane_x);
            let x1 = (x_at(ge) - x_at(gt)).max(8.0) + x_at(gt);
            let y = cue_lane.top() + c.kind.track() as f32 * TRACK_H;
            let row = Rect::from_min_max(
                pos2(x0 - EDGE_OUT, y),
                pos2(x1.max(x0 + 4.0) + EDGE_OUT, y + TRACK_H),
            );
            if !row.contains(p) {
                continue;
            }
            // Distance to the visible edge decides — nearer edge wins on
            // narrow blocks, so any block can be trimmed however small.
            let mode = cue_drag_mode(p.x, x0, x1);
            let grab = if mode == CueDrag::Move {
                t_at(p.x) - gt
            } else {
                0.0
            };
            let d = (p.y - row.center().y).abs();
            if best.is_none_or(|b| d < b.3) {
                best = Some((i, mode, grab, d));
            }
        }
        best.map(|(i, m, g, _)| (i, m, g))
    };
    if let Some(p) = cue_resp.interact_pointer_pos() {
        if let Some((i, mode, _grab)) = cue_at(p) {
            // Show the affordance before the press: grab in the middle,
            // resize on the trim edges.
            ui.ctx().set_cursor_icon(match mode {
                CueDrag::Move => egui::CursorIcon::Grab,
                _ => egui::CursorIcon::ResizeHorizontal,
            });
            if cue_resp.secondary_clicked() {
                cue_del = Some(i);
            } else if cue_resp.clicked() {
                *sel_cue = Some(i);
                *sel_clip = None;
                clicked_thing = true;
            }
        }
    }
    // Classify drags by where the button went DOWN, not where the pointer
    // is when the drag registers — a fast flick off an edge can already be
    // outside the row by then, which used to start no drag at all.
    if cue_resp.drag_started() {
        if let Some(p) = ui.ctx().input(|i| i.pointer.press_origin()) {
            if let Some((i, mode, grab)) = cue_at(p) {
                *cue_drag = Some((i, mode, grab));
            }
        }
    }
    // Keep the resize cursor while a trim is in flight.
    if let Some((_, mode, _)) = cue_drag {
        if matches!(mode, CueDrag::TrimStart | CueDrag::TrimEnd) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
    }
    if let Some((i, mode, grab)) = *cue_drag {
        if ui.ctx().input(|i| i.pointer.any_down()) {
            if let Some(p) = pointer() {
                match mode {
                    CueDrag::Move => {
                        let t = (t_at(p.x) - grab).max(0.0);
                        if let Some((ci, c)) = doc.clip_at(t) {
                            let mut beat = c.beat_at(t - c.offset_s);
                            if snap {
                                beat = (beat * 4.0).round() / 4.0;
                            }
                            if let Some(cue) = doc.cues.get_mut(i) {
                                cue.clip = ci;
                                cue.beat = beat.max(0.0);
                                *dirty = true;
                            }
                        }
                    }
                    CueDrag::TrimStart | CueDrag::TrimEnd => {
                        // Trim against the cue's OWN clip — the edge can sit
                        // beyond neighbouring clips without re-pinning.
                        let t = t_at(p.x).max(0.0);
                        let ci = doc.cues.get(i).map(|c| c.clip);
                        if let Some((ci, c)) =
                            ci.and_then(|ci| doc.clips.get(ci).map(|c| (ci, (c.offset_s, c))))
                        {
                            let mut beat = c.1.beat_at(t - c.0);
                            if snap {
                                beat = (beat * 4.0).round() / 4.0;
                            }
                            if let Some(cue) = doc.cues.get_mut(i) {
                                let end = cue.beat + cue.beats;
                                match mode {
                                    CueDrag::TrimStart => {
                                        cue.beat = beat.min(end - 0.25);
                                        cue.beats = (end - cue.beat).max(0.25);
                                    }
                                    _ => {
                                        cue.clip = ci;
                                        cue.beats = (beat - cue.beat).max(0.25);
                                    }
                                }
                                *dirty = true;
                            }
                        }
                    }
                }
            }
        } else {
            *cue_drag = None;
            doc.sort_cues();
        }
    }
    if let Some(i) = cue_del {
        doc.cues.remove(i);
        *dirty = true;
        if *sel_cue == Some(i) {
            *sel_cue = None;
        }
    }

    // --- Scrub, cursor, playhead ----------------------------------------------
    // Drag starting on the ruler scrubs the playhead; a click anywhere on
    // empty strip space moves it (Seek works stopped or playing).
    if resp.drag_started() && clip_drag.is_none() && cue_drag.is_none() {
        if pointer().is_some_and(|p| ruler.contains(p) && p.x >= lane_x) {
            *scrub = true;
        }
    }
    if *scrub {
        if ui.ctx().input(|i| i.pointer.any_down()) {
            if let Some(p) = pointer() {
                let t = t_at(p.x);
                if (t - *cursor_s).abs() > 0.01 {
                    *cursor_s = t;
                    cmd.push(UiCommand::Song(SongCtl::Seek(t)));
                }
            }
        } else {
            *scrub = false;
        }
    }
    let strip_clicked = (resp.clicked() || cue_resp.clicked()) && !clicked_thing;
    if strip_clicked {
        if let Some(p) = resp
            .interact_pointer_pos()
            .or_else(|| cue_resp.interact_pointer_pos())
            .or_else(pointer)
            .filter(|p| content.contains(*p))
        {
            *sel_cue = None;
            *sel_clip = None;
            *cursor_s = t_at(p.x);
            cmd.push(UiCommand::Song(SongCtl::Seek(*cursor_s)));
        }
    }
    let cur_x = x_at(*cursor_s);
    painter.vline(
        cur_x,
        egui::Rangef::new(rect.top(), bar.top()),
        Stroke::new(1.0, Color32::from_gray(120)),
    );
    let ph_x = x_at(pos_s);
    let ph_col = if recording {
        Color32::from_rgb(255, 80, 80)
    } else {
        Color32::from_rgb(255, 210, 90)
    };
    painter.vline(
        ph_x,
        egui::Rangef::new(rect.top(), bar.top()),
        Stroke::new(1.5, ph_col),
    );
    painter.add(Shape::convex_polygon(
        vec![
            pos2(ph_x - 5.0, rect.top()),
            pos2(ph_x + 5.0, rect.top()),
            pos2(ph_x, rect.top() + 7.0),
        ],
        ph_col,
        Stroke::NONE,
    ));
    // Keep the playhead in view while playing.
    if mode == PlayMode::Playing && (ph_x > rect.right() - 40.0 || ph_x < lane_x) {
        *scroll = (pos_s - 40.0 / pps_v).clamp(0.0, max_scroll);
    }

    // --- Horizontal scrollbar --------------------------------------------------
    // Proportional thumb over the whole doc. Drag it, click the track to
    // page, or grab the track beside the thumb to centre it on the cursor.
    let sb = ui.interact(bar, egui::Id::new("hscroll"), Sense::click_and_drag());
    painter.rect_filled(bar, 4.0, Color32::from_gray(24));
    if max_scroll > 0.0 {
        let visible_s = total_s - max_scroll;
        let tw = (bar.width() * (visible_s / total_s) as f32).clamp(20.0, bar.width());
        let range = (bar.width() - tw).max(1.0);
        let frac_p = (*scroll / max_scroll).clamp(0.0, 1.0) as f32;
        let thumb = Rect::from_min_size(
            pos2(bar.left() + frac_p * range, bar.top() + 1.5),
            vec2(tw, bar.height() - 3.0),
        );
        if sb.drag_started() {
            *sb_grab = pointer().map(|p| {
                if thumb.contains(p) {
                    p.x - thumb.left()
                } else {
                    tw / 2.0 // grabbed the track — centre the thumb on the cursor
                }
            });
        }
        if sb.dragged() {
            if let (Some(g), Some(p)) = (*sb_grab, pointer()) {
                let f = ((p.x - bar.left() - g) / range).clamp(0.0, 1.0) as f64;
                *scroll = f * max_scroll;
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else {
            *sb_grab = None;
            if sb.clicked() {
                // Page in the click direction (never when the click lands
                // on the thumb itself).
                if let Some(p) = pointer().filter(|p| !thumb.contains(*p)) {
                    let dir = if p.x > thumb.center().x { 1.0 } else { -1.0 };
                    *scroll = (*scroll + dir * visible_s * 0.9).clamp(0.0, max_scroll);
                }
            }
        }
        painter.rect_filled(
            thumb,
            4.0,
            if sb.hovered() || sb.dragged() {
                Color32::from_gray(130)
            } else {
                Color32::from_gray(95)
            },
        );
        if sb.hovered() && !sb.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
}

// ---------------------------------------------------------------------------
// Inspector: edit the selected cue's params, or the selected clip's offset.
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn inspector(
    ui: &mut egui::Ui,
    doc_opt: &mut Option<Timeline>,
    scenes: &[String],
    routines: &[String],
    sel_cue: Option<usize>,
    sel_clip: Option<usize>,
    cursor_s: f64,
    snap: bool,
    dirty: &mut bool,
    cmd: &mut Vec<UiCommand>,
) {
    let Some(doc) = doc_opt.as_mut() else {
        return;
    };
    ui.horizontal(|ui| {
        if let Some(i) = sel_cue {
            let when = doc.cues.get(i).and_then(|c| doc.cue_time(c)).unwrap_or(0.0);
            if let Some(cue) = doc.cues.get_mut(i) {
                ui.label(format!("cue {} @ {} ({})", i + 1, fmt_time(when), cue.kind.label()));
                ui.label("@");
                let mut beat = cue.beat;
                if ui
                    .add(egui::DragValue::new(&mut beat).speed(0.1).suffix(" bt"))
                    .changed()
                {
                    cue.beat = beat;
                    *dirty = true;
                }
                ui.label("len");
                let mut beats = cue.beats;
                if ui
                    .add(egui::DragValue::new(&mut beats).speed(0.1).range(0.25..=512.0).suffix(" bt"))
                    .changed()
                {
                    cue.beats = beats;
                    *dirty = true;
                }
                let mut kind = cue.kind.clone();
                if cue_param_ui(ui, &mut kind, scenes, routines, egui::Id::new(("insp", i))) {
                    cue.kind = kind;
                    *dirty = true;
                }
                if ui.button("preview").clicked() {
                    cmd.push(UiCommand::FireCue(cue.kind.clone()));
                }
                if ui.button("delete").clicked() {
                    doc.cues.remove(i);
                    *dirty = true;
                }
            }
        } else if let Some(i) = sel_clip {
            if let Some(c) = doc.clips.get_mut(i) {
                ui.label(format!("clip: {}", c.name));
                ui.label("offset");
                let mut off = c.offset_s;
                if ui.add(egui::DragValue::new(&mut off).speed(0.1).suffix(" s")).changed() {
                    c.offset_s = off.max(0.0);
                    *dirty = true;
                }
                ui.small(format!("{:.1} BPM · {}", c.bpm, fmt_time(c.duration_s)));
                if ui.button("remove from timeline").clicked() {
                    doc.remove_clip(i);
                    *dirty = true;
                }
            }
        } else {
            ui.small("click a block to edit · drag its edges to resize · right-click deletes · drag from the palette to add · Ctrl+scroll zooms");
            if doc.clip_at(cursor_s).is_some() && ui.button("+ marker at cursor").clicked() {
                let (ci, c) = doc.clip_at(cursor_s).unwrap();
                let mut beat = c.beat_at(cursor_s - c.offset_s);
                if snap {
                    beat = (beat * 4.0).round() / 4.0;
                }
                doc.cues.push(Cue { clip: ci, beat: beat.max(0.0), beats: 4.0, kind: CueKind::NextScene });
                doc.sort_cues();
                *dirty = true;
            }
        }
    });
}

/// Load a mid-sequence frame from `dancers/<name>/frames/` as an egui texture.
fn load_clip_thumb(ctx: &egui::Context, name: &str) -> Option<egui::TextureHandle> {
    let dir = crate::dancer::find_dancer_dir()?.join(name).join("frames");
    let n = std::fs::read_dir(&dir).ok()?.filter_map(|e| e.ok()).count();
    if n == 0 {
        return None;
    }
    let path = dir.join(format!("{:04}.png", n / 2));
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let tex = ctx.load_texture(
        format!("clipthumb:{name}"),
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &img),
        egui::TextureOptions::LINEAR,
    );
    Some(tex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_drag_mode_picks_nearest_edge() {
        // Wide block 100..200: inside zones trim, centre moves.
        assert_eq!(cue_drag_mode(103.0, 100.0, 200.0), CueDrag::TrimStart);
        assert_eq!(cue_drag_mode(197.0, 100.0, 200.0), CueDrag::TrimEnd);
        assert_eq!(cue_drag_mode(150.0, 100.0, 200.0), CueDrag::Move);
        // Overshoot: aiming a few px past the drawn edge still trims.
        assert_eq!(cue_drag_mode(97.0, 100.0, 200.0), CueDrag::TrimStart);
        assert_eq!(cue_drag_mode(204.0, 100.0, 200.0), CueDrag::TrimEnd);
        // …but not forever: well clear of the block is a move/hover.
        assert_eq!(cue_drag_mode(80.0, 100.0, 200.0), CueDrag::Move);
        // Narrow block (w=10, zone=4.5): nearer edge wins mid-block.
        assert_eq!(cue_drag_mode(102.0, 100.0, 110.0), CueDrag::TrimStart);
        assert_eq!(cue_drag_mode(108.0, 100.0, 110.0), CueDrag::TrimEnd);
    }
}
