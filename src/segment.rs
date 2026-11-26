//! Person segmentation for the camera: who's the person, what's the room.
//!
//! Google's MediaPipe selfie segmentation (landscape, 256x144; Apache 2.0;
//! the ONNX conversion from huggingface.co/onnx-community/
//! mediapipe_selfie_segmentation_landscape), run on rten, the same pure-Rust
//! runtime as Beat This!. It's 450 kB and ships inside the app: no download
//! and no network. The camera thread runs it on each frame when the cut-out
//! is wanted and writes the mask into the frame's alpha channel, so the
//! camera texture carries it (`cam_tex.a`) with no texture of its own.

use anyhow::{anyhow, Context, Result};
use rten::{Model, NodeId, Value};
use rten_tensor::AsView;

/// The model, bundled (assets/; its SHA-256 is pinned by a test).
pub const MODEL: &[u8] = include_bytes!("../assets/selfie_segmentation_landscape.onnx");
/// The model's input size.
pub const W: usize = 256;
pub const H: usize = 144;

pub struct Segmenter {
    model: Model,
    input: NodeId,
    output: NodeId,
    /// Input laid out [1, H, W, 3] (else [1, 3, H, W]).
    nhwc: bool,
    /// Last mask, for smoothing over frames (less flicker at the edges).
    prev: Vec<f32>,
}

impl Segmenter {
    pub fn new() -> Result<Self> {
        let model = Model::load(MODEL.to_vec()).context("loading the segmentation model")?;
        let input = *model.input_ids().first().ok_or_else(|| anyhow!("model has no input"))?;
        let output = *model.output_ids().first().ok_or_else(|| anyhow!("model has no output"))?;
        let shape = model.input_shape(0).unwrap_or_default();
        let dims: Vec<Option<usize>> = shape
            .iter()
            .map(|d| match d {
                rten::Dimension::Fixed(n) => Some(*n),
                _ => None,
            })
            .collect();
        let nhwc = dims.get(3) == Some(&Some(3));
        Ok(Segmenter { model, input, output, nhwc, prev: Vec::new() })
    }

    /// Describe the model's input and output (for `--seg-test`).
    pub fn describe(&self) -> String {
        let name = |id| self.model.node_info(id).and_then(|i| i.name().map(str::to_string)).unwrap_or_default();
        let shape = |id| self.model.node_info(id).and_then(|i| i.shape()).map(|s| format!("{s:?}")).unwrap_or_default();
        format!(
            "input {} {} ({}), output {} {}",
            name(self.input),
            shape(self.input),
            if self.nhwc { "NHWC" } else { "NCHW" },
            name(self.output),
            shape(self.output)
        )
    }

    /// The person mask (W x H, 0..1) for a W x H RGB image.
    pub fn run_raw(&self, rgb: &[u8]) -> Result<Vec<f32>> {
        if rgb.len() != W * H * 3 {
            return Err(anyhow!("expected {W}x{H} RGB"));
        }
        let mut x = vec![0f32; W * H * 3];
        for i in 0..W * H {
            for c in 0..3 {
                let v = rgb[i * 3 + c] as f32 / 255.0;
                if self.nhwc {
                    x[i * 3 + c] = v;
                } else {
                    x[c * W * H + i] = v;
                }
            }
        }
        let shape: [usize; 4] = if self.nhwc { [1, H, W, 3] } else { [1, 3, H, W] };
        let v = Value::from_shape(&shape[..], x).map_err(|e| anyhow!("input tensor: {e}"))?;
        let out = self
            .model
            .run(vec![(self.input, (&v).into())], &[self.output], None)
            .map_err(|e| anyhow!("segmentation: {e}"))?;
        let t = out
            .into_iter()
            .next()
            .and_then(|o| o.into_tensor::<f32>())
            .ok_or_else(|| anyhow!("segmentation: output isn't f32"))?;
        let m: Vec<f32> = t.to_vec();
        if m.len() < W * H {
            return Err(anyhow!("segmentation: output {} values, expected {}", m.len(), W * H));
        }
        // [1, H, W, 1] or [1, 1, H, W]: either way W*H values in row order.
        Ok(m[..W * H].to_vec())
    }

