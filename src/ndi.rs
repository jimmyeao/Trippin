//! NDI output — ships the composited frame to the network (OBS, Resolume,
//! vMix, another machine running an NDI monitor). The runtime library is
//! loaded dynamically at run time: users install NDI Tools / the NDI Runtime
//! redistributable, and Trippin builds and runs fine without it — the output
//! just reports "runtime not found" instead of crashing.
//!
//! Only the sender side is wrapped. `NDIlib_v5` is an append-only function
//! table; the prefix below mirrors `Processing.NDI.DynamicLoad.h` exactly,
//! so an older runtime exposing fewer functions still loads — the missing
//! tail functions would just be null, and we never call past what we check.

use std::ffi::{CString, c_char, c_void};

use anyhow::{Context, Result, anyhow};

#[cfg(windows)]
const LIB_NAME: &str = "Processing.NDI.Lib.x64.dll";
#[cfg(target_os = "macos")]
const LIB_NAME: &str = "libndi.dylib";
#[cfg(not(any(windows, target_os = "macos")))]
const LIB_NAME: &str = "libndi.so.6";

/// Ask the SDK to stamp its own timecode on each frame.
const TIMECODE_SYNTHESIZE: i64 = i64::MAX;
const FOURCC_BGRA: u32 = u32::from_le_bytes(*b"BGRA");
const FRAME_PROGRESSIVE: i32 = 1;

#[repr(C)]
struct SendCreate {
    name: *const c_char,
    groups: *const c_char,
    clock_video: bool,
    clock_audio: bool,
}

#[repr(C)]
pub struct VideoFrameV2 {
    xres: i32,
    yres: i32,
    fourcc: u32,
    rate_n: i32,
    rate_d: i32,
    aspect: f32,
    format: i32,
    timecode: i64,
    data: *const u8,
    line_stride: i32,
    metadata: *const c_char,
    timestamp: i64,
}

impl VideoFrameV2 {
    /// One progressive BGRA frame over `data` — `stride` is the padded row
    /// pitch, so the mapped readback can be handed over with no repack.
    pub fn bgra(w: u32, h: u32, fps: u32, data: *const u8, stride: u32) -> Self {
        Self {
            xres: w as i32,
            yres: h as i32,
            fourcc: FOURCC_BGRA,
            rate_n: fps.max(1) as i32 * 1000,
            rate_d: 1000,
            aspect: w as f32 / h.max(1) as f32,
            format: FRAME_PROGRESSIVE,
            timecode: TIMECODE_SYNTHESIZE,
            data,
            line_stride: stride as i32,
            metadata: std::ptr::null(),
            timestamp: TIMECODE_SYNTHESIZE,
        }
    }
}

type SendInst = *mut c_void;
type FindInst = *mut c_void;
type RecvInst = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Source {
    name: *const c_char,
    /// `p_utf8_url_address` — endpoint address, UTF-8.
    url: *const c_char,
}

#[repr(C)]
struct FindCreate {
    show_local_sources: bool,
    groups: *const c_char,
    extra_ips: *const c_char,
}

#[repr(C)]
struct RecvCreate {
    source: Source,
    /// `NDIlib_recv_color_format_BGRX_BGRA` — matches the sender exactly.
    color_format: i32,
    /// `NDIlib_recv_bandwidth_highest`.
    bandwidth: i32,
    allow_video_fields: bool,
}

/// Prefix of `NDIlib_v5` — field order matches Processing.NDI.DynamicLoad.h.
/// Padding slots stand in for functions we don't call.
#[repr(C)]
struct Vtable {
    initialize: Option<unsafe extern "C" fn() -> bool>,
    destroy: Option<unsafe extern "C" fn()>,
    version: Option<unsafe extern "C" fn() -> *const c_char>,
    is_supported_cpu: Option<unsafe extern "C" fn() -> bool>,
    _find_create: usize,
    find_create_v2: Option<unsafe extern "C" fn(*const FindCreate) -> FindInst>,
    find_destroy: Option<unsafe extern "C" fn(FindInst)>,
    _find_get_sources: usize,
    send_create: Option<unsafe extern "C" fn(*const SendCreate) -> SendInst>,
    send_destroy: Option<unsafe extern "C" fn(SendInst)>,
    _send_video: usize,
    _send_video_async: usize,
    _send_audio: usize,
    _send_metadata: usize,
    _send_capture: usize,
    _send_free_metadata: usize,
    _send_get_tally: usize,
    send_get_no_connections: Option<unsafe extern "C" fn(SendInst, u32) -> i32>,
    _send_clear_metadata: usize,
    _send_add_metadata: usize,
    _send_set_failover: usize,
    recv_create_v2: Option<unsafe extern "C" fn(*const RecvCreate) -> RecvInst>,
    _recv_create: usize,
    recv_destroy: Option<unsafe extern "C" fn(RecvInst)>,
    _pad_24_41: [usize; 18],
    find_wait_for_sources: Option<unsafe extern "C" fn(FindInst, u32) -> bool>,
    find_get_current_sources: Option<unsafe extern "C" fn(FindInst, *mut u32) -> *const Source>,
    _util_44: usize,
    _util_45: usize,
    _util_46: usize,
    recv_free_video_v2: Option<unsafe extern "C" fn(RecvInst, *const VideoFrameV2)>,
    _recv_free_audio_v2: usize,
    recv_capture_v2: Option<
        unsafe extern "C" fn(RecvInst, *mut VideoFrameV2, *mut c_void, *mut c_void, u32) -> i32,
    >,
    send_video_v2: Option<unsafe extern "C" fn(SendInst, *const VideoFrameV2)>,
    _send_video_async_v2: usize,
}

