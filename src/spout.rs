//! Spout2 sender (Windows) — native, no SpoutLibrary.dll. Spout shares a
//! D3D11 texture between processes: the sender creates a texture with a
//! legacy shared handle, publishes that handle + size in a shared-memory
//! block named after the sender, and lists the name in the
//! `SpoutSenderNames` registry (256-byte slots, sorted, empty slot ends the
//! list). Receivers (OBS's Spout2 plugin, Resolume, TouchDesigner…) open
//! the handle on their own device. Each named block is guarded by a
//! `<name>_mutex`; texture writes by `<name>_SpoutAccessMutex`.
//!
//! Frames arrive from the output tap's CPU readback (BGRA, sRGB-encoded),
//! so this runs on the output worker thread and costs one
//! `UpdateSubresource` per frame.

use std::ffi::CString;

use anyhow::{Context, Result, anyhow};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HMODULE, INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_RESOURCE_MISC_SHARED, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::IDXGIResource;
use windows::Win32::System::Memory::{
    CreateFileMappingA, FILE_MAP_ALL_ACCESS, MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS,
    MapViewOfFile, PAGE_READWRITE, UnmapViewOfFile, VirtualQuery,
};
use windows::Win32::System::Threading::{CreateMutexA, ReleaseMutex, WaitForSingleObject};
use windows::core::{Interface, PCSTR};

const NAME_LEN: usize = 256;
const MAX_SENDERS: usize = 64;
/// `SharedTextureInfo`: handle, width, height, format, usage (u32 each),
/// description[256], partnerId.
const INFO_SIZE: usize = 280;

fn cstr(s: &str) -> CString {
    CString::new(s.replace('\0', "")).unwrap_or_default()
}

/// A named shared-memory block plus its `<name>_mutex`.
struct Shm {
    map: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
    mutex: HANDLE,
    size: usize,
}

impl Shm {
    fn open(name: &str, size: usize) -> Result<Shm> {
        let n = cstr(name);
        unsafe {
            // Opens the existing block if one is there (size is then its own).
            let map = CreateFileMappingA(INVALID_HANDLE_VALUE, None, PAGE_READWRITE, 0, size as u32, PCSTR(n.as_ptr() as _))
                .with_context(|| format!("shared memory {name}"))?;
            let view = MapViewOfFile(map, FILE_MAP_ALL_ACCESS, 0, 0, 0);
            if view.Value.is_null() {
                let _ = CloseHandle(map);
                return Err(anyhow!("map {name}"));
            }
            // An existing block may be smaller than asked (another app's
            // MaxSenders) — never touch past what's really mapped.
            let mut mbi = MEMORY_BASIC_INFORMATION::default();
            VirtualQuery(Some(view.Value), &mut mbi, std::mem::size_of::<MEMORY_BASIC_INFORMATION>());
            let size = size.min(mbi.RegionSize.max(1));
            let m = cstr(&format!("{name}_mutex"));
            let mutex = CreateMutexA(None, false, PCSTR(m.as_ptr() as _)).unwrap_or_default();
            Ok(Shm { map, view, mutex, size })
        }
    }

    fn lock(&self) -> bool {
        if self.mutex.is_invalid() {
            return true;
        }
        let r = unsafe { WaitForSingleObject(self.mutex, 100) };
        r == WAIT_OBJECT_0 || r == WAIT_ABANDONED
    }

    fn unlock(&self) {
        if !self.mutex.is_invalid() {
            unsafe {
                let _ = ReleaseMutex(self.mutex);
            }
        }
    }

    fn bytes(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.view.Value as *mut u8, self.size) }
    }
}

impl Drop for Shm {
    fn drop(&mut self) {
        unsafe {
            let _ = UnmapViewOfFile(self.view);
            let _ = CloseHandle(self.map);
            if !self.mutex.is_invalid() {
                let _ = CloseHandle(self.mutex);
            }
        }
    }
}

fn read_names(b: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    for slot in b.chunks_exact(NAME_LEN).take(MAX_SENDERS * 4) {
        if slot[0] == 0 {
            break;
        }
        let end = slot.iter().position(|&c| c == 0).unwrap_or(NAME_LEN);
        out.push(String::from_utf8_lossy(&slot[..end]).into_owned());
    }
    out
}

