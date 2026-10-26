// unity_synthwave: the HQ version of the 2D synthwave scene: flying low
// down a neon road through gridded hills toward a striped retro sun (an
// Alice-generated sky on the horizon), the land dark glass reflecting the
// sunset (HqSynth.shader).
//  - Motion: the flight speed rides the smooth energy clock; the camera
//    banks and drifts across the road, swinging back with the phrase.
//  - Shape/light: each kick sends a pulse of light rolling down the grid
//    toward the camera; the bass swells the grid's glow; breakdowns slow
//    the flight to a cruise. No beat flashes (a flight scene).
// Controls: speed, hills, grid glow, grid size, pulses, sun glow, bank.
using UnityEngine;

namespace TrippinStage
{
    public sealed class SynthwaveShow : KitShow
    {
        public Material synthMat, skyMat;

        [TrippinParam("Speed", 0.2f, 3f)] public float speed = 1f;
        [TrippinParam("Hills", 0f, 2.5f)] public float hills = 1f;
        [TrippinParam("Grid glow", 0.2f, 3f)] public float glow = 1f;
        [TrippinParam("Grid size", 4f, 16f)] public float cell = 8f;
        [TrippinParam("Pulses", 0f, 2f)] public float pulses = 1f;
        [TrippinParam("Sun glow", 0.3f, 2.5f)] public float sun = 1f;
        [TrippinParam("Bank", 0f, 2f)] public float bank = 1f;

        Material _m, _sky, _grad;
        readonly ClkAcc _fly = new ClkAcc();
        float _kickPrev, _glow;
        Vector4 _pulse = new Vector4(-99f, -99f, -99f, -99f);
        int _pulses;

        protected override void Build()
        {
            _m = new Material(synthMat);
            Kit.Part(transform, "land", Kit.GridMesh(300, 360, "land"), _m, new Vector3(0f, 0f, -20f), Vector3.one);
            // Behind the sunset picture, a gradient sky its edges fade into.
            _grad = Kit.Fullscreen(transform, horizonMat, "sky gradient");
            _grad.SetFloat("_SkyMode", 0f);
            _grad.SetFloat("_SunH", -0.3f);
            _sky = new Material(skyMat);
            _sky.SetFloat("_URange", 1f);
            _sky.SetFloat("_EdgeFade", 0.22f);
            HqBackdrops.Make(transform, _sky, null, new Vector3(0f, -95f, 860f), 1250f, 1250f * 768f / 1344f);
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            float fly = _fly.Step(rx.clk, speed) * 7f;
            _m.SetFloat("_Scroll", fly);

            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && rx.calm < 0.6f;
            _kickPrev = rx.kick;
            if (kick && pulses > 0f) { _pulse[_pulses++ & 3] = 420f; }
            for (int k = 0; k < 4; k++) _pulse[k] -= dt * 160f;
            _m.SetVector("_Pulse", pulses > 0f ? _pulse : Vector4.one * -999f);

            _glow = Eased.Follow(_glow, 0.8f + 0.7f * rx.bassFast, 10f, 3f, dt);
            float gain = rx.Gain(0.85f, 1.2f);
            _m.SetFloat("_Hills", hills);
            _m.SetFloat("_Grid", glow * _glow);
            _m.SetFloat("_Cell", cell);
            Color pink = Kit.Hue(0.0f), cyan = Kit.Hue(0.55f);
            _m.SetColor("_LineA", pink * 1.4f);
            _m.SetColor("_LineB", cyan * 1.2f);
            _m.SetColor("_FogCol", new Color(0.45f, 0.1f, 0.32f) * (0.7f + 0.3f * sun));
            _m.SetColor("_SunCol", new Color(1.3f, 0.55f, 0.3f) * sun);
            _m.SetFloat("_Gain", gain);
            _sky.SetFloat("_Gain", 0.9f * sun * (0.85f + 0.2f * rx.lum));
            _sky.SetFloat("_Lit", 0.6f);
            _sky.SetFloat("_Haze", 0f);
            _grad.SetFloat("_SkyHue", 0.92f);
            _grad.SetFloat("_SkyGain", 0.55f * sun);
            _grad.SetFloat("_SkyClk", rx.clk);

            // Low over the road, drifting across it and banking into the drift.
            float sway = Mathf.Sin(rx.phrase * 0.5f) * bank;
            var pos = new Vector3(sway * 7f, 3.2f + 0.8f * Mathf.Sin(rx.phrase * 0.25f), 0f);
            rig.Move(cam, pos, pos + new Vector3(sway * 4f, 6f, 120f), dt, 1.2f);
            cam.transform.rotation = cam.transform.rotation * Quaternion.AngleAxis(-4f * sway, Vector3.forward);
        }
    }
}