    /// The mask for a W x H RGB image, smoothed with the last one: the edges
    /// settle instead of flickering, while a move still shows within a few
    /// frames.
    pub fn run(&mut self, rgb: &[u8]) -> Result<Vec<f32>> {
        let m = self.run_raw(rgb)?;
        if self.prev.len() == m.len() {
            for (p, &v) in self.prev.iter_mut().zip(&m) {
                *p += (v - *p) * 0.6;
            }
        } else {
            self.prev = m;
        }
        Ok(self.prev.clone())
    }
}

/// Write `mask` (W x H) into the alpha channel of an RGBA image of size
/// `ow` x `oh`, scaled up bilinearly.
pub fn mask_into_alpha(mask: &[f32], rgba: &mut [u8], ow: usize, oh: usize) {
    if mask.len() < W * H || rgba.len() < ow * oh * 4 {
        return;
    }
    let sx = W as f32 / ow as f32;
    let sy = H as f32 / oh as f32;
    for y in 0..oh {
        let fy = ((y as f32 + 0.5) * sy - 0.5).clamp(0.0, (H - 1) as f32);
        let y0 = fy as usize;
        let y1 = (y0 + 1).min(H - 1);
        let ty = fy - y0 as f32;
        for x in 0..ow {
            let fx = ((x as f32 + 0.5) * sx - 0.5).clamp(0.0, (W - 1) as f32);
            let x0 = fx as usize;
            let x1 = (x0 + 1).min(W - 1);
            let tx = fx - x0 as f32;
            let a = mask[y0 * W + x0] * (1.0 - tx) + mask[y0 * W + x1] * tx;
            let b = mask[y1 * W + x0] * (1.0 - tx) + mask[y1 * W + x1] * tx;
            let v = a * (1.0 - ty) + b * ty;
            rgba[(y * ow + x) * 4 + 3] = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }
}

/// A W x H RGB copy of an RGBA image (box-filtered down).
pub fn downscale_rgb(rgba: &[u8], iw: usize, ih: usize) -> Vec<u8> {
    let img = image::RgbaImage::from_raw(iw as u32, ih as u32, rgba.to_vec()).unwrap_or_default();
    let small = image::imageops::resize(&img, W as u32, H as u32, image::imageops::FilterType::Triangle);
    small.pixels().flat_map(|p| [p[0], p[1], p[2]]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    /// The bundled model is the one we checked (onnx-community's landscape
    /// selfie segmentation, downloaded 2026-11-26).
    #[test]
    fn bundled_model_is_pinned() {
        let got = format!("{:x}", Sha256::digest(MODEL));
        assert_eq!(got, "7a0adcfdb1715d3b0ff0f61486d8a181c33f4a208343a5abfccbc6540e872d24");
    }

    /// It loads and gives a mask of the right size in 0..1.
    #[test]
    fn model_runs() {
        let s = Segmenter::new().unwrap();
        let rgb = vec![128u8; W * H * 3];
        let m = s.run_raw(&rgb).unwrap();
        assert_eq!(m.len(), W * H);
        assert!(m.iter().all(|v| v.is_finite() && (-0.01..=1.01).contains(v)), "{}", s.describe());
    }

    #[test]
    fn mask_scales_into_alpha() {
        let mut mask = vec![0.0; W * H];
        for y in 0..H {
            for x in W / 2..W {
                mask[y * W + x] = 1.0;
            }
        }
        let (ow, oh) = (640, 360);
        let mut px = vec![9u8; ow * oh * 4];
        mask_into_alpha(&mask, &mut px, ow, oh);
        assert_eq!(px[3], 0, "left is background");
        assert_eq!(px[(10 * ow + ow - 1) * 4 + 3], 255, "right is the person");
        assert_eq!(px[0], 9, "colour untouched");
    }
}