fn write_names(b: &mut [u8], names: &[String]) {
    let slots = b.len() / NAME_LEN;
    for (i, slot) in b.chunks_exact_mut(NAME_LEN).enumerate() {
        slot.fill(0);
        if let Some(n) = names.get(i) {
            let n = n.as_bytes();
            let k = n.len().min(NAME_LEN - 1);
            slot[..k].copy_from_slice(&n[..k]);
        }
        if i + 1 >= slots {
            break;
        }
    }
}

pub struct Sender {
    pub name: String,
    width: u32,
    height: u32,
    _device: ID3D11Device,
    ctx: ID3D11DeviceContext,
    tex: ID3D11Texture2D,
    _info: Shm,
    names: Shm,
    access: HANDLE,
}

impl Sender {
    pub fn new(name: &str, width: u32, height: u32) -> Result<Sender> {
        unsafe {
            let mut device = None;
            let mut ctx = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut ctx),
            )
            .context("D3D11 device")?;
            let device: ID3D11Device = device.context("no D3D11 device")?;
            let ctx: ID3D11DeviceContext = ctx.context("no D3D11 context")?;
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
                CPUAccessFlags: 0,
                MiscFlags: D3D11_RESOURCE_MISC_SHARED.0 as u32,
            };
            let mut tex = None;
            device.CreateTexture2D(&desc, None, Some(&mut tex)).context("shared texture")?;
            let tex: ID3D11Texture2D = tex.context("no texture")?;
            let handle = tex.cast::<IDXGIResource>()?.GetSharedHandle().context("shared handle")?;

            // Register a unique name in the sender list.
            let mut names = Shm::open("SpoutSenderNames", MAX_SENDERS * NAME_LEN)?;
            let mut chosen = name.to_string();
            if names.lock() {
                let mut list = read_names(names.bytes());
                let mut n = 2;
                while list.contains(&chosen) {
                    chosen = format!("{name}_{n}");
                    n += 1;
                }
                list.push(chosen.clone());
                list.sort();
                if list.len() * NAME_LEN < names.size {
                    write_names(names.bytes(), &list);
                }
                names.unlock();
            }

            // The sender's info block: where and what the texture is.
            let mut info = Shm::open(&chosen, INFO_SIZE)?;
            if info.lock() {
                let b = info.bytes();
                b.fill(0);
                b[0..4].copy_from_slice(&(handle.0 as usize as u32).to_le_bytes());
                b[4..8].copy_from_slice(&width.to_le_bytes());
                b[8..12].copy_from_slice(&height.to_le_bytes());
                b[12..16].copy_from_slice(&(DXGI_FORMAT_B8G8R8A8_UNORM.0 as u32).to_le_bytes());
                let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
                let e = exe.as_bytes();
                let k = e.len().min(255);
                b[20..20 + k].copy_from_slice(&e[..k]);
                info.unlock();
            }

            // Become the active sender when there isn't a live one, so a
            // receiver with nothing picked connects to us.
            if let Ok(mut active) = Shm::open("ActiveSenderName", NAME_LEN) {
                if active.lock() {
                    let cur = read_names(active.bytes()).into_iter().next();
                    let live = read_names(names.bytes());
                    if cur.is_none_or(|c| !live.contains(&c)) {
                        write_names(active.bytes(), std::slice::from_ref(&chosen));
                    }
                    active.unlock();
                }
            }

            let a = cstr(&format!("{chosen}_SpoutAccessMutex"));
            let access = CreateMutexA(None, false, PCSTR(a.as_ptr() as _)).unwrap_or_default();
            Ok(Sender {
                name: chosen,
                width,
                height,
                _device: device,
                ctx,
                tex,
                _info: info,
                names,
                access,
            })
        }
    }

    /// Upload one BGRA frame (`stride` bytes per row) into the shared texture.
    pub fn send(&self, data: &[u8], stride: u32) {
        if data.len() < (stride * self.height) as usize || stride < self.width * 4 {
            return;
        }
        unsafe {
            let locked = if self.access.is_invalid() {
                false
            } else {
                let r = WaitForSingleObject(self.access, 67);
                if r != WAIT_OBJECT_0 && r != WAIT_ABANDONED {
                    return; // a receiver is holding it — drop this frame
                }
                true
            };
            self.ctx.UpdateSubresource(&self.tex, 0, None, data.as_ptr() as _, stride, 0);
            self.ctx.Flush();
            if locked {
                let _ = ReleaseMutex(self.access);
            }
        }
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        if self.names.lock() {
            let mut list = read_names(self.names.bytes());
            list.retain(|n| *n != self.name);
            write_names(self.names.bytes(), &list);
            self.names.unlock();
        }
        if !self.access.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.access);
            }
        }
    }
}

