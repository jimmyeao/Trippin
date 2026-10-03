// unity_tesseract: a rotating hypercube (a 4D cube projected into 3D) with a
// smaller one nested inside it, drawn as thin lasers.
//  - Shape: the 4D -> 3D projection distance follows the eased bass (the
//    cube folds through itself harder on the low end), and the whole figure
//    swells with the kick.
//  - Motion: six rotation planes turn at different rates on the smooth
//    energy clock, each swung by its own phrase-length sine so the tumble
//    keeps reversing.
//  - Luminance: edges nearer in the fourth dimension glow brighter, and the
//    whole figure follows the music's loudness.
//  - Drops: a build contracts it dim; the drop blows it outward (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class TesseractShow : KitShow
    {
        const int Edges = 32;
        BeamPool _beams;
        readonly float[] _a = new float[6];
        readonly float[] _v = new float[4];
        // The six rotation planes (axis pairs) and their rates (radians per clock beat).
        static readonly int[,] Plane = { { 0, 1 }, { 0, 2 }, { 1, 2 }, { 0, 3 }, { 1, 3 }, { 2, 3 } };
        static readonly float[] Rate = { 0.031f, 0.023f, 0.017f, 0.041f, 0.029f, 0.019f };

        protected override void Build()
        {
            Env();
            _beams = new BeamPool(transform, beamMat, Edges * 2, "tesseract", true, 0.05f);
        }

        // 4D vertex v (bit b = sign on axis b) rotated and projected to 3D.
        Vector4 Project(int v, float scale, float d, out float w)
        {
            for (int b = 0; b < 4; b++) _v[b] = ((v >> b) & 1) * 2f - 1f;
            for (int k = 0; k < 6; k++)
            {
                int i = Plane[k, 0], j = Plane[k, 1];
                float c = Mathf.Cos(_a[k]), s = Mathf.Sin(_a[k]);
                float x = _v[i] * c - _v[j] * s, y = _v[i] * s + _v[j] * c;
                _v[i] = x; _v[j] = y;
            }
            w = _v[3];
            float f = scale / (d - _v[3]);
            return new Vector4(_v[0] * f, _v[1] * f, _v[2] * f, 0f);
        }

        protected override void Frame(ShowState s, float dt)
        {
            for (int k = 0; k < 6; k++)
                _a[k] = Rate[k] * rx.clk + 0.6f * Mathf.Sin(rx.phrase * (1f + k * 0.13f) + k);
            float gain = rx.Gain();
            float d = 2.4f - 0.55f * rx.bassFast;
            float size = 8f * (1f + 0.22f * rx.bassFast + 0.12f * rx.kick) * (1f - 0.4f * rx.tension) * (1f + 0.5f * rx.impact);
            var centre = new Vector3(0f, 10f, 0f);
            int n = 0;
            for (int layer = 0; layer < 2; layer++)
            {
                float scale = size * (layer == 0 ? 1f : 0.55f) * (d - 0.4f);
                for (int v = 0; v < 16; v++)
                    for (int b = 0; b < 4; b++)
                    {
                        int u = v ^ (1 << b);
                        if (u < v) continue;
                        float w0, w1;
                        Vector4 p0 = Project(v, scale, d, out w0);
                        Vector4 p1 = Project(u, scale, d, out w1);
                        float w = 0.5f * (w0 + w1);
                        float near = 0.5f + 0.5f * w;
                        _beams.Set(n++, centre + new Vector3(p0.x, p0.y, p0.z), centre + new Vector3(p1.x, p1.y, p1.z),
                            Kit.Hue(0.55f + 0.3f * w + 0.15f * layer), gain * (0.35f + 0.9f * near) * (layer == 0 ? 1f : 0.8f));
                    }
            }
            rig.Orbit(cam, rx, 28f, 8f, 10f, dt);
        }
    }
}
