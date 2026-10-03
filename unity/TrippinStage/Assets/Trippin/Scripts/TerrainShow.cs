// unity_terrain: a low flight over a neon wire landscape toward a huge sun.
// A flight scene: no beat-synced flashes (AGENTS.md); luminance follows eased
// levels only.
//  - Shape: the hills' height follows the eased bass (the landscape heaves on
//    the low end) and swells on a drop; the sun swells with the bass.
//  - Motion: the flight speed is the smooth energy clock (the ground rushes
//    faster as the track builds); the corridor width and the camera sway with
//    the phrase.
//  - Luminance: the grid and the sun follow the music's loudness.
//  - Drops: a build flattens it dim; the drop heaves the hills up (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class TerrainShow : KitShow
    {
        Material _m;
        GlowPool _sun;

        protected override void Build()
        {
            Mesh mesh = Kit.GridMesh(160, 220, "terrain");
            _m = new Material(surfaceMat);
            _m.SetFloat("_Mode", 0f);
            _m.SetFloat("_SizeX", 140f);
            _m.SetFloat("_SizeZ", 220f);
            _m.SetFloat("_Grid", 0.18f);
            _m.SetFloat("_Fade", 110f);
            _m.SetFloat("_Fill", 0.6f);
            Kit.Part(transform, "terrain", mesh, _m, new Vector3(0f, 0f, 110f), Vector3.one);
            _sun = new GlowPool(transform, glowMat, 2, "sun", false);
            haze = new HazeSet(transform, hazeMat, new[] { new Vector3(0f, 14f, 150f) }, new[] { new Vector2(220f, 50f) }, new[] { 0.2f }, new[] { 0.09f });
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.35f, 1.6f);
            float amp = (7f + 4f * rx.bassSlow + 5f * rx.bassFast + 6f * rx.impact) * (1f - 0.55f * rx.tension);
            _m.SetFloat("_Amp", amp);
            _m.SetFloat("_Scroll", rx.clk * 4f);
            _m.SetFloat("_Intensity", 0.55f * gain);
            _m.SetFloat("_Hue", 0.5f + 0.1f * Mathf.Sin(rx.phrase * 0.5f));
            _m.SetVector("_P0", new Vector4(14f + 5f * Mathf.Sin(rx.phrase), 22f, 0f, 0f));
            Quaternion face = Quaternion.identity;
            _sun.Set(0, new Vector3(0f, 16f, 205f), 120f * (1f + 0.2f * rx.bassFast), 0.1f, gain * 0.9f, face);
            _sun.Set(1, new Vector3(0f, 16f, 204f), 70f * (1f + 0.25f * rx.bassFast), 0.02f, gain * 1.2f, face);
            float sway = Mathf.Sin(rx.phrase);
            rig.Move(cam, new Vector3(6f * sway, 4.5f + 1.2f * Mathf.Sin(rx.beat / 32f * Mathf.PI * 2f), 0f), new Vector3(3f * sway, 7f, 60f), dt, 1.2f);
        }
    }
}
