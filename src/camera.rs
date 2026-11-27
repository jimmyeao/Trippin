//! Webcam as a source (Settings > Camera; `camera_*` scenes).
//!
//! Off until a camera is picked: nothing is opened, and no camera scene is
//! in rotation. With one picked, a capture thread (`nokhwa`: Media
//! Foundation on Windows, AVFoundation on macOS) keeps the newest frame,
//! fitted to the video texture's size (VID_W x VID_H, RGBA), and the render
//! loop uploads it into `vid_tex` slot 0 while a `camera_*` scene is on
//! air. `LIVE` is true while frames arrive: it gates the camera scenes and
//! lights the panel's "camera on" marker.
//!
//! macOS: the camera needs permission (`nokhwa_initialize` asks; the app's
//! Info.plist must carry NSCameraUsageDescription). Not compiled for macOS
//! from Windows yet: the M2 has to test it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Frames are arriving from the picked camera.
pub static LIVE: AtomicBool = AtomicBool::new(false);

/// Something wants the person cut-out (the Person key, or the dancer as
/// you): the camera thread runs the segmentation model and puts the mask in
/// each frame's alpha (segment.rs). Set by the render loop each frame.
pub static WANT_CUTOUT: AtomicBool = AtomicBool::new(false);

/// The cameras found by the last `refresh_names` (human names).
static NAMES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static LISTING: AtomicBool = AtomicBool::new(false);

/// The newest frame: a serial (bumped per frame) and its RGBA mip chain
/// (`gfx::cam_mips`).
pub type Frame = Option<(u64, Arc<Vec<u8>>)>;

pub struct Capture {
    want: Arc<Mutex<String>>,
    frame: Arc<Mutex<Frame>>,
}

impl Capture {
    /// Start the capture thread (idle until `set` names a camera).
    pub fn start() -> Self {
        let want = Arc::new(Mutex::new(String::new()));
        let frame: Arc<Mutex<Frame>> = Arc::default();
        let (w, f) = (want.clone(), frame.clone());
        std::thread::Builder::new()
            .name("camera".into())
            .spawn(move || run(w, f))
            .ok();
        Capture { want, frame }
    }

    /// The camera to capture (a name, or a part of one); "" closes it.
    pub fn set(&self, name: &str) {
        let mut w = self.want.lock().unwrap_or_else(|e| e.into_inner());
        if *w != name {
            *w = name.to_string();
        }
    }

    /// The newest frame if it's newer than `seen`.
    pub fn newer(&self, seen: u64) -> Option<(u64, Arc<Vec<u8>>)> {
        let f = self.frame.lock().unwrap_or_else(|e| e.into_inner());
        f.as_ref().filter(|(s, _)| *s != seen).cloned()
    }
}

/// The cameras found last time (`refresh_names` updates it).
pub fn names() -> Vec<String> {
    NAMES.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// List the cameras again, on a thread (the query can take a moment).
pub fn refresh_names() {
    if LISTING.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::spawn(|| {
        let n = std::panic::catch_unwind(list).unwrap_or_default();
        *NAMES.lock().unwrap_or_else(|e| e.into_inner()) = n;
        LISTING.store(false, Ordering::Release);
    });
}

#[cfg(any(windows, target_os = "macos"))]
fn list() -> Vec<String> {
    permit();
    nokhwa::query(nokhwa::utils::ApiBackend::Auto)
        .map(|v| v.iter().map(|c| c.human_name()).collect())
        .unwrap_or_else(|e| {
            eprintln!("camera: listing failed: {e}");
            Vec::new()
        })
}

#[cfg(not(any(windows, target_os = "macos")))]
fn list() -> Vec<String> {
    Vec::new()
}

/// macOS asks for camera permission once; elsewhere nothing to do.
#[cfg(any(windows, target_os = "macos"))]
fn permit() {
    #[cfg(target_os = "macos")]
    {
        static ASKED: std::sync::Once = std::sync::Once::new();
        ASKED.call_once(|| {
            nokhwa::nokhwa_initialize(|ok| {
                if !ok {
                    eprintln!("camera: permission refused (System Settings > Privacy > Camera)");
                }
            });
        });
    }
}

/// One control of a camera scene (Perform > scene controls). Values are
/// kept per scene in `Settings::show_params` (as the Unity show knobs are)
/// and reach the shader in `Uniforms::roto4`/`roto5.x`, in table order.
pub struct SceneParam {
    pub id: &'static str,
    pub label: &'static str,
    pub tip: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    /// Shown as whole numbers.
    pub whole: bool,
}

const fn p(id: &'static str, label: &'static str, tip: &'static str, min: f32, max: f32, default: f32) -> SceneParam {
    SceneParam { id, label, tip, min, max, default, whole: false }
}

const OUTLINE: [SceneParam; 5] = [
    p("cam_glow", "Glow", "How bright the traced lines are", 0.0, 2.0, 1.0),
    p("cam_detail", "Detail", "Low: only the big outlines. High: finer lines (and more camera noise)", 0.0, 1.0, 0.35),
    p("cam_trails", "Trails", "How long the lines' trails last", 0.0, 0.95, 0.82),
    p("cam_ripple", "Ripple", "How much the lines ripple with the mids", 0.0, 2.0, 1.0),
    p("cam_stream", "Stream", "How fast the trails stream out from the centre", 0.0, 2.0, 1.0),
];

const KALEIDO: [SceneParam; 4] = [
    SceneParam { whole: true, ..p("cam_segments", "Segments", "How many mirrored segments", 3.0, 12.0, 8.0) },
    p("cam_zoom", "Zoom", "How far into the camera the mandala looks", 0.5, 2.0, 1.0),
    p("cam_spin", "Spin", "How fast it turns", 0.0, 2.0, 1.0),
    p("cam_palette", "Palette", "0: the camera's own colours. 1: the show's palette", 0.0, 1.0, 0.3),
];

const SLITSCAN: [SceneParam; 4] = [
    p("cam_speed", "Speed", "How fast the smear runs to the left", 0.0, 2.0, 1.0),
    p("cam_wave", "Wave", "How much the ribbons wave with the bass", 0.0, 2.0, 1.0),
    p("cam_fade", "Fade", "How quickly old ribbons fade", 0.0, 1.0, 0.3),
    p("cam_palette", "Palette", "How much ageing ribbons take the show's palette", 0.0, 1.0, 0.4),
];

/// The controls of a camera scene (none for others; camera_roto has the
/// rotoscope look, `Settings::roto_*`).
pub fn scene_params(scene: &str) -> &'static [SceneParam] {
    match scene {
        "camera_outline" => &OUTLINE,
        "camera_kaleido" => &KALEIDO,
        "camera_slitscan" => &SLITSCAN,
        _ => &[],
    }
}

