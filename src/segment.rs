//! Person segmentation for the camera: who's the person, what's the room.
//!
//! Two models, both Apache 2.0, both run on rten (the same pure-Rust runtime
//! as Beat This!):
//!
//! - **MODNet** (portrait matting, ZHKKKe/MODNet, the ONNX conversion from
//!   huggingface.co/Xenova/modnet): the one we want. It knows a chair or a
//!   shelf behind you isn't you, where MediaPipe's selfie model kept the
//!   whole shelf (owner: "leaves a lot of the background behind"). 26 MB,
//!   so it's downloaded on first use to `<data dir>/models` (SHA-checked),
//!   like the beat model.
//! - **MediaPipe selfie segmentation** (landscape, 256x144; bundled in
//!   assets/, 450 kB): used until MODNet is downloaded, or if it can't be.
//!
//! The mask goes into the frame's alpha channel, so the camera texture
//! carries it (`cam_tex.a`) with no texture of its own. MODNet takes 30-60
//! ms a frame on the CPU, so the camera thread hands frames to a worker
//! (`Cutter`) and applies the newest mask it has: the camera never waits.

use anyhow::{anyhow, Context, Result};
use rten::{Model, NodeId, RunOptions, ThreadPool, Value};
use rten_tensor::AsView;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, OnceLock};

/// The cut-out's own threads: MODNet on the app's shared 2-thread rten pool
/// took ~100 ms a frame (and would hold up the beat model); on 4 of its own
/// it was ~65 ms, on 6 ~58 ms. Built on first use, idle while the cut-out is off.
fn pool() -> Arc<ThreadPool> {
    static POOL: OnceLock<Arc<ThreadPool>> = OnceLock::new();
    POOL.get_or_init(|| {
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        Arc::new(ThreadPool::with_num_threads((n / 2).clamp(2, 6)))
    })
    .clone()
}

/// The bundled fallback model (assets/; its SHA-256 is pinned by a test).
pub const SELFIE: &[u8] = include_bytes!("../assets/selfie_segmentation_landscape.onnx");
const SELFIE_W: usize = 256;
const SELFIE_H: usize = 144;

/// MODNet, pinned to a commit of the Hugging Face repo and checked by hash.
const MODNET_FILE: &str = "modnet.onnx";
const MODNET_URL: &str =
    "https://huggingface.co/Xenova/modnet/resolve/fa2fa546052fba4c08921230a26cc69a333fca12/onnx/model.onnx";
const MODNET_SHA: &str = "07c308cf0fc7e6e8b2065a12ed7fc07e1de8febb7dc7839d7b7f15dd66584df9";
/// MODNet's input size: 16:9-ish and a multiple of 32. At 320 wide it
/// picked up bits of the shelf; 384 is clean.
const MODNET_W: usize = 384;
const MODNET_H: usize = 224;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Selfie,
    Modnet,
}

impl Kind {
    fn size(self) -> (usize, usize) {
        match self {
            Kind::Selfie => (SELFIE_W, SELFIE_H),
            Kind::Modnet => (MODNET_W, MODNET_H),
        }
    }
}

/// A person mask, `w` x `h`, 0..1.
pub struct Mask {
    pub w: usize,
    pub h: usize,
    pub v: Vec<f32>,
}

pub struct Segmenter {
    model: Model,
    input: NodeId,
    output: NodeId,
    kind: Kind,
    /// Last mask, for smoothing over frames (less flicker at the edges).
    prev: Vec<f32>,
}

/// Where MODNet is: the download dir, then `models/` in the working dir or
/// next to the exe.
fn modnet_path() -> Option<PathBuf> {
    let mut c = vec![crate::config::data_dir().join("models"), PathBuf::from("models")];
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf())) {
        c.push(dir.join("models"));
    }
    c.into_iter().map(|d| d.join(MODNET_FILE)).find(|p| p.is_file())
}

/// Download MODNet to `<data dir>/models` (blocking; checked against its
/// SHA-256).
pub fn download_modnet() -> Result<PathBuf> {
    let dir = crate::config::data_dir().join("models");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let dest = dir.join(MODNET_FILE);
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(600)))
        .tls_config(crate::config::tls())
        .build()
        .new_agent();
    let resp = agent.get(MODNET_URL).call().context("downloading the cut-out model")?;
    let mut reader = resp.into_body().into_reader();
    let tmp = dest.with_extension("part");
    let mut file = std::fs::File::create(&tmp)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = reader.read(&mut buf).context("reading the cut-out model")?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        hasher.update(&buf[..n]);
    }
    drop(file);
    if format!("{:x}", hasher.finalize()) != MODNET_SHA {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow!("cut-out model: checksum mismatch"));
    }
    std::fs::rename(&tmp, &dest)?;
    Ok(dest)
}

impl Segmenter {
    /// MODNet if it's on disk, else the bundled selfie model.
    pub fn new() -> Result<Self> {
        if let Some(p) = modnet_path() {
            match std::fs::read(&p).map_err(anyhow::Error::from).and_then(|b| Self::load(b, Kind::Modnet)) {
                Ok(s) => return Ok(s),
                Err(e) => eprintln!("cut-out: {}: {e:#}; using the built-in model", p.display()),
            }
        }
        Self::selfie()
    }

