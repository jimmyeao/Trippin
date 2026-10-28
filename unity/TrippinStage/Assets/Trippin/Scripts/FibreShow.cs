// unity_fibres: a dark field of 70,000 fibre-optic strands, a lawn of light
// swaying in the wind, each strand's tip glowing (HqFibre.shader, drawn
// procedurally). Units are metres.
//  - Shape: the field bends with the music. The bass sends a gust rolling
//    across it (four slots, a strong bass hit or a kick launching one into
//    the most-faded slot), each kick pushes a ring out from the middle, the
//    strands always swaying a little; the tips glow with the spectrum, bass
//    in the middle of the field and highs out at the sides, and brighten
//    where a gust or ring passes.
//  - Motion: the gusts roll at a steady speed, their direction swinging
//    with the phrase; the sway runs on the smooth energy clock; the camera
//    drifts through four shots (low across the field, high above it to show
//    the gusts, from the side, three-quarter), cutting every 8 bars. No
//    depth of field: the strands are additive and write no depth.
// Controls: wind, glow, strand width, drift.
using UnityEngine;

namespace TrippinStage
{
    public sealed class FibreShow : KitShow
    {
        public Material fibreMat;

        [TrippinParam("Wind", 0f, 2f)] public float wind = 1f;
        [TrippinParam("Glow", 0.2f, 2f)] public float glow = 1f;
        [TrippinParam("Strand width", 0.5f, 2f)] public float width = 1f;
        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;

        const int Cols = 320, Rows = 220, N = Cols * Rows, VPS = 6 * 6 + 6;
        const float HalfW = 20f, Near = -6f, Far = 34f;

        Material _m;
        RenderParams _rp;
        readonly Vector4[] _gust = new Vector4[4], _ring = new Vector4[4];
        readonly Vector4[] _gustOut = new Vector4[4], _ringOut = new Vector4[4];
        readonly ClkAcc _clk = new ClkAcc();
        int _shot = -1;
        float _t, _kickPrev, _bassPrev, _lastGust = -99f, _imp;

        protected override void Build()
        {
            Env(90f, 0.05f, true);
            _m = new Material(fibreMat);
            _m.SetVector("_Field", new Vector4(HalfW, Near, Far, 0f));
            _m.SetFloat("_Cols", Cols); _m.SetFloat("_Rows", Rows);
            _rp = new RenderParams(_m) { worldBounds = new Bounds(new Vector3(0f, 1f, 14f), new Vector3(48f, 6f, 48f)) };
            for (int i = 0; i < 4; i++) { _gust[i] = new Vector4(-99f, 0f, 0f, 0f); _ring[i] = new Vector4(-99f, 0f, 0f, 0f); }
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            _t += dt;
            float b = rx.beatS;
            float calm = rx.calm;
            _imp = rx.impact > _imp ? Mathf.Lerp(_imp, rx.impact, 1f - Mathf.Exp(-dt * 4f)) : rx.impact;

            // Gusts: a kick or a bass swell launches one (at most every half beat), the drop a big one.
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && calm < 0.5f;
            _kickPrev = rx.kick;
            bool swell = rx.bassFast > 0.6f && _bassPrev <= 0.6f;
            _bassPrev = rx.bassFast;
            float spb = 60f / Mathf.Max(s.bpm, 60f);
            if ((kick || swell || rx.impact > 0.9f && _imp < 0.5f) && _t - _lastGust > spb * 0.5f && wind > 0f)
            {
                _lastGust = _t;
                int best = 0;
                for (int i = 1; i < 4; i++) if (_gust[i].x < _gust[best].x) best = i;
                _gust[best] = new Vector4(_t, (0.35f + 0.45f * rx.bassFast + 0.5f * _imp) * wind, 2.2f + 1.5f * rx.bassFast, 0f);
                if (kick)
                {
                    best = 0;
                    for (int i = 1; i < 4; i++) if (_ring[i].x < _ring[best].x) best = i;
                    _ring[best] = new Vector4(_t, (0.25f + 0.35f * rx.kick) * wind, 0f, 0f);
                }
            }
            // Gusts roll across at 7 m/s from the far side of the field along the wind direction.
            float ang = Mathf.PI * 0.5f + 0.7f * Mathf.Sin(rx.phrase * 0.5f);
            var dir = new Vector2(Mathf.Cos(ang), -Mathf.Sin(ang));
            float start = -45f;
            for (int i = 0; i < 4; i++)
            {
                float age = _t - _gust[i].x;
                _gustOut[i] = age < 0f || age > 13f ? new Vector4(-999f, 0f, 1f, 0f)
                    : new Vector4(start + age * 7f, _gust[i].y * Mathf.SmoothStep(0f, 1f, age / 0.3f) * Mathf.Exp(-age * 0.15f), _gust[i].z, 0f);
                float ra = _t - _ring[i].x;
                _ringOut[i] = ra < 0f || ra > 3f ? Vector4.zero
                    : new Vector4(ra * 6f, _ring[i].y * Mathf.Exp(-ra * 1.1f) * Mathf.SmoothStep(0f, 1f, ra / 0.12f), 0f, 0f);
            }
            _m.SetVector("_GustDir", new Vector4(dir.x, dir.y, 0f, 0f));
            _m.SetVectorArray("_Gust", _gustOut);
            _m.SetVectorArray("_Ring", _ringOut);
            _m.SetFloat("_T", _clk.Step(rx.clk, 0.9f) * (1f - 0.4f * calm));
            _m.SetFloat("_Hue", b / 512f);
            _m.SetFloat("_Width", 0.0035f * width);
            _m.SetFloat("_Tip", 0.03f * Mathf.Sqrt(width));
            _m.SetFloat("_Gain", 1.8f * glow * rx.Gain(0.85f, 1.2f));
            Graphics.RenderPrimitives(_rp, MeshTopology.Triangles, N * VPS);
            hazeScale = 0.6f;

            int sb = Mathf.FloorToInt(b / 32f);
            if (sb != _shot) { _shot = sb; rig.Snap(); }
            float u = Mathf.Repeat(b, 32f) / 32f;
            float sw = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * drift;
            Vector3 pos, look;
            switch (((sb % 4) + 4) % 4)
            {
                default:
                case 0: pos = new Vector3(2f * sw, 2.6f, -7f); look = new Vector3(0f, 0f, 6f); break;
                case 1: pos = new Vector3(2f * sw, 9f, -6f); look = new Vector3(0f, 0f, 8f); break;
                case 2: pos = new Vector3(-12f, 3.5f, 4f + 2f * sw); look = new Vector3(0f, 0f, 10f); break;
                case 3: pos = new Vector3(7f + sw, 3.2f, -7f); look = new Vector3(-1f, 0f, 7f); break;
            }
            rig.Move(cam, pos, look, dt, 3f);
            // No depth of field (AGENTS): additive strands write no depth.
        }
    }
}
