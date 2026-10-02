// Sends the output to Trippin through the shared-memory frame file Trippin
// passes as `-trippinFrame <path>` (see src/engine.rs): a 64-byte header
// (magic, version, width, height, seq at offset 16) then RGBA8 pixels, top
// row first. Async GPU readback keeps the render loop from stalling; each
// finished readback is written under a seqlock — seq goes odd while
// writing, even (+2) when done — so Trippin never shows a torn frame.
// Cross-platform: a memory-mapped file works the same on Windows and macOS.

using System.IO;
using System.IO.MemoryMappedFiles;
using System.Threading;
using Unity.Collections.LowLevel.Unsafe;
using UnityEngine;
using UnityEngine.Rendering;

namespace TrippinStage
{
    public unsafe class FrameExporter : MonoBehaviour
    {
        const uint Magic = 0x46545254; // "TRTF"
        const int Header = 64;

        RenderTexture _src;
        MemoryMappedFile _mmf;
        MemoryMappedViewAccessor _view;
        byte* _ptr;
        int _w, _h, _inflight;
        ulong _seq;

        public static bool TryStart(GameObject host, RenderTexture rt)
        {
            var a = System.Environment.GetCommandLineArgs();
            string path = null;
            for (int i = 0; i + 1 < a.Length; i++)
                if (a[i] == "-trippinFrame") path = a[i + 1];
            if (path == null) return false;
            var e = host.AddComponent<FrameExporter>();
            e._src = rt;
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
            Debug.Log($"[FrameExporter] sending {_w}x{_h} to {path}");
        }

        void LateUpdate()
        {
            // Reads what the camera rendered last frame — one frame of latency.
            if (_ptr == null || _inflight >= 2) return;
            _inflight++;
            AsyncGPUReadback.Request(_src, 0, TextureFormat.RGBA32, OnDone);
        }

        void OnDone(AsyncGPUReadbackRequest r)
        {
            _inflight--;
            if (r.hasError || _ptr == null) return;
            var data = r.GetData<byte>();
            int row = _w * 4;
            if (data.Length < row * _h) return;
            byte* src = (byte*)data.GetUnsafeReadOnlyPtr();
            ulong* seq = (ulong*)(_ptr + 16);
            *seq = _seq + 1; // writing
            Thread.MemoryBarrier();
            byte* dst = _ptr + Header;
            // GPU readback rows run bottom-up on every graphics API Unity
            // uses here; Trippin wants the top row first.
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
        }
    }
}