    /// The bundled selfie model.
    pub fn selfie() -> Result<Self> {
        Self::load(SELFIE.to_vec(), Kind::Selfie)
    }

    /// MODNet, downloading it first if needed (blocking).
    pub fn modnet() -> Result<Self> {
        let p = match modnet_path() {
            Some(p) => p,
            None => download_modnet()?,
        };
        Self::load(std::fs::read(&p)?, Kind::Modnet)
    }

    fn load(bytes: Vec<u8>, kind: Kind) -> Result<Self> {
        let model = Model::load(bytes).context("loading the segmentation model")?;
        let input = *model.input_ids().first().ok_or_else(|| anyhow!("model has no input"))?;
        let output = *model.output_ids().first().ok_or_else(|| anyhow!("model has no output"))?;
        Ok(Segmenter { model, input, output, kind, prev: Vec::new() })
    }

    pub fn is_modnet(&self) -> bool {
        self.kind == Kind::Modnet
    }

    /// Describe the model (for `--seg-test`).
    pub fn describe(&self) -> String {
        let (w, h) = self.kind.size();
        format!("{:?} at {w}x{h}", self.kind)
    }

    /// The model's input size.
    pub fn size(&self) -> (usize, usize) {
        self.kind.size()
    }

    /// The raw person mask for an RGB image at `size()`.
    pub fn run_raw(&self, rgb: &[u8]) -> Result<Mask> {
        let (w, h) = self.kind.size();
        if rgb.len() != w * h * 3 {
            return Err(anyhow!("expected {w}x{h} RGB"));
        }
        // NCHW; the selfie model takes 0..1, MODNet -1..1.
        let mut x = vec![0f32; w * h * 3];
        for i in 0..w * h {
            for c in 0..3 {
                let v = rgb[i * 3 + c] as f32 / 255.0;
                x[c * w * h + i] = match self.kind {
                    Kind::Selfie => v,
                    Kind::Modnet => v * 2.0 - 1.0,
                };
            }
        }
        let v = Value::from_shape(&[1, 3, h, w][..], x).map_err(|e| anyhow!("input tensor: {e}"))?;
        let out = self
            .model
            .run(
                vec![(self.input, (&v).into())],
                &[self.output],
                Some({
                    let mut o = RunOptions::default();
                    o.thread_pool = Some(pool());
                    o.timing = std::env::var_os("TRIPPIN_SEG_TIMING").is_some();
                    o
                }),
            )
            .map_err(|e| anyhow!("segmentation: {e}"))?;
        let t = out
            .into_iter()
            .next()
            .and_then(|o| o.into_tensor::<f32>())
            .ok_or_else(|| anyhow!("segmentation: output isn't f32"))?;
        let m: Vec<f32> = t.to_vec();
        if m.len() < w * h {
            return Err(anyhow!("segmentation: output {} values, expected {}", m.len(), w * h));
        }
        // [1, 1, H, W] or [1, H, W, 1]: either way W*H values in row order.
        Ok(Mask { w, h, v: m[..w * h].to_vec() })
    }

    /// The mask, smoothed with the last one: the edges settle instead of
    /// flickering, while a move still shows within a few frames.
    pub fn run(&mut self, rgb: &[u8]) -> Result<Mask> {
        let m = self.run_raw(rgb)?;
        if self.prev.len() == m.v.len() {
            for (p, &v) in self.prev.iter_mut().zip(&m.v) {
                *p += (v - *p) * 0.6;
            }
        } else {
            self.prev = m.v.clone();
        }
        Ok(Mask { w: m.w, h: m.h, v: self.prev.clone() })
    }

    /// Cut the person out of an RGBA image in place (blocking): the mask
    /// goes into the alpha.
    pub fn cut(&mut self, rgba: &mut [u8], iw: usize, ih: usize) -> Result<()> {
        let (w, h) = self.kind.size();
        let m = self.run(&downscale_rgb(rgba, iw, ih, w, h))?;
        mask_into_alpha(&m, rgba, iw, ih);
        Ok(())
    }
}

/// The cut-out off the camera thread: a worker runs the model on the newest
/// frame it's handed, and `cut` applies the newest mask it has made, so a
/// 40 ms model never holds the camera up. It starts on the bundled model
/// and switches to MODNet once that's loaded (downloaded first if needed).
pub struct Cutter {
    tx: mpsc::SyncSender<(Vec<u8>, usize, usize)>,
    rx: mpsc::Receiver<Mask>,
    last: Option<Mask>,
}

