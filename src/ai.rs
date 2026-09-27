//! AI show builder — "analyse this track and write the cue list for me".
//!
//! The provider never hears audio: a local analysis pass computes per-bar
//! energy / onset / vocal / air features plus structural markers (sections,
//! builds, drops), that summary goes to the model, and the model returns
//! cues in a small JSON dialect mapped onto `CueKind`. BYOAI: Anthropic,
//! OpenAI, Gemini, and any OpenAI-compatible endpoint (Groq, Mistral,
//! Ollama…) — keys live in `trippin.json` (gitignored) or the provider's
//! usual env var.

use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::audio::{FFT_SIZE, HOP};
use crate::config::{Fx, Mode, Tristate};
use crate::text::{TextPos, TextStyle};
use crate::timeline::{Clip, Cue, CueKind};

// ---------------------------------------------------------------------------
// Provider configuration
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiProvider {
    #[default]
    Anthropic,
    OpenAi,
    Gemini,
    /// Any OpenAI-compatible `/v1/chat/completions` endpoint — Groq, Mistral,
    /// Ollama, LM Studio…
    Compatible,
}

impl AiProvider {
    pub const ALL: [AiProvider; 4] = [
        Self::Anthropic,
        Self::OpenAi,
        Self::Gemini,
        Self::Compatible,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic (Claude)",
            Self::OpenAi => "OpenAI",
            Self::Gemini => "Google Gemini",
            Self::Compatible => "OpenAI-compatible",
        }
    }

    pub fn default_endpoint(self) -> &'static str {
        match self {
            Self::Anthropic => "https://api.anthropic.com/v1/messages",
            Self::OpenAi => "https://api.openai.com/v1/chat/completions",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta",
            Self::Compatible => "http://localhost:11434/v1/chat/completions",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => "claude-sonnet-4-5",
            Self::OpenAi => "gpt-4o",
            Self::Gemini => "gemini-3.8-flash",
            Self::Compatible => "",
        }
    }

    /// Env vars tried when the settings key field is blank.
    pub fn env_keys(self) -> &'static [&'static str] {
        match self {
            Self::Anthropic => &["TRIPPIN_AI_KEY", "ANTHROPIC_API_KEY"],
            Self::OpenAi => &["TRIPPIN_AI_KEY", "OPENAI_API_KEY"],
            Self::Gemini => &["TRIPPIN_AI_KEY", "GEMINI_API_KEY", "GOOGLE_API_KEY"],
            Self::Compatible => &["TRIPPIN_AI_KEY", "OPENAI_API_KEY"],
        }
    }
}

/// A snapshot of the provider fields — taken on the UI thread, used by the
/// worker so no lock is held across the network call.
#[derive(Clone)]
pub struct AiConf {
    pub provider: AiProvider,
    pub endpoint: String,
    pub model: String,
    pub key: String,
}

impl AiConf {
    pub fn from_settings(s: &crate::config::Settings) -> Self {
        let p = s.ai_provider;
        // Paste-proofing: strip quotes, whitespace and a stray "Bearer ".
        let clean = |v: String| {
            let v = v.trim().trim_matches('"').trim_matches('\'').trim();
            v.strip_prefix("Bearer ").unwrap_or(v).trim().to_string()
        };
        let key = if s.ai_key.trim().is_empty() {
            p.env_keys()
                .iter()
                .find_map(|k| std::env::var(k).ok())
                .map(clean)
                .filter(|v| !v.is_empty())
                .unwrap_or_default()
        } else {
            clean(s.ai_key.clone())
        };
        Self {
            provider: p,
            endpoint: if s.ai_endpoint.trim().is_empty() {
                p.default_endpoint().into()
            } else {
                s.ai_endpoint.trim().into()
            },
            model: if s.ai_model.trim().is_empty() {
                p.default_model().into()
            } else {
                s.ai_model.trim().into()
            },
            key,
        }
    }
}

// ---------------------------------------------------------------------------
// Local analysis — energy / onsets / vocal / air per bar, plus structure.
// ---------------------------------------------------------------------------

/// Per-bar features and detected structure for one clip.
pub struct ClipAnalysis {
    pub clip: usize,
    pub name: String,
    pub bpm: f64,
    pub bars: usize,
    pub beats: f64,
    /// Mean RMS per bar, normalised 0..1.
    pub energy: Vec<f32>,
    /// Onset density per bar (fraction of hops above an adaptive floor), 0..1.
    pub onsets: Vec<f32>,
    /// Vocal likelihood 0..1 (tonal, sustained mid-band energy).
    pub vocal: Vec<f32>,
    /// >5 kHz energy share per bar — risers and bright sections lift this.
    pub air: Vec<f32>,
    /// Labelled spans: (kind, from_bar, to_bar inclusive).
    pub sections: Vec<(String, usize, usize)>,
    /// The same labels unmerged, one per ~4-bar phrase block. This is the
    /// granularity the model plans at.
    pub blocks: Vec<(String, usize, usize)>,
    /// Bar indices where a drop lands.
    pub drops: Vec<usize>,
    /// (from_bar, to_bar) build-ups preceding drops.
    pub builds: Vec<(usize, usize)>,
    /// (from_bar, to_bar) vocal-heavy spans.
    pub vocals: Vec<(usize, usize)>,
}

/// Full analysis from decoded audio (`Song::load` result).
pub fn analyze_song(clip: usize, song: &crate::song::Song) -> ClipAnalysis {
    let (rms, mid_share, air_share, mid_flat) = band_frames(&song.mono, song.sr);
    let vocal = vocal_envelope(&rms, &mid_share, &mid_flat);
    let hops_per_beat = song.sr as f64 / HOP as f64 / (song.bpm / 60.0);
    let beats = ((song.duration - song.first_beat).max(0.0)) * song.bpm / 60.0;
    let bars = (beats / 4.0).ceil() as usize;
    let hop_at_beat =
        |b: f64| ((song.first_beat + b * 60.0 / song.bpm) * song.sr as f64 / HOP as f64) as usize;

    let mut energy = vec![0.0f32; bars];
    let mut onsets = vec![0.0f32; bars];
    let mut voc = vec![0.0f32; bars];
    let mut air = vec![0.0f32; bars];
    let on_floor = onset_floor(&song.onsets);
    for bar in 0..bars {
        let h0 = hop_at_beat(bar as f64 * 4.0);
        let h1 = hop_at_beat(bar as f64 * 4.0 + 4.0).min(rms.len());
        if h1 <= h0 {
            break;
        }
        energy[bar] = mean(&rms[h0..h1]);
        onsets[bar] = rms[h0..h1]
            .iter()
            .enumerate()
            .filter(|(i, _)| song.onsets.get(h0 + i).is_some_and(|&v| v > on_floor))
            .count() as f32
            / (h1 - h0) as f32;
        voc[bar] = mean(&vocal[h0.min(vocal.len() - 1)..h1.min(vocal.len())]);
        air[bar] = mean(&air_share[h0.min(air_share.len() - 1)..h1.min(air_share.len())]);
    }
    let _ = hops_per_beat;
    norm_inplace(&mut energy);
    norm_inplace(&mut onsets);
    norm_inplace(&mut voc);
    finish_analysis(clip, &song.name, song.bpm, beats, energy, onsets, voc, air)
}

