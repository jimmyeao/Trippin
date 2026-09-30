//! External video output. When enabled, `Renderer::render` re-runs the
//! present pass (post FX, overlays and text included) into an offscreen BGRA
//! target at the output resolution, copies it to a staging buffer, and a
//! worker thread ships each frame to the sinks: NDI (network) and Spout
//! (Windows GPU texture sharing — OBS's Spout2 source). The render thread
//! never waits on a sink — if both staging buffers are still in flight the
//! frame is just dropped. Frames carry alpha (transparent-background mode).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::ndi;

/// The tap renders into this — `present.wgsl` outputs linear colour, so the
/// sRGB view does the same encode the swapchain does. BGRA byte order is
/// NDI's native fast path (no swizzle before send).
pub const OUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

const STAGING: usize = 2;

/// `lut[a * 256 + v]`: sRGB byte `v` of a premultiplied colour at alpha
/// `a` → the straight-alpha sRGB byte (divide in linear light).
fn unpremul_lut() -> Vec<u8> {
    let dec = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let enc = |l: f32| if l <= 0.0031308 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
    let mut lut = vec![0u8; 256 * 256];
    for a in 0..256usize {
        for v in 0..256usize {
            lut[a * 256 + v] = if a == 0 {
                0
            } else {
                let l = (dec(v as f32 / 255.0) / (a as f32 / 255.0)).min(1.0);
                (enc(l) * 255.0 + 0.5) as u8
            };
        }
    }
    lut
}

/// User-facing output configuration — any change rebuilds the output.
#[derive(Clone, PartialEq)]
pub struct Conf {
    /// Source name as receivers (OBS) will see it — NDI and Spout alike.
    pub name: String,
    pub ndi: bool,
    pub spout: bool,
    /// Transparent background: frames are un-premultiplied before sending
    /// (NDI and Spout receivers expect straight alpha).
    pub transparent: bool,
    /// Frame height; width is the 16:9 match (720→1280, 1080→1920, 2160→3840).
    pub height: u32,
    /// Frame cadence cap.
    pub fps: u32,
}

impl Conf {
    pub fn size(&self) -> (u32, u32) {
        (self.height * 16 / 9, self.height)
    }
}

pub struct Output {
    pub conf: Conf,
    pub width: u32,
    pub height: u32,
    pub tex: wgpu::Texture,
    pub view: wgpu::TextureView,
    /// Uniform buffer with res_x/res_y set to the output size — the present
    /// shader's fx/grain maths wants the real output resolution.
    pub uniform_buf: wgpu::Buffer,
    /// Same layout as the scene bind groups; `bind_groups[i]` samples
    /// `targets[i]`. Rebuilt when the renderer's targets are.
    pub bind_groups: [wgpu::BindGroup; 2],
    pub staging: Arc<Vec<wgpu::Buffer>>,
    in_flight: Vec<Arc<AtomicBool>>,
    /// Staging indices encoded this frame, mapped after the queue submit.
    pending: Vec<usize>,
    pub row_bytes: u32,
    tx: Option<mpsc::Sender<usize>>,
    worker: Option<JoinHandle<()>>,
    conns: Arc<AtomicI32>,
    /// Sinks that came up (for the status line), and why others didn't.
    ndi_on: bool,
    spout_name: Option<String>,
    errs: Vec<String>,
    interval: Duration,
    last: Instant,
}

