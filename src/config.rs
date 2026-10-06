//! User settings: key bindings, modes, scene and dancer preferences. Saved as
//! `trippin.json` in the working directory and edited from the control panel.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use winit::keyboard::Key;

/// Everything a hotkey can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Action {
    NextScene,
    PrevScene,
    ModeAuto,
    ModeStatic,
    ModeManual,
    ToggleRandom,
    ToggleDancer,
    NextClip,
    NextStyle,
    CycleCanon,
    Blackout,
    Strobe,
    Fullscreen,
    MarkDownbeat,
    LatencyDown,
    LatencyUp,
    CycleFx,
    ReloadShaders,
    TogglePanel,
    ToggleEditor,
    LeaveFullscreen,
    TimelinePlay,
    TimelineRecord,
    ShowNowPlaying,
    SaveClip,
    RecordSet,
    MarkPhrase,
    ToggleLogo,
    ToggleName,
    ToggleTicker,
    /// Recall the Look on slot 1..=8 (unbound by default; see `looks.rs`).
    Look1,
    Look2,
    Look3,
    Look4,
    Look5,
    Look6,
    Look7,
    Look8,
    /// Step through the Styles: off, then each one in turn (unbound by default).
    NextTheme,
}

impl Action {
    pub const ALL: [Action; 39] = [
        Action::NextScene,
        Action::PrevScene,
        Action::ModeAuto,
        Action::ModeStatic,
        Action::ModeManual,
        Action::ToggleRandom,
        Action::ToggleDancer,
        Action::NextClip,
        Action::NextStyle,
        Action::CycleCanon,
        Action::Blackout,
        Action::Strobe,
        Action::Fullscreen,
        Action::MarkDownbeat,
        Action::LatencyDown,
        Action::LatencyUp,
        Action::CycleFx,
        Action::ReloadShaders,
        Action::TogglePanel,
        Action::ToggleEditor,
        Action::LeaveFullscreen,
        Action::TimelinePlay,
        Action::TimelineRecord,
        Action::ShowNowPlaying,
        Action::SaveClip,
        Action::RecordSet,
        Action::MarkPhrase,
        Action::ToggleLogo,
        Action::ToggleName,
        Action::ToggleTicker,
        Action::Look1,
        Action::Look2,
        Action::Look3,
        Action::Look4,
        Action::Look5,
        Action::Look6,
        Action::Look7,
        Action::Look8,
        Action::NextTheme,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::NextScene => "Next scene",
            Action::PrevScene => "Previous scene",
            Action::ModeAuto => "Mode: auto (cuts on phrases and drops)",
            Action::ModeStatic => "Mode: static (hold the current scene)",
            Action::ModeManual => "Mode: manual (nothing changes by itself)",
            Action::ToggleRandom => "Random / sequential scene order",
            Action::ToggleDancer => "Dancer on / off",
            Action::NextClip => "Next dancer routine",
            Action::NextStyle => "Next dancer look",
            Action::CycleCanon => "Canon: auto / on / off",
            Action::Blackout => "Blackout (fade to black)",
            Action::Strobe => "Strobe on / off (flashes on the drum hits)",
            Action::Fullscreen => "Fullscreen on / off",
            Action::MarkDownbeat => "Mark this beat as the downbeat",
            Action::LatencyDown => "Latency -5 ms (visuals later)",
            Action::LatencyUp => "Latency +5 ms (visuals earlier)",
            Action::CycleFx => "Effect: off / mirror / kaleido",
            Action::ReloadShaders => "Reload shaders",
            Action::TogglePanel => "Show / hide this control panel",
            Action::ToggleEditor => "Show the timeline editor",
            Action::LeaveFullscreen => "Leave fullscreen",
            Action::TimelinePlay => "Timeline play / pause",
            Action::TimelineRecord => "Timeline record on / off",
            Action::ShowNowPlaying => "Show the now-playing card again",
            Action::SaveClip => "Save a clip (the last N seconds)",
            Action::RecordSet => "Record the whole set: start / stop",
            Action::MarkPhrase => "Mark phrase start (this beat = bar 1)",
            Action::ToggleLogo => "Logo on / off",
            Action::ToggleName => "DJ name on / off",
            Action::ToggleTicker => "Scrolling ticker on / off",
            Action::Look1 => "Look 1 (the saved Look on slot 1)",
            Action::Look2 => "Look 2 (the saved Look on slot 2)",
            Action::Look3 => "Look 3 (the saved Look on slot 3)",
            Action::Look4 => "Look 4 (the saved Look on slot 4)",
            Action::Look5 => "Look 5 (the saved Look on slot 5)",
            Action::Look6 => "Look 6 (the saved Look on slot 6)",
            Action::Look7 => "Look 7 (the saved Look on slot 7)",
            Action::Look8 => "Look 8 (the saved Look on slot 8)",
            Action::NextTheme => "Next Style (off, Dance, House, Pop...)",
        }
    }

    /// The Look slot (1..=8) this action recalls, if it is one of `Look1..8`.
    pub fn look_slot(self) -> Option<u8> {
        Some(match self {
            Action::Look1 => 1,
            Action::Look2 => 2,
            Action::Look3 => 3,
            Action::Look4 => 4,
            Action::Look5 => 5,
            Action::Look6 => 6,
            Action::Look7 => 7,
            Action::Look8 => 8,
            _ => return None,
        })
    }

    pub fn default_key(self) -> &'static str {
        match self {
            Action::NextScene => "ArrowRight",
            Action::PrevScene => "ArrowLeft",
            Action::ModeAuto => "A",
            Action::ModeStatic => "H",
            Action::ModeManual => "M",
            Action::ToggleRandom => "R",
            Action::ToggleDancer => "D",
            Action::NextClip => "C",
            Action::NextStyle => "S",
            Action::CycleCanon => "V",
            Action::Blackout => "B",
            Action::Strobe => "Z",
            Action::Fullscreen => "F",
            Action::MarkDownbeat => "Space",
            Action::LatencyDown => "[",
            Action::LatencyUp => "]",
            Action::CycleFx => "X",
            Action::ReloadShaders => "F5",
            Action::TogglePanel => "F1",
            // No F-keys on Touch Bar Macs — F2 is a brightness key there.
            Action::ToggleEditor => {
                if cfg!(target_os = "macos") {
                    "E"
                } else {
                    "F2"
                }
            }
            Action::LeaveFullscreen => "Escape",
            Action::TimelinePlay => "T",
            Action::TimelineRecord => "G",
            Action::ShowNowPlaying => "N",
            Action::SaveClip => "K",
            Action::RecordSet => "J",
            Action::MarkPhrase => "O",
            Action::ToggleLogo => "L",
            Action::ToggleName => "Y",
            Action::ToggleTicker => "W",
            // Unbound: sixteen default keys would collide with the rest.
            Action::Look1
            | Action::Look2
            | Action::Look3
            | Action::Look4
            | Action::Look5
            | Action::Look6
            | Action::Look7
            | Action::Look8
            | Action::NextTheme => "",
        }
    }
}