impl Cutter {
    pub fn start() -> Self {
        let (tx, frx) = mpsc::sync_channel::<(Vec<u8>, usize, usize)>(1);
        let (mtx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("cut-out".into())
            .spawn(move || {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker(frx, mtx)));
            })
            .ok();
        Cutter { tx, rx, last: None }
    }

    /// Put the person mask into `rgba`'s alpha: the newest mask made, and
    /// hand this frame to the worker if it's free. Until the first mask
    /// the frame is fully cut away (nothing rather than the whole room).
    pub fn cut(&mut self, rgba: &mut [u8], iw: usize, ih: usize) {
        let _ = self.tx.try_send((rgba.to_vec(), iw, ih));
        while let Ok(m) = self.rx.try_recv() {
            self.last = Some(m);
        }
        match &self.last {
            Some(m) => mask_into_alpha(m, rgba, iw, ih),
            None => rgba.chunks_mut(4).for_each(|p| p[3] = 0),
        }
    }
}

fn worker(frx: mpsc::Receiver<(Vec<u8>, usize, usize)>, mtx: mpsc::Sender<Mask>) {
    // The bundled model straight away; MODNet loads (or downloads) beside it.
    let mut seg = match Segmenter::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("camera: cut-out unavailable: {e:#}");
            return;
        }
    };
    let (btx, brx) = mpsc::channel();
    if !seg.is_modnet() {
        std::thread::spawn(move || {
            match Segmenter::modnet() {
                Ok(s) => {
                    let _ = btx.send(s);
                }
                Err(e) => eprintln!("camera: better cut-out model unavailable: {e:#}"),
            }
        });
    }
    while let Ok((px, iw, ih)) = frx.recv() {
        if let Ok(s) = brx.try_recv() {
            seg = s;
        }
        let (w, h) = seg.size();
        if let Ok(m) = seg.run(&downscale_rgb(&px, iw, ih, w, h)) {
            if mtx.send(m).is_err() {
                return;
            }
        }
    }
}

/// Write `mask` into the alpha channel of an RGBA image of size `ow` x
/// `oh`, scaled up bilinearly.
pub fn mask_into_alpha(mask: &Mask, rgba: &mut [u8], ow: usize, oh: usize) {
    let (w, h) = (mask.w, mask.h);
    if w == 0 || h == 0 || mask.v.len() < w * h || rgba.len() < ow * oh * 4 {
        return;
    }
    let m = &mask.v;
    let sx = w as f32 / ow as f32;
    let sy = h as f32 / oh as f32;
    for y in 0..oh {
        let fy = ((y as f32 + 0.5) * sy - 0.5).clamp(0.0, (h - 1) as f32);
        let y0 = fy as usize;
        let y1 = (y0 + 1).min(h - 1);
        let ty = fy - y0 as f32;
        for x in 0..ow {
            let fx = ((x as f32 + 0.5) * sx - 0.5).clamp(0.0, (w - 1) as f32);
            let x0 = fx as usize;
            let x1 = (x0 + 1).min(w - 1);
            let tx = fx - x0 as f32;
            let a = m[y0 * w + x0] * (1.0 - tx) + m[y0 * w + x1] * tx;
            let b = m[y1 * w + x0] * (1.0 - tx) + m[y1 * w + x1] * tx;
            let v = a * (1.0 - ty) + b * ty;
            rgba[(y * ow + x) * 4 + 3] = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }
}

/// A `w` x `h` RGB copy of an `iw` x `ih` RGBA image (filtered down).
pub fn downscale_rgb(rgba: &[u8], iw: usize, ih: usize, w: usize, h: usize) -> Vec<u8> {
    let img = image::RgbaImage::from_raw(iw as u32, ih as u32, rgba.to_vec()).unwrap_or_default();
    let small = image::imageops::resize(&img, w as u32, h as u32, image::imageops::FilterType::Triangle);
    small.pixels().flat_map(|p| [p[0], p[1], p[2]]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bundled model is the one we checked (onnx-community's landscape
    /// selfie segmentation, downloaded 2026-11-26).
    #[test]
    fn bundled_model_is_pinned() {
        let got = format!("{:x}", Sha256::digest(SELFIE));
        assert_eq!(got, "7a0adcfdb1715d3b0ff0f61486d8a181c33f4a208343a5abfccbc6540e872d24");
    }

    /// It loads and gives a mask of the right size in 0..1.
    #[test]
    fn model_runs() {
        let s = Segmenter::selfie().unwrap();
        let rgb = vec![128u8; SELFIE_W * SELFIE_H * 3];
        let m = s.run_raw(&rgb).unwrap();
        assert_eq!(m.v.len(), SELFIE_W * SELFIE_H);
        assert!(m.v.iter().all(|v| v.is_finite() && (-0.01..=1.01).contains(v)), "{}", s.describe());
    }

    #[test]
    fn mask_scales_into_alpha() {
        let (w, h) = (256, 144);
        let mut v = vec![0.0; w * h];
        for y in 0..h {
            for x in w / 2..w {
                v[y * w + x] = 1.0;
            }
        }
        let mask = Mask { w, h, v };
        let (ow, oh) = (640, 360);
        let mut px = vec![9u8; ow * oh * 4];
        mask_into_alpha(&mask, &mut px, ow, oh);
        assert_eq!(px[3], 0, "left is background");
        assert_eq!(px[(10 * ow + ow - 1) * 4 + 3], 255, "right is the person");
        assert_eq!(px[0], 9, "colour untouched");
    }
}