/// Receive one frame from sender `name` the way any Spout receiver does
/// (registry → info block → open the shared handle on our own device) and
/// return it as straight RGBA8 — `trippin --spout-grab <name> <out.png>`.
pub fn grab(name: &str) -> Result<(u32, u32, Vec<u8>)> {
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_USAGE_STAGING,
    };
    let mut names = Shm::open("SpoutSenderNames", MAX_SENDERS * NAME_LEN)?;
    let list = read_names(names.bytes());
    if !list.iter().any(|n| n == name) {
        return Err(anyhow!("no Spout sender \"{name}\" (have: {})", list.join(", ")));
    }
    let mut info = Shm::open(name, INFO_SIZE)?;
    let b = info.bytes();
    let handle = u32::from_le_bytes(b[0..4].try_into()?);
    unsafe {
        let mut dev = None;
        let mut ctx = None;
        D3D11CreateDevice(None, D3D_DRIVER_TYPE_HARDWARE, HMODULE::default(), D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION, Some(&mut dev), None, Some(&mut ctx))?;
        let dev: ID3D11Device = dev.context("no device")?;
        let ctx: ID3D11DeviceContext = ctx.context("no context")?;
        let mut shared: Option<ID3D11Texture2D> = None;
        dev.OpenSharedResource(HANDLE(handle as usize as _), &mut shared)?;
        let shared = shared.context("open shared texture")?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        shared.GetDesc(&mut desc);
        let (w, h) = (desc.Width, desc.Height);
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.MiscFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        let mut stage = None;
        dev.CreateTexture2D(&desc, None, Some(&mut stage))?;
        let stage: ID3D11Texture2D = stage.context("staging")?;
        ctx.CopyResource(&stage, &shared);
        let mut m = D3D11_MAPPED_SUBRESOURCE::default();
        ctx.Map(&stage, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
        let src = std::slice::from_raw_parts(m.pData as *const u8, (m.RowPitch * h) as usize);
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            for p in src[y * m.RowPitch as usize..][..w as usize * 4].chunks_exact(4) {
                out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
            }
        }
        ctx.Unmap(&stage, 0);
        Ok((w, h, out))
    }
}

/// `trippin --spout-test`: publish a moving test pattern for 20 s.
pub fn test() -> Result<()> {
    let (w, h) = (640u32, 360u32);
    let s = Sender::new("Trippin test", w, h)?;
    println!("Spout sender \"{}\" up — open a Spout receiver (OBS Spout2 source)", s.name);
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let t0 = std::time::Instant::now();
    while t0.elapsed().as_secs() < 20 {
        let t = t0.elapsed().as_secs_f32();
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                buf[i] = ((x as f32 / w as f32) * 255.0) as u8;
                buf[i + 1] = ((y as f32 / h as f32) * 255.0) as u8;
                buf[i + 2] = (((t * 2.0).sin() * 0.5 + 0.5) * 255.0) as u8;
                buf[i + 3] = 255;
            }
        }
        s.send(&buf, w * 4);
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Receive our own frame the way a Spout receiver does: find the name in
    /// the registry, read the info block, open the shared handle on a
    /// separate device and read the pixels back.
    #[test]
    fn receiver_sees_frame() {
        let (w, h) = (64u32, 32u32);
        let Ok(s) = Sender::new("Trippin unit test", w, h) else {
            return; // no D3D11 hardware (CI)
        };
        let mut px = vec![0u8; (w * h * 4) as usize];
        for (i, p) in px.chunks_exact_mut(4).enumerate() {
            p.copy_from_slice(&[(i % 251) as u8, 7, 200, 255]);
        }
        s.send(&px, w * 4);

        let (gw, gh, rgba) = grab(&s.name).unwrap();
        assert_eq!((gw, gh), (w, h));
        let i = (5 * w as usize + 9) * 4;
        assert_eq!(&rgba[i..i + 4], &[px[i + 2], px[i + 1], px[i], px[i + 3]]);
        let mut names = Shm::open("SpoutSenderNames", MAX_SENDERS * NAME_LEN).unwrap();
        drop(s);
        assert!(!read_names(names.bytes()).contains(&"Trippin unit test".to_string()));
    }
}