/// The loaded NDI runtime. Leaked on first use — the runtime itself is
/// process-lifetime, so we never unload it.
pub struct Ndi {
    vt: &'static Vtable,
}

impl Ndi {
    pub fn load() -> Result<Ndi> {
        unsafe {
            let lib = load_lib()?;
            let load: libloading::Symbol<unsafe extern "C" fn() -> *const Vtable> =
                lib.get(b"NDIlib_v5_load").context("NDIlib_v5_load")?;
            let vt = load();
            if vt.is_null() || (*vt).send_create.is_none() || (*vt).send_video_v2.is_none() {
                return Err(anyhow!("NDI runtime too old (v5 send API missing)"));
            }
            if let Some(init) = (*vt).initialize {
                init();
            }
            std::mem::forget(lib);
            Ok(Ndi { vt: &*vt })
        }
    }

    pub fn version(&self) -> String {
        unsafe { version_str(self.vt) }
    }

    pub fn sender(&self, name: &str) -> Result<Sender> {
        let cname = CString::new(name).unwrap_or_else(|_| CString::new("Trippin").unwrap());
        let create = SendCreate {
            name: cname.as_ptr(),
            groups: std::ptr::null(),
            clock_video: true,
            clock_audio: false,
        };
        let inst = unsafe { (self.vt.send_create.unwrap())(&create) };
        if inst.is_null() {
            return Err(anyhow!("NDI send_create failed"));
        }
        Ok(Sender { inst, vt: self.vt })
    }

    /// `trippin --ndi-monitor [name]`: list discoverable sources, attach to
    /// the first matching `filter`, and count incoming frames — a quick
    /// end-to-end network check that needs no NDI Tools install.
    pub fn monitor(&self, filter: Option<&str>, seconds: u32) -> Result<()> {
        let vt = self.vt;
        let (fc, fw, fg, fd, rc, cap, free, rd) = (
            vt.find_create_v2,
            vt.find_wait_for_sources,
            vt.find_get_current_sources,
            vt.find_destroy,
            vt.recv_create_v2,
            vt.recv_capture_v2,
            vt.recv_free_video_v2,
            vt.recv_destroy,
        );
        let (Some(fc), Some(fw), Some(fg), Some(fd), Some(rc), Some(cap), Some(free), Some(rd)) =
            (fc, fw, fg, fd, rc, cap, free, rd)
        else {
            return Err(anyhow!("NDI runtime too old for receive"));
        };
        unsafe {
            let create = FindCreate {
                show_local_sources: true,
                groups: std::ptr::null(),
                extra_ips: std::ptr::null(),
            };
            let finder = fc(&create);
            if finder.is_null() {
                return Err(anyhow!("NDI find_create failed"));
            }
            // Discover: poll for up to 15s, printing each new source.
            let mut picked: Option<Source> = None;
            let mut seen: Vec<String> = Vec::new();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            while std::time::Instant::now() < deadline && picked.is_none() {
                fw(finder, 1000);
                let mut n = 0u32;
                let srcs = fg(finder, &mut n);
                for i in 0..n {
                    let s = *srcs.add(i as usize);
                    let name = std::ffi::CStr::from_ptr(s.name)
                        .to_string_lossy()
                        .into_owned();
                    let url = source_url(&s);
                    let tag = format!("{name}  @  {url}");
                    if !seen.contains(&tag) {
                        println!("found: {tag}");
                        seen.push(tag);
                    }
                    if filter.is_none_or(|f| name.contains(f)) {
                        picked = Some(s);
                    }
                }
            }
            let Some(src) = picked else {
                fd(finder);
                return Err(anyhow!(
                    "no NDI source {} found in 15s",
                    filter.map_or("at all", |f| f)
                ));
            };
            println!(
                "receiving \"{}\"  @  {}…",
                std::ffi::CStr::from_ptr(src.name).to_string_lossy(),
                source_url(&src)
            );
            // The Source borrows finder memory — create the receiver before
            // destroying the finder.
            let recv = rc(&RecvCreate {
                source: src,
                color_format: 0, // BGRX_BGRA
                bandwidth: 100,  // highest
                allow_video_fields: false,
            });
            fd(finder);
            if recv.is_null() {
                return Err(anyhow!("NDI recv_create failed"));
            }
            let t0 = std::time::Instant::now();
            let end = t0 + std::time::Duration::from_secs(seconds.max(1) as u64);
            let mut frames = 0u64;
            let mut other = std::collections::BTreeMap::<i32, u64>::new();
            while std::time::Instant::now() < end {
                let mut vf = VideoFrameV2::bgra(0, 0, 1, std::ptr::null(), 0);
                match cap(
                    recv,
                    &mut vf,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    1000,
                ) {
                    1 => {
                        frames += 1;
                        if frames == 1 {
                            println!(
                                "first frame: {}x{} @ {}/{} fps",
                                vf.xres, vf.yres, vf.rate_n, vf.rate_d
                            );
                            // Pixel sanity — catches an all-black or
                            // mis-strided feed that counting alone can't.
                            if !vf.data.is_null() && vf.xres > 0 && vf.yres > 0 {
                                let stride = vf.line_stride.max(0) as usize;
                                let tight = vf.xres as usize * 4;
                                if stride >= tight {
                                    let row = (vf.yres as usize / 2) * stride;
                                    let px = vf.data.add(row) as *const u32;
                                    let mut sum = 0u64;
                                    let n = (vf.xres as usize).min(256);
                                    for i in 0..n {
                                        let p = u64::from(*px.add(i));
                                        sum += (p & 0xff) + ((p >> 8) & 0xff) + ((p >> 16) & 0xff);
                                    }
                                    println!(
                                        "midline mean channel value: {:.0}/255",
                                        sum as f64 / (n * 3) as f64
                                    );
                                }
                            }
                        }
                        free(recv, &vf);
                    }
                    t => *other.entry(t).or_default() += 1,
                }
            }
            rd(recv);
            if !other.is_empty() {
                println!("non-video returns: {other:?}");
            }
            let secs = t0.elapsed().as_secs_f64();
            println!(
                "{frames} frames in {secs:.1}s — {:.1} fps received",
                frames as f64 / secs
            );
            Ok(())
        }
    }
}