/// Analysis from a clip's cached strip data (used when the audio file is
/// missing): energy from the ~1600-point overview, onsets from the stored
/// envelope. Vocal/air can't be recovered — reported as silence.
pub fn analyze_cached(clip: usize, c: &Clip) -> ClipAnalysis {
    let beats = ((c.duration_s - c.first_beat).max(0.0)) * c.bpm / 60.0;
    let bars = (beats / 4.0).ceil() as usize;
    let mut energy = vec![0.0f32; bars];
    let mut onsets = vec![0.0f32; bars];
    let on_floor = onset_floor(&c.onsets);
    for bar in 0..bars {
        // Overview is a fixed-res strip, not beat-aligned — map by position.
        let f0 = (bar as f64 / bars as f64 * c.overview.len() as f64) as usize;
        let f1 = (((bar + 1) as f64 / bars as f64) * c.overview.len() as f64) as usize;
        if f1 > f0 {
            energy[bar] = mean(&c.overview[f0..f1.min(c.overview.len())]);
        }
        let h0 = (c.first_beat + bar as f64 * 4.0 * 60.0 / c.bpm) * c.onset_fps;
        let h1 = (c.first_beat + (bar as f64 + 1.0) * 4.0 * 60.0 / c.bpm) * c.onset_fps;
        let (h0, h1) = (h0 as usize, (h1 as usize).min(c.onsets.len()));
        if h1 > h0 {
            onsets[bar] = c.onsets[h0..h1].iter().filter(|&&v| v > on_floor).count() as f32
                / (h1 - h0) as f32;
        }
    }
    norm_inplace(&mut energy);
    norm_inplace(&mut onsets);
    finish_analysis(
        clip,
        &c.name,
        c.bpm,
        beats,
        energy,
        onsets,
        vec![0.0; bars],
        vec![0.0; bars],
    )
}

/// One FFT pass per HOP: RMS plus band shares and mid-band spectral flatness
/// (the vocal heuristic's raw material).
fn band_frames(mono: &[f32], sr: u32) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    let fft = FftPlanner::new().plan_fft_forward(FFT_SIZE);
    let window: Vec<f32> = (0..FFT_SIZE)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos())
        .collect();
    let hz = sr as f32 / FFT_SIZE as f32;
    let mid = |i: usize| (300.0..3400.0).contains(&(i as f32 * hz));
    let bass = |i: usize| i as f32 * hz < 150.0;
    let air = |i: usize| i as f32 * hz > 5000.0;
    let n = mono.len() / HOP;
    let (mut rms, mut mid_s, mut air_s, mut flat) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    let mut spec = vec![Complex::new(0.0f32, 0.0); FFT_SIZE];
    for h in 0..n {
        let frame = &mono[h * HOP..(h * HOP + FFT_SIZE).min(mono.len())];
        let mut e = 0.0f32;
        for (i, (&s, &w)) in frame.iter().zip(&window).enumerate() {
            e += s * s;
            spec[i] = Complex::new(s * w, 0.0);
        }
        for c in &mut spec[frame.len()..] {
            *c = Complex::new(0.0, 0.0);
        }
        rms.push((e / FFT_SIZE as f32).sqrt());
        fft.process(&mut spec);
        let (mut lo, mut mi, mut ai, mut mid_g, mut mid_a, mut mid_n) =
            (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0usize);
        for (i, c) in spec[..FFT_SIZE / 2].iter().enumerate() {
            let m = c.norm();
            if bass(i) {
                lo += m;
            } else if mid(i) {
                mi += m;
                mid_g += (m + 1e-6).ln();
                mid_a += m;
                mid_n += 1;
            } else if air(i) {
                ai += m;
            }
        }
        let total = lo + mi + ai + 1e-9;
        // Shares are meaningless in silence (noise floor dominates) — gate
        // them by the frame's own loudness.
        let gate = ((e / FFT_SIZE as f32).sqrt() * 25.0).min(1.0);
        mid_s.push(mi / total * gate);
        air_s.push(ai / total * gate);
        flat.push(if mid_n > 0 {
            ((mid_g / mid_n as f32).exp()) / (mid_a / mid_n as f32 + 1e-9)
        } else {
            1.0
        });
    }
    (rms, mid_s, air_s, flat)
}

/// Sustained *tonal* mid-band energy = vocals (and sustained synth leads —
/// close enough for arrangement purposes). Smoothed over ~1 s.
fn vocal_envelope(rms: &[f32], mid_share: &[f32], flat: &[f32]) -> Vec<f32> {
    let n = rms.len();
    let mut out = vec![0.0f32; n];
    let mut ema = 0.0f32;
    let alpha = 0.08f32; // ~1 s at 86 hops/sec
    for i in 0..n {
        let raw = mid_share[i] * (1.0 - flat[i]).max(0.0) * (rms[i] * 8.0).min(1.0);
        ema += alpha * (raw - ema);
        out[i] = ema;
    }
    out
}

/// Onset threshold: median + spread so quiet sections still register.
fn onset_floor(env: &[f32]) -> f32 {
    if env.is_empty() {
        return f32::MAX;
    }
    let mut v: Vec<f32> = env.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let med = v[v.len() / 2];
    med + (v[(v.len() as f32 * 0.9) as usize] - med) * 0.3
}

/// Sections + drop/build/vocal markers from the per-bar arrays.
fn finish_analysis(
    clip: usize,
    name: &str,
    bpm: f64,
    beats: f64,
    energy: Vec<f32>,
    onsets: Vec<f32>,
    vocal: Vec<f32>,
    air: Vec<f32>,
) -> ClipAnalysis {
    let bars = energy.len();
    // Smoothed contour for structure decisions (keep raw `energy` for output).
    let sm: Vec<f32> = (0..bars)
        .map(|b| mean(&energy[b.saturating_sub(1)..(b + 2).min(bars)]))
        .collect();

    // Drops: raw energy leaps to ≥70% of peak after a quieter patch — the raw
    // value, not the smoothed one, so a sharp 1-2 bar climax still counts.
    let mut drops = Vec::new();
    for b in 1..bars {
        if energy[b] >= 0.7 {
            let from = b.saturating_sub(6);
            let before = mean(&sm[from..b]);
            if energy[b] - before >= 0.3 && drops.last().is_none_or(|&d| b - d >= 8) {
                drops.push(b);
            }
        }
    }
    // No contrast big enough to call a drop — give the model the loudest bar
    // anyway so the show still has a climax.
    if drops.is_empty() && bars >= 8 {
        if let Some((b, &v)) = energy.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)) {
            if v > 0.6 {
                drops.push(b);
            }
        }
    }
    // Builds: a ≥2-bar rising run immediately preceding a drop.
    let mut builds = Vec::new();
    for &d in &drops {
        let mut s = d;
        while s > 0 && sm[s - 1] < sm[s] - 0.01 && d - s < 6 {
            s -= 1;
        }
        if d - s >= 2 {
            builds.push((s, d.saturating_sub(1)));
        }
    }
    // Vocal spans: ≥4-bar runs over the floor.
    let mut vocals = Vec::new();
    let mut run = None;
    for b in 0..bars {
        if vocal[b] > 0.65 {
            run.get_or_insert(b);
        } else if let Some(s) = run.take() {
            if b - s >= 4 {
                vocals.push((s, b - 1));
            }
        }
    }
    if let Some(s) = run.take().filter(|s| bars - s >= 4) {
        vocals.push((s, bars - 1));
    }

    // Sections: 4-bar blocks — musical phrases are 8/16 beats anyway, and
    // block-grain keeps the labels clean (no 1-bar thrash between classes).
    let mut sections: Vec<(String, usize, usize)> = Vec::new();
    let mut blocks: Vec<(String, usize, usize)> = Vec::new();
    let mut seen_loud = false;
    let mut b = 0usize;
    while b < bars {
        let e = (b + 4).min(bars) - 1;
        // A short tail merges into the previous block rather than becoming
        // its own mislabelled sliver.
        if e + 1 == bars && e - b + 1 < 4 && !sections.is_empty() {
            let prev = sections.last_mut().unwrap();
            prev.2 = e;
            if let Some(bk) = blocks.last_mut() {
                bk.2 = e;
            }
            // A quiet tail reads as the outro, not another breakdown.
            if prev.0 == "breakdown" {
                prev.0 = "outro".into();
            }
            break;
        }
        let m = mean(&sm[b..=e]);
        let has_drop = drops.iter().any(|&d| (b..=e).contains(&d));
        let has_build = builds.iter().any(|&(s, _)| (b..=e).contains(&s));
        let kind = if m < 0.08 {
            "silent"
        } else if m < 0.35 {
            if !seen_loud {
                "intro"
            } else if e + 1 == bars {
                "outro"
            } else {
                "breakdown"
            }
        } else if has_drop {
            "drop"
        } else if has_build {
            "build"
        } else if m >= 0.75 {
            "peak"
        } else {
            "groove"
        };
        if matches!(kind, "drop" | "peak" | "groove") {
            seen_loud = true;
        }
        blocks.push((kind.to_string(), b, e));
        match sections.last_mut() {
            Some(prev) if prev.0 == kind => prev.2 = e,
            _ => sections.push((kind.to_string(), b, e)),
        }
        b = e + 1;
    }

    ClipAnalysis {
        clip,
        name: name.into(),
        bpm,
        bars,
        beats,
        energy,
        onsets,
        vocal,
        air,
        sections,
        blocks,
        drops,
        builds,
        vocals,
    }
}

