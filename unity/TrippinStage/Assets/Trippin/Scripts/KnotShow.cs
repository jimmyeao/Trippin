// unity_knot: a thick torus knot of light with a comet racing round it, a
// thin counter-rotating ghost knot, and beads streaming along the curve over
// a reflective floor. The winding (p, q) changes every 8 bars, morphing over
// two beats.
//  - Shape: the knot is a live spectrum along its length (tube radius and
//    brightness follow the band at each point), the tube swells with the
//    eased bass and kick, the loop radius with the mids, and the highs ripple
//    along the curve.
//  - Motion: the comet runs round the knot on the smooth energy clock, the
//    beads stream at speeds that follow it, and the whole knot spins and
//    swings direction with the phrase.
//  - Luminance: the tail glows with the music's loudness; the comet head is
//    brightest.
//  - Drops: a build draws the knot tight and dim; the drop expands it (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class KnotShow : KitShow
    {
        const int Segs = 200, Beads = 60;
        static readonly int[,] Pq = { { 2, 3 }, { 3, 2 }, { 3, 4 }, { 2, 5 }, { 4, 3 }, { 5, 2 } };
        TubeRibbon _main, _ghost;
        GlowPool _beads;
        readonly Vector3[] _a = new Vector3[Segs + 1], _b = new Vector3[Segs + 1];
        readonly Vector3[] _ga = new Vector3[Segs + 1], _gb = new Vector3[Segs + 1];
        int _cur = -1, _prev;
        float _start;

        protected override void Build()
        {
            Env();
            _main = new TubeRibbon(transform, ribbonMat, Segs + 1, 10, "knot", true);
            _main.Mat.SetFloat("_PulseFreq", 1f);
            _main.Mat.SetFloat("_PulseSharp", 6f);
            _main.Mat.SetFloat("_PulseAmt", 0.8f);
            _ghost = new TubeRibbon(transform, ribbonMat, Segs + 1, 6, "knot ghost", true);
            _ghost.Mat.SetFloat("_PulseAmt", 0.2f);
            _ghost.Mat.SetFloat("_PulseFreq", 3f);
            _beads = new GlowPool(transform, glowMat, Beads, "bead", true);
        }

        void Curve(Vector3[] pts, int p, int q, float spin, float scale, float tube)
        {
            float R = 6.5f * scale * (1f + 0.2f * rx.midFast) * (1f - 0.35f * rx.tension) * (1f + 0.4f * rx.impact);
            float r = tube * (1f + 0.5f * rx.bassFast + 0.2f * rx.kick);
            for (int i = 0; i <= Segs; i++)
            {
                float u = i / (float)Segs * Mathf.PI * 2f;
                float rip = 0.3f * rx.highFast * Mathf.Sin(u * 19f + rx.clkHigh * 0.4f);
                float k = R + r * Mathf.Cos(q * u) + rip;
                pts[i] = new Vector3(k * Mathf.Cos(p * u + spin), 10f + r * Mathf.Sin(q * u) * 1.2f, k * Mathf.Sin(p * u + spin));
            }
        }

        protected override void Frame(ShowState s, float dt)
        {
            int f = (Mathf.FloorToInt(rx.beat / 4f) / 8) % Pq.GetLength(0);
            if (_cur < 0) { _cur = _prev = f; _start = rx.beat - 4f; }
            else if (f != _cur) { _prev = _cur; _cur = f; _start = rx.beat; }
            float m = Mathf.SmoothStep(0f, 1f, (rx.beat - _start) / 2f);
            float spin = rx.clk * 0.03f + 0.8f * Mathf.Sin(rx.phrase);
            Curve(_a, Pq[_prev, 0], Pq[_prev, 1], spin, 1f, 2.8f);
            Curve(_b, Pq[_cur, 0], Pq[_cur, 1], spin, 1f, 2.8f);
            int gp = Pq.GetLength(0);
            Curve(_ga, Pq[(_prev + 1) % gp, 0], Pq[(_prev + 1) % gp, 1], -spin * 0.7f, 1.35f, 3.4f);
            Curve(_gb, Pq[(_cur + 1) % gp, 0], Pq[(_cur + 1) % gp, 1], -spin * 0.7f, 1.35f, 3.4f);
            float gain = rx.Gain();
            float head = rx.clk * 0.02f - Mathf.Floor(rx.clk * 0.02f);
            for (int i = 0; i <= Segs; i++)
            {
                float t = i / (float)Segs;
                float sym = Mathf.Abs(Mathf.Sin(t * Mathf.PI * 3f));
                float lvl = rx.Spec(sym);
                _main.P[i] = Vector3.Lerp(_a[i], _b[i], m);
                _main.R[i] = 0.26f * (1f + 1.1f * lvl + 0.5f * rx.kick);
                float d = head - t; d -= Mathf.Floor(d);
                float comet = Mathf.Pow(1f - d, 3f);
                _main.K[i] = 0.3f + 0.8f * lvl + 1.1f * comet;
                _main.Hu[i] = 0.1f + t * 0.8f;
                _ghost.P[i] = Vector3.Lerp(_ga[i], _gb[i], m);
                _ghost.R[i] = 0.12f * (1f + 0.8f * rx.Spec(1f - sym));
                _ghost.K[i] = 0.25f + 0.5f * rx.Spec(1f - sym);
                _ghost.Hu[i] = 0.55f + t * 0.4f;
            }
            _main.Mat.SetFloat("_Phase", -rx.clk * 0.12f);
            _main.Mat.SetFloat("_Hue", 0.0f);
            _ghost.Mat.SetFloat("_Phase", rx.clk * 0.1f);
            _ghost.Mat.SetFloat("_Hue", 0.15f);
            _main.Apply(gain);
            _ghost.Apply(gain * 0.7f);

            Quaternion face = cam.transform.rotation;
            for (int i = 0; i < Beads; i++)
            {
                float sp = 0.012f + 0.02f * Kit.H(i, 1);
                float tt = Mathf.Repeat(Kit.H(i, 2) + rx.clk * sp * (Kit.H(i, 3) > 0.5f ? 1f : -1f), 1f);
                float fi = tt * Segs;
                int i0 = Mathf.Min((int)fi, Segs - 1);
                Vector3 pos = Vector3.Lerp(_main.P[i0], _main.P[i0 + 1], fi - i0);
                float lvl = rx.Spec(Mathf.Abs(Mathf.Sin(tt * Mathf.PI * 3f)));
                _beads.Set(i, pos, 0.5f + 0.9f * Kit.H(i, 4) + 1.2f * lvl, 0.1f + tt * 0.8f, gain * (0.35f + 0.9f * lvl), face);
            }
            rig.Orbit(cam, rx, 26f, 12f, 10f, dt);
        }
    }
}
