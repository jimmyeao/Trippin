// unity_nebula: a drifting deep-space nebula. Twelve huge sheets of soft
// smoke at different depths, each tinted by the stage palette, with a field
// of distant stars; the camera drifts through the layers.
//  - Shape: each depth layer is tied to one slice of the spectrum, so its
//    density swells with that part of the mix: the nebula's structure is a
//    live spectrum from the near layers (lows) to the far ones (highs).
//  - Motion: the layers slide sideways at different speeds on the smooth
//    energy clock (parallax), and the camera sways with the phrase.
//  - Luminance: every layer follows the music's loudness; stars swell with
//    the eased highs.
//  - Drops: a build thickens the whole thing dim; the drop floods it.
using UnityEngine;

namespace TrippinStage
{
    public sealed class NebulaShow : KitShow
    {
        const int Layers = 12, Stars = 80;
        Material[] _m;
        Transform[] _t;
        GlowPool _stars;
        readonly float[] _base = new float[Layers];

        protected override void Build()
        {
            var pos = new Vector3[Layers];
            var size = new Vector2[Layers];
            var hue = new float[Layers];
            for (int i = 0; i < Layers; i++)
            {
                pos[i] = new Vector3(0f, 12f + Mathf.Sin(i * 1.7f) * 5f, 8f + i * 9f);
                size[i] = new Vector2(160f + i * 6f, 80f);
                hue[i] = i * 0.08f;
                _base[i] = 0.05f + 0.04f * (i % 3);
            }
            _m = Kit.HazeLayers(transform, hazeMat, pos, size, hue);
            _t = new Transform[Layers];
            for (int i = 0; i < Layers; i++) _t[i] = null; // positions are set through the layer's own transform below
            _stars = new GlowPool(transform, glowMat, Stars, "star", false);
            // Find the layer transforms Kit created (children named "haze i").
            for (int i = 0; i < Layers; i++) _t[i] = transform.Find("haze " + i);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.55f, 1.4f);
            for (int i = 0; i < Layers; i++)
            {
                float bin = i / (Layers - 1f);
                float dens = _base[i] * (0.6f + 1.3f * rx.lum + 0.4f * rx.midFast) * (0.6f + 0.9f * rx.Spec(bin))
                             * (1f + 0.6f * rx.tension + 0.9f * rx.impact);
                _m[i].SetFloat("_Density", dens * gain);
                if (_t[i] != null)
                {
                    float x = Mathf.Sin(rx.clk * 0.01f * (0.5f + i * 0.1f) + i * 2f) * 12f;
                    float y = 12f + Mathf.Sin(i * 1.7f) * 5f + Mathf.Sin(rx.clk * 0.006f * (1f + i * 0.07f) + i) * 3f;
                    _t[i].localPosition = new Vector3(x, y, 8f + i * 9f);
                }
            }
            Quaternion face = cam.transform.rotation;
            for (int i = 0; i < Stars; i++)
            {
                var p = new Vector3((Kit.H(i, 1) - 0.5f) * 150f, 2f + Kit.H(i, 2) * 40f, 40f + Kit.H(i, 3) * 80f);
                float tw = 0.8f + 0.2f * Mathf.Sin(rx.clkHigh * 0.2f + i);
                _stars.Set(i, p, (0.8f + Kit.H(i, 4) * 1.6f) * (0.8f + 0.7f * rx.highFast), 0.3f + Kit.H(i, 5) * 0.5f, gain * 0.7f * tw, face);
            }
            float sway = Mathf.Sin(rx.phrase * 0.5f);
            rig.Move(cam, new Vector3(sway * 10f, 10f, -20f + 6f * Mathf.Sin(rx.beat / 256f * Mathf.PI * 2f)), new Vector3(0f, 12f, 60f), dt, 0.5f);
        }
    }
}
