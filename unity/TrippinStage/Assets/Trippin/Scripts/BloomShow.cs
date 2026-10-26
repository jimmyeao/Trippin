// unity_bloom: a flower of light, three layers of glowing petals turning
// against each other, opening and closing with the music.
//  - Shape: the petals open with the slow bass and spread on the drop; each
//    kick sends a pulse of light out from the centre along every petal; the
//    highs make the petal tips sparkle.
//  - Motion: the layers turn on the energy clock in opposite directions,
//    and their spin swings round with each phrase.
//  - Breakdowns: the flower closes into a slow-turning bud.
using UnityEngine;

namespace TrippinStage
{
    public sealed class BloomShow : KitShow
    {
        const int Layers = 3, Petals = 10, K = 48;
        GlowPool _pts;
        float _pulse = 99f, _kickPrev;

        protected override void Build()
        {
            _pts = new GlowPool(transform, glowMat, Layers * Petals * K, "petal", false);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.8f, 2.0f);
            Quaternion face = cam.transform.rotation;
            if (rx.kick > 0.55f && _kickPrev <= 0.55f) _pulse = 0f;
            _kickPrev = rx.kick;
            _pulse += dt * 1.8f;
            float open = Mathf.Clamp01(0.35f + 0.45f * rx.bassSlow + 0.2f * rx.impact - 0.3f * rx.calm);
            float dir = Mathf.Sin(rx.phrase * 0.5f);
            int n = 0;
            for (int ly = 0; ly < Layers; ly++)
            {
                float spin = (ly % 2 == 0 ? 1f : -1f) * (rx.clk * 0.02f * (1f + ly * 0.3f)) + 0.4f * dir * (ly + 1);
                float len = (5f + ly * 3.2f) * (0.7f + 0.5f * open);
                float lift = ly * 0.8f;
                for (int p = 0; p < Petals; p++)
                {
                    float a0 = spin + p * Mathf.PI * 2f / Petals + ly * 0.31f;
                    float width = (0.18f + 0.22f * open) * (1f - ly * 0.15f);
                    for (int k = 0; k < K; k++)
                    {
                        // Up one edge of the petal and back down the other.
                        float u = (float)k / (K - 1);
                        float side = u < 0.5f ? 1f : -1f;
                        float v = u < 0.5f ? u * 2f : (1f - u) * 2f;
                        float r = len * v;
                        float a = a0 + side * width * Mathf.Sin(Mathf.PI * v);
                        float y = lift + (1f - open) * r * 0.55f;
                        var pos = new Vector3(Mathf.Cos(a) * r, y, Mathf.Sin(a) * r);
                        float pd = (v - _pulse) / 0.09f;
                        float pulse = Mathf.Exp(-pd * pd);
                        float tip = Mathf.Pow(v, 6f) * rx.highFast;
                        float size = (0.45f + 0.2f * (1f - v)) * (1f + 1.2f * pulse + 1.5f * tip);
                        float hue = ly * 0.18f + v * 0.25f;
                        _pts.Set(n++, pos, size, hue, gain * 1.7f * (0.5f + 0.5f * v) * (1f + 1.4f * pulse + tip), face);
                    }
                }
            }
            rig.Orbit(cam, rx, 15f, 20f + 6f * Mathf.Sin(rx.phrase * 0.25f), 0.4f, dt, 0.6f, 0.15f);
        }
    }
}
