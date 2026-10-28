// unity_kinetic: a kinetic light installation: a 18 x 18 grid of glowing
// balls, each on its own winch cable from a dark ceiling, moving together
// as one surface over a hazy hall floor (GearKit + HqGear). Units are metres.
//  - Shape: the surface the balls make is the music. A new figure every 4
//    bars, morphing in over a beat: a spectrogram (the live spectrum across
//    the grid from the centre out, the rows behind it the spectrum a moment
//    ago), ripples from the centre, a tilted plane turning, a saddle, and a
//    dome the bass breathes. The bass sets every figure's depth; each kick
//    sends a ring through the grid (four slots, a new kick taking the
//    most-faded one).
//  - Motion: the ripples and saddle travel on the smooth energy clock, the
//    plane turns once every 8 bars on Trippin's beat, and the camera drifts
//    through four shots, cutting every 8 bars. Count fixed; a breakdown
//    flattens the surface and slows it, it never removes balls.
// Controls: depth, glow, haze, drift.
using UnityEngine;

namespace TrippinStage
{
    public sealed class KineticShow : KitShow
    {
        public Material gearMat;

        [TrippinParam("Depth", 0.2f, 2f)] public float depth = 1f;
        [TrippinParam("Glow", 0.2f, 2f)] public float glow = 1f;
        [TrippinParam("Haze", 0f, 2f)] public float hazeAmt = 1f;
        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;

        const int G = 18, Hist = 18, Lag = 3;
        const float Pitch = 0.7f, CeilY = 9f, MidY = 4.6f, Swing = 2.1f, BallR = 0.13f;

        GearMats _m;
        Material _ball, _cable, _ceil, _cap;
        Mesh _ballM, _unit, _ceilM, _capM;
        readonly float[] _y = new float[G * G], _y0 = new float[G * G];
        readonly float[][] _hist = new float[Hist * Lag][];
        readonly Vector4[] _rings = new Vector4[4];   // x: start time, y: strength
        readonly Vector4[] _lp = new Vector4[4], _lc = new Vector4[4];
        int _histHead, _form = -1, _shot = -1;
        float _t, _bass, _mid, _high, _kickPrev, _flat;

        protected override void Build()
        {
            Env(120f, 0.08f, true);
            _m = new GearMats(gearMat);
            _ball = _m.M(0.85f, 0.85f, 0.86f, 0f, 0.55f);
            _cable = _m.M(0.05f, 0.05f, 0.055f, 0.8f, 0.4f);
            _ceil = _m.M(0.02f, 0.02f, 0.022f, 0.3f, 0.6f);
            _cap = _m.M(0.1f, 0.1f, 0.11f, 1f, 0.3f);
            var prof = new Vector2[13];
            for (int i = 0; i <= 12; i++)
            {
                float a = i / 12f * Mathf.PI;
                prof[i] = new Vector2(Mathf.Sin(a) * BallR, -Mathf.Cos(a) * BallR);
            }
            _ballM = GearMesh.Lathe(prof, 20, "kinetic ball");
            _unit = GearMesh.RoundedBox(Vector3.one, 0.0f, 1, "cable");
            _ceilM = GearMesh.RoundedBox(new Vector3(G * Pitch + 1.6f, 0.3f, G * Pitch + 1.6f), 0.05f, 2, "ceiling");
            _capM = GearMesh.Lathe(new[] { new Vector2(0f, 0.06f), new Vector2(0.045f, 0.06f), new Vector2(0.045f, 0.06f), new Vector2(0.045f, 0f), new Vector2(0.045f, 0f), new Vector2(0f, 0f) }, 12, "winch");
            for (int i = 0; i < _hist.Length; i++) _hist[i] = new float[G / 2];
            for (int i = 0; i < _y.Length; i++) _y[i] = _y0[i] = 0f;
            for (int i = 0; i < 4; i++) _rings[i] = new Vector4(-99f, 0f, 0f, 0f);
        }

