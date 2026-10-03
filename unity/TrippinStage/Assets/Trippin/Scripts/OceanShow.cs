// unity_ocean: a night sea of wire swells under a huge moon, with four
// searchlights sweeping the sky from the horizon, seen skimming the surface.
//  - Shape: the swell height follows the eased bass (slow presence plus the
//    fast level) and heaves on a drop; the moon swells with the bass.
//  - Motion: the waves travel on the smooth energy clock; the wind direction
//    (and so the swell) turns with the phrase; the searchlights sweep and
//    reverse with the phrase.
//  - Luminance: the grid, moon and searchlights follow the music's loudness.
//  - Drops: a build calms the sea dim; the drop heaves it up (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class OceanShow : KitShow
    {
        Material _m;
        GlowPool _moon;
        BeamPool _lights;

        protected override void Build()
        {
            Mesh mesh = Kit.GridMesh(200, 200, "ocean");
            _m = new Material(surfaceMat);
            _m.SetFloat("_Mode", 1f);
            _m.SetFloat("_SizeX", 260f);
            _m.SetFloat("_SizeZ", 260f);
            _m.SetFloat("_Grid", 0.14f);
            _m.SetFloat("_Fade", 160f);
            _m.SetFloat("_Fill", 1.4f);
            Kit.Part(transform, "ocean", mesh, _m, new Vector3(0f, 0f, 130f), Vector3.one);
            _moon = new GlowPool(transform, glowMat, 2, "moon", false);
            _lights = new BeamPool(transform, beamMat, 4, "searchlight", false, 0.25f, 0.02f, 10f, 0.7f, 0.5f, 0.2f);
            haze = new HazeSet(transform, hazeMat, new[] { new Vector3(0f, 16f, 170f) }, new[] { new Vector2(260f, 60f) }, new[] { 0.5f }, new[] { 0.08f });
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.5f, 1.35f);
            float amp = (0.35f + 0.5f * rx.bassSlow + 0.45f * rx.bassFast + 0.5f * rx.impact) * (1f - 0.5f * rx.tension);
            _m.SetFloat("_Amp", amp);
            _m.SetFloat("_Phase", rx.clk * 0.35f);
            _m.SetFloat("_Intensity", 0.6f * gain);
            _m.SetFloat("_Hue", 0.55f + 0.08f * Mathf.Sin(rx.phrase * 0.5f));
            _m.SetVector("_P0", new Vector4(0.9f + 0.6f * Mathf.Sin(rx.phrase), 0f, 0f, 0f));
            Quaternion face = Quaternion.identity;
            _moon.Set(0, new Vector3(0f, 38f, 240f), 90f * (1f + 0.15f * rx.bassFast), 0.55f, gain * 0.9f, face);
            _moon.Set(1, new Vector3(0f, 38f, 239f), 50f, 0.5f, gain * 1.2f, face);
            for (int i = 0; i < 4; i++)
            {
                float side = i < 2 ? -1f : 1f;
                var from = new Vector3(side * (30f + 12f * (i % 2)), 0.5f, 170f);
                float sweep = Mathf.Sin(rx.phrase * 0.5f + i * 1.4f) * 22f * (1f + 0.4f * rx.highFast);
                var to = from + new Vector3(sweep - side * 20f, 90f, -10f);
                _lights.Set(i, from, to, Kit.Hue(0.5f + i * 0.08f), gain * (0.5f + 0.8f * rx.midFast));
            }
            float sway = Mathf.Sin(rx.phrase * 0.5f);
            rig.Move(cam, new Vector3(5f * sway, 6f, 0f), new Vector3(2f * sway, 6f, 80f), dt, 1f);
        }
    }
}
