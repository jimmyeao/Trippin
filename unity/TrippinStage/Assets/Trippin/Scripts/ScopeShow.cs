// unity_scope: close-ups of an analogue oscilloscope on the booth table,
// drawing the music itself: Trippin's triggered waveform goes into the
// beam, and the green phosphor (ScopeTrace.compute) holds a fading trail
// of where it has been (HqScope.shader for the CRT face, GearKit + HqGear
// for the case, bezel, knobs and inputs). 1 unit = 10 cm.
//  - Shape: the trace IS the waveform. A new display every 4 bars,
//    morphing in over a beat: the classic sweep, an XY figure (the waveform
//    against itself a quarter-window later, the way a stereo pair draws on
//    a scope), a ring the waveform wobbles, and a 3D knot of three delayed
//    copies. The bass swells the figure; the beam brightens where it moves
//    slowly, as on a real tube.
//  - Motion: the ring turns and the knot rotates on the smooth energy clock,
//    the XY figure's axis swings with the phrase, the timebase knob clicks
//    round with each new display, and the camera drifts through four shots
//    of the instrument, cutting every 8 bars.
// Controls: gain, persistence, colour, booth lamp, club lights, drift.
using UnityEngine;

namespace TrippinStage
{
    public sealed class ScopeShow : KitShow
    {
        public Material gearMat, clubMat, crtMat;
        public ComputeShader traceCs;

        [TrippinParam("Gain", 0.3f, 2f)] public float gainAmt = 1f;
        [TrippinParam("Persistence", 0f, 1f)] public float persist = 0.55f;
        [TrippinParam("Colour", 0f, 1f)] public float colour = 0f;
        [TrippinParam("Booth lamp", 0.3f, 2f)] public float lamp = 1f;
        [TrippinParam("Club lights", 0f, 2f)] public float club = 1f;
        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;

        const int NP = 160, TW = 512, TH = 416;
        static readonly Vector3 Screen = new Vector3(-0.75f, 0.85f, -2.24f);

        GearMats _m;
        Booth _booth;
        Material _case, _panel, _bezel, _face, _ring, _bnc;
        Mesh _caseM, _panelM, _bezelM, _screenM, _knobL, _knobS, _bncM, _footM, _handleM, _ledM;
        RenderTexture _trace;
        int _k;
        readonly Vector4[] _pts = new Vector4[NP];
        readonly Vector2[] _p = new Vector2[NP], _p0 = new Vector2[NP];
        int _form = -1, _shot = -1;
        float _peak = 0.2f, _bass, _tb, _vd;

        protected override void Build()
        {
            _m = new GearMats(gearMat);
            _booth = new Booth(transform, clubMat, _m);
            _case = _m.M(0.62f, 0.6f, 0.55f, 0.1f, 0.55f);                 // the cream-grey case
            _panel = _m.M(0.07f, 0.075f, 0.08f, 0.6f, 0.4f, 0.4f);          // brushed dark front panel
            _bezel = _m.M(0.02f, 0.02f, 0.022f, 0f, 0.45f);
            _ring = _m.M(0.75f, 0.75f, 0.77f, 1f, 0.25f);
            _bnc = _m.chrome;
            _face = new Material(crtMat);
            _caseM = GearMesh.RoundedBox(new Vector3(3.3f, 1.6f, 4.4f), 0.08f, 3, "scope case");
            _panelM = GearMesh.RoundedBox(new Vector3(3.2f, 1.5f, 0.06f), 0.02f, 2, "scope panel");
            _bezelM = GearMesh.RoundedBox(new Vector3(1.24f, 1.02f, 0.12f), 0.08f, 3, "scope bezel");
            _screenM = GearMesh.Quad(1.0f, 0.8f, "crt");
            _knobL = GearMesh.Knob(0.17f, 0.16f, "volts knob");
            _knobS = GearMesh.Knob(0.09f, 0.1f, "small knob");
            _bncM = GearMesh.Lathe(new[] { new Vector2(0f, 0.16f), new Vector2(0.045f, 0.16f), new Vector2(0.045f, 0.16f), new Vector2(0.06f, 0.1f), new Vector2(0.06f, 0f), new Vector2(0.06f, 0f), new Vector2(0f, 0f) }, 20, "bnc");
            _footM = GearMesh.RoundedBox(new Vector3(0.3f, 0.08f, 0.3f), 0.03f, 2, "foot");
            _handleM = GearMesh.RoundedBox(new Vector3(0.12f, 0.12f, 3.6f), 0.05f, 2, "handle");
            _ledM = GearMesh.Lathe(new[] { new Vector2(0f, 0.05f), new Vector2(0.03f, 0.04f), new Vector2(0.03f, 0f), new Vector2(0f, 0f) }, 12, "led");
            _trace = new RenderTexture(TW, TH, 0, RenderTextureFormat.ARGBHalf) { enableRandomWrite = true, filterMode = FilterMode.Bilinear, wrapMode = TextureWrapMode.Clamp, name = "scope trace" };
            _trace.Create();
            _k = traceCs.FindKernel("Trace");
            traceCs.SetTexture(_k, "_Out", _trace);
            traceCs.SetVector("_Size", new Vector4(TW, TH, 0f, 0f));
            _face.SetTexture("_Trace", _trace);
        }

