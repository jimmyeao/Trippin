// unity_tunnel: a flight down a snaking wire tunnel toward a light at the end.
// A flight scene: no beat-synced flashes (AGENTS.md); luminance follows eased
// levels only.
//  - Shape: the tunnel's radius wobbles with the eased bass (it breathes on the
//    low end) and a travelling pulse of light runs along it with the energy
//    clock.
//  - Motion: the flight speed is the smooth energy clock (it surges on a
//    drop); the tunnel's bends swing with the phrase, and the camera rolls
//    with it.
//  - Luminance: the lines and the light at the end follow the music's loudness.
//  - Drops: a build narrows it dim; the drop blasts it wide open (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class TunnelShow : KitShow
    {
        Material _m;
        GlowPool _end;

        protected override void Build()
        {
            Mesh mesh = Kit.GridMesh(96, 220, "tube");
            _m = new Material(tubeMat);
            Kit.Part(transform, "tube", mesh, _m, Vector3.zero, Vector3.one);
            _end = new GlowPool(transform, glowMat, 2, "light", false);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.35f, 1.6f);
            _m.SetFloat("_Radius", 9f * (1f - 0.3f * rx.tension) * (1f + 0.35f * rx.impact));
            _m.SetFloat("_Scroll", rx.clk * 5f);
            _m.SetFloat("_Phase", rx.clk * 0.4f);
            _m.SetFloat("_Wob", 0.12f + 0.35f * rx.bassFast);
            _m.SetFloat("_BendAmp", 3.5f * (0.6f + 0.4f * Mathf.Sin(rx.phrase)));
            _m.SetFloat("_BendPhase", rx.clk * 0.01f + Mathf.Sin(rx.phrase));
            _m.SetFloat("_Intensity", 0.55f * gain);
            _m.SetFloat("_Hue", 0.5f + 0.1f * Mathf.Sin(rx.phrase * 0.5f));
            Quaternion face = Quaternion.identity;
            _end.Set(0, new Vector3(0f, 0f, 190f), 50f * (1f + 0.3f * rx.bassFast), 0.1f, gain * 0.8f, face);
            _end.Set(1, new Vector3(0f, 0f, 189f), 24f, 0.0f, gain * 1.2f, face);
            rig.Move(cam, Vector3.zero, new Vector3(0f, 0f, 40f), dt, 8f);
            cam.transform.rotation = cam.transform.rotation * Quaternion.AngleAxis(5f * Mathf.Sin(rx.phrase), Vector3.forward);
        }
    }
}