fn mean(v: &[f32]) -> f32 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f32>() / v.len() as f32
    }
}
/// 2-dp rounded as f64 — serializes "0.03", not "0.0299999993".
fn r2(v: &[f32]) -> Vec<f64> {
    v.iter()
        .map(|x| ((*x as f64) * 100.0).round() / 100.0)
        .collect()
}
fn norm_inplace(v: &mut [f32]) {
    let m = v.iter().cloned().fold(0.0f32, f32::max);
    if m > 1e-9 {
        v.iter_mut().for_each(|x| *x /= m);
    }
}

// ---------------------------------------------------------------------------
// The show request: prompt → provider → cues.
// ---------------------------------------------------------------------------

/// Analyse every clip, ask the provider for a cue list, return the cues
/// plus a human-readable note of what was analysed and any skipped items.
pub fn build_show(
    clips: &[Clip],
    scenes: &[String],
    routines: &[String],
    conf: &AiConf,
) -> Result<(Vec<Cue>, String)> {
    if clips.is_empty() {
        return Err(anyhow!("no clips on the timeline"));
    }
    let mut notes = Vec::new();
    let mut analyses = Vec::new();
    for (i, c) in clips.iter().enumerate() {
        match crate::song::load(&c.song) {
            Ok(song) => analyses.push(analyze_song(i, &song)),
            Err(e) => {
                analyses.push(analyze_cached(i, c));
                notes.push(format!(
                    "{}: audio unreadable ({e:#}); used cached strip",
                    c.name
                ));
            }
        }
    }
    let system = system_prompt();
    let user = user_prompt(&analyses, scenes, routines);
    let text = request_json(conf, &system, &user)
        .with_context(|| format!("{} request failed", conf.provider.label()))?;
    let (cues, mut warnings) = parse_response(&text, clips, &analyses, scenes, routines)?;
    notes.append(&mut warnings);
    let note = if notes.is_empty() {
        format!("{} cues", cues.len())
    } else {
        format!("{} cues — {}", cues.len(), notes.join("; "))
    };
    Ok((cues, note))
}

fn system_prompt() -> String {
    "You are the show director for Trippin, a music-reactive DJ visuals app. For each song \
clip you are given a flow map (one char per bar: `.` silent, `-` low, `+` mid, `*` high, \
`#` peak energy; a `v` on the vocals line marks sung bars) and a `blocks` table that \
splits the track into ~4-bar phrases, each labelled intro / groove / build / drop / \
peak / breakdown / outro / silent with its mean energy. You direct the show BLOCK BY \
BLOCK — that is the job: make a deliberate choice for every phrase.\n\
\n\
Output ONLY a JSON object {\"plan\":[...]} — no prose, no markdown. One entry per block, \
in order, covering EVERY block of every clip:\n\
{\n\
  \"clip\": 0,       // clip index, optional (default 0)\n\
  \"block\": 3,      // the block's \"i\" from the table — required\n\
  \"scene\": \"name\", // REQUIRED: scene for this block (ONLY names from the scene list)\n\
  \"dancer\": \"off\" | \"on\" | \"ROUTINE_NAME\",  // optional; routine name implies on\n\
  \"look\": 0|1|2,   // optional: 0=shadow, 1=neon, 2=strobe\n\
  \"trails\": true|false, \"canon\": \"auto\"|\"on\"|\"off\", // optional dancer extras\n\
  \"fx\": \"off\"|\"mirror_x\"|\"mirror_y\"|\"quad\"|\"kaleido6\"|\"kaleido8\"|\"auto\", // optional\n\
  \"blackout\": true, // optional: 1-beat dip to black just before the block starts\n\
  \"text\": {\"text\":\"...\",\"style\":\"neon|fire|wave|glitch|pulse|chrome\",\"pos\":\"top|middle|bottom\",\"lane\":0|1}\n\
}\n\
Omit optional fields when there's nothing to change — they all latch.\n\
\n\
Show-craft rules:\n\
- EVERY block gets a plan entry, even repeats — no gaps. Scenes latch, so the last \
block's scene plays the song out.\n\
- Vary the scene with the music: a new scene whenever the block's kind or energy level \
changes is the norm; 2-3 consecutive blocks on one scene is the MAXIMUM, and only inside \
one long uniform section. Across a whole track expect 5-8 different scenes.\n\
- Match mood to energy: sparse/dark scenes for intro/breakdown/outro; the most intense \
scenes for drop/peak. Read the flow map — it tells you the song's shape.\n\
- The final block may be scene \"void\" ONLY when it's the song's actual outro/fade — \
never void mid-track, never as a transition.\n\
- dancer: \"off\" for intro, breakdowns and outro; on for groove/build/drop/peak, \
especially vocal spans. She should be off for roughly a third of a typical track — \
never on start to finish. Name a ROUTINE (from the routine list) at least every other \
time she comes back on so the movement varies — never one routine for the whole track.\n\
- builds: brighten/accelerate — trails on, canon on, or a sharper scene.\n\
- drop blocks: the scene change lands ON the drop; an optional \"blackout\":true on the \
block just before reads well.\n\
- vocal spans (v in the vocals map): dancer + \"look\":1 (neon) reads well.\n\
- fx: use sparingly — a transform for a drop or a kaleido for a trippy breakdown, then \
back to \"off\". Never left on for the whole track.\n\
- text: 0-2 cards for the whole track, meaningful words only (song/artist name, a \
shout-out). NEVER literal labels like \"DROP\" or \"BUILD\". When in doubt, omit.\n\
- Contrast beats chaos — don't stack dancer+trails+canon+fx on every block.\n\
\n\
Example excerpt (blocks 0-7 of a clip — note the per-block decisions, dancer toggling, \
routines changing):\n\
{\"plan\":[\n\
 {\"block\":0,\"scene\":\"aurora\",\"dancer\":\"off\",\"text\":{\"text\":\"NORTHERN\",\"style\":\"neon\",\"pos\":\"top\"}},\n\
 {\"block\":1,\"scene\":\"levels\"},\n\
 {\"block\":2,\"scene\":\"pulse_grid\",\"dancer\":\"stock_disco\"},\n\
 {\"block\":3,\"scene\":\"pulse_grid\",\"trails\":true},\n\
 {\"block\":4,\"scene\":\"rave_hall\",\"dancer\":\"stock_heels\",\"look\":1,\"blackout\":true},\n\
 {\"block\":5,\"scene\":\"chrome_bloom\",\"canon\":\"on\"},\n\
 {\"block\":6,\"scene\":\"aurora\",\"dancer\":\"off\",\"fx\":\"kaleido6\"},\n\
 {\"block\":7,\"scene\":\"gyroid_drift\",\"fx\":\"off\"}\n\
]}"
        .to_string()
}

