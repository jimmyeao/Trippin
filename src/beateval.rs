//! `trippin --beat-eval rec.wav [from-to]`: score the live beat tracker on a
//! recording against Beat This! run over the whole file (the "ground truth",
//! cached next to the file as `<name>.gt.tsv`).
//!
//! The analyser is fed the file sample by sample, exactly as live capture
//! feeds it, and its beat position is read at each ground-truth beat, in
//! the newest sample's time, as the render loop would see it before the
//! latency setting. Reported:
//! - phase error (ms; negative = the live beat lands *after* the real one),
//! - slips (the live count not advancing by exactly one per real beat),
//! - bar accuracy (the live "one" on the real downbeat) and bar jumps,
//! - phrase breaks: anything that moves the live bar grid against the real
//!   one — what restarts the phrase counter mid-section.
//!
//! The model checks run in step with the audio (`nn_sync`), which is slightly
//! kinder than live, where a window's result lands ~0.5 s later.

use super::*;
use crate::beats::Beat;
use std::path::Path;

/// One analysis hop: newest-sample time, live beat position, downbeat slot,
/// tempo, calm, coasting.
struct Hop {
    t: f64,
    pos: f64,
    down: u64,
    bpm: f32,
    calm: f32,
    coast: bool,
}

fn ground_truth(path: &Path, mono: &[f32], sr: u32) -> Result<Vec<Beat>> {
    let cache = path.with_extension("gt.tsv");
    if let Ok(txt) = std::fs::read_to_string(&cache) {
        let v: Vec<Beat> = txt
            .lines()
            .filter_map(|l| {
                let mut it = l.split('\t');
                Some(Beat {
                    t: it.next()?.parse().ok()?,
                    conf: it.next()?.parse().ok()?,
                    down: it.next()? == "1",
                })
            })
            .collect();
        if !v.is_empty() {
            return Ok(v);
        }
    }
    crate::beats::wait_ready()?;
    let t0 = Instant::now();
    let beats = crate::beats::Tracker::load()?.detect(mono, sr)?;
    println!("ground truth: {} beats in {:.1}s", beats.len(), t0.elapsed().as_secs_f32());
    let txt: String = beats
        .iter()
        .map(|b| format!("{:.4}\t{:.3}\t{}\n", b.t, b.conf, b.down as u8))
        .collect();
    let _ = std::fs::write(&cache, txt);
    Ok(beats)
}

/// The live analyser's log over a run, plus the auto-pilot on default
/// settings fed every hop: its cuts and phrase re-anchors show what the
/// phrasing does on this audio.
struct Sim {
    hops: Vec<Hop>,
    dir: crate::director::Director,
    settings: crate::config::Settings,
    usable: Vec<usize>,
    /// (t, pos, bars since the last cut)
    cuts: Vec<(f64, f64, i64)>,
    /// (t, anchor shift in beats)
    anchors: Vec<(f64, f64)>,
    last_anchor: f64,
    last_cut_pos: f64,
    from: f64,
    to: f64,
}

impl Sim {
    fn new(span: Option<(f64, f64)>) -> Self {
        let (from, to) = span.unwrap_or((0.0, f64::MAX));
        Self {
            hops: Vec::new(),
            dir: crate::director::Director::new(),
            settings: crate::config::Settings::default(),
            usable: (0..24).collect(),
            cuts: Vec::new(),
            anchors: Vec::new(),
            last_anchor: f64::NAN,
            last_cut_pos: 0.0,
            from,
            to,
        }
    }

    /// One analysis hop whose newest sample is at `t` seconds.
    fn hop(&mut self, a: &Analyzer, t: f64) {
        let pos = a.beat_count as f64 + a.phase as f64;
        if t >= self.from && t <= self.to {
            let ev = self.dir.update(&a.f, pos, 1.0 / a.fps, &self.usable, &self.settings);
            if ev.cut {
                self.cuts.push((t, pos, ((pos - self.last_cut_pos) / 4.0).round() as i64));
                self.last_cut_pos = pos;
            }
            let anc = self.dir.phrase_anchor();
            if anc.is_finite() && anc != self.last_anchor {
                if self.last_anchor.is_finite() {
                    self.anchors.push((t, anc - self.last_anchor));
                }
                self.last_anchor = anc;
            }
        }
        self.hops.push(Hop {
            t,
            pos,
            down: a.downbeat,
            bpm: a.f.bpm,
            calm: a.f.calm,
            coast: a.coasting,
        });
    }
}

