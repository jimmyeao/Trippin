// Receives Trippin's show-state feed (src/link.rs): one JSON datagram per
// rendered frame on 127.0.0.1:9137. Exposes the latest state and pushes it
// to shader globals every frame. With no feed for a second it falls back to
// a synthetic 126 BPM groove (like `trippin --snap`), so the stage animates
// on its own for testing and screenshots.

using System;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading;
using UnityEngine;

namespace TrippinStage
{
    [Serializable]
    public class ShowState
    {
        public float time, dt, bpm = 126f, beat, beat_phase, bar_phase;
        public float bass, mid, high, energy, onset, kick, build;
        public float intensity = 0.7f, calm, flow, hue, flash, master = 1f;
        public float[] lvl4 = new float[4], hits4 = new float[4], pres4 = new float[4], clock4 = new float[4];
        public float[] spectrum = new float[32];
        public float[] palette = new float[24];
        public string scene = "";
        public bool cut, drums = true;
    }

    [DefaultExecutionOrder(-100)]
    public class TrippinLink : MonoBehaviour
    {
        public int port = 9137;

        /// Latest state (live feed, or the synthetic groove when there's none).
        public static ShowState State { get; private set; } = new ShowState();
        /// True while datagrams are arriving.
        public static bool Live { get; private set; }

        UdpClient _udp;
        Thread _thread;
        volatile bool _run;
        string _pending;
        readonly object _lock = new object();
        float _lastPacket = -10f;
        float _synthBeat;

        static readonly int ID_Beat = Shader.PropertyToID("_TBeat");
        static readonly int ID_BeatPhase = Shader.PropertyToID("_TBeatPhase");
        static readonly int ID_BarPhase = Shader.PropertyToID("_TBarPhase");
        static readonly int ID_Kick = Shader.PropertyToID("_TKick");
        static readonly int ID_Onset = Shader.PropertyToID("_TOnset");
        static readonly int ID_Energy = Shader.PropertyToID("_TEnergy");
        static readonly int ID_Intensity = Shader.PropertyToID("_TIntensity");
        static readonly int ID_Calm = Shader.PropertyToID("_TCalm");
        static readonly int ID_Flow = Shader.PropertyToID("_TFlow");
        static readonly int ID_Lvl = Shader.PropertyToID("_TLvl");
        static readonly int ID_Hits = Shader.PropertyToID("_THits");
        static readonly int ID_Pres = Shader.PropertyToID("_TPres");
        static readonly int ID_Clock = Shader.PropertyToID("_TClock");
        static readonly int ID_Pal = Shader.PropertyToID("_TPal");
        static readonly int ID_Spec = Shader.PropertyToID("_TSpec");
        readonly Vector4[] _pal = new Vector4[8];
        readonly Vector4[] _spec = new Vector4[8];

        // Launched by Trippin (src/engine.rs): it passes the feed port and the
        // frame file, and we quit if its feed goes quiet — no orphans.
        bool _managed;
        float _heard;

        void OnEnable()
        {
            var a = System.Environment.GetCommandLineArgs();
            for (int i = 0; i + 1 < a.Length; i++)
            {
                if (a[i] == "-trippinPort" && int.TryParse(a[i + 1], out var p)) port = p;
                if (a[i] == "-trippinFrame") _managed = true;
            }
            _heard = Time.realtimeSinceStartup;
            try
            {
                _udp = new UdpClient(new IPEndPoint(IPAddress.Loopback, port));
                _run = true;
                _thread = new Thread(Listen) { IsBackground = true, Name = "trippin-link" };
                _thread.Start();
            }
            catch (Exception e)
            {
                Debug.LogWarning($"TrippinLink: can't listen on {port}: {e.Message}");
            }
        }

        void OnDisable()
        {
            _run = false;
            _udp?.Close();
            _udp = null;
        }

        void Listen()
        {
            var any = new IPEndPoint(IPAddress.Any, 0);
            while (_run)
            {
                try
                {
                    var bytes = _udp.Receive(ref any);
                    var json = Encoding.UTF8.GetString(bytes);
                    lock (_lock) _pending = json;
                }
                catch (SocketException) { }
                catch (ObjectDisposedException) { return; }
            }
        }

        void Update()
        {
            string json;
            lock (_lock) { json = _pending; _pending = null; }
            if (json != null)
            {
                try
                {
                    var s = JsonUtility.FromJson<ShowState>(json);
                    if (s != null)
                    {
                        State = s;
                        _lastPacket = Time.unscaledTime;
                    }
                }
                catch (Exception) { }
            }
            Live = Time.unscaledTime - _lastPacket < 1f;
            if (Live) _heard = Time.realtimeSinceStartup;
            if (_managed && Time.realtimeSinceStartup - _heard > 15f)
            {
                Debug.Log("[TrippinLink] Trippin's feed stopped — quitting");
                Application.Quit();
            }
            if (!Live) Synthesize();
            PushGlobals(State);
        }

