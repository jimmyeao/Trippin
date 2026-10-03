// unity_warp: a hyperspace flight. 320 streaks of light rush out of the
// distance past the camera.
// This is a flight scene: no beat-synced flashes (AGENTS.md) - luminance and
// shape follow eased levels only.
//  - Shape: streak length follows the eased loudness and bass (the warp
//    stretches on the loud parts), and the whole tunnel pulls tighter on a
//    build and bursts on a drop.
//  - Motion: the speed is the smooth energy clock (it surges on a drop); the
//    camera rolls and the flight line sways with phrase-length sines.
//  - Luminance: streaks glow with the music's loudness.
using UnityEngine;

namespace TrippinStage
{
    public sealed class WarpShow : KitShow
    {
        const int N = 320;
        const float Far = 140f;
        BeamPool _beams;

        protected override void Build()
        {
            _beams = new BeamPool(transform, beamMat, N, "streak", false, 0.04f, 0f, 30f, 0.2f, 0.8f, 0.3f);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.5f, 1.3f);
            float stretch = 3f + 22f * rx.lum + 10f * rx.bassFast + 14f * rx.impact;
            float cx = 3f * Mathf.Sin(rx.phrase), cy = 1.5f * Mathf.Sin(rx.phrase * 0.7f + 1f);
            float squeeze = 1f - 0.3f * rx.tension;
            for (int i = 0; i < N; i++)
            {
                float a = Kit.H(i, 1) * Mathf.PI * 2f;
                float r = (4f + Kit.H(i, 2) * Kit.H(i, 2) * 28f) * squeeze;
                float z = rx.clk * 0.05f * (0.6f + 0.8f * Kit.H(i, 4)) + Kit.H(i, 3);
                z -= Mathf.Floor(z);
                z = Far * (1f - z);
                float near = 1f - z / Far;
                float len = stretch * (0.3f + near) * (0.4f + Kit.H(i, 5));
                var p0 = new Vector3(Mathf.Cos(a) * r + cx, Mathf.Sin(a) * r + cy, z);
                var p1 = p0 + new Vector3(0f, 0f, len);
                _beams.Set(i, p0, p1, Kit.Hue(0.4f + 0.3f * Kit.H(i, 7)), gain * (0.3f + 0.9f * near) * (0.5f + Kit.H(i, 6)));
            }
            rig.Move(cam, new Vector3(0f, 0f, 0f), new Vector3(cx * 0.5f, cy * 0.5f, 50f), dt, 6f);
            cam.transform.rotation = cam.transform.rotation * Quaternion.AngleAxis(4f * Mathf.Sin(rx.phrase), Vector3.forward);
        }
    }
}
