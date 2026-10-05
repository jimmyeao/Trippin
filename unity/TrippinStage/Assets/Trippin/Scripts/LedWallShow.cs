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
//  - Owner ("not reactive in some displays, only on the VU meter"): every
//    pattern now changes SHAPE with the music, not just brightness - ripples
//    are rings launched by the kicks, the plasma is poured into a liquid
//    spectrum (each column filled to its band's height), and the diamond
//    rings tighten on the kicks and stretch with the highs.
using UnityEngine;

namespace TrippinStage
{
    public sealed class LedWallShow : KitShow
    {
        const int W = 36, H = 20, Patterns = 4;
        const float Pitch = 1.0f, Z = 10f;
        GlowPool _cells;
        int _cur = -1, _prev;
        float _start, _kickPrev;
        // Kick rings: a new kick takes the most-faded slot (AGENTS.md: or one slow ring fills them all).
        readonly float[] _ringR = { 99f, 99f, 99f, 99f }, _ringA = new float[4];

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
                case 1: // ripples: a ring out from the centre on every kick, over a faint swell
                {
                    float d = Mathf.Sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy) * 1.4f);
                    float v = 0.3f * (0.5f + 0.5f * Mathf.Sin(d * 0.9f - rx.clk * 0.6f)) * (0.4f + rx.bassFast);
                    for (int k = 0; k < _ringR.Length; k++)
                    {
                        float e = (d - _ringR[k]) / 2.0f;
                        v += _ringA[k] * Mathf.Exp(-e * e);
                    }
                    return Mathf.Clamp01(v);
                }
                case 2: // liquid spectrum: plasma poured into each column up to its band's height
                {
                    float v = 0.5f + 0.25f * Mathf.Sin(x * 0.5f + rx.clk * 0.2f) + 0.25f * Mathf.Sin(y * 0.6f - rx.clk * 0.15f + Mathf.Sin(x * 0.3f));
                    float bin = Mathf.Abs(x - cx) / cx;                       // mirrored: bass in the middle
                    float level = (0.12f + 1.1f * rx.Spec(0.05f + 0.85f * bin)) * H;
                    float fill = Mathf.Clamp01((level - y) / 2.5f);           // soft surface
                    return Mathf.Clamp01((v - 0.15f) * (1.2f + 1.5f * rx.midFast)) * fill;
                }
                default: // diamond rings: tighten on the kicks, stretch with the highs
                {
                    float d = Mathf.Abs(x - cx) + Mathf.Abs(y - cy);
                    float k = 0.7f * (1f + 0.6f * rx.kick) / (1f + 0.5f * rx.highFast);
                    float w = 0.5f + 0.5f * Mathf.Sin(d * k - rx.clk * 0.5f);
                    return w * w * (0.45f + 0.6f * rx.highFast);
                }
            }
        }

        protected override void Frame(ShowState s, float dt)
        {
            int p = (Mathf.FloorToInt(rx.beatS / 4f) / 4) % Patterns;
            if (_cur < 0) { _cur = _prev = p; _start = rx.beatS - 4f; }
            else if (p != _cur) { _prev = _cur; _cur = p; _start = rx.beatS; }
            float m = Mathf.SmoothStep(0f, 1f, rx.beatS - _start);
            if (rx.kick > 0.55f && _kickPrev <= 0.55f)
            {
                int slot = 0;
                for (int k = 1; k < _ringA.Length; k++) if (_ringA[k] < _ringA[slot]) slot = k;
                _ringR[slot] = 0f; _ringA[slot] = 1.0f + 0.6f * rx.bassFast;
            }
            _kickPrev = rx.kick;
            for (int k = 0; k < _ringA.Length; k++) { _ringR[k] += 12f * dt; _ringA[k] *= Mathf.Exp(-0.7f * dt); }
            float gain = rx.Gain(0.5f, 1.4f);
            float cx = (W - 1) * 0.5f, cy = (H - 1) * 0.5f;
            Quaternion face = Quaternion.identity; // the wall faces the audience (-z)
            for (int j = 0; j < H; j++)
                for (int i = 0; i < W; i++)
                {
                    float v = Mathf.Lerp(Pattern(_prev, i, j, cx, cy), Pattern(_cur, i, j, cx, cy), m);
                    var pos = new Vector3((i - cx) * Pitch, 2f + j * Pitch, Z);
                    float lit = 0.06f + v;                           // a faint grid even where dark
                    _cells.Set(j * W + i, pos, 0.55f + 0.65f * v, 0.1f * _cur + i * 0.012f + j * 0.008f, gain * lit * 1.4f, face);
                }
            rig.Orbit(cam, rx, 20f, 11f, 12f, dt, 0.4f);
        }
    }
}