/// A camera scene's saved value for `p` (its default when unset).
pub fn param_value(saved: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, f32>>, scene: &str, p: &SceneParam) -> f32 {
    saved
        .get(scene)
        .and_then(|m| m.get(p.id))
        .copied()
        .filter(|v| v.is_finite())
        .unwrap_or(p.default)
        .clamp(p.min, p.max)
}

/// A camera scene's controls as (roto4, roto5) for the shader; the
/// rotoscope look's defaults for any other scene.
pub fn scene_uniforms(saved: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, f32>>, scene: &str) -> ([f32; 4], [f32; 4]) {
    if scene_params(scene).is_empty() {
        return ([1.0, 0.5, 0.35, 4.0], [0.4, 0.0, 0.0, 0.0]);
    }
    let mut v = [0.0f32; 8];
    for (i, p) in scene_params(scene).iter().enumerate().take(5) {
        v[i] = param_value(saved, scene, p);
    }
    ([v[0], v[1], v[2], v[3]], [v[4], 0.0, 0.0, 0.0])
}

/// Fit an RGB frame into VID_W x VID_H (cover: fill, crop the overflow),
/// as RGBA.
pub fn fit(img: &image::RgbImage) -> Vec<u8> {
    use crate::gfx::{VID_H, VID_W};
    let (w, h) = (img.width().max(1), img.height().max(1));
    let rgba = image::DynamicImage::ImageRgb8(img.clone()).to_rgba8();
    if (w, h) == (VID_W, VID_H) {
        return rgba.into_raw();
    }
    let k = (VID_W as f32 / w as f32).max(VID_H as f32 / h as f32);
    let (sw, sh) = (((w as f32 * k).ceil() as u32).max(VID_W), ((h as f32 * k).ceil() as u32).max(VID_H));
    let big = image::imageops::resize(&rgba, sw, sh, image::imageops::FilterType::Triangle);
    image::imageops::crop_imm(&big, (sw - VID_W) / 2, (sh - VID_H) / 2, VID_W, VID_H)
        .to_image()
        .into_raw()
}

