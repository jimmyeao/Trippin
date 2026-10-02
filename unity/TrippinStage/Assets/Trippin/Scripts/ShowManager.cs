// Switches between the engine's shows. Each show is a child GameObject with
// a matching Trippin scene name (shaders/scenes/<name>.wgsl in Trippin):
// when Trippin's auto-pilot cuts to "unity_crystals", the feed carries that
// name and this activates the Crystals show. With no feed it cycles through
// them every 16 bars as a demo. Only the active show runs (inactive objects
// get no Update), and each show drives the shared camera itself.

using UnityEngine;

namespace TrippinStage
{
    public class ShowManager : MonoBehaviour
    {
        public Camera cam;
        public RenderTexture output;
        public GameObject[] shows;
        public string[] names;

        int _cur = -1;

        void Start()
        {
            Application.targetFrameRate = 60;
            QualitySettings.vSyncCount = 0;
            Application.runInBackground = true;
            StageRecorder.TryStart(gameObject, output);
            // Optional fixed show: -show unity_flow
            var a = System.Environment.GetCommandLineArgs();
            for (int i = 0; i + 1 < a.Length; i++)
                if (a[i] == "-show") _forced = a[i + 1];
            for (int i = 0; i < shows.Length; i++) shows[i].SetActive(false);
        }

        string _forced;

        void Update()
        {
            var s = TrippinLink.State;
            string want;
            if (_forced != null) want = _forced;
            else if (TrippinLink.Live) want = s.scene;
            else want = names[(Mathf.FloorToInt(s.beat / 4f) / 16) % names.Length];
            int idx = System.Array.IndexOf(names, want);
            if (idx < 0) idx = _cur < 0 ? 0 : _cur;
            if (idx != _cur)
            {
                if (_cur >= 0) shows[_cur].SetActive(false);
                shows[idx].SetActive(true);
                _cur = idx;
            }
        }

        void OnGUI()
        {
            if (output != null && Event.current.type == EventType.Repaint)
                GUI.DrawTexture(new Rect(0, 0, Screen.width, Screen.height), output, ScaleMode.ScaleToFit, false);
        }
    }
}
