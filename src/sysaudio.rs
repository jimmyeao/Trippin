//! System audio capture via ScreenCaptureKit — the same API OBS uses for
//! desktop audio on macOS. It taps the output mix off the display stream:
//! no mic, no BlackHole, no multi-output device. macOS asks once for
//! screen/system-audio recording permission (per app; the Terminal or the
//! bundled .app, whichever launched the process).
//!
//! Needs macOS 13+ (audio capture); `capturesVideo` needs 14+, so it's sent
//! dynamically behind a respondsToSelector check.

use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchQueueAttr, DispatchRetained};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AllocAnyThread, DefinedClass, define_class, msg_send, sel};
use objc2_core_audio_types::{
    AudioBufferList, kAudioFormatFlagIsFloat, kAudioFormatFlagIsNonInterleaved,
    kAudioFormatFlagIsSignedInteger, kAudioFormatLinearPCM,
};
use objc2_core_foundation::CFRetained;
use objc2_core_media::{
    CMAudioFormatDescriptionGetStreamBasicDescription, CMBlockBuffer, CMSampleBuffer,
};
use objc2_foundation::{NSArray, NSError};
use objc2_screen_capture_kit::{
    SCContentFilter, SCShareableContent, SCStream, SCStreamConfiguration, SCStreamOutput,
    SCStreamOutputType, SCWindow,
};

/// Capture runs at this rate (`SCStreamConfiguration.sampleRate`).
pub const SAMPLE_RATE: f32 = 48_000.0;

struct SinkIvars {
    tx: SyncSender<Vec<f32>>,
    /// Log unexpected sample formats once, not per buffer.
    bad_format: AtomicBool,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "TrippinAudioSink"]
    #[ivars = SinkIvars]
    struct AudioSink;

    unsafe impl NSObjectProtocol for AudioSink {}

    unsafe impl SCStreamOutput for AudioSink {
        // SAFETY: signature matches the protocol method.
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn did_output(
            &self,
            _stream: &SCStream,
            sample_buffer: &CMSampleBuffer,
            ty: SCStreamOutputType,
        ) {
            if ty == SCStreamOutputType::Audio {
                // SAFETY: the sample buffer is valid for the duration of the callback.
                unsafe { push_audio(self.ivars(), sample_buffer) };
            }
        }
    }
);

impl AudioSink {
    fn new(tx: SyncSender<Vec<f32>>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(SinkIvars {
            tx,
            bad_format: AtomicBool::new(false),
        });
        // SAFETY: `init` on NSObject takes no arguments.
        unsafe { msg_send![super(this), init] }
    }
}

/// Read one PCM sample of `bytes` wide at `p`, normalised to -1..1.
unsafe fn pcm_at(p: *const u8, bytes: usize, float: bool, signed: bool) -> Option<f32> {
    unsafe {
        match (float, signed, bytes) {
            (true, _, 4) => Some(*(p as *const f32)),
            (false, true, 2) => Some(*(p as *const i16) as f32 / i16::MAX as f32),
            (false, true, 4) => Some(*(p as *const i32) as f32 / i32::MAX as f32),
            _ => None,
        }
    }
}

/// Pull mono f32 out of an audio CMSampleBuffer and hand it to the analyser.
unsafe fn push_audio(iv: &SinkIvars, buf: &CMSampleBuffer) {
    unsafe {
        if !buf.is_valid() || !buf.data_is_ready() {
            return;
        }
        let Some(desc) = buf.format_description() else {
            return;
        };
        let Some(asbd) = CMAudioFormatDescriptionGetStreamBasicDescription(&desc).as_ref() else {
            return;
        };
        let float = asbd.mFormatFlags & kAudioFormatFlagIsFloat != 0;
        let signed = asbd.mFormatFlags & kAudioFormatFlagIsSignedInteger != 0;
        let bps = asbd.mBitsPerChannel as usize / 8;
        let channels = (asbd.mChannelsPerFrame as usize).max(1);
        if asbd.mFormatID != kAudioFormatLinearPCM || bps == 0 || !(float || signed) {
            if !iv.bad_format.swap(true, Ordering::Relaxed) {
                eprintln!(
                    "system audio: unexpected format (id {:#x}, flags {:#x}, {}-bit)",
                    asbd.mFormatID, asbd.mFormatFlags, asbd.mBitsPerChannel
                );
            }
            return;
        }

        // The buffer list is variable-length: size it first, then fill it.
        let mut needed: usize = 0;
        let status = buf.audio_buffer_list_with_retained_block_buffer(
            &mut needed,
            ptr::null_mut(),
            0,
            None,
            None,
            0,
            ptr::null_mut(),
        );
        if status != 0 || needed == 0 {
            return;
        }
        let mut storage = vec![0u8; needed];
        let mut block: *mut CMBlockBuffer = ptr::null_mut();
        let status = buf.audio_buffer_list_with_retained_block_buffer(
            ptr::null_mut(),
            storage.as_mut_ptr() as *mut AudioBufferList,
            needed,
            None,
            None,
            0,
            &mut block,
        );
        if status != 0 || block.is_null() {
            return;
        }
        // `block` arrives retained; keep the AudioBufferList's data alive
        // while we read it, then release on drop.
        let _block = CFRetained::from_raw(NonNull::new_unchecked(block));

        let abl = &*(storage.as_ptr() as *const AudioBufferList);
        let buffers =
            std::slice::from_raw_parts(abl.mBuffers.as_ptr(), abl.mNumberBuffers as usize);
        let planar = asbd.mFormatFlags & kAudioFormatFlagIsNonInterleaved != 0;

        // Planar buffers carry one channel each; interleaved pack all of them.
        let mut mono: Vec<f32> = Vec::new();
        for (i, ab) in buffers.iter().enumerate() {
            if ab.mData.is_null() || ab.mDataByteSize == 0 {
                continue;
            }
            let here = (ab.mNumberChannels as usize).max(1);
            let frames = ab.mDataByteSize as usize / (bps * here);
            if mono.len() < frames {
                mono.resize(frames, 0.0);
            }
            let data = ab.mData as *const u8;
            let ch_base = if planar { i } else { 0 };
            for f in 0..frames {
                for c in 0..here {
                    let Some(v) = pcm_at(data.add((f * here + c) * bps), bps, float, signed) else {
                        return;
                    };
                    if ch_base + c < channels {
                        mono[f] += v;
                    }
                }
            }
        }
        for v in mono.iter_mut() {
            *v /= channels as f32;
        }
        // Drop audio rather than block the capture queue if analysis stalls.
        let _ = iv.tx.try_send(mono);
    }
}

