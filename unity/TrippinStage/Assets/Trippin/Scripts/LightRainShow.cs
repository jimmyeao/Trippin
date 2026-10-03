// unity_lightrain: 240 streaks of light falling through a hazy night, with a
// wet-floor reflection.
//  - Shape: each streak's length follows the eased loudness and bass, so the
//    rain stretches into long needles on the loud parts and shortens to
//    sparks in the quiet; the slant (wind) swings with the phrase.
//  - Motion: the fall is a function of the smooth energy clock (deterministic
//    per streak, no integration of audio), so the rain speeds up as the
//    track builds and surges on a drop.
//  - Luminance: the whole sheet follows the music's loudness.
//  - Drops: a build thins and dims it; the drop is a downpour (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class LightRainShow : KitShow
    {
        const int N = 240;
        const float Top = 40f;
        BeamPool _beams;

        protected override void Build()
        {
            Env(150f, 0.06f);
            _beams = new BeamPool(transform, beamMat, N, "rain", true, 0.03f, 0f, 30f, 0.4f);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.5f, 1.3f);
            float lean = 0.25f * Mathf.Sin(rx.phrase);
            float lenBase = 2f + 6f * rx.lum + 4f * rx.bassFast + 8f * rx.impact;
            float thin = 1f - 0.5f * rx.tension;
            for (int i = 0; i < N; i++)
            {
                float x = (Kit.H(i, 1) - 0.5f) * 70f, z = (Kit.H(i, 2) - 0.5f) * 50f + 6f;
                float speed = 0.6f + 0.8f * Kit.H(i, 3);
                float fall = rx.clk * 0.06f * speed + Kit.H(i, 4);
                fall -= Mathf.Floor(fall);
                float y = Top * (1f - fall);
                float len = (0.5f + Kit.H(i, 5)) * lenBase * thin;
                var p0 = new Vector3(x + lean * y, y, z);
                var p1 = new Vector3(x + lean * (y + len), y + len, z);
                float edge = Mathf.Clamp01(y / 4f) * Mathf.Clamp01((Top - y) / 6f);
                _beams.Set(i, p0, p1, Kit.Hue(0.3f + x * 0.008f), gain * (0.4f + 0.7f * Kit.H(i, 6)) * edge * thin);
            }
            rig.Orbit(cam, rx, 26f, 3.5f, 9f, dt, 0.5f, 0.1f);
        }
    }
}
