// unity_knot: a torus knot of light with a comet head racing round it; the
// knot changes to a different (p, q) winding every 8 bars, morphing over two
// beats.
//  - Shape: the tube radius swells with the eased bass and the kick, the
//    loop radius with the mids, and the highs ripple along the curve.
//  - Motion: the comet runs round the knot on the smooth energy clock, the
//    whole knot spins and swings direction with the phrase.
//  - Luminance: the tail glows with the music's loudness; the comet head is
//    brightest.
//  - Drops: a build draws the knot tight and dim; the drop expands it.
using UnityEngine;

namespace TrippinStage
{
    public sealed class KnotShow : KitShow
    {
        const int Segs = 160;
        static readonly int[,] Pq = { { 2, 3 }, { 3, 2 }, { 3, 4 }, { 2, 5 }, { 4, 3 }, { 5, 2 } };
        BeamPool _beams;
        readonly Vector3[] _a = new Vector3[Segs + 1], _b = new Vector3[Segs + 1];
        int _cur = -1, _prev;
        float _start;

        protected override void Build()
        {
            Env();
            _beams = new BeamPool(transform, beamMat, Segs, "knot", true, 0.06f);
        }

        void Curve(Vector3[] pts, int p, int q, float spin)
        {
            float R = 6f * (1f + 0.2f * rx.midFast) * (1f - 0.35f * rx.tension) * (1f + 0.4f * rx.impact);
            float r = 2.6f * (1f + 0.5f * rx.bassFast + 0.2f * rx.kick);
            for (int i = 0; i <= Segs; i++)
            {
                float u = i / (float)Segs * Mathf.PI * 2f;
                float rip = 0.35f * rx.highFast * Mathf.Sin(u * 19f + rx.clkHigh * 0.4f);
                float k = R + r * Mathf.Cos(q * u) + rip;
                float x = k * Mathf.Cos(p * u + spin), y = k * Mathf.Sin(p * u + spin);
                pts[i] = new Vector3(x, 10f + r * Mathf.Sin(q * u) * 1.2f, y);
            }
        }

        protected override void Frame(ShowState s, float dt)
        {
            int f = (Mathf.FloorToInt(rx.beat / 4f) / 8) % Pq.GetLength(0);
            if (_cur < 0) { _cur = _prev = f; _start = rx.beat - 4f; }
            else if (f != _cur) { _prev = _cur; _cur = f; _start = rx.beat; }
            float m = Mathf.SmoothStep(0f, 1f, (rx.beat - _start) / 2f);
            float spin = rx.clk * 0.03f + 0.8f * Mathf.Sin(rx.phrase);
            Curve(_a, Pq[_prev, 0], Pq[_prev, 1], spin);
            Curve(_b, Pq[_cur, 0], Pq[_cur, 1], spin);
            float gain = rx.Gain();
            float head = rx.clk * 0.02f - Mathf.Floor(rx.clk * 0.02f);
            for (int i = 0; i < Segs; i++)
            {
                Vector3 p0 = Vector3.Lerp(_a[i], _b[i], m), p1 = Vector3.Lerp(_a[i + 1], _b[i + 1], m);
                float t = (i + 0.5f) / Segs;
                float d = head - t; d -= Mathf.Floor(d);        // 0 at the head, 1 at the end of the tail
                float glow = Mathf.Pow(1f - d, 2.5f);
                _beams.Set(i, p0, p1, Kit.Hue(0.2f + t * 0.9f), gain * (0.3f + 1.5f * glow));
            }
            rig.Orbit(cam, rx, 30f, 12f, 10f, dt);
        }
    }
}
