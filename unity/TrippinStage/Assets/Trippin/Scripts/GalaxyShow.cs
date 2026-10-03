// unity_galaxy: a spiral galaxy of 700 stars with a glowing core, seen from
// above at an angle, turning slowly.
//  - Shape: the spiral's winding follows the slow bass (the arms wrap
//    tighter on the heavy parts), the core swells with the eased bass, and
//    the stars swell with the eased highs.
//  - Motion: differential rotation (the inner stars turn faster) on the
//    smooth energy clock; the camera tilt and the turn swing with the phrase.
//  - Luminance: stars and core follow the music's loudness; each star
//    twinkles slowly (a slow sine, never a flash).
//  - Drops: a build draws the arms in dim; the drop sends the galaxy open.
using UnityEngine;

namespace TrippinStage
{
    public sealed class GalaxyShow : KitShow
    {
        const int N = 700;
        GlowPool _stars, _core;

        protected override void Build()
        {
            _stars = new GlowPool(transform, glowMat, N, "star", false);
            _core = new GlowPool(transform, glowMat, 2, "core", false);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.5f, 1.35f);
            Quaternion face = cam.transform.rotation;
            float tight = 0.18f + 0.1f * rx.bassSlow;
            float open = (1f - 0.3f * rx.tension) * (1f + 0.35f * rx.impact);
            float tw = rx.clkHigh * 0.3f;
            for (int i = 0; i < N; i++)
            {
                int arm = i % 2;
                float r = 1f + 22f * Mathf.Sqrt(Kit.H(i, 1));
                float th = arm * Mathf.PI + r * tight + rx.clk * 0.006f * (8f / (r + 4f)) + 0.15f * Mathf.Sin(rx.phrase);
                float scatter = (Kit.H(i, 2) - 0.5f) * (2.2f + r * 0.12f);
                float px = Mathf.Cos(th) * r + Mathf.Cos(th + 1.5708f) * scatter;
                float pz = Mathf.Sin(th) * r + Mathf.Sin(th + 1.5708f) * scatter;
                float py = (Kit.H(i, 3) - 0.5f) * 1.5f * (1f - r / 26f);
                float twinkle = 0.85f + 0.15f * Mathf.Sin(tw + i);
                float size = (0.4f + Kit.H(i, 4) * 0.9f) * (0.8f + 0.6f * rx.highFast);
                _stars.Set(i, new Vector3(px, py, pz) * open, size, 0.05f + r * 0.02f, gain * (0.3f + 0.7f * Kit.H(i, 5)) * twinkle, face);
            }
            _core.Set(0, Vector3.zero, 9f * (1f + 0.5f * rx.bassFast), 0.08f, gain * 1.3f, face);
            _core.Set(1, Vector3.zero, 4.5f * (1f + 0.4f * rx.bassFast), 0.0f, gain * 1.6f, face);
            float tilt = 18f + 6f * Mathf.Sin(rx.phrase * 0.5f);
            rig.Orbit(cam, rx, 34f, tilt, 0f, dt, 1f, 0.15f);
        }
    }
}
