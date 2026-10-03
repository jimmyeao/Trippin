// unity_gyroscope: five concentric rings of light turning about different
// axes, like a gyroscope.
//  - Shape: each ring's radius breathes with the eased level of its own band
//    (bass, mids, mid-highs, highs, then bass again) and ripples with the
//    spectrum round its circumference.
//  - Motion: each ring turns on one of the smooth band energy clocks, and
//    its tilt swings on a phrase-length sine, so they keep reversing.
//  - Luminance: each ring is as bright as its band is loud.
//  - Drops: a build draws the rings in dim; the drop throws them wide.
using UnityEngine;

namespace TrippinStage
{
    public sealed class GyroscopeShow : KitShow
    {
        const int Rings = 5, Segs = 64;
        BeamPool _beams;
        readonly Vector3[] _pt = new Vector3[Segs + 1];

        protected override void Build()
        {
            Env();
            _beams = new BeamPool(transform, beamMat, Rings * Segs, "gyro", false, 0.06f);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain();
            var centre = new Vector3(0f, 11f, 0f);
            float spread = (1f - 0.4f * rx.tension) * (1f + 0.5f * rx.impact);
            int n = 0;
            for (int k = 0; k < Rings; k++)
            {
                float band = rx.Band(k);
                float rad = (3.5f + 2.2f * k) * (1f + 0.28f * band) * spread;
                float c = Rx.Clock(s, 1 + k % 3);
                Quaternion q = Quaternion.AngleAxis((c * 0.02f * (k % 2 == 0 ? 1f : -1f) + 0.8f * Mathf.Sin(rx.phrase + k)) * Mathf.Rad2Deg, new Vector3(0.2f + 0.2f * k, 1f, 0.4f - 0.1f * k).normalized)
                              * Quaternion.AngleAxis((25f + 18f * k) * Mathf.Sin(rx.phrase * 0.5f + k * 1.3f), Vector3.right);
                for (int i = 0; i <= Segs; i++)
                {
                    float a = i / (float)Segs * Mathf.PI * 2f;
                    float rip = 1f + 0.18f * rx.Spec(Mathf.Abs(Mathf.Sin(a * 0.5f)));
                    _pt[i] = centre + q * new Vector3(Mathf.Cos(a) * rad * rip, Mathf.Sin(a) * rad * rip, 0f);
                }
                float lvl = 0.45f + 1.1f * band + 0.3f * rx.lum;
                for (int i = 0; i < Segs; i++)
                    _beams.Set(n++, _pt[i], _pt[i + 1], Kit.Hue(0.12f * k + i * 0.003f), gain * lvl);
            }
            rig.Orbit(cam, rx, 32f, 12f, 11f, dt);
        }
    }
}