impl Output {
    pub fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        targets: &[wgpu::Texture; 2],
        res: &crate::render::FrameRes,
        conf: Conf,
    ) -> Result<Output> {
        let (width, height) = conf.size();
        let mut errs = Vec::new();
        // Fail cheap: the runtime probe happens before any GPU allocation.
        let sender = if conf.ndi {
            match ndi::Ndi::load().and_then(|lib| {
                let s = lib.sender(&conf.name)?;
                println!("NDI runtime {} — sender \"{}\"", lib.version(), conf.name);
                Ok(s)
            }) {
                Ok(s) => Some(s),
                // With Spout also on, a missing NDI runtime shouldn't take
                // Spout down with it.
                Err(e) if conf.spout => {
                    errs.push(format!("NDI: {e:#}"));
                    None
                }
                Err(e) => return Err(e),
            }
        } else {
            None
        };
        let ndi_on = sender.is_some();
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ndi out"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ndi uniforms"),
            size: std::mem::size_of::<crate::render::Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_groups = Self::bind(device, layout, sampler, targets, res, &uniform_buf);

        let row_bytes = (width * 4).div_ceil(256) * 256;
        let buf_size = (row_bytes * height) as u64;
        let staging = Arc::new(
            (0..STAGING)
                .map(|_| {
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("ndi staging"),
                        size: buf_size,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    })
                })
                .collect::<Vec<_>>(),
        );
        let in_flight: Vec<Arc<AtomicBool>> = (0..STAGING)
            .map(|_| Arc::new(AtomicBool::new(false)))
            .collect();
        let conns = Arc::new(AtomicI32::new(0));

        let (tx, rx) = mpsc::channel::<usize>();
        // The Spout sender's D3D11 objects live on the worker thread; it
        // reports back whether the sender came up.
        let (up_tx, up_rx) = mpsc::channel::<Result<String, String>>();
        let worker = {
            let staging = staging.clone();
            let in_flight = in_flight.clone();
            let conns = conns.clone();
            let fps = conf.fps.max(1);
            let want_spout = conf.spout;
            let unpremul = conf.transparent.then(unpremul_lut);
            let mut scratch = Vec::new();
            let spout_name = conf.name.clone();
            std::thread::spawn(move || {
                #[cfg(windows)]
                let spout = if want_spout {
                    match crate::spout::Sender::new(&spout_name, width, height) {
                        Ok(s) => {
                            let _ = up_tx.send(Ok(s.name.clone()));
                            Some(s)
                        }
                        Err(e) => {
                            let _ = up_tx.send(Err(format!("Spout: {e:#}")));
                            None
                        }
                    }
                } else {
                    None
                };
                #[cfg(not(windows))]
                {
                    let _ = (&spout_name, want_spout);
                    let _ = up_tx.send(Err("Spout is Windows-only (use NDI on macOS)".into()));
                }
                drop(up_tx);
                let mut counted = Instant::now() - Duration::from_secs(10);
                let mut sent = 0u64;
                let mut logged = Instant::now();
                loop {
                    match rx.recv_timeout(Duration::from_secs(2)) {
                        Ok(i) => {
                            let buf = &staging[i];
                            if let Ok(mapped) = buf.slice(..).get_mapped_range() {
                                let range: &[u8] = match &unpremul {
                                    Some(lut) => {
                                        scratch.clear();
                                        scratch.extend_from_slice(&mapped);
                                        for p in scratch.chunks_exact_mut(4) {
                                            let row = &lut[p[3] as usize * 256..][..256];
                                            p[0] = row[p[0] as usize];
                                            p[1] = row[p[1] as usize];
                                            p[2] = row[p[2] as usize];
                                        }
                                        &scratch
                                    }
                                    None => &mapped,
                                };
                                #[cfg(windows)]
                                if let Some(s) = &spout {
                                    s.send(range, row_bytes);
                                }
                                if let Some(sender) = &sender {
                                    let frame = ndi::VideoFrameV2::bgra(
                                        width,
                                        height,
                                        fps,
                                        range.as_ptr(),
                                        row_bytes,
                                    );
                                    let t = Instant::now();
                                    sender.send(&frame);
                                    let ms = t.elapsed().as_millis();
                                    if ms > 30 {
                                        eprintln!("ndi: send blocked {ms}ms");
                                    }
                                }
                            }
                            buf.unmap();
                            in_flight[i].store(false, Ordering::Release);
                            sent += 1;
                            if let (Some(sender), true) =
                                (&sender, logged.elapsed() > Duration::from_secs(5))
                            {
                                logged = Instant::now();
                                eprintln!("ndi: {sent} sent, {} receivers", sender.connections(0));
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    if let (Some(sender), true) =
                        (&sender, counted.elapsed() > Duration::from_secs(2))
                    {
                        counted = Instant::now();
                        conns.store(sender.connections(0), Ordering::Relaxed);
                    }
                }
            })
        };

        let mut spout_name = None;
        if conf.spout {
            match up_rx.recv_timeout(Duration::from_secs(5)) {
                Ok(Ok(n)) => {
                    println!("Spout sender \"{n}\"");
                    spout_name = Some(n);
                }
                Ok(Err(e)) => errs.push(e),
                Err(_) => errs.push("Spout: sender didn't start".into()),
            }
        }
        if !ndi_on && spout_name.is_none() {
            drop(tx);
            let _ = worker.join();
            return Err(anyhow::anyhow!(errs.join(" · ")));
        }

        Ok(Output {
            ndi_on,
            spout_name,
            errs,
            interval: Duration::from_secs_f64(1.0 / f64::from(conf.fps.max(1))),
            last: Instant::now() - Duration::from_secs(1),
            conf,
            width,
            height,
            tex,
            view,
            uniform_buf,
            bind_groups,
            staging,
            in_flight,
            pending: Vec::new(),
            row_bytes,
            tx: Some(tx),
            worker: Some(worker),
            conns,
        })
    }

    fn bind(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        targets: &[wgpu::Texture; 2],
        res: &crate::render::FrameRes,
        uniform_buf: &wgpu::Buffer,
    ) -> [wgpu::BindGroup; 2] {
        let make = |t: &wgpu::Texture| {
            let view = t.create_view(&Default::default());
            let [e3, e4, e5, e6, e7] = res.entries();
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ndi"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    e3,
                    e4,
                    e5,
                    e6,
                    e7,
                ],
            })
        };
        [make(&targets[0]), make(&targets[1])]
    }

    /// The scene targets were rebuilt (window resize) — rebind to the new views.
    pub fn rebind(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        targets: &[wgpu::Texture; 2],
        res: &crate::render::FrameRes,
    ) {
        self.bind_groups = Self::bind(
            device,
            layout,
            sampler,
            targets,
            res,
            &self.uniform_buf,
        );
    }

    /// True when enough time has passed for another output frame.
    pub fn due(&mut self) -> bool {
        if self.last.elapsed() >= self.interval {
            self.last = Instant::now();
            true
        } else {
            false
        }
    }

    /// A staging buffer not still being read by the worker, if any.
    pub fn free_staging(&self) -> Option<usize> {
        self.in_flight
            .iter()
            .position(|f| !f.load(Ordering::Acquire))
    }

    /// Mark staging buffer `i` for readback — mapped once the submit lands.
    pub fn mark_pending(&mut self, i: usize) {
        self.in_flight[i].store(true, Ordering::Release);
        self.pending.push(i);
    }

    /// Post-submit: kick the async maps for buffers encoded this frame. The
    /// callbacks run on the next `device.poll` and hand the worker an index.
    pub fn map_pending(&mut self) {
        for i in self.pending.drain(..) {
            let tx = self.tx.as_ref().unwrap().clone();
            let flag = self.in_flight[i].clone();
            self.staging[i]
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |res| {
                    if res.is_ok() {
                        let _ = tx.send(i);
                    } else {
                        flag.store(false, Ordering::Release);
                    }
                });
        }
    }

    /// One-line status for the panel.
    pub fn status(&self) -> String {
        let mut parts = Vec::new();
        if self.ndi_on {
            let n = self.conns.load(Ordering::Relaxed);
            parts.push(match n {
                0 => format!("NDI \"{}\" — waiting for receivers", self.conf.name),
                1 => format!("NDI \"{}\" — 1 receiver", self.conf.name),
                n => format!("NDI \"{}\" — {n} receivers", self.conf.name),
            });
        }
        if let Some(n) = &self.spout_name {
            parts.push(format!("Spout \"{n}\" — live"));
        }
        parts.extend(self.errs.iter().cloned());
        parts.join("\n")
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        drop(self.tx.take());
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }
}
