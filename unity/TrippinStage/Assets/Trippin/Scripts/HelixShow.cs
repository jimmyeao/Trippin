// unity_helix: a three-strand light helix standing in a hazy hall, with
// rungs between the strands, reflected in a wet floor.
//  - Shape: the helix swells in radius with the eased bass and the kick, and
//    bulges at each height where the spectrum is loud (low notes at the
//    base, highs at the top), so its outline is a live spectrum.
//  - Motion: it spins on the smooth energy clock and reverses direction on a
//    phrase-length sine; the pitch tightens with the mids.
//  - Luminance: each segment glows with the eased level of its own
//    frequency band, so quiet music dims it and a full mix blooms.
//  - Drops: a build pulls the strands in tight and dim; the drop blows them
//    open (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class HelixShow : KitShow
    {
        const int Strands = 3, Pts = 48, Rungs = 24;
        const float Height = 26f;
        BeamPool _beams;
        readonly Vector3[] _p = new Vector3[Strands * Pts];

        protected override void Build()
        {
            Env();
            _beams = new BeamPool(transform, beamMat, Strands * (Pts - 1) + Rungs, "helix", true, 0.05f);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float spin = rx.clk * 0.08f + 1.2f * Mathf.Sin(rx.phrase);
            float twist = 0.34f + 0.08f * rx.midFast;
            float r0 = 5.5f * (1f - 0.45f * rx.tension) * (1f + 0.6f * rx.impact);
            float gain = rx.Gain();
            for (int st = 0; st < Strands; st++)
                for (int i = 0; i < Pts; i++)
                {
                    float u = i / (Pts - 1f);
                    float y = 0.5f + Height * u;
                    float r = r0 * (1f + 0.25f * Mathf.Sin(y * 0.3f + rx.clk * 0.1f) + 0.45f * rx.bassFast + 0.15f * rx.kick + 0.5f * rx.Spec(u));
                    float th = y * twist + st * Mathf.PI * 2f / Strands + spin;
                    _p[st * Pts + i] = new Vector3(Mathf.Cos(th) * r, y, Mathf.Sin(th) * r);
                }
            int n = 0;
            for (int st = 0; st < Strands; st++)
                for (int i = 0; i < Pts - 1; i++)
                {
                    float u = i / (Pts - 1f);
                    float lvl = rx.Spec(u);
                    _beams.Set(n++, _p[st * Pts + i], _p[st * Pts + i + 1], Kit.Hue(0.4f + 0.12f * st + u * 0.35f), gain * (0.45f + 1.1f * lvl));
                }
            for (int k = 0; k < Rungs; k++)
            {
                int i = k * (Pts - 1) / Rungs + 1;
                float u = i / (Pts - 1f);
                _beams.Set(n++, _p[i], _p[Pts + i], Kit.Hue(0.1f + u * 0.4f), gain * (0.4f + 0.9f * rx.Spec(u)));
            }
            rig.Orbit(cam, rx, 40f, 12f, 13f, dt);
        }
    }
}