/// `--analyze <file>`: decode + analyse one track and print the summary the
/// model would see — the prompt-tuning / sanity-check path.
pub fn analyze_file(path: &std::path::Path) -> Result<String> {
    let song = crate::song::load(path)?;
    let a = analyze_song(0, &song);
    let payload = serde_json::to_string_pretty(&json!({
        "name": a.name, "bpm": (a.bpm * 10.0).round() / 10.0,
        "bars": a.bars, "beats": a.beats.round() as i64,
        "flow": flow_map(&a.energy),
        "vocals": vocal_map(&a.vocal),
        "sections": a.sections.iter().map(|(k,s,e)| json!({"kind":k,"from":s,"to":e})).collect::<Vec<_>>(),
        "blocks": a.blocks.iter().enumerate().map(|(i,(k,s,e))| json!({"i":i,"kind":k,"bars":[s,e]})).collect::<Vec<_>>(),
        "drops": a.drops,
        "builds": a.builds.iter().map(|(s,e)| json!([s,e])).collect::<Vec<_>>(),
        "vocal_spans": a.vocals.iter().map(|(s,e)| json!([s,e])).collect::<Vec<_>>(),
        "energy": r2(&a.energy), "onsets": r2(&a.onsets),
        "vocal": r2(&a.vocal), "air": r2(&a.air),
    }))?;
    Ok(payload)
}

/// Per-bar energy as a single string — a contour the model can read at a
/// glance instead of inferring from a float array. `.` silent, `-` low,
/// `+` mid, `*` high, `#` peak.
fn flow_map(energy: &[f32]) -> String {
    energy
        .iter()
        .map(|&e| {
            if e < 0.08 {
                '.'
            } else if e < 0.3 {
                '-'
            } else if e < 0.55 {
                '+'
            } else if e < 0.8 {
                '*'
            } else {
                '#'
            }
        })
        .collect()
}

/// `v` where the vocal likelihood is high — reads as a second lane under
/// the flow map.
fn vocal_map(vocal: &[f32]) -> String {
    vocal
        .iter()
        .map(|&v| if v > 0.65 { 'v' } else { ' ' })
        .collect()
}

fn user_prompt(analyses: &[ClipAnalysis], scenes: &[String], routines: &[String]) -> String {
    let clips: Vec<Value> = analyses
        .iter()
        .map(|a| {
            json!({
                "clip": a.clip, "name": a.name, "bpm": (a.bpm * 10.0).round() / 10.0,
                "bars": a.bars,
                "beats": a.beats.round() as i64,
                "flow": flow_map(&a.energy),
                "vocals": vocal_map(&a.vocal),
                "blocks": a.blocks.iter().enumerate().map(|(i, (k, s, e))| json!({
                    "i": i, "kind": k, "bars": [s, e],
                    "beats": [s * 4, (e + 1) * 4 - 1],
                    "energy": (mean(&a.energy[*s..=(*e).min(a.energy.len() - 1)]) * 100.0).round() / 100.0,
                })).collect::<Vec<_>>(),
                "drops": a.drops, "builds": a.builds.iter().map(|(s,e)| json!([s,e])).collect::<Vec<_>>(),
                "vocal_spans": a.vocals.iter().map(|(s,e)| json!([s,e])).collect::<Vec<_>>(),
            })
        })
        .collect();
    serde_json::to_string(&json!({
        "scenes": scenes,
        "routines": routines,
        "note": "flow/vocals are one char per bar: energy . - + * # and v for vocals. \
                 blocks are the ~4-bar phrases you plan against — every block needs an entry.",
        "clips": clips,
    }))
    .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Provider calls — one function per API shape; all blocking, run off-thread.
// ---------------------------------------------------------------------------

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(180)))
        .http_status_as_error(false)
        .build()
        .into()
}

/// POST a JSON body; retries 429/5xx and transport errors with backoff —
/// overloaded models answer 503 "high demand" and a couple of retries ride
/// through it (this runs on the worker thread, so sleeping is fine).
/// Anything else (401/404/400…) is fatal immediately.
fn post(url: &str, headers: &[(&str, &str)], body: &Value) -> Result<Value> {
    let mut last = String::new();
    for attempt in 0..3 {
        let mut req = agent().post(url);
        for &(k, v) in headers {
            req = req.header(k, v);
        }
        match req.send_json(body) {
            Ok(mut resp) => {
                let status = resp.status();
                let text = resp.body_mut().read_to_string().unwrap_or_default();
                if status.is_success() {
                    return serde_json::from_str(&text).context("parsing response JSON");
                }
                last = format!("HTTP {status}: {}", &text[..text.len().min(400)]);
                if status.as_u16() != 429 && !status.is_server_error() {
                    return Err(anyhow!(last));
                }
            }
            Err(e) => last = format!("{e}"),
        }
        if attempt < 2 {
            std::thread::sleep(Duration::from_secs(2 << attempt)); // 2 s, 4 s
        }
    }
    Err(anyhow!(last))
}

fn request_json(conf: &AiConf, system: &str, user: &str) -> Result<String> {
    match conf.provider {
        AiProvider::Anthropic => anthropic(conf, system, user),
        AiProvider::Gemini => gemini(conf, system, user),
        AiProvider::OpenAi | AiProvider::Compatible => chat_completions(conf, system, user),
    }
}

fn anthropic(conf: &AiConf, system: &str, user: &str) -> Result<String> {
    if conf.key.is_empty() {
        return Err(anyhow!(
            "no API key — set it in the dialog or ANTHROPIC_API_KEY"
        ));
    }
    if conf.model.is_empty() {
        return Err(anyhow!("no model set"));
    }
    let body = json!({
        "model": conf.model,
        "max_tokens": 8192,
        "system": system,
        "messages": [{"role": "user", "content": [{"type": "text", "text": user}]}],
        "tools": [{
            "name": "emit_plan",
            "description": "Emit the designed show as the per-block plan",
            "input_schema": {
                "type": "object",
                "properties": {
                    "plan": {"type": "array", "items": {"type": "object"}},
                    "cues": {"type": "array", "items": {"type": "object"}}
                },
                "required": ["plan"]
            }
        }],
        "tool_choice": {"type": "tool", "name": "emit_plan"}
    });
    let v = post(
        &conf.endpoint,
        &[
            ("x-api-key", conf.key.as_str()),
            ("anthropic-version", "2023-06-01"),
        ],
        &body,
    )?;
    // Preferred: the forced tool_use block; fallback: any text block.
    for c in v["content"].as_array().cloned().unwrap_or_default() {
        if c["type"].as_str() == Some("tool_use") {
            return Ok(c["input"].to_string());
        }
    }
    for c in v["content"].as_array().cloned().unwrap_or_default() {
        if let Some(t) = c["text"].as_str() {
            return Ok(t.to_string());
        }
    }
    Err(anyhow!("empty response"))
}

