// Sends the output to Trippin through the shared-memory frame file Trippin
// passes as `-trippinFrame <path>` (see src/engine.rs): a 64-byte header
// (magic, version, width, height, seq at offset 16) then RGBA8 pixels, top
// row first. Each frame is written under a seqlock — seq goes odd while
// writing, even (+2) when done — so Trippin never shows a torn frame.
// Cross-platform: a memory-mapped file works the same on Windows and macOS.
//
// Readback (`-trippinReadback blit|direct|sync`, default blit):
//  - blit: copy the camera's RT into a plain colour-only RT and async-read
//    that copy, both in one command buffer. Apple-silicon GPUs returned the
//    camera RT itself with garbage on every 32-row tile boundary when it
//    was read back directly (its tiled/compressed layout leaking through);
//    the copy is an ordinary texture written by a later pass.
//  - direct: AsyncGPUReadback on the camera RT (the original path).
//  - sync: ReadPixels, like StageRecorder — stalls the GPU, last resort.

using System.IO;
using System.IO.MemoryMappedFiles;
using System.Threading;
using Unity.Collections;
using Unity.Collections.LowLevel.Unsafe;
using UnityEngine;
using UnityEngine.Rendering;

namespace TrippinStage
{
    public unsafe class FrameExporter : MonoBehaviour
    {
        const uint Magic = 0x46545254; // "TRTF"
        const int Header = 64;

        enum Mode { Blit, Direct, Sync }

        RenderTexture _src, _copy;
        Texture2D _cpu;
        CommandBuffer _cmd;
        Mode _mode = Mode.Blit;
        MemoryMappedFile _mmf;
        MemoryMappedViewAccessor _view;
        byte* _ptr;
        int _w, _h, _inflight;
        ulong _seq;

        public static bool TryStart(GameObject host, RenderTexture rt)
        {
            var a = System.Environment.GetCommandLineArgs();
            string path = null, mode = null;
            for (int i = 0; i + 1 < a.Length; i++)
            {
                if (a[i] == "-trippinFrame") path = a[i + 1];
                if (a[i] == "-trippinReadback") mode = a[i + 1];
            }
            if (path == null) return false;
            var e = host.AddComponent<FrameExporter>();
            e._src = rt;
            if (mode == "direct") e._mode = Mode.Direct;
            else if (mode == "sync") e._mode = Mode.Sync;
            try { e.Open(path); }
            catch (System.Exception ex)
            {
                Debug.LogError($"[FrameExporter] can't open {path}: {ex.Message}");
                Destroy(e);
                return false;
            }
            return true;
        }

        void Open(string path)
        {
            // Trippin holds the file open: share it (CreateFromFile(path)
            // asks for exclusive access and fails on Windows).
            var fs = new FileStream(path, FileMode.Open, FileAccess.ReadWrite, FileShare.ReadWrite | FileShare.Delete);
            _mmf = MemoryMappedFile.CreateFromFile(fs, null, 0, MemoryMappedFileAccess.ReadWrite, HandleInheritability.None, false);
            _view = _mmf.CreateViewAccessor(0, 0, MemoryMappedFileAccess.ReadWrite);
            byte* p = null;
            _view.SafeMemoryMappedViewHandle.AcquirePointer(ref p);
            _ptr = p + _view.PointerOffset;
            if (*(uint*)_ptr != Magic) throw new IOException("not a Trippin frame file");
            _w = *(int*)(_ptr + 8);
            _h = *(int*)(_ptr + 12);
            if (_w != _src.width || _h != _src.height)
                throw new IOException($"frame file is {_w}x{_h}, output is {_src.width}x{_src.height}");
            _seq = *(ulong*)(_ptr + 16) & ~1UL;
            if (_mode == Mode.Blit)
            {
                // Same sRGB-ness as the source, so the blit preserves bytes.
                _copy = new RenderTexture(_w, _h, 0, RenderTextureFormat.ARGB32,
                    _src.sRGB ? RenderTextureReadWrite.sRGB : RenderTextureReadWrite.Linear)
                { name = "Trippin frame copy", useMipMap = false, antiAliasing = 1 };
                _copy.Create();
                _cmd = new CommandBuffer { name = "Trippin frame export" };
            }
            else if (_mode == Mode.Sync)
                _cpu = new Texture2D(_w, _h, TextureFormat.RGBA32, false, !_src.sRGB);
            Debug.Log($"[FrameExporter] sending {_w}x{_h} to {path} (readback {_mode})");
        }

        void LateUpdate()
        {
            // Runs after ShowManager's LateUpdate rendered the camera.
            if (_ptr == null) return;
            if (_mode == Mode.Sync)
            {
                var prev = RenderTexture.active;
                RenderTexture.active = _src;
                _cpu.ReadPixels(new Rect(0, 0, _w, _h), 0, 0, false);
                RenderTexture.active = prev;
                Write(_cpu.GetRawTextureData<byte>());
                return;
            }
            if (_inflight >= 2) return;
            _inflight++;
            if (_mode == Mode.Direct)
            {
                AsyncGPUReadback.Request(_src, 0, TextureFormat.RGBA32, OnDone);
                return;
            }
            _cmd.Clear();
            _cmd.Blit(_src, _copy);
            _cmd.RequestAsyncReadback(_copy, 0, TextureFormat.RGBA32, OnDone);
            Graphics.ExecuteCommandBuffer(_cmd);
        }

        void OnDone(AsyncGPUReadbackRequest r)
        {
            _inflight--;
            if (r.hasError || _ptr == null) return;
            Write(r.GetData<byte>());
        }

        void Write(NativeArray<byte> data)
        {
            int row = _w * 4;
            if (data.Length < row * _h) return;
            byte* src = (byte*)data.GetUnsafeReadOnlyPtr();
            ulong* seq = (ulong*)(_ptr + 16);
            *seq = _seq + 1; // writing
            Thread.MemoryBarrier();
            byte* dst = _ptr + Header;
            // Unity's readback rows (async and ReadPixels alike) run bottom-up;
            // Trippin wants the top row first.
            for (int y = 0; y < _h; y++)
                UnsafeUtility.MemCpy(dst + (long)y * row, src + (long)(_h - 1 - y) * row, row);
            Thread.MemoryBarrier();
            _seq += 2;
            *seq = _seq; // done
            if (_seq <= 2) Debug.Log("[FrameExporter] first frame written");
        }

        void OnDestroy()
        {
            if (_view != null)
            {
                _view.SafeMemoryMappedViewHandle.ReleasePointer();
                _view.Dispose();
            }
            _mmf?.Dispose();
            _ptr = null;
            _cmd?.Release();
            if (_copy != null) _copy.Release();
        }
    }
}