fn shareable_content() -> Result<Retained<SCShareableContent>> {
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(
        move |content: *mut SCShareableContent, error: *mut NSError| {
            let res = match unsafe { Retained::retain(content) } {
                Some(c) => Ok(c),
                None => {
                    // Screen capture never prompts on modern macOS — the user has
                    // to switch the responsible app on in System Settings.
                    let msg = unsafe { error.as_ref() }
                        .map(|e| e.localizedDescription().to_string())
                        .unwrap_or_else(|| "no shareable content".into());
                    Err(format!(
                        "{msg} — grant “Screen & System Audio Recording” to the \
                     app that launched trippin in System Settings → Privacy & Security"
                    ))
                }
            };
            let _ = tx.send(res);
        },
    );
    unsafe {
        SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(
            true, false, &block,
        );
    }
    rx.recv_timeout(Duration::from_secs(15))
        .context("ScreenCaptureKit timed out")?
        .map_err(|e| anyhow!("{e}"))
}

/// Keep-alive handle for the capture. The `Retained`s just need to outlive
/// the session — all real work happens on the stream's own dispatch queue,
/// so holding them on the render thread is fine.
pub struct SystemCapture {
    _stream: Retained<SCStream>,
    _sink: Retained<AudioSink>,
    _queue: DispatchRetained<DispatchQueue>,
}

// SAFETY: nothing here is touched after start() returns; the SCStream/sink
// are only retained so capture keeps running. Apple's SCStream may be used
// from any thread.
unsafe impl Send for SystemCapture {}
unsafe impl Sync for SystemCapture {}

/// Start capturing the system output mix. Errors here mean the pre-capture
/// setup failed; permission denial arrives asynchronously on the start
/// handler (the analyser just sees silence).
pub fn start(tx: SyncSender<Vec<f32>>) -> Result<SystemCapture> {
    if AnyClass::get(c"SCStream").is_none() {
        anyhow::bail!("ScreenCaptureKit needs macOS 13 or newer");
    }
    let content = shareable_content()?;
    let displays = unsafe { content.displays() };
    let display = displays
        .firstObject()
        .context("no display to capture audio from (is screen/audio recording allowed?)")?;

    let excluded: Retained<NSArray<SCWindow>> = NSArray::new();
    let filter = unsafe {
        SCContentFilter::initWithDisplay_excludingWindows(
            SCContentFilter::alloc(),
            &display,
            &excluded,
        )
    };

    let config = unsafe { SCStreamConfiguration::new() };
    unsafe {
        config.setCapturesAudio(true);
        config.setExcludesCurrentProcessAudio(true);
        config.setSampleRate(SAMPLE_RATE as isize);
        config.setChannelCount(2);
        // A video stream is still required underneath — keep it microscopic
        // and never register a screen output, so no frames are delivered.
        config.setWidth(2);
        config.setHeight(2);
        config.setShowsCursor(false);
        // macOS 14+: audio-only capture (lighter permission, no video frames).
        if config.respondsToSelector(sel!(setCapturesVideo:)) {
            let _: () = msg_send![&config, setCapturesVideo: false];
        }
    }

    let stream = unsafe {
        SCStream::initWithFilter_configuration_delegate(SCStream::alloc(), &filter, &config, None)
    };
    let sink = AudioSink::new(tx);
    let queue = DispatchQueue::new("trippin.system-audio", DispatchQueueAttr::SERIAL);
    unsafe {
        stream
            .addStreamOutput_type_sampleHandlerQueue_error(
                ProtocolObject::from_ref(&*sink),
                SCStreamOutputType::Audio,
                Some(&queue),
            )
            .map_err(|e| anyhow!("SCStream addStreamOutput: {}", e.localizedDescription()))?;
    }

    let block = RcBlock::new(|error: *mut NSError| {
        if let Some(e) = unsafe { error.as_ref() } {
            eprintln!(
                "system audio capture failed: {} — use --mic or --device",
                e.localizedDescription()
            );
        }
    });
    unsafe { stream.startCaptureWithCompletionHandler(Some(&block)) };

    Ok(SystemCapture {
        _stream: stream,
        _sink: sink,
        _queue: queue,
    })
}