/// How much the auto-pilot is allowed to change on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Cut scenes every phrase and on drops; the dancer follows the track.
    Auto,
    /// Hold the current scene; the dancer still changes with the phrases.
    Static,
    /// Nothing changes unless you do it.
    Manual,
}

/// Whether the seasonal scenes (Halloween, Christmas, fireworks) rotate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Seasonal {
    /// Only in season (see `SEASONS`).
    Auto,
    Always,
    Off,
}

/// Seasonal scenes and when they're in season: (month, day) ranges, inclusive.
pub const SEASONS: [(&str, &[((u32, u32), (u32, u32))]); 3] = [
    ("halloween", &[((10, 1), (11, 2))]),
    ("christmas", &[((12, 1), (12, 27))]),
    // Bonfire Night and New Year.
    (
        "fireworks",
        &[((11, 1), (11, 8)), ((12, 28), (12, 31)), ((1, 1), (1, 2))],
    ),
];

/// None if `scene` isn't seasonal, otherwise whether it's in season on (month, day).
pub fn in_season(scene: &str, date: (u32, u32)) -> Option<bool> {
    let (_, ranges) = SEASONS.iter().find(|(name, _)| *name == scene)?;
    Some(ranges.iter().any(|(from, to)| *from <= date && date <= *to))
}

/// Today's (month, day) in UTC, from the system clock.
pub fn today() -> (u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    month_day(secs)
}

/// (month, day) of a Unix timestamp, via Howard Hinnant's days-to-civil algorithm.
fn month_day(secs: i64) -> (u32, u32) {
    let z = secs.div_euclid(86_400) + 719_468;
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (month, day)
}

