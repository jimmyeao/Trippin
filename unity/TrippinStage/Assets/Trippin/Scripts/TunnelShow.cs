// unity_tunnel: a flight down a snaking sci-fi corridor of lit panels toward a
// light at the end. A flight scene: no beat-synced flashes (AGENTS.md);
// luminance follows eased levels only. Drawn as a full-screen pass by the
// Corridor shader (no geometry).
//  - Shape: the ribs along the corridor deepen with the eased bass (it breathes
//    on the low end), and the wall panels are a live spectrum wrapped round the
//    circumference (each sector lit by its own band).
//  - Motion: the flight speed is the smooth energy clock (it surges on a
//    drop); the corridor's bends swing with the phrase, and the camera rolls
//    with it; light packets run down the strips on the same clock.
//  - Luminance: the panels, rings and the light at the end follow the music's
//    loudness.
//  - Drops: a build narrows it dim; the drop blasts it wide open (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class TunnelShow : KitShow
    {
        Material _m;

        protected override void Build()
        {
            _m = Kit.Fullscreen(transform, corridorMat, "corridor");
        }

        protected override void Frame(ShowState s, float dt)
        {
            _m.SetFloat("_Gain", rx.Gain(0.6f, 2.2f));
            _m.SetFloat("_Radius", 9f * (1f - 0.3f * rx.tension) * (1f + 0.35f * rx.impact));
            _m.SetFloat("_Scroll", rx.clk * 5f);
            _m.SetFloat("_Phase", rx.clk * 0.4f);
            _m.SetFloat("_Rib", 0.08f + 0.3f * rx.bassFast + 0.1f * rx.impact);
            _m.SetFloat("_BendAmp", 3.5f * (0.6f + 0.4f * Mathf.Sin(rx.phrase)));
            _m.SetFloat("_BendPhase", rx.clk * 0.01f + Mathf.Sin(rx.phrase));
            _m.SetFloat("_Hue", 0.5f + 0.1f * Mathf.Sin(rx.phrase * 0.5f));
            rig.Move(cam, Vector3.zero, new Vector3(0f, 0f, 40f), dt, 8f);
            cam.transform.rotation = cam.transform.rotation * Quaternion.AngleAxis(6f * Mathf.Sin(rx.phrase), Vector3.forward);
        }
    }
}
