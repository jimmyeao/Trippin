// unity_rain: the HQ version of the 2D city rain scene: a neon street at
// night seen through a rain-streaked window (HqRain.shader, an Alice-
// generated alley behind fogged glass, drops that refract it).
//  - Motion: the drops run faster and more of them break away as the music
//    drives (the glass's clock runs on the eased energy); the view drifts
//    slowly with the phrase, turning back the other way.
//  - Shape: the bass swells the condensation's clearing (eased) and the
//    street's neon brightens with the energy; breakdowns slow the rain to
//    a few sliding drops on misted glass.
// Controls: rain, droplets, condensation, background blur, neon, drift.
using UnityEngine;

namespace TrippinStage
{
    public sealed class RainShow : KitShow
    {
        public Material rainMat;

        [TrippinParam("Rain", 0.2f, 3f)] public float rain = 1f;
        [TrippinParam("Droplets", 0f, 2f)] public float droplets = 1f;
        [TrippinParam("Condensation", 0f, 1.5f)] public float fog = 1f;
        [TrippinParam("Background blur", 1f, 7f)] public float blur = 4.5f;
        [TrippinParam("Neon", 0.2f, 2.5f)] public float neon = 1f;
        [TrippinParam("Drift", 0f, 3f)] public float drift = 1f;

        public override bool UsesCamera => false;

        Material _m;
        float _t, _run, _pan;

        protected override void Build()
        {
            _m = Kit.Fullscreen(transform, rainMat, "rain");
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            // The glass's own clock: faster with the energy (eased), slow in breakdowns.
            _run = Eased.Follow(_run, (0.55f + 0.9f * rx.lum + 0.4f * rx.bassFast) * (1f - 0.6f * rx.calm), 2f, 1f, dt);
            _t += dt * (0.6f + 0.8f * _run);
            _pan += dt * 0.004f * drift * Mathf.Sin(rx.phrase * 0.5f);
            _m.SetFloat("_T", _t);
            _m.SetFloat("_Run", rain * (0.6f + _run));
            _m.SetFloat("_Static", droplets);
            _m.SetFloat("_Fog", fog * (1f - 0.3f * rx.bassFast));
            _m.SetFloat("_Blur", blur);
            _m.SetFloat("_Lights", neon * (0.6f + 0.7f * rx.lum + 0.3f * rx.highFast));
            _m.SetVector("_Pan", new Vector4(Mathf.Sin(_pan * 6f) * 0.05f, Mathf.Cos(_pan * 4f) * 0.02f, 0f, 0f));
            _m.SetFloat("_Gain", rx.Gain(0.85f, 1.15f));
        }
    }
}