pub fn beat_eval(path: &Path, span: Option<(f64, f64)>) -> Result<()> {
    let (mono, sr) = crate::song::decode(path)?;
    let (_tx, rx) = mpsc::channel();
    let shared: SharedFeatures = Arc::new(Mutex::new(Features::default()));
    let mut a = Analyzer::new(sr as f32, shared, rx, None);
    a.nn_sync = true;
    // TRIPPIN_NO_NN=1: the fallback without the model (onset comb only).
    if std::env::var_os("TRIPPIN_NO_NN").is_some() {
        crate::beats::set_enabled(false);
    }
    let mut sim = Sim::new(span);
    let t0 = Instant::now();
    for (i, &s) in mono.iter().enumerate() {
        if a.push(s) {
            sim.hop(&a, (i + 1) as f64 / sr as f64);
        }
    }
    println!(
        "{}: {:.0}s analysed in {:.1}s",
        path.display(),
        mono.len() as f64 / sr as f64,
        t0.elapsed().as_secs_f32()
    );
    score(path, &mono, sr, sim)
}

/// `--beat-eval-live <device|""> <secs> out.wav`: the real-time path — the
/// analyser runs on live capture with the neural windows on their own
/// thread (results land ~0.5 s late, as in the app) while the input is
/// recorded; then the live log is scored against the recording.
pub fn beat_eval_live(device: Option<&str>, secs: f32, out: &Path) -> Result<()> {
    if std::env::var_os("TRIPPIN_NO_NN").is_some() {
        crate::beats::set_enabled(false);
    } else {
        crate::beats::wait_ready()?;
    }
    let (rx, sr, name, _backend, _dead) = AudioEngine::open(device, true)?;
    println!("Live: {secs:.0} s from {name} ({sr} Hz) -> {}", out.display());
    let (_ctx, crx) = mpsc::channel();
    let shared: SharedFeatures = Arc::new(Mutex::new(Features::default()));
    let mut a = Analyzer::new(sr, shared, crx, None);
    let mut sim = Sim::new(None);
    let total = (secs * sr) as usize;
    let mut mono: Vec<f32> = Vec::with_capacity(total);
    let mut next_note = sr as usize * 60;
    while mono.len() < total {
        let chunk = rx.recv_timeout(Duration::from_secs(5))?;
        for s in chunk {
            mono.push(s);
            if a.push(s) {
                sim.hop(&a, mono.len() as f64 / sr as f64);
            }
        }
        if mono.len() >= next_note {
            println!("  {} min  {:.1} BPM  bar {}", mono.len() / (sr as usize * 60), a.f.bpm, a.downbeat);
            next_note += sr as usize * 60;
        }
    }
    write_wav(out, sr as u32, &mono)?;
    score(out, &mono, sr as u32, sim)
}

/// 32-bit float mono WAV.
fn write_wav(path: &Path, sr: u32, mono: &[f32]) -> Result<()> {
    use std::io::Write;
    let n = mono.len() as u32;
    let mut w = std::io::BufWriter::new(std::fs::File::create(path)?);
    w.write_all(b"RIFF")?;
    w.write_all(&(36 + n * 4).to_le_bytes())?;
    w.write_all(b"WAVEfmt ")?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&3u16.to_le_bytes())?;
    w.write_all(&1u16.to_le_bytes())?;
    w.write_all(&sr.to_le_bytes())?;
    w.write_all(&(sr * 4).to_le_bytes())?;
    w.write_all(&4u16.to_le_bytes())?;
    w.write_all(&32u16.to_le_bytes())?;
    w.write_all(b"data")?;
    w.write_all(&(n * 4).to_le_bytes())?;
    for s in mono {
        w.write_all(&s.to_le_bytes())?;
    }
    w.flush()?;
    Ok(())
}

