// unity_pendulum: a pendulum wave. 28 pendulums hang from a rail, each
// swinging a few more times per cycle than the last, so the line of bobs
// weaves into travelling waves, then snaps back into step and starts again.
//  - Shape: the swing amplitude follows the eased bass and the kick.
//  - Motion: the phase runs on the smooth energy clock (the wave speeds up
//    with the track), with a phrase-length drift in the starting phase.
//  - Luminance: each bob glows with the eased level of its own band.
//  - Drops: a build stills the swing dim; the drop throws it wide.
using UnityEngine;

namespace TrippinStage
{
    public sealed class PendulumShow : KitShow
    {
        const int N = 28;
        const float Rod = 14f, Top = 22f;
        BeamPool _rods;
        GlowPool _bobs;

        protected override void Build()
        {
            Env();
            _rods = new BeamPool(transform, beamMat, N + 1, "rod", true, 0.035f);
            _bobs = new GlowPool(transform, glowMat, N, "bob", true);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain();
            float amp = (0.35f + 0.55f * rx.bassFast + 0.2f * rx.kick) * (1f - 0.6f * rx.tension) * (1f + 0.8f * rx.impact);
            // One full wave cycle per 96 clock beats (~45 s at a steady groove; faster as the track builds).
            float basePhase = Mathf.PI * 2f * rx.clk / 96f + 0.4f * Mathf.Sin(rx.phrase);
            _rods.Set(N, new Vector3(-16f, Top, 0f), new Vector3(16f, Top, 0f), Kit.Hue(0.6f), gain * 0.8f, 0.07f);
            for (int i = 0; i < N; i++)
            {
                float x = (i - (N - 1) * 0.5f) * 1.15f;
                float th = amp * Mathf.Sin(basePhase * (8 + i));
                var pivot = new Vector3(x, Top, 0f);
                var bob = pivot + new Vector3(0f, -Rod * Mathf.Cos(th), Rod * Mathf.Sin(th));
                float band = rx.Spec(i / (N - 1f));
                float h = 0.05f + i * 0.026f;
                _rods.Set(i, pivot, bob, Kit.Hue(h), gain * (0.4f + 0.5f * band));
                _bobs.Set(i, bob, 1.5f + 1.2f * band, h, gain * (0.7f + 1.1f * band), cam.transform.rotation);
            }
            rig.Orbit(cam, rx, 36f, 11f, 12f, dt, 0.9f);
        }
    }
}
