// unity_highlands: the dawn-valley flight of shaders/scenes/highlands.wgsl,
// rebuilt for the Unity engine so it runs on machines whose GPUs can't raymarch
// it. Same staging: a carved corridor between ridged peaks, a low warm sun
// ahead, mist pooling on the valley floor, the flight on the energy clock.
// A flight scene: no beat-synced flashes (AGENTS.md).
//  - Shape: the ridges sharpen and rise with the slow bass presence.
//  - Motion: the flight speed is the smooth energy clock; the camera sways
//    and banks gently with the phrase.
//  - Luminance: terrain, mist and sun follow the eased loudness; the mist
//    thickens a little in breakdowns.
//  - Drops: a build dims the light; the drop warms the sun through.
using UnityEngine;

namespace TrippinStage
{
    public sealed class HighlandsShow : KitShow
    {
        public Material highlandsMat;

        Material _land, _sky;

        protected override void Build()
        {
            _sky = Kit.Fullscreen(transform, horizonMat, "dawn sky");
            _sky.SetFloat("_SkyMode", 0f);
            _sky.SetFloat("_SunH", 0.10f);
            _sky.SetFloat("_SunSize", 0.12f);
            _land = new Material(highlandsMat);
            _land.SetFloat("_SunH", 0.10f);
            _land.SetFloat("_SunSize", 0.12f);
            // Long corridor ahead of the camera: x in +-170, z in -20..880.
            Kit.Part(transform, "valley", Kit.GridMesh(240, 480, "valley"), _land,
                new Vector3(0f, 0f, 430f), Vector3.one);
            // Mist banks lying across the valley floor.
            haze = new HazeSet(transform, hazeMat,
                new[] { new Vector3(0, 6f, 60f), new Vector3(0, 8f, 140f), new Vector3(0, 12f, 260f), new Vector3(0, 16f, 420f) },
                new[] { new Vector2(160f, 18f), new Vector2(220f, 30f), new Vector2(300f, 50f), new Vector2(380f, 70f) },
                new[] { 0.75f, 0.75f, 0.75f, 0.75f },
                new[] { 0.10f, 0.08f, 0.07f, 0.06f });
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.6f, 1.4f);
            // Dawn palette: warm low sun, cool indigo zenith — fixed hue window,
            // not the track palette (this scene's look is the dawn light).
            float hue = 0.02f;
            _sky.SetFloat("_SkyHue", hue);
            _sky.SetFloat("_SkyGain", gain);
            _sky.SetFloat("_SkyClk", rx.clk);
            _land.SetFloat("_SkyHue", hue);
            _land.SetFloat("_SkyGain", gain);
            _land.SetFloat("_SkyClk", rx.clk);
            _land.SetFloat("_Gain", gain);
            // The ridges sharpen with the slow bass; the mist stirs and warms
            // with the music, thicker when the drums are out.
            _land.SetFloat("_Ridge", Mathf.Clamp01(rx.bassSlow * 1.2f));
            _land.SetFloat("_Mist", 0.4f + 0.5f * rx.lum + 0.25f * rx.calm);
            _land.SetFloat("_Scroll", rx.clk * 6f);

            float sway = Mathf.Sin(rx.phrase);
            float bob = Mathf.Sin(rx.beat / 16f * Mathf.PI * 2f);
            rig.Move(cam,
                new Vector3(9f * sway, 8f + 1.5f * bob, 0f),
                new Vector3(16f * Mathf.Sin(rx.phrase * 0.6f), 2f, 300f), dt, 1.0f);
        }
    }
}
