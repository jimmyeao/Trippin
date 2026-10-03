// Offline capture: `TrippinStage.exe -record <dir> [-recordSeconds 64] [-recordFps 30]`
// renders on a fixed game clock (Time.captureFramerate), so the clip is
// smooth however long each frame really takes, writes every frame of the
// output texture as a JPEG, then quits. Encode with ffmpeg afterwards.

using System.IO;
using UnityEngine;

namespace TrippinStage
{
    public class StageRecorder : MonoBehaviour
    {
        public RenderTexture source;
        string _dir;
        int _fps = 30, _frames, _n;
        Texture2D _tex;

        public static bool TryStart(GameObject host, RenderTexture rt)
        {
            var a = System.Environment.GetCommandLineArgs();
            string dir = null;
            float secs = 64f;
            int fps = 30;
            for (int i = 0; i + 1 < a.Length; i++)
            {
                if (a[i] == "-record") dir = a[i + 1];
                if (a[i] == "-recordSeconds") float.TryParse(a[i + 1], out secs);
                if (a[i] == "-recordFps") int.TryParse(a[i + 1], out fps);
            }
            if (dir == null) return false;
            var r = host.AddComponent<StageRecorder>();
            r.source = rt;
            r._dir = dir;
            r._fps = Mathf.Max(1, fps);
            r._frames = Mathf.CeilToInt(secs * r._fps);
            Directory.CreateDirectory(dir);
            Time.captureFramerate = r._fps;
            return true;
        }

        void LateUpdate()
        {
            // Read last frame's output (the camera has rendered it by now).
            if (_tex == null) _tex = new Texture2D(source.width, source.height, TextureFormat.RGB24, false);
            var prev = RenderTexture.active;
            RenderTexture.active = source;
            _tex.ReadPixels(new Rect(0, 0, source.width, source.height), 0, 0, false);
            _tex.Apply(false);
            RenderTexture.active = prev;
            File.WriteAllBytes(Path.Combine(_dir, $"f{_n:D5}.jpg"), _tex.EncodeToJPG(90));
            _n++;
            if (_n >= _frames)
            {
                Debug.Log($"[Recorder] wrote {_n} frames to {_dir}");
                Application.Quit();
            }
        }
    }
}
