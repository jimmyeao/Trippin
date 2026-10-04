// unity_ledwall: a wall of 36 x 20 LED cells that shows a different live
// pattern every four bars, crossfading over a beat: spectrum bars, radial
// ripples, plasma, diamond waves. A display, so its cells may change; the wall
// itself never moves.
//  - Shape: the patterns are built from the eased spectrum (bars), bass
//    (ripples), mids and highs (plasma, diamonds), so the wall is a live
//    portrait of the mix.
//  - Motion: pattern travel is the smooth energy clock; the camera drifts in
//    front of the wall with the phrase.
//  - Luminance: each cell's brightness is its pattern value times the
//    music's eased loudness.
//  - Drops: a build dims the wall to a faint grid; the drop floods it.
using UnityEngine;

namespace TrippinStage
{
    public sealed class LedWallShow : KitShow
    {
        const int W = 36, H = 20, Patterns = 4;
        const float Pitch = 1.0f, Z = 10f;
        GlowPool _cells;
        int _cur = -1, _prev;
        float _start;

        protected override void Build()
        {
            Env(140f, 0.05f);
            _cells = new GlowPool(transform, glowMat, W * H, "cell", false);
        }

        float Pattern(int p, float x, float y, float cx, float cy)
        {
            switch (p)
            {
                case 0: // spectrum bars: column -> bin, lit up to its height
                {
                    float bin = x / (W - 1f);
                    float bar = Mathf.Clamp01(rx.Spec(bin) * 1.25f) * H;
                    return y < bar ? 1f - 0.45f * y / H : 0f;
                }
                case 1: // ripples from the centre, deeper with the bass
                {
                    float d = Mathf.Sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy) * 1.4f);
                    float w = 0.5f + 0.5f * Mathf.Sin(d * 0.9f - rx.clk * 0.6f);
                    return w * w * (0.35f + 0.9f * rx.bassFast);
                }
                case 2: // plasma
                {
                    float v = 0.5f + 0.25f * Mathf.Sin(x * 0.5f + rx.clk * 0.2f) + 0.25f * Mathf.Sin(y * 0.6f - rx.clk * 0.15f + Mathf.Sin(x * 0.3f));
                    return Mathf.Clamp01((v - 0.25f) * (1.2f + 1.5f * rx.midFast));
                }
                default: // diamond waves
                {
                    float d = Mathf.Abs(x - cx) + Mathf.Abs(y - cy);
                    float w = 0.5f + 0.5f * Mathf.Sin(d * 0.7f - rx.clk * 0.5f);
                    return w * (0.3f + 0.8f * rx.highFast + 0.4f * rx.kick);
                }
            }
        }

        protected override void Frame(ShowState s, float dt)
        {
            int p = (Mathf.FloorToInt(rx.beat / 4f) / 4) % Patterns;
            if (_cur < 0) { _cur = _prev = p; _start = rx.beat - 4f; }
            else if (p != _cur) { _prev = _cur; _cur = p; _start = rx.beat; }
            float m = Mathf.SmoothStep(0f, 1f, rx.beat - _start);
            float gain = rx.Gain(0.5f, 1.4f);
            float cx = (W - 1) * 0.5f, cy = (H - 1) * 0.5f;
            Quaternion face = Quaternion.identity; // the wall faces the audience (-z)
            for (int j = 0; j < H; j++)
                for (int i = 0; i < W; i++)
                {
                    float v = Mathf.Lerp(Pattern(_prev, i, j, cx, cy), Pattern(_cur, i, j, cx, cy), m);
                    var pos = new Vector3((i - cx) * Pitch, 2f + j * Pitch, Z);
                    float lit = 0.06f + v;                           // a faint grid even where dark
                    _cells.Set(j * W + i, pos, 0.55f + 0.65f * v, 0.1f * _cur + i * 0.012f + j * 0.008f, gain * lit * 0.9f, face);
                }
            rig.Orbit(cam, rx, 20f, 11f, 12f, dt, 0.4f);
        }
    }
}