fn score(path: &Path, mono: &[f32], sr: u32, sim: Sim) -> Result<()> {
    let gt = ground_truth(path, mono, sr)?;
    // TRIPPIN_WIN_TEST=1: the model on 15 s windows (as live) against the
    // model on the whole file — does the window shift its beat times?
    if std::env::var_os("TRIPPIN_WIN_TEST").is_some() {
        let mut tr = crate::beats::Tracker::load()?;
        let w = (15.0 * sr as f32) as usize;
        let mut start = 0usize;
        let mut all: Vec<f64> = Vec::new();
        while start + w <= mono.len() {
            let beats = tr.detect(&mono[start..start + w], sr)?;
            let mut d: Vec<f64> = beats
                .iter()
                .filter(|b| b.conf >= 0.5 && b.t > 1.0 && b.t < 14.0)
                .filter_map(|b| {
                    let t = start as f64 / sr as f64 + b.t as f64;
                    let g = gt.iter().filter(|g| g.conf >= 0.5).min_by(|x, y| ((x.t as f64 - t).abs()).total_cmp(&(y.t as f64 - t).abs()))?;
                    let e = (t - g.t as f64) * 1000.0;
                    (e.abs() < 60.0).then_some(e)
                })
                .collect();
            d.sort_by(f64::total_cmp);
            if !d.is_empty() {
                println!("win {:5.0}s  {} beats  window - whole file: median {:+.1} ms", start as f64 / sr as f64, d.len(), d[d.len() / 2]);
            }
            all.extend(d);
            start += (5.0 * sr as f32) as usize;
        }
        all.sort_by(f64::total_cmp);
        if !all.is_empty() {
            println!("window vs whole file: median {:+.1} ms over {} beats", all[all.len() / 2], all.len());
        }
    }
    let Sim { hops, cuts, anchors, from, to, .. } = sim;
    let hop_at = |t: f64| -> Option<(f64, &Hop)> {
        let i = hops.partition_point(|h| h.t <= t);
        if i == 0 || i >= hops.len() {
            return None;
        }
        let (h0, h1) = (&hops[i - 1], &hops[i]);
        let f = (t - h0.t) / (h1.t - h0.t);
        // Interpolate within the hop, but not across a jump.
        let pos = if (h1.pos - h0.pos).abs() < 0.2 { h0.pos + (h1.pos - h0.pos) * f } else { h0.pos };
        Some((pos, h0))
    };

    // Confident ground-truth beats, with the local beat period (median of
    // the surrounding intervals) and a ground-truth beat index that steps
    // over beats the model missed.
    let conf: Vec<&Beat> = gt.iter().filter(|b| b.conf >= 0.5).collect();
    let ibis: Vec<f64> = conf.windows(2).map(|w| (w[1].t - w[0].t) as f64).collect();
    let local_ibi = |i: usize| -> f64 {
        let lo = i.saturating_sub(8);
        let hi = (i + 8).min(ibis.len());
        let mut v: Vec<f64> = ibis[lo..hi].to_vec();
        v.sort_by(|a, b| a.total_cmp(b));
        v.get(v.len() / 2).copied().unwrap_or(0.5)
    };

    struct Row {
        t: f64,
        err_ms: f64,
        err_b: f64,
        calm: bool,
        coast: bool,
    }
    let mut rows: Vec<Row> = Vec::new();
    let mut slips: Vec<(f64, i64)> = Vec::new();
    let mut bar_jumps: Vec<(f64, i64, i64)> = Vec::new();
    let (mut downs, mut downs_ok, mut downs_calm, mut downs_calm_ok) = (0u32, 0u32, 0u32, 0u32);
    let mut prev: Option<(f64, i64, f64)> = None; // (t, live n, gt ibi)
    let mut prev_k: Option<i64> = None;
    let mut win_bpm_err: Vec<f64> = Vec::new();
    let mut level_skip = 0u32;
    for (i, b) in conf.iter().enumerate() {
        let t = b.t as f64;
        if t < from || t > to {
            continue;
        }
        let Some((pos, h)) = hop_at(t) else { continue };
        let ibi = local_ibi(i);
        // The reference at another metrical level (double-time hats, a
        // half-time read) can't score the live beat: count it, skip it.
        if ((60.0 / ibi) / h.bpm as f64 - 1.0).abs() > 0.04 {
            level_skip += 1;
            prev = None;
            continue;
        }
        let n = pos.round() as i64;
        let e = pos - n as f64;
        rows.push(Row { t, err_ms: e * ibi * 1000.0, err_b: e, calm: h.calm > 0.5, coast: h.coast });
        win_bpm_err.push(h.bpm as f64 - 60.0 / ibi);
        if let Some((pt, pn, pibi)) = prev {
            // Consecutive real beats (no model gap) must advance by one.
            let steps = ((t - pt) / pibi).round() as i64;
            if steps >= 1 && steps <= 8 {
                let d = n - pn;
                if d != steps {
                    slips.push((t, d - steps));
                }
            }
        }
        prev = Some((t, n, ibi));
        if b.down {
            let k = (n - h.down as i64).rem_euclid(4);
            downs += 1;
            downs_ok += (k == 0) as u32;
            if h.calm > 0.5 {
                downs_calm += 1;
                downs_calm_ok += (k == 0) as u32;
            }
            if let Some(pk) = prev_k {
                if pk != k {
                    bar_jumps.push((t, pk, k));
                }
            }
            prev_k = Some(k);
        }
    }
    if rows.is_empty() {
        println!("no ground-truth beats in range");
        return Ok(());
    }

    // A timeline every 15 s.
    println!("\n    t   live  real   err ms (med  |abs|)  hit%  bar  calm coast  slips jumps");
    let mut w = rows[0].t - rows[0].t % 15.0;
    let end = rows.last().unwrap().t;
    while w <= end {
        let r: Vec<&Row> = rows.iter().filter(|r| r.t >= w && r.t < w + 15.0).collect();
        if !r.is_empty() {
            let mut e: Vec<f64> = r.iter().map(|r| r.err_ms).collect();
            e.sort_by(|a, b| a.total_cmp(b));
            let med = e[e.len() / 2];
            let abs = e.iter().map(|v| v.abs()).sum::<f64>() / e.len() as f64;
            let hit = r.iter().filter(|r| r.err_ms.abs() <= 35.0).count() as f64 / r.len() as f64;
            let mid = w + 7.5;
            let (live_bpm, calm, coast, k) = hop_at(mid)
                .map(|(p, h)| (h.bpm, h.calm, h.coast, ((p.round() as i64 - h.down as i64).rem_euclid(4))))
                .unwrap_or((0.0, 0.0, false, 0));
            let i = conf.partition_point(|b| (b.t as f64) < mid).min(ibis.len().saturating_sub(1));
            let real = 60.0 / local_ibi(i);
            let s = slips.iter().filter(|s| s.0 >= w && s.0 < w + 15.0).count();
            let j = bar_jumps.iter().filter(|s| s.0 >= w && s.0 < w + 15.0).count();
            let _ = k;
            let bar_ok = {
                let d: Vec<&&Beat> = conf.iter().filter(|b| b.down && (b.t as f64) >= w && (b.t as f64) < w + 15.0).collect();
                let ok = d
                    .iter()
                    .filter(|b| {
                        hop_at(b.t as f64).is_some_and(|(p, h)| (p.round() as i64 - h.down as i64).rem_euclid(4) == 0)
                    })
                    .count();
                if d.is_empty() { "  -".to_string() } else { format!("{:3.0}", 100.0 * ok as f64 / d.len() as f64) }
            };
            println!(
                "{:5.0}  {:5.1} {:5.1}   {:+6.1} {:6.1}   {:4.0}  {}%  {:.2} {:5}  {:5} {:5}",
                w,
                live_bpm,
                real,
                med,
                abs,
                hit * 100.0,
                bar_ok,
                calm,
                if coast { "yes" } else { "" },
                s,
                j
            );
        }
        w += 15.0;
    }

    let summary = |name: &str, f: &dyn Fn(&Row) -> bool| {
        let r: Vec<&Row> = rows.iter().filter(|r| f(r)).collect();
        if r.is_empty() {
            return;
        }
        let mut e: Vec<f64> = r.iter().map(|r| r.err_ms).collect();
        e.sort_by(|a, b| a.total_cmp(b));
        let med = e[e.len() / 2];
        let mean = e.iter().sum::<f64>() / e.len() as f64;
        let abs = e.iter().map(|v| v.abs()).sum::<f64>() / e.len() as f64;
        let hit = r.iter().filter(|r| r.err_ms.abs() <= 35.0).count() as f64 / r.len() as f64;
        let off = r.iter().filter(|r| r.err_b.abs() > 0.25).count() as f64 / r.len() as f64;
        println!(
            "{name:<10} beats {:5}  median {:+6.1} ms  mean {:+6.1}  |err| {:5.1}  within 35ms {:5.1}%  off-beat {:4.1}%",
            r.len(),
            med,
            mean,
            abs,
            hit * 100.0,
            off * 100.0
        );
    };
    println!();
    summary("all", &|_| true);
    summary("beats", &|r| !r.calm);
    summary("breakdown", &|r| r.calm);
    summary("coasting", &|r| r.coast);
    let mut be: Vec<f64> = win_bpm_err.iter().map(|v| v.abs()).collect();
    be.sort_by(|a, b| a.total_cmp(b));
    println!(
        "tempo      |live - real| median {:.2} BPM, p90 {:.2}",
        be[be.len() / 2],
        be[be.len() * 9 / 10]
    );
    println!(
        "bars       one on the real downbeat {:.1}% ({}/{}), in breakdowns {:.1}% ({}/{})",
        100.0 * downs_ok as f64 / downs.max(1) as f64,
        downs_ok,
        downs,
        100.0 * downs_calm_ok as f64 / downs_calm.max(1) as f64,
        downs_calm_ok,
        downs_calm
    );
    let fmt = |v: &[(f64, String)]| v.iter().take(40).map(|(t, s)| format!("{t:.0}s{s}")).collect::<Vec<_>>().join(" ");
    println!(
        "slips      {}  {}",
        slips.len(),
        fmt(&slips.iter().map(|&(t, d)| (t, format!("({d:+})"))).collect::<Vec<_>>())
    );
    println!(
        "bar jumps  {}  {}",
        bar_jumps.len(),
        fmt(&bar_jumps.iter().map(|&(t, a, b)| (t, format!("({a}>{b})"))).collect::<Vec<_>>())
    );
    println!(
        "reference  {level_skip} beats skipped (reference at another metrical level than the live tempo)"
    );
    // The live grid itself: anything that re-labels bars mid-section — the
    // bar's one moving, or the position jumping instead of flowing.
    let mut moves: Vec<(f64, String)> = Vec::new();
    for w in hops.windows(2) {
        if w[0].t < from || w[1].t > to {
            continue;
        }
        if w[1].down != w[0].down {
            moves.push((w[1].t, format!("(bar {}>{})", w[0].down, w[1].down)));
        }
        let expect = (w[1].t - w[0].t) * w[0].bpm as f64 / 60.0;
        let d = w[1].pos - w[0].pos - expect;
        if d.abs() > 0.08 {
            moves.push((w[1].t, format!("(jump {d:+.2})")));
        }
    }
    println!("live grid  {} bar moves/jumps  {}", moves.len(), fmt(&moves));
    let on_bar = cuts
        .iter()
        .filter(|c| hop_at(c.0).is_some_and(|(p, h)| (p - h.down as f64).rem_euclid(4.0) < 0.15))
        .count();
    println!(
        "phrasing  {} cuts ({} on a bar line), re-anchors {}: {}",
        cuts.len(),
        on_bar,
        anchors.len(),
        fmt(&anchors.iter().map(|&(t, d)| (t, format!("({d:+.0})"))).collect::<Vec<_>>())
    );
    println!(
        "cuts       {}",
        fmt(&cuts.iter().map(|&(t, _, b)| (t, format!("[{b}]"))).collect::<Vec<_>>())
    );

    // Independent reference, no model involved: the low-band (kick) onset
    // strength averaged by live beat phase. Where the live beat sits on the
    // kicks, it peaks sharply at phase 0. Per minute, and overall.
    let rise = low_rise(&mono, sr);
    let bins = 48usize;
    let mut total = vec![0.0f64; bins];
    let mut rows: Vec<(f64, Vec<f64>, f32)> = Vec::new();
    let mut cur = vec![0.0f64; bins];
    let mut seg = (from.max(0.0) / 60.0).floor() * 60.0;
    let mut bpm_acc = (0.0f64, 0u32);
    for (i, &r) in rise.iter().enumerate() {
        let t = i as f64 / 1000.0;
        if t < from || t > to {
            continue;
        }
        if t >= seg + 60.0 {
            rows.push((seg, std::mem::replace(&mut cur, vec![0.0; bins]), (bpm_acc.0 / bpm_acc.1.max(1) as f64) as f32));
            bpm_acc = (0.0, 0);
            seg += 60.0;
        }
        let Some((pos, h)) = hop_at(t) else { continue };
        if h.calm > 0.5 || r <= 0.0 {
            continue;
        }
        let b = ((pos.rem_euclid(1.0)) * bins as f64) as usize % bins;
        cur[b] += r;
        total[b] += r;
        bpm_acc.0 += h.bpm as f64;
        bpm_acc.1 += 1;
    }
    rows.push((seg, cur, (bpm_acc.0 / bpm_acc.1.max(1) as f64) as f32));
    // Peak phase (signed beats, -0.5..0.5) by a circular mean over the
    // bins near the max, and peak-to-mean sharpness.
    let peak = |p: &[f64]| -> Option<(f64, f64)> {
        let mean = p.iter().sum::<f64>() / p.len() as f64;
        if mean <= 0.0 {
            return None;
        }
        let m = (0..p.len()).max_by(|&a, &b| p[a].total_cmp(&p[b]))?;
        let (mut sx, mut sy) = (0.0, 0.0);
        for d in -2i64..=2 {
            let j = (m as i64 + d).rem_euclid(p.len() as i64) as usize;
            let a = std::f64::consts::TAU * j as f64 / p.len() as f64;
            sx += p[j] * a.cos();
            sy += p[j] * a.sin();
        }
        Some((sy.atan2(sx) / std::f64::consts::TAU, p[m] / mean))
    };
    println!("
kick phase profile (independent of the model): where the low-band onsets land on the live beat");
    for (t, p, bpm) in &rows {
        if let Some((ph, sharp)) = peak(p) {
            let spark: String = (0..16)
                .map(|k| {
                    let v = (0..3).map(|j| p[(k * 3 + j + bins - 1) % bins]).sum::<f64>();
                    let mx = p.iter().cloned().fold(0.0, f64::max) * 3.0;
                    [' ', '.', ':', '-', '=', '+', '*', '#'][((v / mx.max(1e-12)) * 7.0).round().clamp(0.0, 7.0) as usize]
                })
                .collect();
            println!(
                "  {:4.0}s  kicks at {:+6.1} ms  sharpness {:4.1}  |{spark}|",
                t,
                ph * 60000.0 / bpm.max(1.0) as f64,
                sharp
            );
        }
    }
    if let Some((ph, sharp)) = peak(&total) {
        let bpm = hops.iter().map(|h| h.bpm as f64).sum::<f64>() / hops.len().max(1) as f64;
        println!(
            "kicks      overall at {:+.1} ms of the live beat (+ = kick after the beat), sharpness {:.1}",
            ph * 60000.0 / bpm,
            sharp
        );
    }
    Ok(())
}

/// Low-band (40-130 Hz) onset strength at 1 ms: the positive log rise of
/// the band's envelope.
fn low_rise(mono: &[f32], sr: u32) -> Vec<f64> {
    // Two one-pole high-passes at 40 Hz, two low-passes at 130 Hz.
    let hp = (-std::f64::consts::TAU * 40.0 / sr as f64).exp();
    let lp = 1.0 - (-std::f64::consts::TAU * 130.0 / sr as f64).exp();
    let (mut h1, mut h2, mut x1, mut y1, mut l1, mut l2) = (0.0f64, 0.0, 0.0, 0.0, 0.0, 0.0);
    let step = (sr / 1000).max(1) as usize;
    let mut env = Vec::with_capacity(mono.len() / step + 1);
    let mut acc = 0.0f64;
    let mut sm = 0.0f64;
    for (i, &x) in mono.iter().enumerate() {
        let x = x as f64;
        h1 = hp * (h1 + x - x1);
        x1 = x;
        h2 = hp * (h2 + h1 - y1);
        y1 = h1;
        l1 += lp * (h2 - l1);
        l2 += lp * (l1 - l2);
        acc += l2 * l2;
        if (i + 1) % step == 0 {
            // ~8 ms smoothing of the power: the band's ripple is 80-260 Hz.
            sm += (acc / step as f64 - sm) * 0.12;
            env.push((sm + 1e-10).ln());
            acc = 0.0;
        }
    }
    let mut out = vec![0.0f64; env.len()];
    for i in 3..env.len() {
        out[i] = (env[i] - env[i - 3]).max(0.0);
    }
    out
}