/// The source's endpoint URL (`p_utf8_url_address` — UTF-8 on all platforms).
unsafe fn source_url(s: &Source) -> String {
    unsafe {
        if s.url.is_null() {
            return "?".into();
        }
        std::ffi::CStr::from_ptr(s.url)
            .to_string_lossy()
            .into_owned()
    }
}

unsafe fn load_lib() -> Result<libloading::Library> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("NDI_RUNTIME_DIR_V6") {
        candidates.push(format!("{dir}/lib").into());
        candidates.push(dir.into());
    }
    #[cfg(windows)]
    {
        for base in [
            r"C:\Program Files\NDI\NDI 6 Runtime\v6",
            r"C:\Program Files\NDI\NDI Runtime\v6",
            r"C:\Program Files\NDI\NDI 6 Tools\Runtime",
        ] {
            candidates.push(base.into());
        }
    }
    #[cfg(target_os = "macos")]
    {
        for base in [
            "/usr/local/lib",
            "/Library/NDI SDK for Apple/lib/x64",
            "/Library/NDI SDK for Apple/lib/arm64",
        ] {
            candidates.push(base.into());
        }
    }
    for dir in &candidates {
        let path = dir.join(LIB_NAME);
        if let Ok(lib) = unsafe { libloading::Library::new(&path) } {
            return Ok(lib);
        }
    }
    // Fall back to the OS loader search path (PATH / DYLD_LIBRARY_PATH).
    unsafe { libloading::Library::new(LIB_NAME) }
        .map_err(|e| anyhow!("NDI runtime not found — install NDI Tools / NDI Runtime ({e})"))
}

unsafe fn version_str(vt: &Vtable) -> String {
    unsafe {
        match vt.version {
            Some(f) => {
                let p = f();
                if p.is_null() {
                    "?".into()
                } else {
                    std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
                }
            }
            None => "?".into(),
        }
    }
}

/// An NDI sender instance. Owned by the output worker thread; sending is a
/// synchronous SDK call (the encode runs on NDI's threads), so `data` only
/// needs to stay valid for the duration of `send`.
pub struct Sender {
    inst: SendInst,
    vt: &'static Vtable,
}

unsafe impl Send for Sender {}

impl Sender {
    pub fn send(&self, frame: &VideoFrameV2) {
        if let Some(f) = self.vt.send_video_v2 {
            unsafe { f(self.inst, frame) };
        }
    }

    /// How many receivers are watching (OBS instances, monitors…). Waits up
    /// to `timeout_ms` for the tally to change; pass 0 to poll.
    pub fn connections(&self, timeout_ms: u32) -> i32 {
        self.vt
            .send_get_no_connections
            .map(|f| unsafe { f(self.inst, timeout_ms) })
            .unwrap_or(-1)
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        if let Some(f) = self.vt.send_destroy {
            unsafe { f(self.inst) };
        }
    }
}
