// unity_ribbons: sixteen ribbons of light weaving round each other through
// the dark, like long-exposure light painting.
//  - Shape: each ribbon's sweep is its own slice of the eased spectrum (bass
//    ribbons swing wide on the heavy parts, treble ones on the hats); every
//    kick sends a bright knot running down all the ribbons.
//  - Motion: the heads fly on the energy clock, the tails following their
//    path; the weave turns the other way each phrase.
//  - Breakdowns: the ribbons slow and drift apart; the drop draws them in.
using UnityEngine;

namespace TrippinStage
{
    public sealed class RibbonsShow : KitShow
    {
        const int R = 16, L = 110;
        [TrippinParam("Size", 0.5f, 2f)] public float scale = 1f;
        [TrippinParam("Weave", 0f, 3f)] public float weaveAmt = 1f;
        [TrippinParam("Kick knot", 0f, 3f)] public float knotAmt = 1f;
        [TrippinParam("Trail length", 0.3f, 2f)] public float trail = 1f;

        GlowPool _pts;
        float _knot = 99f, _kickPrev;

        protected override void Build()
        {
            _pts = new GlowPool(transform, glowMat, R * L, "ribbon", false);
        }

        static Vector3 Path(int r, float t, float amp)
        {
            float a = 1f + 0.13f * r, b = 1.3f + 0.07f * r, c = 0.7f + 0.11f * r;
            float ph = r * 0.7f;
            return new Vector3(Mathf.Sin(a * t + ph) * amp * 1.6f, Mathf.Sin(b * t + ph * 1.3f) * amp * 0.8f + 4f, Mathf.Sin(c * t + ph * 0.5f) * amp);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.8f, 2.0f);
            Quaternion face = cam.transform.rotation;
            if (rx.kick > 0.55f && _kickPrev <= 0.55f) _knot = 0f;
            _kickPrev = rx.kick;
            _knot += dt * 2.2f;
            float t = rx.clk * 0.05f;
            Quaternion weave = Quaternion.Euler(0f, 25f * weaveAmt * Mathf.Sin(rx.phrase * 0.5f) + t * 10f, 0f);
            float spread = 1f + 0.4f * rx.calm - 0.2f * rx.tension + 0.25f * rx.impact;
            for (int r = 0; r < R; r++)
            {
                float amp = (6f + 5f * rx.Spec((r + 0.5f) / R)) * spread * scale;
                float hue = (float)r / R * 0.6f + 0.05f * Mathf.Sin(t);
                for (int j = 0; j < L; j++)
                {
                    float lag = j * 0.022f * trail;
                    Vector3 p = weave * Path(r, t - lag, amp);
                    float tail = 1f - (float)j / L;
                    float kd = (j / (float)L - _knot) / 0.06f;
                    float knot = Mathf.Exp(-kd * kd) * knotAmt;
                    float size = (0.75f * tail + 0.18f) * (1f + 1.4f * knot);
                    _pts.Set(r * L + j, p, size, hue, gain * 1.6f * (0.25f + 0.9f * tail) * (1f + 1.5f * knot), face);
                }
            }
            rig.Orbit(cam, rx, 19f, 6f, 4f, dt, 1f, 0.15f);
        }
    }
}