fn chat_completions(conf: &AiConf, system: &str, user: &str) -> Result<String> {
    if conf.provider == AiProvider::OpenAi && conf.key.is_empty() {
        return Err(anyhow!(
            "no API key — set it in the dialog or OPENAI_API_KEY"
        ));
    }
    if conf.model.is_empty() {
        return Err(anyhow!("no model set"));
    }
    let mut body = json!({
        "model": conf.model,
        "temperature": 0.6,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]
    });
    // `response_format` is real-OpenAI-only; compatible endpoints vary.
    if conf.provider == AiProvider::OpenAi {
        body["response_format"] = json!({"type": "json_object"});
    }
    let auth = format!("Bearer {}", conf.key);
    let headers: &[(&str, &str)] = if conf.key.is_empty() {
        &[]
    } else {
        &[("authorization", auth.as_str())]
    };
    let v = post(&conf.endpoint, headers, &body)?;
    v["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("no choices in response"))
}

fn gemini(conf: &AiConf, system: &str, user: &str) -> Result<String> {
    if conf.key.is_empty() {
        return Err(anyhow!(
            "no API key — set it in the dialog or GEMINI_API_KEY"
        ));
    }
    if conf.model.is_empty() {
        return Err(anyhow!("no model set"));
    }
    let url = format!(
        "{}/models/{}:generateContent",
        conf.endpoint.trim_end_matches('/'),
        conf.model
    );
    let body = json!({
        "system_instruction": {"parts": [{"text": system}]},
        "contents": [{"role": "user", "parts": [{"text": user}]}],
        // No temperature override — Gemini 3.x docs warn it can degrade output.
        "generationConfig": {"responseMimeType": "application/json"}
    });
    let v = post(&url, &[("x-goog-api-key", conf.key.as_str())], &body)?;
    let mut out = String::new();
    if let Some(parts) = v["candidates"][0]["content"]["parts"].as_array() {
        for p in parts {
            if let Some(t) = p["text"].as_str() {
                out.push_str(t);
            }
        }
    }
    if out.is_empty() {
        return Err(anyhow!("empty response"));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Response → cues. Tolerant: markdown fences, stringly bools, fuzzy names.
// ---------------------------------------------------------------------------

/// Full response parse: prefers the per-block `plan`, merges any extra
/// free-form `cues` (models sometimes emit both), and applies the shared
/// post-pass rules. `analyses` carries the block tables the plan indexes.
pub fn parse_response(
    text: &str,
    clips: &[Clip],
    analyses: &[ClipAnalysis],
    scenes: &[String],
    routines: &[String],
) -> Result<(Vec<Cue>, Vec<String>)> {
    let v = extract_json(text)?;
    let mut cues = Vec::new();
    let mut warnings = Vec::new();

    let mut got_plan = false;
    if let Some(plan) = v["plan"].as_array() {
        got_plan = !plan.is_empty();
        expand_plan(plan, analyses, scenes, routines, &mut cues, &mut warnings);
    }
    if let Some(arr) = v["cues"].as_array() {
        for (i, item) in arr.iter().enumerate() {
            match raw_to_cue(item, clips, scenes, routines) {
                Ok(c) => cues.push(c),
                Err(e) => warnings.push(format!("cue {i}: {e}")),
            }
        }
    } else if !got_plan {
        return Err(anyhow!("response has no \"plan\" or \"cues\" array"));
    }

    cues.sort_by(|a, b| a.clip.cmp(&b.clip).then_with(|| a.beat.total_cmp(&b.beat)));
    enforce_void(&mut cues, clips, &mut warnings);
    if cues.is_empty() {
        return Err(anyhow!("no usable cues in response"));
    }
    Ok((cues, warnings))
}

/// Legacy/raw cue-array parse (tests; `parse_response` covers production).
#[cfg(test)]
pub fn parse_cues(
    text: &str,
    clips: &[Clip],
    scenes: &[String],
    routines: &[String],
) -> Result<(Vec<Cue>, Vec<String>)> {
    let v = extract_json(text)?;
    let arr = v["cues"]
        .as_array()
        .ok_or_else(|| anyhow!("response has no \"cues\" array"))?;
    let mut cues = Vec::new();
    let mut warnings = Vec::new();
    for (i, item) in arr.iter().enumerate() {
        match raw_to_cue(item, clips, scenes, routines) {
            Ok(c) => cues.push(c),
            Err(e) => warnings.push(format!("cue {i}: {e}")),
        }
    }
    cues.sort_by(|a, b| a.clip.cmp(&b.clip).then_with(|| a.beat.total_cmp(&b.beat)));
    enforce_void(&mut cues, clips, &mut warnings);
    if cues.is_empty() {
        return Err(anyhow!("no usable cues in response"));
    }
    Ok((cues, warnings))
}

fn extract_json(text: &str) -> Result<Value> {
    let s = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```");
    let s = s.trim_end_matches("```").trim();
    let start = s
        .find('{')
        .ok_or_else(|| anyhow!("no JSON object in response"))?;
    let end = s.rfind('}').ok_or_else(|| anyhow!("unterminated JSON"))?;
    serde_json::from_str(&s[start..=end]).context("response is not valid JSON")
}

/// Void discipline: scene cues LATCH, so a "void" cue blacks out everything
/// after it until the next scene. Void is only legal as a clip's FINAL
/// scene cue, starting in its last ~8 beats (the end fade). Anything else
/// leaves the song unlit — drop it.
fn enforce_void(cues: &mut Vec<Cue>, clips: &[Clip], warnings: &mut Vec<String>) {
    for (ci, clip) in clips.iter().enumerate() {
        let total = clip.beat_at(clip.duration_s);
        let last_scene = cues
            .iter()
            .rposition(|c| c.clip == ci && matches!(c.kind, CueKind::Scene(_)));
        let voids: Vec<usize> = cues
            .iter()
            .enumerate()
            .filter(|(_, c)| c.clip == ci && matches!(&c.kind, CueKind::Scene(n) if n == "void"))
            .map(|(i, _)| i)
            .collect();
        // Remove back-to-front so earlier indices stay valid.
        for pos in voids.into_iter().rev() {
            let c = &cues[pos];
            let legal = Some(pos) == last_scene && c.beat >= total - 8.0;
            if !legal {
                warnings.push(format!(
                    "dropped void at clip {ci} beat {:.0} — void only plays a song out",
                    c.beat
                ));
                cues.remove(pos);
            }
        }
    }
}

/// Expand the model's per-block plan into cues. The plan says what SHOULD
/// be true at each phrase block; we emit cues only when the latched state
/// actually changes, and hard-cap how long one scene may run.
fn expand_plan(
    plan: &[Value],
    analyses: &[ClipAnalysis],
    scenes: &[String],
    routines: &[String],
    cues: &mut Vec<Cue>,
    warnings: &mut Vec<String>,
) {
    /// Blocks of identical scene before a forced change (~12 bars).
    const MAX_SCENE_RUN: usize = 3;
    /// Dancer-on blocks between routine rotations (~32 bars).
    const ROUTINE_STALE: usize = 8;

    for a in analyses {
        // Index this clip's plan entries by block number.
        let mut by_block: Vec<Option<&Value>> = vec![None; a.blocks.len()];
        for item in plan {
            let clip = item["clip"].as_u64().unwrap_or(0) as usize;
            if clip != a.clip {
                continue;
            }
            let bi = item["block"]
                .as_u64()
                .map(|b| b as usize)
                .or_else(|| item["b"].as_u64().map(|b| b as usize));
            match bi {
                Some(b) if b < a.blocks.len() => by_block[b] = Some(item),
                _ => warnings.push(format!("plan: unknown block index {:?}", item["block"])),
            }
        }

        // Latched state — emit a cue only on change.
        let mut scene = String::new();
        let mut run = 0usize;
        let mut chosen: Vec<String> = Vec::new(); // scenes the model picked, for forced cuts
        let mut palette = 0usize; // deterministic fallback rotation
        let mut dancer = false;
        let mut routine = String::new();
        let mut routine_age = 0usize;
        let mut look: Option<Option<usize>> = None;
        let mut trails: Option<bool> = None;
        let mut canon: Option<Tristate> = None;
        let mut fx = Fx::Off;
        let mut fx_auto = false;

        for (bi, (_, from_bar, to_bar)) in a.blocks.iter().enumerate() {
            let beat = (*from_bar * 4) as f64;
            let span = ((to_bar - from_bar + 1) * 4) as f64;
            let item = by_block[bi];

            // --- scene: required per block; missing field = keep current
            let want = item.and_then(|it| it["scene"].as_str()).map(|n| {
                find_name(n, scenes).unwrap_or_else(|| {
                    warnings.push(format!("plan: unknown scene {n:?} at block {bi}"));
                    scene.clone()
                })
            });
            let effective = want.unwrap_or_else(|| scene.clone());
            if effective == scene {
                run += 1;
            } else {
                run = 1;
            }
            let mut emit = effective.clone();
            if run > MAX_SCENE_RUN && !effective.is_empty() {
                // Force a cut: prefer the model's least-recent other pick,
                // else rotate the palette away from the current scene.
                emit = chosen
                    .iter()
                    .rev()
                    .find(|s| **s != effective)
                    .cloned()
                    .unwrap_or_else(|| {
                        let mut s = effective.clone();
                        for _ in 0..scenes.len().max(1) {
                            palette = (palette + 1) % scenes.len().max(1);
                            if scenes[palette] != effective {
                                s = scenes[palette].clone();
                                break;
                            }
                        }
                        s
                    });
                warnings.push(format!(
                    "plan: '{effective}' ran {run} blocks — cut to '{emit}' at block {bi}"
                ));
                run = 1;
            }
            if !emit.is_empty() && emit != scene {
                // A void on the final block is the end fade — place it in
                // the last couple of bars so enforce_void keeps it.
                let at = if emit == "void" && bi + 1 == a.blocks.len() {
                    (a.beats - 8.0).max(beat)
                } else {
                    beat
                };
                cues.push(Cue {
                    clip: a.clip,
                    beat: at,
                    beats: a.beats - at,
                    kind: CueKind::Scene(emit.clone()),
                });
                scene = emit.clone();
                if emit != "void" && !chosen.contains(&emit) {
                    chosen.push(emit);
                }
            }

            let Some(it) = item else { continue };

            // --- dancer: "off" | "on" | routine name
            match it["dancer"].as_str() {
                Some("off") => {
                    if dancer {
                        dancer = false;
                        cues.push(Cue {
                            clip: a.clip,
                            beat,
                            beats: 4.0,
                            kind: CueKind::Dancer(false),
                        });
                    }
                }
                Some("on") => {
                    if !dancer {
                        dancer = true;
                        cues.push(Cue {
                            clip: a.clip,
                            beat,
                            beats: 4.0,
                            kind: CueKind::Dancer(true),
                        });
                    }
                }
                Some(name) => {
                    if !dancer {
                        dancer = true;
                        cues.push(Cue {
                            clip: a.clip,
                            beat,
                            beats: 4.0,
                            kind: CueKind::Dancer(true),
                        });
                    }
                    match find_name(name, routines) {
                        Some(r) if r != routine => {
                            routine = r.clone();
                            routine_age = 0;
                            cues.push(Cue {
                                clip: a.clip,
                                beat,
                                beats: 4.0,
                                kind: CueKind::Clip(r),
                            });
                        }
                        Some(_) => {}
                        None => {
                            warnings.push(format!("plan: unknown routine {name:?} at block {bi}"))
                        }
                    }
                }
                None => {
                    if it["dancer"].as_bool() == Some(true) && !dancer {
                        dancer = true;
                        cues.push(Cue {
                            clip: a.clip,
                            beat,
                            beats: 4.0,
                            kind: CueKind::Dancer(true),
                        });
                    } else if it["dancer"].as_bool() == Some(false) && dancer {
                        dancer = false;
                        cues.push(Cue {
                            clip: a.clip,
                            beat,
                            beats: 4.0,
                            kind: CueKind::Dancer(false),
                        });
                    }
                }
            }
            if dancer {
                routine_age += 1;
                if routine_age > ROUTINE_STALE {
                    routine_age = 0;
                    routine.clear(); // unknown until the engine picks next
                    cues.push(Cue {
                        clip: a.clip,
                        beat,
                        beats: 4.0,
                        kind: CueKind::NextClip,
                    });
                }
            } else {
                routine_age = 0;
            }

            // --- latched extras, emit on change only
            if let Some(l) = it.get("look").filter(|v| !v.is_null()) {
                let want = l
                    .as_u64()
                    .map(|n| Some((n as usize).min(crate::dancer::STYLES.len() - 1)));
                if want != look {
                    look = want;
                    cues.push(Cue {
                        clip: a.clip,
                        beat,
                        beats: 4.0,
                        kind: CueKind::Look(want.flatten()),
                    });
                }
            }
            if let Some(t) = it["trails"].as_bool() {
                if Some(t) != trails {
                    trails = Some(t);
                    cues.push(Cue {
                        clip: a.clip,
                        beat,
                        beats: 4.0,
                        kind: CueKind::Trails(t),
                    });
                }
            }
            if let Some(c) = it["canon"].as_str() {
                let want = match c {
                    "on" => Tristate::On,
                    "off" => Tristate::Off,
                    _ => Tristate::Auto,
                };
                if Some(want) != canon {
                    canon = Some(want);
                    cues.push(Cue {
                        clip: a.clip,
                        beat,
                        beats: 4.0,
                        kind: CueKind::Canon(want),
                    });
                }
            }
            if let Some(f) = it["fx"].as_str() {
                match f {
                    "auto" => {
                        if !fx_auto {
                            fx_auto = true;
                            cues.push(Cue {
                                clip: a.clip,
                                beat,
                                beats: 4.0,
                                kind: CueKind::FxAuto(true),
                            });
                        }
                    }
                    name => {
                        let want = match name {
                            "mirror_x" | "mirrorx" | "mirror x" => Fx::MirrorX,
                            "mirror_y" | "mirrory" | "mirror y" => Fx::MirrorY,
                            "quad" => Fx::Quad,
                            "kaleido6" | "kaleido_6" | "kaleidoscope6" => Fx::Kaleido6,
                            "kaleido8" | "kaleido_8" | "kaleidoscope8" | "kaleido" => Fx::Kaleido8,
                            _ => Fx::Off,
                        };
                        if fx_auto {
                            fx_auto = false;
                            cues.push(Cue {
                                clip: a.clip,
                                beat,
                                beats: 4.0,
                                kind: CueKind::FxAuto(false),
                            });
                        }
                        if want != fx {
                            fx = want;
                            cues.push(Cue {
                                clip: a.clip,
                                beat,
                                beats: 4.0,
                                kind: CueKind::Fx(want),
                            });
                        }
                    }
                }
            }

            // --- one-shots
            if it["blackout"].as_bool() == Some(true) {
                cues.push(Cue {
                    clip: a.clip,
                    beat: (beat - 1.0).max(0.0),
                    beats: 1.0,
                    kind: CueKind::Blackout(true),
                });
            }
            if let Some(t) = it.get("text") {
                let mut t = t.clone();
                t["kind"] = json!("text");
                t["beat"] = json!(beat);
                t["beats"] = json!(span);
                match raw_to_cue(&t, &[], scenes, routines) {
                    Ok(mut c) => {
                        c.clip = a.clip;
                        cues.push(c);
                    }
                    Err(e) => warnings.push(format!("plan: {e}")),
                }
            }
        }
    }
}

fn raw_to_cue(item: &Value, clips: &[Clip], scenes: &[String], routines: &[String]) -> Result<Cue> {
    let get = |k: &str| item.get(k).filter(|v| !v.is_null());
    let kind = get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing kind"))?
        .to_lowercase()
        .replace(['-', ' '], "_");
    let clip = get("clip")
        .and_then(as_f64)
        .map(|c| c as usize)
        .unwrap_or(0);
    if clip >= clips.len().max(1) && !clips.is_empty() {
        return Err(anyhow!("clip index {clip} out of range"));
    }
    let beat = get("beat").and_then(as_f64).unwrap_or(0.0).max(0.0);
    let beats = get("beats")
        .and_then(as_f64)
        .unwrap_or(4.0)
        .clamp(0.25, 512.0);
    let max_beat = clips
        .get(clip)
        .map(|c| c.beat_at(c.duration_s))
        .unwrap_or(f64::MAX);
    if beat >= max_beat {
        return Err(anyhow!("beat {beat} past clip end ({max_beat:.0})"));
    }
    let on = || get("on").and_then(as_bool).unwrap_or(true);
    let kind = match kind.as_str() {
        "scene" | "scene_cut" | "visual" => CueKind::Scene(
            find_name(
                get("name")
                    .or_else(|| get("scene"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("scene needs a name"))?,
                scenes,
            )
            .ok_or_else(|| anyhow!("unknown scene"))?,
        ),
        "next_scene" => CueKind::NextScene,
        "prev_scene" | "previous_scene" => CueKind::PrevScene,
        "mode" => CueKind::Mode(match str_field(get("mode")).as_deref() {
            Some("auto") => Mode::Auto,
            Some("static") => Mode::Static,
            _ => Mode::Manual,
        }),
        "dancer" | "dancer_on" | "dancer_off" => {
            CueKind::Dancer(if kind.ends_with("_off") { false } else { on() })
        }
        "routine" | "clip" => CueKind::Clip(
            find_name(
                get("name")
                    .or_else(|| get("routine"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("routine needs a name"))?,
                routines,
            )
            .ok_or_else(|| anyhow!("unknown routine"))?,
        ),
        "next_routine" | "next_clip" => CueKind::NextClip,
        "next_look" | "next_style" => CueKind::NextLook,
        "look" | "style" => CueKind::Look(
            get("look")
                .or_else(|| get("style_index"))
                .and_then(as_f64)
                .map(|l| (l as usize).min(crate::dancer::STYLES.len() - 1)),
        ),
        "trails" => CueKind::Trails(on()),
        "canon" => CueKind::Canon(match str_field(get("canon")).as_deref() {
            Some("on") => Tristate::On,
            Some("off") => Tristate::Off,
            _ => Tristate::Auto,
        }),
        "blackout" | "black_out" => CueKind::Blackout(on()),
        "fx" | "transform" | "effect" => CueKind::Fx(
            match str_field(get("fx"))
                .or_else(|| get("name").and_then(|v| str_field(Some(v))))
                .as_deref()
            {
                Some("mirror_x" | "mirrorx" | "mirror x") => Fx::MirrorX,
                Some("mirror_y" | "mirrory" | "mirror y") => Fx::MirrorY,
                Some("quad") => Fx::Quad,
                Some("kaleido6" | "kaleido_6" | "kaleidoscope6") => Fx::Kaleido6,
                Some("kaleido8" | "kaleido_8" | "kaleidoscope8" | "kaleido") => Fx::Kaleido8,
                _ => Fx::Off,
            },
        ),
        "fx_auto" | "auto_fx" => CueKind::FxAuto(on()),
        "text" | "text_card" => {
            let text = get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("text cue needs text"))?
                .to_string();
            CueKind::Text(crate::text::TextSpec {
                text,
                style: match str_field(get("style")).as_deref() {
                    Some("fire") => TextStyle::Fire,
                    Some("wave") => TextStyle::Wave,
                    Some("glitch") => TextStyle::Glitch,
                    Some("pulse") => TextStyle::Pulse,
                    Some("chrome") => TextStyle::Chrome,
                    _ => TextStyle::Neon,
                },
                pos: match str_field(get("pos"))
                    .or_else(|| str_field(get("position")))
                    .as_deref()
                {
                    Some("top") => TextPos::Top,
                    Some("bottom") => TextPos::Bottom,
                    _ => TextPos::Center,
                },
                lane: get("lane")
                    .and_then(as_f64)
                    .map(|l| (l as u8) % crate::text::TEXT_SLOTS as u8)
                    .unwrap_or(0),
            })
        }
        other => return Err(anyhow!("unknown kind {other:?}")),
    };
    Ok(Cue {
        clip,
        beat,
        beats,
        kind,
    })
}

fn str_field(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str)
        .map(|s| s.to_lowercase().replace(['-', ' '], "_"))
}

fn as_f64(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.trim().parse().ok())
}

fn as_bool(v: &Value) -> Option<bool> {
    v.as_bool()
        .or_else(|| match v.as_str()?.to_lowercase().as_str() {
            "true" | "on" | "yes" | "1" => Some(true),
            "false" | "off" | "no" | "0" => Some(false),
            _ => None,
        })
}

/// Exact → case-insensitive → prefix match against the palette — separators
/// normalised so "Rave Hall" finds `rave_hall`.
fn find_name(want: &str, pool: &[String]) -> Option<String> {
    let norm = |s: &str| s.trim().to_lowercase().replace(['-', ' ', '.'], "_");
    let w = norm(want);
    pool.iter()
        .find(|n| n.as_str() == want)
        .or_else(|| pool.iter().find(|n| norm(n) == w))
        .or_else(|| pool.iter().find(|n| norm(n).starts_with(&w)))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip() -> Clip {
        Clip {
            song: "x.mp3".into(),
            name: "x".into(),
            offset_s: 0.0,
            bpm: 120.0,
            first_beat: 0.0,
            duration_s: 64.0, // 128 beats
            onsets: vec![],
            onset_fps: 0.0,
            overview: vec![],
        }
    }

    #[test]
    fn parses_a_messy_response() {
        let scenes = vec!["rave_hall".to_string(), "void".to_string()];
        let routines = vec!["stock_arms".to_string()];
        let text = r#"```json
{"cues": [
  {"beat": 0, "beats": 16, "kind": "scene", "name": "Rave Hall"},
  {"beat": 16, "kind": "dancer", "on": true},
  {"beat": 20, "kind": "routine", "name": "stock_arms"},
  {"beat": 24, "beats": 1, "kind": "blackout", "on": "on"},
  {"beat": 32, "kind": "fx", "fx": "kaleido8"},
  {"beat": 40, "beats": 8, "kind": "text", "text": "LET'S GO", "style": "glitch", "pos": "top", "lane": 1},
  {"beat": 48, "kind": "scene", "name": "bogus_scene"},
  {"beat": 999, "kind": "scene", "name": "void"},
  {"kind": "trails", "on": "yes"}
]}
```"#;
        let (cues, warnings) =
            parse_cues(text, &[clip()], &scenes, &routines).expect("should parse");
        // bogus scene + out-of-range beat dropped, the rest survive.
        assert_eq!(cues.len(), 7);
        assert_eq!(warnings.len(), 2);
        assert!(matches!(&cues[0].kind, CueKind::Scene(n) if n == "rave_hall"));
        // trails defaults to beat 0 → sorts alongside the opening scene.
        assert!(matches!(cues[1].kind, CueKind::Trails(true)));
        assert!(matches!(cues[2].kind, CueKind::Dancer(true)));
        assert!(matches!(&cues[3].kind, CueKind::Clip(n) if n == "stock_arms"));
        assert!(matches!(cues[4].kind, CueKind::Blackout(true)));
        assert!(matches!(cues[5].kind, CueKind::Fx(Fx::Kaleido8)));
        assert!(
            matches!(&cues[6].kind, CueKind::Text(s) if s.text == "LET'S GO" && s.lane == 1 && s.style == TextStyle::Glitch)
        );
    }

    #[test]
    fn rejects_empty_cue_list() {
        assert!(parse_cues("{\"cues\":[]}", &[clip()], &[], &[]).is_err());
    }

    fn analysis() -> ClipAnalysis {
        // 8 blocks × 4 bars = 32 bars = 128 beats, matching clip().
        let kinds = [
            "intro",
            "groove",
            "groove",
            "build",
            "drop",
            "peak",
            "breakdown",
            "outro",
        ];
        ClipAnalysis {
            clip: 0,
            name: "x".into(),
            bpm: 120.0,
            bars: 32,
            beats: 128.0,
            energy: vec![0.5; 32],
            onsets: vec![0.5; 32],
            vocal: vec![0.0; 32],
            air: vec![0.0; 32],
            sections: vec![],
            blocks: kinds
                .iter()
                .enumerate()
                .map(|(i, k)| (k.to_string(), i * 4, i * 4 + 3))
                .collect(),
            drops: vec![16],
            builds: vec![],
            vocals: vec![],
        }
    }

    #[test]
    fn plan_expands_to_cues() {
        let scenes = vec![
            "aurora".to_string(),
            "rave_hall".to_string(),
            "levels".to_string(),
            "void".to_string(),
        ];
        let routines = vec!["stock_arms".to_string(), "stock_heels".to_string()];
        let text = r#"{"plan":[
          {"block":0,"scene":"aurora","dancer":"off"},
          {"block":1,"scene":"aurora","dancer":"stock_arms"},
          {"block":2,"scene":"aurora"},
          {"block":3,"scene":"aurora"},
          {"block":4,"scene":"Rave Hall","look":1,"blackout":true},
          {"block":5,"scene":"levels","dancer":"stock_heels","trails":true,"fx":"kaleido6"},
          {"block":6,"scene":"levels","dancer":"off","fx":"off"},
          {"block":7,"scene":"void"}
        ]}"#;
        let (cues, warnings) = parse_response(text, &[clip()], &[analysis()], &scenes, &routines)
            .expect("should parse");

        // Block 4 forced a cut: aurora ran 4 blocks > MAX_SCENE_RUN.
        let scene_names: Vec<&str> = cues
            .iter()
            .filter_map(|c| match &c.kind {
                CueKind::Scene(n) => Some(n.as_str()),
                _ => None,
            })
            .collect();
        // aurora (block 0), forced cut to rave_hall at block 3 (the palette
        // fallback happens to be the model's block-4 pick, so no extra cue),
        // levels (5), void (7).
        assert_eq!(scene_names, vec!["aurora", "rave_hall", "levels", "void"]);
        // Final void lands inside the last 8 beats (128 - 8 = 120).
        let void_cue = cues
            .iter()
            .find(|c| matches!(&c.kind, CueKind::Scene(n) if n == "void"))
            .unwrap();
        assert!(void_cue.beat >= 120.0);

        // Dancer: on with stock_arms at block 1, routine change to
        // stock_heels at block 5, off at block 6.
        assert!(cues.iter().any(|c| matches!(c.kind, CueKind::Dancer(true))));
        assert!(
            cues.iter()
                .any(|c| matches!(c.kind, CueKind::Dancer(false)))
        );
        assert!(
            cues.iter()
                .any(|c| matches!(&c.kind, CueKind::Clip(n) if n == "stock_heels"))
        );
        assert!(
            cues.iter()
                .any(|c| matches!(c.kind, CueKind::Look(Some(1))))
        );
        assert!(cues.iter().any(|c| matches!(c.kind, CueKind::Trails(true))));
        assert!(
            cues.iter()
                .any(|c| matches!(c.kind, CueKind::Fx(Fx::Kaleido6)))
        );
        assert!(cues.iter().any(|c| matches!(c.kind, CueKind::Fx(Fx::Off))));
        // The pre-drop blackout dips one beat before block 4 (beat 63).
        assert!(
            cues.iter()
                .any(|c| matches!(c.kind, CueKind::Blackout(true)) && c.beat == 63.0)
        );
        assert!(warnings.iter().any(|w| w.contains("ran 4 blocks")));
    }

    #[test]
    fn void_only_at_song_end() {
        // 128-beat clip: void at 24 (mid-song) and 100 (early tail) are
        // dropped; the fade at 122 survives as the final scene cue.
        let scenes = vec!["rave_hall".to_string(), "void".to_string()];
        let text = r#"{"cues":[
          {"beat":0,"kind":"scene","name":"rave_hall"},
          {"beat":24,"kind":"scene","name":"void"},
          {"beat":32,"kind":"scene","name":"rave_hall"},
          {"beat":100,"kind":"scene","name":"void"},
          {"beat":108,"kind":"scene","name":"rave_hall"},
          {"beat":122,"kind":"scene","name":"void"}
        ]}"#;
        let (cues, warnings) = parse_cues(text, &[clip()], &scenes, &[]).expect("should parse");
        let voids: Vec<f64> = cues
            .iter()
            .filter(|c| matches!(&c.kind, CueKind::Scene(n) if n == "void"))
            .map(|c| c.beat)
            .collect();
        assert_eq!(voids, vec![122.0]);
        assert_eq!(warnings.len(), 2);
    }
}
