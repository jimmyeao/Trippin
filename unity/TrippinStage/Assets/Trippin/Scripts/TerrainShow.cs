// unity_terrain: a low flight over a shaded neon mountain range toward a huge
// banded sun at dusk. A flight scene: no beat-synced flashes (AGENTS.md);
// luminance follows eased levels only. The Land shader draws the mountains
// (an opaque lit surface with glowing contours and grid, not a wireframe) and
// the Horizon shader the sky, so the range fades into the sky with no edge.
//  - Shape: the mountains left and right of the flight corridor are a live
//    spectrum (each ridge's height follows its band), the whole range heaves
//    with the eased bass and swells on a drop, swells run out along the valley
//    from the camera on the beat-clock; the sun swells with the bass and its
//    scan bands thicken.
//  - Motion: the flight speed is the smooth energy clock (the ground rushes
//    faster as the track builds); the corridor width and the camera sway with
//    the phrase.
//  - Luminance: the surface, grid and sun follow the music's loudness.
//  - Drops: a build flattens it dim; the drop heaves the range up (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class TerrainShow : KitShow
    {
        Material _land, _sky;

        protected override void Build()
        {
            _sky = Kit.Fullscreen(transform, horizonMat, "dusk sky");
            _sky.SetFloat("_SkyMode", 0f);
            _sky.SetFloat("_SunH", 0.12f);
            _sky.SetFloat("_SunSize", 0.17f);
            _land = new Material(landMat);
            _land.SetFloat("_Mode", 0f);
            _land.SetFloat("_SizeX", 300f);
            _land.SetFloat("_SizeZ", 300f);
            _land.SetFloat("_Grid", 0.12f);
            _land.SetFloat("_Fade", 120f);
            _land.SetFloat("_SunH", 0.12f);
            _land.SetFloat("_SunSize", 0.17f);
            Kit.Part(transform, "terrain", Kit.GridMesh(220, 260, "terrain"), _land, new Vector3(0f, 0f, 150f), Vector3.one);
        }

        void Sky(Material m, float gain)
        {
            m.SetFloat("_SkyHue", 0.05f + 0.08f * Mathf.Sin(rx.phrase * 0.5f));
            m.SetFloat("_SkyGain", gain);
            m.SetFloat("_SkyClk", rx.clk);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.55f, 1.5f);
            Sky(_sky, gain);
            Sky(_land, gain);
            _land.SetFloat("_Gain", gain);
            float amp = (13f + 5f * rx.bassSlow + 6f * rx.bassFast + 7f * rx.impact) * (1f - 0.55f * rx.tension);
            _land.SetFloat("_Amp", amp);
            _land.SetFloat("_Scroll", rx.clk * 4f);
            _land.SetFloat("_Phase", rx.clk * 0.5f);
            _land.SetVector("_P0", new Vector4(16f + 6f * Mathf.Sin(rx.phrase), 28f, 150f, 0f));
            float sway = Mathf.Sin(rx.phrase);
            rig.Move(cam, new Vector3(6f * sway, 8f + 1.2f * Mathf.Sin(rx.beat / 32f * Mathf.PI * 2f), 0f), new Vector3(3f * sway, 10f, 100f), dt, 1.2f);
        }
    }
}