#[cfg(any(windows, target_os = "macos"))]
fn run(want: Arc<Mutex<String>>, frame: Arc<Mutex<Frame>>) {
    use nokhwa::pixel_format::RgbFormat;
    use nokhwa::utils::{ApiBackend, CameraFormat, FrameFormat, RequestedFormat, RequestedFormatType, Resolution};
    let mut serial = 0u64;
    let mut retry_at = Instant::now();
    loop {
        let name = want.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if name.trim().is_empty() || Instant::now() < retry_at {
            LIVE.store(false, Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }
        permit();
        // Open the camera whose name matches (exact first, then a part).
        let opened = std::panic::catch_unwind(|| -> Result<nokhwa::Camera, String> {
            let cams = nokhwa::query(ApiBackend::Auto).map_err(|e| e.to_string())?;
            let pick = cams
                .iter()
                .find(|c| c.human_name() == name)
                .or_else(|| cams.iter().find(|c| c.human_name().to_lowercase().contains(&name.to_lowercase())))
                .ok_or_else(|| format!("no camera named {name:?}"))?;
            // `Closest` only matches its exact frame format: Mac cameras
            // (FaceTime HD, Continuity) offer YUYV/NV12 and no MJPEG, so an
            // MJPEG-only request never opened them. Try each in turn, then
            // whatever the camera has.
            let mut tries: Vec<RequestedFormatType> = [FrameFormat::MJPEG, FrameFormat::YUYV, FrameFormat::NV12]
                .into_iter()
                .map(|f| RequestedFormatType::Closest(CameraFormat::new(Resolution::new(1280, 720), f, 30)))
                .collect();
            tries.push(RequestedFormatType::None);
            let mut err = String::from("no usable format");
            let mut cam = None;
            for t in tries {
                match nokhwa::Camera::new(pick.index().clone(), RequestedFormat::new::<RgbFormat>(t)) {
                    Ok(c) => {
                        cam = Some(c);
                        break;
                    }
                    Err(e) => err = e.to_string(),
                }
            }
            let mut cam = cam.ok_or(err)?;
            cam.open_stream().map_err(|e| e.to_string())?;
            Ok(cam)
        });
        let mut cam = match opened {
            Ok(Ok(c)) => {
                println!("camera: {name} open ({})", c.resolution());
                c
            }
            Ok(Err(e)) => {
                eprintln!("camera: {e}");
                retry_at = Instant::now() + Duration::from_secs(3);
                continue;
            }
            Err(_) => {
                eprintln!("camera: {name}: the driver panicked");
                retry_at = Instant::now() + Duration::from_secs(3);
                continue;
            }
        };
        // Capture until the pick changes or the camera fails.
        let mut last_ok = Instant::now();
        // The cut-out worker, started the first time the cut-out is wanted.
        let mut seg: Option<crate::segment::Cutter> = None;
        loop {
            if *want.lock().unwrap_or_else(|e| e.into_inner()) != name {
                break;
            }
            let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                cam.frame().ok().and_then(|b| b.decode_image::<RgbFormat>().ok())
            }))
            .ok()
            .flatten();
            match got {
                Some(img) => {
                    serial = serial.wrapping_add(1);
                    let mut level0 = fit(&img);
                    if WANT_CUTOUT.load(Ordering::Relaxed) {
                        use crate::gfx::{VID_H, VID_W};
                        seg.get_or_insert_with(crate::segment::Cutter::start).cut(
                            &mut level0,
                            VID_W as usize,
                            VID_H as usize,
                        );
                    }
                    let chain = crate::gfx::cam_mips(&level0);
                    *frame.lock().unwrap_or_else(|e| e.into_inner()) = Some((serial, Arc::new(chain)));
                    last_ok = Instant::now();
                    LIVE.store(true, Ordering::Relaxed);
                }
                None => {
                    if last_ok.elapsed() > Duration::from_secs(2) {
                        eprintln!("camera: {name}: no frames, reopening");
                        retry_at = Instant::now() + Duration::from_secs(2);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cam.stop_stream()));
        LIVE.store(false, Ordering::Relaxed);
        *frame.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn run(_want: Arc<Mutex<String>>, _frame: Arc<Mutex<Frame>>) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Any camera size fills the video texture exactly (cropped, not
    /// stretched or letterboxed).
    #[test]
    fn fit_fills_the_video_texture() {
        use crate::gfx::{VID_H, VID_W};
        for (w, h) in [(640, 480), (1920, 1080), (1280, 720), (720, 1280)] {
            let img = image::RgbImage::from_pixel(w, h, image::Rgb([10, 200, 30]));
            let px = fit(&img);
            assert_eq!(px.len(), (VID_W * VID_H * 4) as usize, "{w}x{h}");
            assert_eq!(&px[..4], &[10, 200, 30, 255]);
        }
    }

    /// The mip chain is the full length and each level averages the last.
    #[test]
    fn mips_halve_and_average() {
        use crate::gfx::{cam_chain_len, cam_mips, VID_H, VID_W};
        let mut l0 = vec![0u8; (VID_W * VID_H * 4) as usize];
        for (i, p) in l0.chunks_mut(4).enumerate() {
            let v = if (i % VID_W as usize) % 2 == 0 { 200 } else { 100 };
            p.copy_from_slice(&[v, v, v, 255]);
        }
        let chain = cam_mips(&l0);
        assert_eq!(chain.len(), cam_chain_len());
        let l1 = &chain[l0.len()..];
        assert_eq!(&l1[..4], &[150, 150, 150, 255]);
    }
}