        // A 126 BPM four-on-the-floor groove with a 32-bar arrangement
        // (16 bars of drums, 8-bar breakdown, 8-bar build back) so every part
        // of the stage gets exercised without Trippin running.
        void Synthesize()
        {
            var s = State;
            s.bpm = 126f;
            float dt = Time.deltaTime;
            _synthBeat += dt * s.bpm / 60f;
            s.beat = _synthBeat;
            s.beat_phase = _synthBeat - Mathf.Floor(_synthBeat);
            s.bar_phase = (_synthBeat % 4f) / 4f;
            float bar = (_synthBeat / 4f) % 32f;
            bool drums = bar < 16f || bar >= 24f;
            s.calm = Mathf.MoveTowards(s.calm, drums ? 0f : 1f, dt * 0.5f);
            s.drums = drums;
            float k = drums ? Mathf.Exp(-s.beat_phase * 7f) : 0f;
            s.kick = k;
            s.onset = drums ? Mathf.Max(k, Mathf.Exp(-((_synthBeat * 2f) % 1f) * 9f) * 0.5f) : 0.1f;
            s.energy = drums ? 0.6f + 0.3f * k : 0.25f;
            s.intensity = Mathf.MoveTowards(s.intensity, drums ? 0.85f : 0.35f, dt * 0.3f);
            s.flow += dt * s.bpm / 60f * (0.4f + 0.8f * s.intensity);
            for (int i = 0; i < 4; i++)
            {
                s.lvl4[i] = 0.35f + 0.4f * (i == 0 ? k : 0.5f + 0.5f * Mathf.Sin(_synthBeat * (1.3f + i))) * (drums ? 1f : 0.5f);
                s.hits4[i] = i == 0 ? k : (drums && (_synthBeat * (i + 1)) % 1f < 0.1f ? 0.6f : 0f);
                s.pres4[i] = 0.5f;
                s.clock4[i] += dt * s.bpm / 60f * (0.3f + 2.4f * Mathf.Pow(s.lvl4[i], 1.6f));
            }
            for (int i = 0; i < 32; i++)
                s.spectrum[i] = Mathf.Clamp01(0.7f - i * 0.015f + 0.3f * Mathf.Sin(_synthBeat * 2f + i * 0.7f)) * (drums ? 1f : 0.6f);
            // Synthwave-ish palette until Trippin sends its own.
            Color[] p = { new Color(0.05f,0.0f,0.3f), new Color(0.3f,0.0f,0.8f), new Color(0.9f,0.0f,0.8f), new Color(1f,0.2f,0.4f),
                          new Color(1f,0.5f,0.1f), new Color(1f,0.9f,0.2f), new Color(0.2f,0.9f,1f), new Color(0.1f,0.4f,1f) };
            for (int i = 0; i < 8; i++) { s.palette[i*3] = p[i].r; s.palette[i*3+1] = p[i].g; s.palette[i*3+2] = p[i].b; }
            s.scene = "unity_stage";
        }

        void PushGlobals(ShowState s)
        {
            Shader.SetGlobalFloat(ID_Beat, s.beat);
            Shader.SetGlobalFloat(ID_BeatPhase, s.beat_phase);
            Shader.SetGlobalFloat(ID_BarPhase, s.bar_phase);
            Shader.SetGlobalFloat(ID_Kick, s.kick);
            Shader.SetGlobalFloat(ID_Onset, s.onset);
            Shader.SetGlobalFloat(ID_Energy, s.energy);
            Shader.SetGlobalFloat(ID_Intensity, s.intensity);
            Shader.SetGlobalFloat(ID_Calm, s.calm);
            Shader.SetGlobalFloat(ID_Flow, s.flow);
            Shader.SetGlobalVector(ID_Lvl, V4(s.lvl4));
            Shader.SetGlobalVector(ID_Hits, V4(s.hits4));
            Shader.SetGlobalVector(ID_Pres, V4(s.pres4));
            Shader.SetGlobalVector(ID_Clock, V4(s.clock4));
            for (int i = 0; i < 8; i++)
            {
                int j = i * 3;
                _pal[i] = s.palette != null && s.palette.Length >= j + 3
                    ? new Vector4(s.palette[j], s.palette[j + 1], s.palette[j + 2], 1f)
                    : Vector4.one;
                int k = i * 4;
                _spec[i] = s.spectrum != null && s.spectrum.Length >= k + 4
                    ? new Vector4(s.spectrum[k], s.spectrum[k + 1], s.spectrum[k + 2], s.spectrum[k + 3])
                    : Vector4.zero;
            }
            Shader.SetGlobalVectorArray(ID_Pal, _pal);
            Shader.SetGlobalVectorArray(ID_Spec, _spec);
        }

        static Vector4 V4(float[] a) =>
            a != null && a.Length >= 4 ? new Vector4(a[0], a[1], a[2], a[3]) : Vector4.zero;

        /// Palette colour at t (0..1), interpolated across the 8 entries.
        public static Color Palette(float t)
        {
            var p = State.palette;
            if (p == null || p.Length < 24) return Color.white;
            t = Mathf.Repeat(t, 1f) * 7f;
            int i = Mathf.Min((int)t, 6);
            float f = t - i;
            Color a = new Color(p[i * 3], p[i * 3 + 1], p[i * 3 + 2]);
            Color b = new Color(p[i * 3 + 3], p[i * 3 + 4], p[i * 3 + 5]);
            return Color.Lerp(a, b, f);
        }
    }
}
