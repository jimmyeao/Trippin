// unity_spectrum: three concentric rings of vertical light bars, the live
// spectrum wrapped round a circle (mirrored left to right), with the two
// outer rings showing the spectrum from a fraction of a second ago, so the
// music ripples outward as a spectrogram.
//  - Shape: bar height IS the eased spectrum (bass at the top of the ring,
//    highs at the bottom), so the figure is a live portrait of the mix.
//  - Motion: the rings counter-rotate on the smooth energy clock, swinging
//    direction with the phrase.
//  - Luminance: each bar glows with the level of its own bin.
//  - Drops: a build draws the rings in dim; the drop flares them (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class SpectrumShow : KitShow
    {
        const int Rings = 3, Bars = 64, Hist = 16;
        static readonly float[] Radius = { 7f, 11f, 15f };
        BeamPool _beams;
        readonly float[][] _hist = new float[Hist][];
        int _head;
        float _stamp;

        protected override void Build()
        {
            Env();
            _beams = new BeamPool(transform, beamMat, Rings * Bars, "bar", true, 0.1f, 0.0f, 24f, 0.4f);
            for (int i = 0; i < Hist; i++) _hist[i] = new float[32];
        }

        protected override void Frame(ShowState s, float dt)
        {
            // Snapshot the eased spectrum every 0.1 s into a ring buffer.
            if (Time.time - _stamp > 0.1f)
            {
                _stamp = Time.time;
                _head = (_head + 1) % Hist;
                for (int i = 0; i < 32; i++) _hist[_head][i] = rx.spec[i];
            }
            float gain = rx.Gain(0.55f, 1.3f);
            float tight = (1f - 0.35f * rx.tension) * (1f + 0.4f * rx.impact);
            int n = 0;
            for (int r = 0; r < Rings; r++)
            {
                float[] src = r == 0 ? rx.spec : _hist[((_head - r * 3) % Hist + Hist) % Hist];
                float spin = (rx.clk * 0.015f * (r % 2 == 0 ? 1f : -1f) + 0.4f * Mathf.Sin(rx.phrase + r)) ;
                for (int i = 0; i < Bars; i++)
                {
                    int j = i < 32 ? i : 63 - i;
                    float lvl = Mathf.Pow(Mathf.Clamp01(src[j]), 0.8f);
                    float a = i / (float)Bars * Mathf.PI * 2f + spin;
                    float rad = Radius[r] * tight;
                    var foot = new Vector3(Mathf.Cos(a) * rad, 0.3f, Mathf.Sin(a) * rad);
                    var top = foot + new Vector3(0f, 0.4f + 14f * lvl * tight, 0f);
                    _beams.Set(n++, foot, top, Kit.Hue(0.02f + j / 31f * 0.8f), gain * (0.25f + 1.4f * lvl) * (1f - 0.18f * r));
                }
            }
            rig.Orbit(cam, rx, 36f, 14f, 5f, dt);
        }
    }
}
