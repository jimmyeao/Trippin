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
        [TrippinParam("Colour", 0f, 1f)] public float hue = 0.55f;
        [TrippinParam("Colour drift", 0f, 0.4f)] public float colourDrift = 0.1f;
        [TrippinParam("Spin", 0f, 3f)] public float spin = 1f;
        [TrippinParam("Camera sway", 0f, 2f)] public float sway = 1f;

        Material _m;

        protected override void Build()
        {
            _m = Kit.Fullscreen(transform, deepMat, "deep space");
        }

        protected override void Frame(ShowState s, float dt)
        {
            _m.SetFloat("_Gain", rx.Gain(0.6f, 1.5f));
            _m.SetFloat("_Hue", hue + colourDrift * Mathf.Sin(rx.phrase * 0.5f));
            _m.SetFloat("_Spin", 0.2f * spin * Mathf.Sin(rx.phrase * 0.5f));
            _m.SetFloat("_Zoom", 1f + 0.4f * rx.tension);
            _m.SetFloat("_Drift", rx.clk);
            float sw = Mathf.Sin(rx.phrase * 0.5f) * sway;
            rig.Move(cam, new Vector3(sw * 60f, 10f + 20f * sway * Mathf.Sin(rx.phrase * 0.3f), -20f + 30f * Mathf.Sin(rx.beat / 256f * Mathf.PI * 2f)), new Vector3(0f, 12f, 60f), dt, 0.5f);
        }
    }
}
