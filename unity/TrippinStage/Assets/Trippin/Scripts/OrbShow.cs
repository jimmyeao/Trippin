// unity_orb: a plasma orb floating over a reflective floor, with two rings of
// light precessing round it.
//  - Shape: the orb's surface is displaced by drifting noise whose depth
//    follows the eased bass (it heaves on the low end) and whose grain
//    follows the eased highs; the rings' beams rise with the spectrum round
//    their circumference.
//  - Motion: the noise drifts and the rings precess on the smooth energy
//    clock; the precession swings direction with the phrase.
//  - Luminance: the orb and the rings follow the music's loudness.
//  - Drops: a build draws the orb in dim; the drop makes it swell (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class OrbShow : KitShow
    {
        const int Rings = 2, Segs = 72;
        Material _m, _mm;
        BeamPool _beams;
        readonly Vector3[] _pt = new Vector3[Segs + 1];

        protected override void Build()
        {
            Env();
            Mesh mesh = SculptureShow.IcoSphere(5);
            mesh.bounds = new Bounds(Vector3.zero, Vector3.one * 40f);
            _m = new Material(orbMat);
            _mm = new Material(orbMat);
            _m.SetFloat("_Radius", 6f);
            _mm.SetFloat("_Radius", 6f);
            Kit.Part(transform, "orb", mesh, _m, new Vector3(0f, 10f, 0f), Vector3.one);
            Kit.Part(transform, "orb reflection", mesh, _mm, new Vector3(0f, -10f, 0f), new Vector3(1f, -1f, 1f));
            _beams = new BeamPool(transform, beamMat, Rings * Segs, "ring", true, 0.06f);
        }

        void Apply(Material m, float inten)
        {
            m.SetFloat("_Amp", 0.8f + 1.4f * rx.bassFast + 0.6f * rx.impact);
            m.SetFloat("_Freq", 1.3f + 1.2f * rx.highFast);
            m.SetFloat("_Phase", rx.clk * 0.25f);
            m.SetFloat("_Hue", 0.5f + 0.1f * Mathf.Sin(rx.phrase));
            m.SetFloat("_Intensity", inten);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.55f, 1.4f);
            Apply(_m, 0.55f * gain);
            Apply(_mm, 0.55f * gain * 0.3f);
            var centre = new Vector3(0f, 10f, 0f);
            float spread = (1f - 0.3f * rx.tension) * (1f + 0.4f * rx.impact);
            int n = 0;
            for (int k = 0; k < Rings; k++)
            {
                float rad = (9.5f + 3f * k) * spread * (1f + 0.1f * rx.bassFast);
                Quaternion q = Quaternion.AngleAxis((rx.clk * 0.03f * (k == 0 ? 1f : -1f) + 0.7f * Mathf.Sin(rx.phrase + k)) * Mathf.Rad2Deg, new Vector3(0.3f + 0.4f * k, 1f, 0.2f).normalized)
                              * Quaternion.AngleAxis(30f + 35f * k, Vector3.right);
                for (int i = 0; i <= Segs; i++)
                {
                    float a = i / (float)Segs * Mathf.PI * 2f;
                    _pt[i] = centre + q * new Vector3(Mathf.Cos(a) * rad, Mathf.Sin(a) * rad, 0f);
                }
                for (int i = 0; i < Segs; i++)
                {
                    float lvl = rx.Spec(Mathf.Abs(Mathf.Sin((i + 0.5f) / Segs * Mathf.PI)));
                    _beams.Set(n++, _pt[i], _pt[i + 1], Kit.Hue(0.5f + 0.15f * k + lvl * 0.2f), gain * (0.35f + 1.1f * lvl));
                }
            }
            rig.Orbit(cam, rx, 30f, 11f, 10f, dt);
        }
    }
}
