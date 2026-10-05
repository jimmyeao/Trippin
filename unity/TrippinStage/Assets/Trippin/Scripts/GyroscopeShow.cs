// unity_gyroscope: a gimbal of five glowing rings turning about different axes
// round a plasma core, with beads orbiting each ring, reflected in a wet floor.
//  - Shape: each ring's radius breathes with the eased level of its own band
//    (bass, mids, mid-highs, highs, then bass again); its tube swells and
//    brightens round the circumference where the spectrum is loud, so each
//    ring is a live spectrum wrapped into a circle; the core heaves with the
//    bass.
//  - Motion: each ring turns on one of the smooth band energy clocks and its
//    tilt swings on a phrase-length sine, so they keep reversing; the beads
//    orbit on the same clocks.
//  - Luminance: each ring is as bright as its band is loud.
//  - Drops: a build draws the rings in dim; the drop throws them wide (eased).
//  - Owner ("out of focus and not very reactive"): crisp tubes (sharper core,
//    less white-hot wash, thinner, glow capped lower so they keep their
//    colour), rings that visibly take the spectrum's shape, kicks that ping
//    them outward, and turns fast enough to read.
using UnityEngine;

namespace TrippinStage
{
    public sealed class GyroscopeShow : KitShow
    {
        const int Rings = 5, Segs = 96, BeadsPer = 3;
        TubeRibbon[] _ring = new TubeRibbon[Rings];
        GlowPool _beads;
        BeamPool _spokes;
        Material _core, _coreM;

        protected override void Build()
        {
            Env();
            for (int k = 0; k < Rings; k++)
            {
                _ring[k] = new TubeRibbon(transform, ribbonMat, Segs + 1, 8, "gyro ring " + k, true);
                _ring[k].Mat.SetFloat("_PulseFreq", 3f);
                _ring[k].Mat.SetFloat("_PulseAmt", 0.5f);
                _ring[k].Mat.SetFloat("_Core", 14f);
                _ring[k].Mat.SetFloat("_White", 0.22f);
            }
            _beads = new GlowPool(transform, glowMat, Rings * BeadsPer, "bead", true);
            _spokes = new BeamPool(transform, beamMat, 10, "spoke", false, 0.12f, 0.004f, 10f, 0.6f, 0.6f, 0f);
            Mesh mesh = SculptureShow.IcoSphere(4);
            mesh.bounds = new Bounds(Vector3.zero, Vector3.one * 40f);
            _core = new Material(orbMat); _coreM = new Material(orbMat);
            _core.SetFloat("_Radius", 2.6f); _coreM.SetFloat("_Radius", 2.6f);
            Kit.Part(transform, "core", mesh, _core, new Vector3(0f, 11f, 0f), Vector3.one);
            Kit.Part(transform, "core reflection", mesh, _coreM, new Vector3(0f, -11f, 0f), new Vector3(1f, -1f, 1f));
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain();
            var centre = new Vector3(0f, 11f, 0f);
            float spread = (1f - 0.4f * rx.tension) * (1f + 0.5f * rx.impact);
            _core.SetFloat("_Amp", 0.7f + 1.3f * rx.bassFast + 0.6f * rx.impact);
            _core.SetFloat("_Freq", 1.4f + 1.1f * rx.highFast);
            _core.SetFloat("_Phase", rx.clk * 0.3f);
            _core.SetFloat("_Hue", 0.55f + 0.1f * Mathf.Sin(rx.phrase));
            _core.SetFloat("_Intensity", 0.8f * gain);
            _coreM.CopyPropertiesFromMaterial(_core);
            _coreM.SetFloat("_Intensity", 0.8f * gain * 0.3f);
            for (int k = 0; k < Rings; k++)
            {
                float band = rx.Band(k);
                float rad = (4.5f + 2.3f * k) * (1f + 0.25f * band + 0.12f * rx.kick) * spread;
                float c = Rx.Clock(s, 1 + k % 3);
                Quaternion q = Quaternion.AngleAxis((c * 0.06f * (k % 2 == 0 ? 1f : -1f) + 0.8f * Mathf.Sin(rx.phrase + k)) * Mathf.Rad2Deg, new Vector3(0.2f + 0.2f * k, 1f, 0.4f - 0.1f * k).normalized)
                              * Quaternion.AngleAxis((25f + 18f * k) * Mathf.Sin(rx.phrase * 0.5f + k * 1.3f), Vector3.right);
                var t = _ring[k];
                for (int i = 0; i <= Segs; i++)
                {
                    float a = i / (float)Segs * Mathf.PI * 2f;
                    float sx = Mathf.Abs(Mathf.Sin(a * 0.5f));
                    float lvl = rx.Spec(sx);
                    // The shape: the spectrum averaged over a few bins, so the ring swells in smooth lobes
                    // (per-bin it read as a jagged, spiky wire).
                    float lob = (rx.Spec(sx - 0.08f) + rx.Spec(sx - 0.04f) + lvl + rx.Spec(sx + 0.04f) + rx.Spec(sx + 0.08f)) * 0.2f;
                    float rip = 1f + 0.2f * (lob - 0.25f);
                    t.P[i] = centre + q * new Vector3(Mathf.Cos(a) * rad * rip, Mathf.Sin(a) * rad * rip, 0f);
                    t.R[i] = 0.13f * (1f + 0.35f * k * 0.15f) * (1f + 0.9f * lob + 0.4f * band);
                    t.K[i] = 0.3f + 0.5f * band + 0.7f * lvl;
                    t.Hu[i] = 0.12f * k + a * 0.04f;
                }
                t.Mat.SetFloat("_Phase", c * 0.25f * (k % 2 == 0 ? 1f : -1f));
                t.Mat.SetFloat("_Hue", 0.3f);
                t.Apply(gain);
                for (int b = 0; b < BeadsPer; b++)
                {
                    float f = Mathf.Repeat(b / (float)BeadsPer + c * 0.04f * (k % 2 == 0 ? 1f : -1f), 1f);
                    int i0 = Mathf.Min((int)(f * Segs), Segs - 1);
                    Vector3 pos = Vector3.Lerp(t.P[i0], t.P[i0 + 1], f * Segs - i0);
                    _beads.Set(k * BeadsPer + b, pos, 0.9f + 1.4f * band, 0.12f * k + 0.1f, gain * (0.6f + 1.1f * band), cam.transform.rotation);
                }
            }
            // Spokes from the core out through the outer ring, one per band slice.
            for (int i = 0; i < 10; i++)
            {
                float a = i / 10f * Mathf.PI * 2f + rx.clk * 0.004f;
                var dir = new Vector3(Mathf.Cos(a), Mathf.Sin(a * 1.7f) * 0.6f, Mathf.Sin(a)).normalized;
                float lvl = rx.Spec(i / 9f);
                _spokes.Set(i, centre + dir * 3f, centre + dir * (16f * spread), Kit.Hue(0.1f * (i % 5)), gain * (0.08f + 0.3f * lvl));
            }
            rig.Orbit(cam, rx, 38f, 12f, 11f, dt);
        }
    }
}
