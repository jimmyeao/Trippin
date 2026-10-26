// Switches between the engine's shows. Each show is a child GameObject with
// a matching Trippin scene name (shaders/scenes/<name>.wgsl in Trippin):
// when Trippin's auto-pilot cuts to "unity_crystals", the feed carries that
// name and this activates the Crystals show. With no feed it cycles through
// them every 16 bars as a demo. Only the active show runs (inactive objects
// get no Update), and each show drives the shared camera itself.

using UnityEngine;

namespace TrippinStage
{
    [DefaultExecutionOrder(-50)]
    public class ShowManager : MonoBehaviour
    {
        public Camera cam;
        public RenderTexture output;
        public GameObject[] shows;
        public string[] names;

        int _cur = -1;
        // Demo cycle (no feed): `-cycleBars N` bars per show (default 16), and
        // `-tourShows a,b,c` to cycle only those shows — for quick recorded tours.
        int _cycleBars = 16;
        string[] _tour;
        // `-knobs 1,2.5,0.6`: the show's controls (manifest order) without a
        // feed, so a -replayFeed recording can test them deterministically.
        float[] _argKnobs;

        /// `-uncapped`: no 60 fps cap, so the frame-time log shows the real
        /// cost (profiling only — Trippin never passes it).
        public static bool Uncapped { get; private set; }
        float _ftSum, _ftLast;
        int _ftN;

        void Start()
        {
            Uncapped = System.Array.IndexOf(System.Environment.GetCommandLineArgs(), "-uncapped") >= 0;
            Application.targetFrameRate = Uncapped ? -1 : 60;
            QualitySettings.vSyncCount = 0;
            Application.runInBackground = true;
            StageRecorder.TryStart(gameObject, output);
            FrameExporter.TryStart(gameObject, output);
            // Optional fixed show: -show unity_flow
            var a = System.Environment.GetCommandLineArgs();
            for (int i = 0; i + 1 < a.Length; i++)
            {
                if (a[i] == "-show") _forced = a[i + 1];
                if (a[i] == "-cycleBars" && int.TryParse(a[i + 1], out var cb) && cb > 0) _cycleBars = cb;
                if (a[i] == "-tourShows") _tour = a[i + 1].Split(',');
                if (a[i] == "-knobs")
                    _argKnobs = System.Array.ConvertAll(a[i + 1].Split(','), v =>
                        float.Parse(v, System.Globalization.CultureInfo.InvariantCulture));
            }
            for (int i = 0; i < shows.Length; i++) shows[i].SetActive(false);
            ShowParams.Init(shows, names);
        }

        string _forced;

        void Update()
        {
            var s = TrippinLink.State;
            DropDirector.Tick(s, Time.deltaTime);
            string want;
            if (_forced != null) want = _forced;
            else if (TrippinLink.Live) want = s.scene;
            else
            {
                string[] list = _tour != null && _tour.Length > 0 ? _tour : names;
                want = list[(Mathf.FloorToInt(s.beat / 4f) / _cycleBars) % list.Length];
            }
            int idx = System.Array.IndexOf(names, want);
            if (idx < 0) idx = _cur < 0 ? 0 : _cur;
            if (idx != _cur)
            {
                if (_cur >= 0) shows[_cur].SetActive(false);
                shows[idx].SetActive(true);
                _cur = idx;
                _ftSum = 0; _ftN = 0; _ftLast = Time.unscaledTime;
            }
            // Trippin's per-show controls for the show on air.
            if (TrippinLink.Live && want == names[_cur]) ShowParams.Apply(_cur, s.knobs);
            else if (!TrippinLink.Live && _argKnobs != null) ShowParams.Apply(_cur, _argKnobs);

            // Frame time for whichever show is up (was StageDirector-only).
            _ftSum += Time.unscaledDeltaTime;
            _ftN++;
            if (Time.unscaledTime - _ftLast > 5f && _ftN > 0)
            {
                Debug.Log($"[Stage] {1000f * _ftSum / _ftN:F2} ms/frame avg over {_ftN} frames, show {names[_cur]}, link {(TrippinLink.Live ? "live" : "synthetic")}{(Uncapped ? ", uncapped" : "")}");
                _ftSum = 0; _ftN = 0; _ftLast = Time.unscaledTime;
            }
        }

        // Headless (-batchmode, how Trippin launches us) the player loop
        // doesn't render cameras by itself — render explicitly, before
        // FrameExporter reads the output back.
        void LateUpdate()
        {
            if (Application.isBatchMode && cam != null) cam.Render();
        }

        void OnGUI()
        {
            if (output != null && !Application.isBatchMode && Event.current.type == EventType.Repaint)
                GUI.DrawTexture(new Rect(0, 0, Screen.width, Screen.height), output, ScaleMode.ScaleToFit, false);
        }
    }
}