        /// The surface for figure k at grid cell (cx, cz) in -1..1 units, roughly -1..1.
        float Figure(int k, int ix, int iz, float b)
        {
            float x = (ix - (G - 1) * 0.5f) / ((G - 1) * 0.5f), z = (iz - (G - 1) * 0.5f) / ((G - 1) * 0.5f);
            float r = Mathf.Sqrt(x * x + z * z);
            float c = rx.clk;
            switch (k)
            {
                default:
                case 0:   // spectrogram: bass in the middle column, highs out to the sides; rows back in time
                {
                    int bin = Mathf.Min(G / 2 - 1, Mathf.Abs(ix - G / 2 + (ix < G / 2 ? 1 : 0)));
                    var row = _hist[((_histHead - iz * Lag) % _hist.Length + _hist.Length) % _hist.Length];
                    return row[bin] * 2.2f - 0.9f;
                }
                case 1:   // ripples out from the centre
                    return Mathf.Cos(r * 7f - c * 1.6f) * (0.55f + 0.45f * (1f - r * 0.5f));
                case 2:   // a tilted plane turning once every 8 bars
                {
                    float a = b / 32f * Mathf.PI * 2f;
                    return (x * Mathf.Cos(a) + z * Mathf.Sin(a)) * (0.7f + 0.4f * _mid);
                }
                case 3:   // a saddle travelling on the clock
                    return Mathf.Sin(x * 3.2f + c * 0.9f) * Mathf.Cos(z * 3.2f - c * 0.7f) * (0.6f + 0.5f * _high);
                case 4:   // a dome the bass breathes
                    return (0.9f - 1.6f * r * r) * (0.5f + 0.8f * _bass);
            }
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            GearDraw.BeginFrame();
            _t += dt;
            float b = rx.beatS;
            _bass = Eased.Follow(_bass, rx.bassFast, 14f, 3.5f, dt);
            _mid = Eased.Follow(_mid, rx.midFast, 14f, 3.5f, dt);
            _high = Eased.Follow(_high, rx.highFast, 14f, 3.5f, dt);
            _flat = Mathf.Lerp(_flat, rx.calm, 1f - Mathf.Exp(-dt * 1.2f));

            // Spectrum history for the spectrogram rows (one slot a frame).
            _histHead = (_histHead + 1) % _hist.Length;
            var cur = _hist[_histHead];
            for (int i = 0; i < cur.Length; i++) cur[i] = rx.Spec((i + 0.5f) / cur.Length);

            // Kick rings: the most-faded slot takes a new kick.
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && rx.calm < 0.5f;
            _kickPrev = rx.kick;
            if (kick)
            {
                int best = 0;
                for (int i = 1; i < 4; i++) if (_rings[i].x < _rings[best].x) best = i;
                _rings[best] = new Vector4(_t, 0.35f + 0.5f * rx.kick, 0f, 0f);
            }

            int blk = Mathf.FloorToInt(b / 16f);
            int[] order = { 0, 1, 2, 0, 3, 4, 0, 1, 3, 2 };
            int form = order[((blk % order.Length) + order.Length) % order.Length];
            if (form != _form)
            {
                System.Array.Copy(_y, _y0, _y.Length);
                _form = form;
            }
            float into = MixClock.Ease(Mathf.Repeat(b, 16f));
            float amp = depth * (0.55f + 0.6f * _bass) * Mathf.Lerp(1f, 0.45f, _flat);
            float gain = rx.Gain(0.85f, 1.2f) * glow;
            float hue0 = b / 512f;

            // Lights: a cool key from the side, two palette washes from below, a dim fill.
            var p1 = Kit.Hue(hue0); var p2 = Kit.Hue(hue0 + 0.4f);
            _lp[0] = new Vector4(-9f, 7f, -6f, 16f); _lc[0] = (Vector4)(new Color(0.6f, 0.68f, 0.85f) * 0.6f);
            _lp[1] = new Vector4(-5f, 0.5f, 5f, 12f); _lc[1] = (Vector4)(p1 * 1.2f * gain);
            _lp[2] = new Vector4(5f, 0.5f, 5f, 12f); _lc[2] = (Vector4)(p2 * 1.2f * gain);
            _lp[3] = new Vector4(0f, 3f, -10f, 12f); _lc[3] = (Vector4)(new Color(0.3f, 0.32f, 0.4f) * 0.3f);
            Shader.SetGlobalVectorArray("_GLPos", _lp);
            Shader.SetGlobalVectorArray("_GLCol", _lc);
            Shader.SetGlobalVector("_GEnvA", (Vector4)(new Color(0.5f, 0.55f, 0.7f) * 0.12f));
            Shader.SetGlobalVector("_GEnvB", new Vector4(p1.r * 0.25f, p1.g * 0.25f, p1.b * 0.25f, 0.5f));
            Shader.SetGlobalVector("_GEnvC", new Vector4(p2.r * 0.25f, p2.g * 0.25f, p2.b * 0.25f, -0.5f));

            GearDraw.Draw(_ceilM, _ceil, GearDraw.T(new Vector3(0f, CeilY + 0.15f, 0f)));
            for (int iz = 0; iz < G; iz++)
            for (int ix = 0; ix < G; ix++)
            {
                int i = iz * G + ix;
                float want = Figure(form, ix, iz, b);
                float f = Mathf.Lerp(_y0[i], want, into);
                if (into >= 1f) _y0[i] = want;
                _y[i] = f;
                float px = (ix - (G - 1) * 0.5f) * Pitch, pz = (iz - (G - 1) * 0.5f) * Pitch;
                float r = Mathf.Sqrt(px * px + pz * pz);
                float ring = 0f;
                for (int k = 0; k < 4; k++)
                {
                    float age = _t - _rings[k].x;
                    if (age < 0f || age > 3f) continue;
                    float front = age * 4.5f;                                  // m/s
                    float d = r - front;
                    ring += _rings[k].y * Mathf.Exp(-d * d * 1.6f) * Mathf.Exp(-age * 0.9f) * Mathf.SmoothStep(0f, 1f, age / 0.15f);
                }
                float y = MidY + Swing * Mathf.Clamp(f * amp + ring, -1.4f, 1.4f);
                var at = new Vector3(px, y, pz);
                float lvl = Mathf.InverseLerp(-1.2f, 1.2f, f * amp + ring);
                var col = Kit.Hue(hue0 + 0.25f * lvl + 0.05f * r);
                var pr = GearDraw.Props;
                pr.SetColor("_Emit", col * (0.35f + 1.1f * lvl) * gain);
                GearDraw.Draw(_ballM, _ball, GearDraw.T(at), pr);
                float len = CeilY - y - BallR;
                GearDraw.Draw(_unit, _cable, Matrix4x4.TRS(new Vector3(px, y + BallR + len * 0.5f, pz), Quaternion.identity, new Vector3(0.008f, len, 0.008f)));
                GearDraw.Draw(_capM, _cap, GearDraw.T(new Vector3(px, CeilY - 0.06f, pz)));
            }
            hazeScale = hazeAmt;

            // Shots: one per 8 bars, the camera placed outside the grid (AGENTS: nothing near the lens).
            int sb = Mathf.FloorToInt(b / 32f);
            if (sb != _shot) { _shot = sb; rig.Snap(); }
            float u = Mathf.Repeat(b, 32f) / 32f;
            float sw = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * drift;
            Vector3 pos, look;
            switch (((sb % 4) + 4) % 4)
            {
                default:
                case 0: pos = new Vector3(3f * sw, 2.4f, -12f); look = new Vector3(0f, MidY, 0f); break;
                case 1: pos = new Vector3(-11f, 1.8f, -10f + 2f * sw); look = new Vector3(0f, MidY + 0.6f, 0f); break;
                case 2: pos = new Vector3(13f, 8.5f, -7f + 2f * sw); look = new Vector3(0f, MidY - 0.4f, 0f); break;
                case 3: pos = new Vector3(2f * sw, 1.2f, -9f); look = new Vector3(0f, MidY + 2.5f, 2f); break;
            }
            rig.Move(cam, pos, look, dt, 3f);
            HqPost.Focus(Vector3.Distance(cam.transform.position, look), rx.calm > 0.5f ? 4f : 5.6f, 50f);
        }
    }
}