/// Whole-frame post effect: mirrors and kaleidoscope. Applied in
/// `present.wgsl` from `u.fx`, so it transforms the scene and dancer alike.
/// `#[serde(other)]` keeps a stale value (e.g. a removed mode) from
/// invalidating the whole settings file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fx {
    MirrorX,
    MirrorY,
    Quad,
    Kaleido6,
    Kaleido8,
    /// Last so `#[serde(other)]` lands here: a removed mode in a stale
    /// trippin.json (e.g. "Invert") falls back to Off instead of being
    /// rejected with the whole settings file.
    #[serde(other)]
    #[default]
    Off,
}

impl Fx {
    pub const ALL: [Fx; 6] = [
        Fx::Off,
        Fx::MirrorX,
        Fx::MirrorY,
        Fx::Quad,
        Fx::Kaleido6,
        Fx::Kaleido8,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Fx::Off => "Off",
            Fx::MirrorX => "Mirror X",
            Fx::MirrorY => "Mirror Y",
            Fx::Quad => "Quad mirror",
            Fx::Kaleido6 => "Kaleido x6",
            Fx::Kaleido8 => "Kaleido x8",
        }
    }

    /// The number handed to the shader (`u.fx`).
    pub fn index(self) -> f32 {
        match self {
            Fx::Off => 0.0,
            Fx::MirrorX => 1.0,
            Fx::MirrorY => 2.0,
            Fx::Quad => 3.0,
            Fx::Kaleido6 => 4.0,
            Fx::Kaleido8 => 5.0,
        }
    }

    pub fn next(self) -> Fx {
        let i = Fx::ALL.iter().position(|&f| f == self).unwrap_or(0);
        Fx::ALL[(i + 1) % Fx::ALL.len()]
    }

    /// Auto pool: MirrorY is excluded — an upside-down dancer mid-set reads
    /// as a glitch rather than an effect (it stays selectable by hand).
    const AUTO: [Fx; 5] = [Fx::Off, Fx::MirrorX, Fx::Quad, Fx::Kaleido6, Fx::Kaleido8];

    /// A random pick for auto mode; never repeats the current effect.
    pub fn random(r: f32, cur: Fx) -> Fx {
        let i = (r * Fx::AUTO.len() as f32) as usize % Fx::AUTO.len();
        let f = Fx::AUTO[i];
        if f == cur {
            Fx::AUTO[(i + 1) % Fx::AUTO.len()]
        } else {
            f
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tristate {
    Auto,
    On,
    Off,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub keys: BTreeMap<Action, String>,
    /// MIDI input port to listen on ("" = off). Note-on presses fire the
    /// action bound in `midi_notes`.
    pub midi_in: String,
    /// Action → MIDI note number, same shape as `keys`. Bound on the Keys
    /// page by learning: click "midi", then hit the pad.
    pub midi_notes: BTreeMap<Action, u8>,
    pub mode: Mode,
    pub random_order: bool,
    /// Bars per scene in auto mode.
    pub phrase_bars: u32,
    /// Cut early when a drop lands.
    pub cut_on_drops: bool,
    pub disabled_scenes: Vec<String>,
    /// Starred scenes — the Perform "fav" chip filters to these.
    pub favourite_scenes: Vec<String>,
    /// The selected Style (`styles.json` id); `None` = off. Only the id is saved:
    /// the pool, palettes and pacing are an overlay on the per-frame settings.
    pub style: Option<String>,
    pub seasonal: Seasonal,
    pub dancer_enabled: bool,
    /// None = auto-pilot picks the look; Some(i) = always that look.
    pub dancer_style: Option<usize>,
    pub canon: Tristate,
    pub disabled_clips: Vec<String>,
    /// Dancer height as a fraction of the screen.
    pub dancer_size: f32,
    /// Ghost echoes of earlier frames trail the dancer's movement.
    pub dancer_trails: bool,
    /// Global colour palette — a WLED-style named gradient (see
    /// `palettes::PALETTES`) every scene's `palette()` samples.
    pub palette: String,
    /// Perform tab: false = scene library (1b), true = live pads view (1c).
    pub perform_pads: bool,
    /// Fixed post effect (ignored while `fx_auto` is on).
    pub fx: Fx,
    /// Pick a fresh post effect on every scene cut.
    pub fx_auto: bool,
    /// Post effect strength 0..1 — blends the transform in `present.wgsl`.
    pub fx_amt: f32,
    /// Raymarched `// @heavy` scenes: Auto follows the detected GPU tier.
    pub heavy_scenes: Tristate,
    /// 2D (non-`@heavy`) scenes in rotation — off leaves only the 3D ones.
    pub flat_scenes: bool,
    /// Detect breakdowns (no drums) and switch the show into its calm mode.
    /// Off = always treat the music as beats.
    pub breakdown_mode: bool,
    /// AI show builder (BYOAI): provider + endpoint/model/key. Blank fields
    /// fall back to the provider's defaults; a blank key falls back to the
    /// provider's usual env var (see `ai::AiProvider::env_keys`).
    pub ai_provider: crate::ai::AiProvider,
    pub ai_endpoint: String,
    pub ai_model: String,
    pub ai_key: String,
    /// Let the model look each track up online (Anthropic web search)
    /// before planning — genre, mood, hook words.
    pub ai_web_search: bool,
    /// Neural beat/downbeat tracking (Beat This!) for song grids — the
    /// model downloads on first use.
    pub beat_model: bool,
    /// External engine link (the Unity stage): send the show-state feed
    /// over UDP and take frames back from a Spout sender as the
    /// `unity_stage` scene.
    pub unity_link: bool,
    /// UDP port the show-state feed goes to (127.0.0.1).
    pub link_port: u16,
    pub latency_ms: f32,
    /// Audio capture source: a device-name substring resolved like
    /// `--device`. "" = the platform default tap (ScreenCaptureKit output
    /// mix on macOS, output loopback on Windows).
    pub audio_in: String,
    pub show_panel: bool,
    /// NDI network output — sends the composited frame (FX + text included)
    /// to OBS/another display. Needs the free NDI runtime installed; a
    /// missing runtime just shows an error in the panel.
    pub ndi_enabled: bool,
    /// The source name receivers see.
    pub ndi_name: String,
    /// Output height: 720 / 1080 / 2160 — width follows at 16:9.
    pub ndi_height: u32,
    /// Output cadence cap.
    pub ndi_fps: u32,
    /// Spout output (Windows): GPU texture sharing with OBS's Spout2 source
    /// on the same PC — no network, no runtime to install.
    pub spout_enabled: bool,
    /// Transparent background: scenes off, the dancer + overlays go out with
    /// alpha (NDI/Spout) to layer over a camera in OBS.
    pub out_transparent: bool,
    /// Clip recorder: keep the last `rec_keep_s` seconds ready to save.
    pub rec_buffer: bool,
    pub rec_keep_s: u32,
    pub rec_layout: crate::rec::Layout,
    /// Clip folder ("" = Videos/Trippin).
    pub rec_dir: String,
    /// ffmpeg binary ("" = find it).
    pub ffmpeg_path: String,
    /// Now playing: where tracks come from (Auto = whichever changed last).
    pub np_source: crate::nowplaying::NpSource,
    /// Hold a new track back until it has stayed this long (seconds) — skips
    /// cue-previews and tracks only auditioned in the headphones.
    pub np_delay_s: f32,
    /// Text-file source: any file another tool keeps up to date.
    pub np_file: String,
    /// Show the on-screen "now playing" card when the track changes.
    pub np_card: bool,
    /// Seconds the card stays up (0 = stays until the next track).
    pub np_hold_s: f32,
    pub np_size: f32,
    /// Branding block: logo PNG + DJ name + social handles in a corner.
    pub brand_on: bool,
    pub brand_name: String,
    pub brand_handles: String,
    pub brand_logo: String,
    /// 0 top-left · 1 top-right · 2 bottom-left · 3 bottom-right.
    pub brand_corner: u8,
    pub brand_size: f32,
    pub brand_opacity: f32,
    /// Accent colour for the card, handles and ticker ("#rrggbb").
    pub brand_color: String,
    /// Per-piece kills inside the branding block — the perform pads and
    /// hotkeys flip these so the logo or the name can drop out mid-set
    /// without touching the layout.
    pub brand_logo_on: bool,
    pub brand_name_on: bool,
    /// Scrolling ticker along the bottom.
    pub ticker_on: bool,
    pub ticker_text: String,
    pub ticker_speed: f32,
    /// LAN remote for the companion iOS app: a WebSocket JSON server on
    /// `remote_port`, advertised over mDNS as `_trippin._tcp`. Clients must
    /// `hello` with `remote_pin`. Off by default — no silent open ports.
    pub remote_on: bool,
    pub remote_port: u16,
    /// 4-digit pairing PIN shown on the Settings tab. Blank while
    /// `remote_on` is regenerated on load so a blanked field can't silently
    /// disable auth.
    pub remote_pin: String,
    /// OSC input (UDP) for TouchOSC/Lemur-style controllers — see the
    /// address table in `osc.rs`.
    pub osc_on: bool,
    pub osc_port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            keys: Action::ALL
                .iter()
                .map(|a| (*a, a.default_key().to_string()))
                .collect(),
            midi_in: String::new(),
            midi_notes: BTreeMap::new(),
            mode: Mode::Auto,
            random_order: true,
            phrase_bars: 16,
            cut_on_drops: true,
            disabled_scenes: Vec::new(),
            style: None,
            favourite_scenes: Vec::new(),
            seasonal: Seasonal::Auto,
            dancer_enabled: true,
            dancer_style: None,
            canon: Tristate::Auto,
            disabled_clips: Vec::new(),
            dancer_size: 0.85,
            dancer_trails: false,
            palette: "rainbow".into(),
            perform_pads: false,
            fx: Fx::Off,
            fx_auto: false,
            fx_amt: 1.0,
            heavy_scenes: Tristate::Auto,
            flat_scenes: true,
            breakdown_mode: true,
            ai_provider: Default::default(),
            ai_endpoint: String::new(),
            ai_model: String::new(),
            ai_key: String::new(),
            ai_web_search: true,
            beat_model: true,
            unity_link: false,
            link_port: 9137,
            latency_ms: 30.0,
            audio_in: String::new(),
            show_panel: true,
            ndi_enabled: false,
            ndi_name: "Trippin".into(),
            ndi_height: 1080,
            ndi_fps: 60,
            spout_enabled: false,
            out_transparent: false,
            rec_buffer: false,
            rec_keep_s: 60,
            rec_layout: crate::rec::Layout::Wide,
            rec_dir: String::new(),
            ffmpeg_path: String::new(),
            np_source: Default::default(),
            np_delay_s: 0.0,
            np_file: String::new(),
            np_card: true,
            np_hold_s: 12.0,
            np_size: 1.0,
            brand_on: false,
            brand_name: String::new(),
            brand_handles: String::new(),
            brand_logo: String::new(),
            brand_corner: 0,
            brand_size: 1.0,
            brand_opacity: 0.9,
            brand_color: "#40d9ff".into(),
            brand_logo_on: true,
            brand_name_on: true,
            ticker_on: false,
            ticker_text: String::new(),
            ticker_speed: 1.0,
            remote_on: false,
            remote_port: 9138,
            remote_pin: String::new(),
            osc_on: false,
            osc_port: 9139,
        }
    }
}

