// unity_nebula: a drifting deep-space nebula, drawn as a full-screen pass from
// the view direction by the DeepSpace shader (no sheets, so no edges): three
// domain-warped smoke layers at different parallax depths, a bright core, a
// faint galactic band and two layers of stars; the camera drifts through them.
//  - Shape: the smoke is stirred by the eased bass and mids (the warp strength
//    bends the filaments), and each depth layer is tied to one slice of the
//    spectrum, so the nebula's structure is a live spectrum.
//  - Motion: the layers slide through the warp field on the smooth energy
//    clock, the whole sky yaws with the phrase, and the camera sways, which
//    shifts the layers against each other (parallax).
//  - Luminance: everything follows the music's loudness; stars swell with the
//    eased highs.
//  - Drops: a build contracts the sky and dims it; the drop opens a ring from
//    the core and floods it (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class NebulaShow : KitShow
    {
        Material _m;

        protected override void Build()
        {
            _m = Kit.Fullscreen(transform, deepMat, "deep space");
        }

        protected override void Frame(ShowState s, float dt)
        {
            _m.SetFloat("_Gain", rx.Gain(0.6f, 1.5f));
            _m.SetFloat("_Hue", 0.55f + 0.1f * Mathf.Sin(rx.phrase * 0.5f));
            _m.SetFloat("_Spin", 0.2f * Mathf.Sin(rx.phrase * 0.5f));
            _m.SetFloat("_Zoom", 1f + 0.4f * rx.tension);
            _m.SetFloat("_Drift", rx.clk);
            float sway = Mathf.Sin(rx.phrase * 0.5f);
            rig.Move(cam, new Vector3(sway * 60f, 10f + 20f * Mathf.Sin(rx.phrase * 0.3f), -20f + 30f * Mathf.Sin(rx.beat / 256f * Mathf.PI * 2f)), new Vector3(0f, 12f, 60f), dt, 0.5f);
        }
    }
}
