// unity_pendulum: a harmonograph. Three coupled pendulums swing over a dark
// floor and each draws a long, tapering ribbon of light behind its pen point;
// the traces weave into slowly turning Lissajous knots. (The old pendulum-wave
// row of bobs was too thin; this keeps the pendulum idea but draws with it.)
//  - Shape: the swing amplitude of each axis follows the eased level of its
//    own band (bass, mids, mid-highs), so the figure swells and flattens with
//    the music; the ribbon's thickness follows the kick and its ripple the
//    highs.
//  - Motion: time runs on the smooth energy clock, so the pens speed up with
//    the track and surge on a drop; the frequency ratios change every 8 bars,
//    morphing over four beats, and the figure turns with the phrase.
//  - Luminance: each ribbon is as bright as the music is loud and fades along
//    its tail; the pen heads are brightest.
//  - Drops: a build tightens the figure dim; the drop throws it wide (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class PendulumShow : KitShow
    {
        const int Traces = 3, Pts = 260, Rods = 6;
        // Frequency ratios (x, y, z) per figure; the three traces share a figure with phase offsets.
        static readonly float[,] Ratios =
        {
            { 2f, 3f, 5f }, { 3f, 4f, 5f }, { 3f, 5f, 7f }, { 2f, 5f, 3f }, { 4f, 5f, 3f }, { 5f, 6f, 4f },
        };
        TubeRibbon[] _t = new TubeRibbon[Traces];
        GlowPool _heads;
        BeamPool _rods;
        int _cur = -1, _prev;
        float _start;

        protected override void Build()
        {
            Env();
            for (int i = 0; i < Traces; i++)
            {
                _t[i] = new TubeRibbon(transform, ribbonMat, Pts, 8, "trace " + i, true);
                _t[i].Mat.SetFloat("_PulseFreq", 4f);
                _t[i].Mat.SetFloat("_PulseAmt", 0.35f);
                _t[i].Mat.SetFloat("_Core", 5f);
            }
            _heads = new GlowPool(transform, glowMat, Traces, "pen", true);
            _rods = new BeamPool(transform, beamMat, Rods, "rod", false, 0.04f, 0f, 30f, 0.2f, 0.98f, 0.3f);
        }

        Vector3 Pos(float T, float m, float ph, float ax, float ay, float az)
        {
            // Frequencies morph between the previous and the current figure while the phase stays continuous.
            float fx = Mathf.Lerp(Ratios[_prev, 0], Ratios[_cur, 0], m);
            float fy = Mathf.Lerp(Ratios[_prev, 1], Ratios[_cur, 1], m);
            float fz = Mathf.Lerp(Ratios[_prev, 2], Ratios[_cur, 2], m);
            return new Vector3(ax * Mathf.Sin(fx * T + ph), ay * Mathf.Sin(fy * T + ph * 1.7f + 1.1f), az * Mathf.Sin(fz * T + ph * 0.6f));
        }

        protected override void Frame(ShowState s, float dt)
        {
            int f = (Mathf.FloorToInt(rx.beat / 4f) / 8) % Ratios.GetLength(0);
            if (_cur < 0) { _cur = _prev = f; _start = rx.beat - 8f; }
            else if (f != _cur) { _prev = _cur; _cur = f; _start = rx.beat; }
            float m = Mathf.SmoothStep(0f, 1f, (rx.beat - _start) / 4f);
            float gain = rx.Gain();
            // 0.45 swung three thick ribbons out so fast the drop read as a lurch (M2:
            // frame change ~55 for 0.3 s vs a 2.4 median); 0.25 keeps the burst.
            float shrink = (1f - 0.4f * rx.tension) * (1f + 0.25f * rx.impact);
            float ax = (7f + 5f * rx.bassFast) * shrink, ay = (6f + 5f * rx.midFast) * shrink, az = (7f + 4f * rx.mhFast) * shrink;
            float T = rx.clk * 0.12f;
            Quaternion turn = Quaternion.AngleAxis((rx.clk * 0.4f + 40f * Mathf.Sin(rx.phrase)) * 1f, Vector3.up);
            var centre = new Vector3(0f, 12f, 0f);
            Quaternion face = cam.transform.rotation;
            for (int tr = 0; tr < Traces; tr++)
            {
                var t = _t[tr];
                float ph = tr * 2.1f;
                for (int i = 0; i < Pts; i++)
                {
                    float u = i / (Pts - 1f);          // 0 = tail, 1 = pen
                    float tt = T - (1f - u) * 5.2f;
                    Vector3 p = Pos(tt, m, ph, ax, ay, az);
                    p += new Vector3(0f, 0.5f * rx.highFast * Mathf.Sin(u * 40f + rx.clkHigh * 0.5f), 0f);
                    t.P[i] = centre + turn * p;
                    float taper = Mathf.Pow(u, 0.8f);
                    t.R[i] = 0.05f + 0.36f * taper * (1f + 0.7f * rx.kick);
                    t.K[i] = (0.15f + 1.5f * taper * taper) * (0.7f + 0.6f * rx.Spec(u * 0.8f + tr * 0.06f));
                    t.Hu[i] = tr * 0.28f + u * 0.3f;
                }
                t.Mat.SetFloat("_Phase", -rx.clk * 0.3f);
                t.Mat.SetFloat("_Hue", 0.1f);
                t.Apply(gain);
                Vector3 head = t.P[Pts - 1];
                _heads.Set(tr, head, 1.6f + 1.4f * rx.kick, tr * 0.28f + 0.3f, gain * (0.9f + 0.9f * rx.lum), face);
                // Two thin suspension rods from the pen to a pivot overhead (the pendulum).
                Vector3 pivA = new Vector3(-8f + tr * 8f, 34f, -6f), pivB = new Vector3(8f - tr * 8f, 34f, 6f);
                _rods.Set(tr * 2, pivA, head, Kit.Hue(0.1f + tr * 0.28f), gain * 0.18f);
                _rods.Set(tr * 2 + 1, pivB, head, Kit.Hue(0.1f + tr * 0.28f), gain * 0.18f);
            }
            rig.Orbit(cam, rx, 38f, 12f, 12f, dt, 0.9f);
        }
    }
}