/// `trippin.json` in the working directory if one exists (handy when running
/// from the repo), otherwise the platform config dir (`%APPDATA%\Trippin`,
/// `~/Library/Application Support/Trippin`, `~/.config/trippin`) so an
/// installed copy can still save.
fn path() -> PathBuf {
    let local = PathBuf::from("trippin.json");
    if local.exists() {
        return local;
    }
    #[cfg(target_os = "windows")]
    let dir = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Trippin"));
    #[cfg(target_os = "macos")]
    let dir = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join("Library/Application Support/Trippin"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|d| d.join("trippin"));
    match dir {
        Some(dir) => {
            let _ = std::fs::create_dir_all(&dir);
            dir.join("trippin.json")
        }
        None => local,
    }
}

/// The folder `trippin.json` lives in — `nowplaying.txt` goes here too.
/// TLS for every HTTP client: trust what the OS trusts (macOS keychain,
/// Windows cert store). ureq's default is a bundled Mozilla root list,
/// which fails with `UnknownIssuer` on networks that inspect HTTPS with
/// their own root (antivirus web shields, VPNs, company proxies) even
/// though browsers there work. Downloads stay SHA-pinned regardless.
pub fn tls() -> ureq::tls::TlsConfig {
    ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build()
}

pub fn data_dir() -> PathBuf {
    match path().parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// A `snake_case` id → a human display title (`laser_show` → "Laser
/// Show"). Scene `// @title` headers and clip.json `title` fields
/// override this; it's the fallback so ids stay stable while the UI
/// reads like English. Acronyms get a fixed spelling rather than
/// naive capitalisation.
pub fn titleize(id: &str) -> String {
    const FIXED: [(&str, &str); 19] = [
        ("unity", "Unity"), ("vj", "VJ"), ("led", "LED"), ("rgb", "RGB"),
        ("uv", "UV"), ("bpm", "BPM"), ("ndi", "NDI"), ("osc", "OSC"),
        ("gpu", "GPU"), ("fx", "FX"), ("sdf", "SDF"), ("ascii", "ASCII"),
        ("io", "IO"), ("xr", "XR"), ("ai", "AI"), ("eq", "EQ"),
        ("vu", "VU"), ("kifs", "KIFS"), ("dj", "DJ"),
    ];
    id.split(['_', '-'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            FIXED
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(w))
                .map(|(_, v)| v.to_string())
                .unwrap_or_else(|| {
                    let mut c = w.chars();
                    match c.next() {
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        None => String::new(),
                    }
                })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Where timeline `.json` docs live — a `timelines/` dir next to
/// `trippin.json`.
pub fn timelines_dir() -> PathBuf {
    path()
        .parent()
        .map(|p| p.join("timelines"))
        .unwrap_or_else(|| PathBuf::from("timelines"))
}

impl Settings {
    pub fn load() -> Self {
        let mut s: Settings = std::fs::read_to_string(path())
            .map(|t| Settings::parse_lenient(&t))
            .unwrap_or_default();
        // Actions added in newer versions get their default key.
        for a in Action::ALL {
            s.keys
                .entry(a)
                .or_insert_with(|| a.default_key().to_string());
        }
        // macOS builds briefly defaulted the panel to P (F1 is a brightness
        // key on Touch Bar machines) — reverted; a saved "P" is that old
        // default, not a deliberate pick.
        #[cfg(target_os = "macos")]
        if s.keys.get(&Action::TogglePanel).is_some_and(|k| k == "P") {
            s.keys.insert(
                Action::TogglePanel,
                Action::TogglePanel.default_key().to_string(),
            );
        }
        // Retired model ids saved by older builds → back to the default.
        if s.ai_model == "gemini-2.5-flash" || s.ai_model == "claude-sonnet-4-5" {
            s.ai_model.clear();
        }
        // A blanked PIN must not silently open the remote to anyone.
        if s.remote_on && s.remote_pin.is_empty() {
            s.remote_pin = crate::remote::new_pin();
        }
        s
    }

    /// Parse trippin.json, keeping everything this build understands. A
    /// strict parse threw the whole file away on one unknown value (say, a
    /// hotkey for an action a newer build added), so every setting,
    /// including the Unity link, reset to its default. Now: hotkey and MIDI
    /// entries for unknown actions are dropped, then each top-level field
    /// that still doesn't fit is skipped on its own.
    pub fn parse_lenient(text: &str) -> Settings {
        use serde_json::Value;
        // Notepad (and PowerShell 5) save UTF-8 with a BOM.
        let text = text.trim_start_matches('\u{feff}');
        if let Ok(s) = serde_json::from_str::<Settings>(text) {
            return s;
        }
        let Ok(Value::Object(mut file)) = serde_json::from_str::<Value>(text) else {
            eprintln!("trippin.json isn't valid JSON: using defaults");
            return Settings::default();
        };
        for key in ["keys", "midi_notes"] {
            if let Some(Value::Object(m)) = file.get_mut(key) {
                m.retain(|name, _| serde_json::from_value::<Action>(Value::String(name.clone())).is_ok());
            }
        }
        let mut merged = serde_json::to_value(Settings::default()).unwrap_or(Value::Null);
        for (k, v) in file {
            let Value::Object(cur) = &merged else { break };
            let mut trial = cur.clone();
            trial.insert(k.clone(), v);
            let trial = Value::Object(trial);
            if serde_json::from_value::<Settings>(trial.clone()).is_ok() {
                merged = trial;
            } else {
                eprintln!("trippin.json: skipped \"{k}\" (not understood by this version)");
            }
        }
        serde_json::from_value(merged).unwrap_or_default()
    }

    pub fn save(&self) {
        match serde_json::to_string_pretty(self) {
            Ok(t) => {
                if let Err(e) = std::fs::write(path(), t) {
                    eprintln!("could not save trippin.json: {e}");
                }
            }
            Err(e) => eprintln!("could not serialise settings: {e}"),
        }
    }

    pub fn action_for(&self, key: &str) -> Option<Action> {
        self.keys
            .iter()
            .find(|(_, k)| k.as_str() == key)
            .map(|(a, _)| *a)
    }

    /// The action bound to a MIDI note number (any channel).
    pub fn midi_action_for(&self, note: u8) -> Option<Action> {
        self.midi_notes
            .iter()
            .find(|(_, n)| **n == note)
            .map(|(a, _)| *a)
    }
}

/// Stable, human-readable name for a key ("A", "Space", "ArrowRight", "F5").
pub fn key_name(key: &Key) -> Option<String> {
    match key {
        Key::Named(n) => Some(format!("{n:?}")),
        Key::Character(c) => Some(c.to_uppercase()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn look_actions_are_complete_unique_and_unbound() {
        let all: std::collections::BTreeSet<_> = Action::ALL.iter().collect();
        assert_eq!(all.len(), Action::ALL.len(), "no action listed twice");
        let slots: Vec<u8> = Action::ALL.iter().filter_map(|a| a.look_slot()).collect();
        assert_eq!(slots, (1..=crate::looks::SLOTS).collect::<Vec<_>>(), "Look1..8 map to slots 1..8, in order");
        for a in Action::ALL.iter().filter(|a| a.look_slot().is_some()) {
            assert_eq!(a.default_key(), "", "{a:?} must start unbound");
        }
        // An unbound action must never answer to a real key press.
        let s = Settings::default();
        assert!(s.action_for("A").is_some_and(|a| a.look_slot().is_none()));
    }

    #[test]
    fn settings_from_a_newer_build_keep_what_this_build_knows() {
        // A newer build's hotkey and an unknown enum value used to throw the
        // whole file away (unity_link silently back to off).
        let text = r#"{
            "unity_link": true,
            "phrase_bars": 8,
            "keys": { "NextScene": "J", "SomeFutureAction": "Q" },
            "mode": "SomeFutureMode",
            "a_field_from_the_future": 3
        }"#;
        let s = Settings::parse_lenient(text);
        assert!(s.unity_link);
        assert_eq!(s.phrase_bars, 8);
        assert_eq!(s.keys.get(&Action::NextScene).map(String::as_str), Some("J"));
        assert!(s.mode == Settings::default().mode);
    }

    #[test]
    fn month_day_matches_known_dates() {
        assert_eq!(month_day(1_790_380_800), (9, 26)); // 2026-09-26
        assert_eq!(month_day(1_709_164_800), (2, 29)); // 2024-02-29 (leap day)
        assert_eq!(month_day(1_767_139_200), (12, 31)); // 2025-12-31
        assert_eq!(month_day(1_767_225_600), (1, 1)); // 2026-01-01
        assert_eq!(month_day(951_868_800), (3, 1)); // 2000-03-01
    }

    #[test]
    fn titleize_makes_display_names() {
        assert_eq!(titleize("laser_show"), "Laser Show");
        assert_eq!(titleize("eq_bars"), "EQ Bars");
        assert_eq!(titleize("vu"), "VU");
        assert_eq!(titleize("unity_stage"), "Unity Stage");
        assert_eq!(titleize("kifs_cathedral"), "KIFS Cathedral");
        assert_eq!(titleize("led_wall"), "LED Wall");
        // A single word capitalises; acronyms only match whole components.
        assert_eq!(titleize("void"), "Void");
        assert_eq!(titleize("clouds"), "Clouds");
    }

    #[test]
    fn seasons() {
        assert_eq!(in_season("kaleido", (10, 31)), None);
        assert_eq!(in_season("halloween", (10, 31)), Some(true));
        assert_eq!(in_season("halloween", (9, 26)), Some(false));
        assert_eq!(in_season("christmas", (12, 24)), Some(true));
        assert_eq!(in_season("fireworks", (11, 5)), Some(true));
        assert_eq!(in_season("fireworks", (12, 31)), Some(true));
        assert_eq!(in_season("fireworks", (1, 1)), Some(true));
        assert_eq!(in_season("fireworks", (1, 3)), Some(false));
    }
}