        void OnDestroy() { if (_trace != null) _trace.Release(); }

        float W(float[] w, float i)
        {
            int n = w.Length;
            float f = Mathf.Repeat(i, n);
            int a = (int)f, b = (a + 1) % n;
            return Mathf.Lerp(w[a], w[b], f - a);
        }

        /// Display k's beam position (divisions) for point j.
        Vector2 Figure(int k, int j, float[] w, float g)
        {
            float u = j / (NP - 1f);
            float c = rx.clk;
            switch (k)
            {
                default:
                case 0:   // the sweep: time across, the waveform up and down
                    return new Vector2(-5f + 10f * u, W(w, u * 63f) * g * 3.2f);
                case 1:   // XY: the waveform against itself a quarter-window later, the axis swinging with the phrase
                {
                    float x = W(w, u * 63f) * g * 3f, y = W(w, u * 63f + 16f) * g * 3f;
                    float a = Mathf.PI * 0.25f + 0.35f * Mathf.Sin(rx.phrase * 0.5f);
                    return new Vector2(x * Mathf.Cos(a) - y * Mathf.Sin(a), x * Mathf.Sin(a) + y * Mathf.Cos(a));
                }
                case 2:   // a ring the waveform wobbles, turning on the clock
                {
                    float a = u * Mathf.PI * 2f + c * 0.25f;
                    float r = 2.4f + W(w, u * 64f) * g * 1.4f;
                    return new Vector2(Mathf.Cos(a) * r, Mathf.Sin(a) * r);
                }
                case 3:   // a knot: three delayed copies as x, y, z, turning on the clock
                {
                    var p = new Vector3(W(w, u * 63f), W(w, u * 63f + 11f), W(w, u * 63f + 23f)) * g * 3.2f;
                    var q = Quaternion.Euler(18f * Mathf.Sin(c * 0.11f), c * 9f, 0f);
                    var r = q * p;
                    float persp = 1f / (1f + r.z * 0.08f);
                    return new Vector2(r.x * persp, r.y * persp);
                }
            }
        }

        static Matrix4x4 Face(Vector3 p, float spin = 0f) =>
            Matrix4x4.TRS(p, Quaternion.Euler(-90f, 0f, 0f) * Quaternion.Euler(0f, spin, 0f), Vector3.one);

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            GearDraw.BeginFrame();
            _booth.Frame(rx, lamp, club);
            float b = rx.beatS;
            _bass = Eased.Follow(_bass, rx.bassFast, 14f, 3.5f, dt);

            // The waveform (or, from a Trippin before 0.26, one made up from the spectrum).
            float[] w = s.wave != null && s.wave.Length == 64 ? s.wave : null;
            if (w == null || System.Array.TrueForAll(w, v => v == 0f))
            {
                w = new float[64];
                for (int i = 0; i < 64; i++)
                {
                    float x = i / 64f * Mathf.PI * 2f;
                    w[i] = rx.Spec(0.05f) * Mathf.Sin(x * 2f) + rx.Spec(0.3f) * 0.6f * Mathf.Sin(x * 5f + rx.clk) + rx.Spec(0.7f) * 0.3f * Mathf.Sin(x * 13f);
                }
            }
            float pk = 0f;
            foreach (var v in w) pk = Mathf.Max(pk, Mathf.Abs(v));
            _peak = pk > _peak ? Mathf.Lerp(_peak, pk, 1f - Mathf.Exp(-dt * 6f)) : Mathf.Lerp(_peak, pk, 1f - Mathf.Exp(-dt * 0.8f));
            float g = gainAmt * (0.85f + 0.35f * _bass) / Mathf.Max(_peak, 0.04f);

            int blk = Mathf.FloorToInt(b / 16f);
            int[] order = { 0, 1, 2, 0, 3, 1, 2, 3 };
            int form = order[((blk % order.Length) + order.Length) % order.Length];
            if (form != _form) { System.Array.Copy(_p, _p0, NP); _form = form; }
            float into = MixClock.Ease(Mathf.Repeat(b, 16f));
            for (int j = 0; j < NP; j++)
            {
                var want = Figure(form, j, w, g);
                want.x = Mathf.Clamp(want.x, -5.4f, 5.4f); want.y = Mathf.Clamp(want.y, -4.4f, 4.4f);
                _p[j] = Vector2.Lerp(_p0[j], want, into);
                if (into >= 1f) _p0[j] = want;
            }
            for (int j = 0; j < NP; j++)
            {
                float seg = j + 1 < NP ? (_p[j + 1] - _p[j]).magnitude : 0.1f;
                // A slow beam writes brighter; per frame, so scale by dt.
                float bright = Mathf.Clamp(0.06f / Mathf.Max(seg, 0.02f), 0.15f, 2.2f) * dt * 30f;
                _pts[j] = new Vector4(_p[j].x, _p[j].y, bright, 0f);
            }
            traceCs.SetVectorArray("_Pts", _pts);
            traceCs.SetInt("_N", NP);
            traceCs.SetFloat("_Decay", Mathf.Pow(Mathf.Lerp(0.02f, 0.55f, persist), dt * 30f));
            traceCs.SetFloat("_Core", 1f / (2f * 0.035f * 0.035f));
            traceCs.SetFloat("_Halo", 1f / (2f * 0.25f * 0.25f));
            traceCs.Dispatch(_k, TW / 8, TH / 8, 1);
            var phos = Color.Lerp(new Color(0.25f, 1f, 0.45f), Kit.Hue(b / 512f), colour);
            _face.SetColor("_Phos", phos);
            _face.SetFloat("_Gain", 0.55f * rx.Gain(0.9f, 1.15f));

