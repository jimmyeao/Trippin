// unity_helix: a double-helix of light (DNA) rising out of a hazy hall,
// reflected in a wet floor, with base-pair rungs between the strands, glowing
// nodes along the backbone and motes drifting up around it.
//  - Shape: the helix is a live spectrum. At every height the radius bulges
//    with the eased level of that height's frequency band (lows at the base,
//    highs at the top); the whole coil swells with the eased bass and kick, the
//    twist tightens with the mids, and each node swells with its own band.
//  - Motion: it spins on the smooth energy clock and reverses direction on a
//    phrase-length sine; bright packets slide up the strands on the same
//    clock; motes rise at a speed that follows the clock.
//  - Luminance: each segment of each strand and every rung is as bright as its
//    band is loud, so quiet music dims it and a full mix blooms.
//  - Drops: a build pulls the strands in tight and dim; the drop blows them
//    open (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class HelixShow : KitShow
    {
        const int Pts = 72, Rungs = 30, RungPts = 5, Nodes = 40, Motes = 70;
        const float Height = 28f;
        TubeRibbon[] _strand = new TubeRibbon[2];
        TubeRibbon _rungs;
        GlowPool _nodes, _motes;
        BeamPool _shafts;
        readonly Vector3[,] _p = new Vector3[2, Pts];

        protected override void Build()
        {
            Env();
            for (int s = 0; s < 2; s++)
            {
                _strand[s] = new TubeRibbon(transform, ribbonMat, Pts, 8, "helix strand " + s, true);
                _strand[s].Mat.SetFloat("_PulseFreq", 5f);
                _strand[s].Mat.SetFloat("_PulseAmt", 0.55f);
                _strand[s].Mat.SetFloat("_Core", 5f);
            }
            _rungs = new TubeRibbon(transform, ribbonMat, Rungs * (RungPts + 2), 6, "helix rungs", true);
            _rungs.Mat.SetFloat("_PulseAmt", 0f);
            _rungs.Mat.SetFloat("_Core", 3f);
            _rungs.Mat.SetFloat("_White", 0.15f);
            _nodes = new GlowPool(transform, glowMat, Nodes, "node", true);
            _motes = new GlowPool(transform, glowMat, Motes, "mote", false);
            _shafts = new BeamPool(transform, beamMat, 8, "shaft", false, 0.18f, 0.006f, 8f, 0.7f, 0.4f, 0f);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain();
            float spin = rx.clk * 0.07f + 1.1f * Mathf.Sin(rx.phrase);
            float twist = 0.3f + 0.1f * rx.midFast;
            float r0 = 5.2f * (1f - 0.45f * rx.tension) * (1f + 0.55f * rx.impact);
            for (int st = 0; st < 2; st++)
            {
                var t = _strand[st];
                for (int i = 0; i < Pts; i++)
                {
                    float u = i / (Pts - 1f);
                    float y = 0.6f + Height * u;
                    float bulge = rx.Spec(u);
                    float r = r0 * (1f + 0.18f * Mathf.Sin(y * 0.33f + rx.clk * 0.08f) + 0.4f * rx.bassFast + 0.12f * rx.kick + 0.55f * bulge);
                    float th = y * twist + st * Mathf.PI + spin;
                    t.P[i] = new Vector3(Mathf.Cos(th) * r, y, Mathf.Sin(th) * r);
                    _p[st, i] = t.P[i];
                    t.R[i] = 0.26f * (1f + 0.9f * bulge + 0.4f * rx.kick);
                    t.K[i] = 0.55f + 1.5f * bulge;
                    t.Hu[i] = (st == 0 ? 0.0f : 0.32f) + u * 0.25f;
                }
                t.Mat.SetFloat("_Phase", rx.clk * 0.5f * (st == 0 ? 1f : -1f));
                t.Mat.SetFloat("_Hue", 0.45f + 0.05f * Mathf.Sin(rx.phrase * 0.5f));
                t.Apply(gain * 0.9f);
            }
            // Rungs: for each, strand A -> strand B across RungPts points, then a dark hop to the next.
            int n = 0;
            for (int k = 0; k < Rungs; k++)
            {
                int i = 2 + k * (Pts - 4) / Rungs;
                float u = i / (Pts - 1f);
                float lvl = rx.Spec(u);
                Vector3 a = _p[0, i], b = _p[1, i];
                for (int j = 0; j < RungPts; j++)
                {
                    float f = j / (RungPts - 1f);
                    float mid = 1f - Mathf.Abs(f * 2f - 1f);
                    _rungs.P[n] = Vector3.Lerp(a, b, f);
                    _rungs.R[n] = 0.1f * (1f + 0.8f * lvl) * (0.6f + 0.4f * (1f - mid));
                    _rungs.K[n] = (0.25f + 1.1f * lvl) * (0.55f + 0.45f * (1f - mid));
                    _rungs.Hu[n] = Mathf.Lerp(0.0f, 0.32f, f) + u * 0.25f;
                    n++;
                }
                // Dark hop to the next rung (zero-intensity points).
                int nk = (k + 1) % Rungs;
                int ni = 2 + nk * (Pts - 4) / Rungs;
                for (int j = 0; j < 2; j++)
                {
                    _rungs.P[n] = j == 0 ? b : _p[0, ni];
                    _rungs.R[n] = 0.01f; _rungs.K[n] = 0f; _rungs.Hu[n] = 0f;
                    n++;
                }
            }
            _rungs.Mat.SetFloat("_Hue", 0.45f);
            _rungs.Apply(gain);

            // Nodes on the backbone, sized by their own band; motes rising round the coil.
            Quaternion face = cam.transform.rotation;
            for (int i = 0; i < Nodes; i++)
            {
                int st = i & 1;
                int idx = 3 + (i >> 1) * (Pts - 6) / (Nodes / 2);
                float u = idx / (Pts - 1f);
                float lvl = rx.Spec(u);
                _nodes.Set(i, _p[st, idx], 0.9f + 2.4f * lvl + 0.8f * rx.kick, 0.45f + (st == 0 ? 0f : 0.32f) + u * 0.25f, gain * (0.3f + 1.1f * lvl), face);
            }
            float rise = rx.clk * 0.05f;
            for (int i = 0; i < Motes; i++)
            {
                float u = Mathf.Repeat(Kit.H(i, 1) + rise * (0.6f + 0.8f * Kit.H(i, 2)), 1f);
                float a = Kit.H(i, 3) * Mathf.PI * 2f + spin * 0.4f * (0.5f + Kit.H(i, 4));
                float rr = r0 * (1.4f + 1.6f * Kit.H(i, 5)) * (1f + 0.3f * rx.bassFast);
                float fade = Mathf.Sin(u * Mathf.PI);
                _motes.Set(i, new Vector3(Mathf.Cos(a) * rr, 0.5f + u * (Height + 4f), Mathf.Sin(a) * rr),
                    0.35f + 0.5f * Kit.H(i, 6), 0.4f + Kit.H(i, 7) * 0.4f, gain * 0.55f * fade * (0.6f + 0.8f * rx.highFast), face);
            }
            // Backlight shafts rising behind the coil, slowly turning with the phrase.
            for (int i = 0; i < 8; i++)
            {
                float a = i / 8f * Mathf.PI * 2f + spin * 0.15f;
                var foot = new Vector3(Mathf.Cos(a) * 26f, 0f, Mathf.Sin(a) * 26f);
                var tip = foot + new Vector3(-Mathf.Cos(a) * 9f, 44f, -Mathf.Sin(a) * 9f);
                _shafts.Set(i, foot, tip, Kit.Hue(0.5f + i * 0.04f), gain * (0.07f + 0.22f * rx.Spec(i / 7f)));
            }
            rig.Orbit(cam, rx, 48f, 12f, 14f, dt);
        }
    }
}