            // The instrument: case, handle, feet, front panel, bezel and screen, controls.
            GearDraw.Draw(_caseM, _case, GearDraw.T(new Vector3(0f, 0.88f, 0f)));
            GearDraw.Draw(_handleM, _case, GearDraw.T(new Vector3(-1.72f, 1.2f, 0f)));
            GearDraw.Draw(_handleM, _case, GearDraw.T(new Vector3(1.72f, 1.2f, 0f)));
            foreach (var fx in new[] { -1.3f, 1.3f }) foreach (var fz in new[] { -1.7f, 1.7f })
                GearDraw.Draw(_footM, _m.rubber, GearDraw.T(new Vector3(fx, 0.04f, fz)));
            GearDraw.Draw(_panelM, _panel, GearDraw.T(new Vector3(0f, 0.88f, -2.21f)));
            GearDraw.Draw(_bezelM, _bezel, GearDraw.T(new Vector3(Screen.x, Screen.y, -2.25f)));
            GearDraw.Draw(_screenM, _face, Face(Screen + new Vector3(0f, 0f, -0.085f)));
            // Volts/div follows the auto-gain; the timebase clicks round to a detent with each display.
            _vd = Mathf.Lerp(_vd, Mathf.Clamp01(Mathf.InverseLerp(0.04f, 0.5f, _peak)), 1f - Mathf.Exp(-dt * 3f));
            _tb = Mathf.Lerp(_tb, form * 30f - 45f, 1f - Mathf.Exp(-dt * 10f));
            float zf = -2.27f;
            void K(Mesh mesh, Material mat, float x, float y, float ang, float r)
            {
                var pr = GearDraw.Props; pr.SetFloat("_Spin", r);
                GearDraw.Draw(mesh, mat, Face(new Vector3(x, y, zf), ang), pr);
            }
            K(_knobL, _m.knob, 0.28f, 1.18f, -120f + 240f * _vd, 0.17f);
            K(_knobL, _m.knob, 0.9f, 1.18f, _tb, 0.17f);
            K(_knobS, _m.knobLight, 0.28f, 0.72f, 30f * Mathf.Sin(rx.phrase * 0.5f), 0.09f);
            K(_knobS, _m.knobLight, 0.9f, 0.72f, 20f, 0.09f);
            K(_knobS, _m.knob, 1.35f, 1.18f, -60f, 0.09f);
            K(_knobS, _m.knob, 1.35f, 0.72f, 45f, 0.09f);
            K(_knobS, _m.knob, -1.45f, 0.24f, 0f, 0.09f);
            K(_knobS, _m.knob, -1.2f, 0.24f, 90f, 0.09f);
            foreach (var bx in new[] { 0.28f, 0.9f, 1.35f })
                GearDraw.Draw(_bncM, _bnc, Face(new Vector3(bx, 0.32f, zf)));
            {
                var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(3f, 0.25f, 0.1f) * 0.35f);
                GearDraw.Draw(_ledM, _m.led, Face(new Vector3(-0.2f, 0.24f, zf)), pr);
            }

            // Shots: one per 8 bars, all on the screen, at a moderate stop (AGENTS: angle, focus, orientation).
            int sb = Mathf.FloorToInt(b / 32f);
            if (sb != _shot) { _shot = sb; rig.Snap(); }
            float uu = Mathf.Repeat(b, 32f) / 32f;
            float sw = Mathf.Sin(uu * Mathf.PI - Mathf.PI * 0.5f) * drift;
            Vector3 pos, look; float fstop = 5.6f;
            switch (((sb % 4) + 4) % 4)
            {
                default:
                case 0: look = Screen; pos = Screen + new Vector3(0.25f * sw, 0.15f, -2.6f); break;
                case 1: look = Screen + new Vector3(0.1f, 0f, 0f); pos = Screen + new Vector3(-1.6f, 0.5f, -2.4f + 0.2f * sw); fstop = 4f; break;
                case 2: look = new Vector3(0.1f, 0.85f, -2.2f); pos = new Vector3(2.6f + 0.4f * sw, 1.9f, -5.5f); fstop = 8f; break;
                case 3: look = Screen; pos = Screen + new Vector3(0.15f + 0.1f * sw, 0.02f, -1.5f); fstop = 4f; break;
            }
            rig.Move(cam, pos, look, dt, 3f);
            HqPost.Focus(Vector3.Distance(cam.transform.position, look), fstop, 50f);
        }
    }
}
