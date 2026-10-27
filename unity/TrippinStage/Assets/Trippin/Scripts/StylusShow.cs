// unity_stylus: an extreme macro of the stylus riding a record groove
// (HqGroove.shader, one ray-marched full-screen pass; world units are 100 µm).
//  - Shape: the grooves' walls swing sideways with the music: the bass cuts
//    the big slow swings, the mids and highs smaller faster ones (each band's
//    amplitude eased from the spectrum), and the diamond tip follows its
//    groove, so it shakes with the sound.
//  - Motion: the record turns under the stylus on the smooth energy clock (in
//    slow motion: the real groove passes at 35 cm a second); the rainbow sheen
//    on the walls slides with the palette; the camera drifts through four
//    angles, cutting every 8 bars.
// Controls: speed, groove swing, sheen, dust, drift.
using UnityEngine;

namespace TrippinStage
{
    public sealed class StylusShow : KitShow
    {
        public Material grooveMat;

        [TrippinParam("Speed", 0.2f, 3f)] public float speed = 1f;
        [TrippinParam("Groove swing", 0f, 2f)] public float swing = 1f;
        [TrippinParam("Sheen", 0f, 2f)] public float sheen = 1f;
        [TrippinParam("Dust", 0f, 3f)] public float dust = 1f;
        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;

        Material _m;
        readonly ClkAcc _clk = new ClkAcc();
        readonly float[] _a = new float[6];
        int _shot = -1;

        protected override void Build()
        {
            _m = Kit.Fullscreen(transform, grooveMat, "groove");
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            float c = _clk.Step(rx.clk, speed);
            // Band amplitudes: lateral swing in 100 µm units, eased so the groove shape breathes.
            float[] want =
            {
                0.42f * rx.bassFast, 0.26f * rx.Spec(0.12f), 0.15f * rx.Spec(0.3f),
                0.09f * rx.Spec(0.5f), 0.05f * rx.Spec(0.7f), 0.03f * rx.highFast,
            };
            for (int i = 0; i < 6; i++) _a[i] = Eased.Follow(_a[i], want[i] * swing, 16f, 3.5f, dt);
            _m.SetVector("_Amp", new Vector4(_a[0], _a[1], _a[2], _a[3]));
            _m.SetVector("_Amp2", new Vector4(_a[4], _a[5], 0f, 0f));
            _m.SetFloat("_Scroll", c * 4f);
            _m.SetFloat("_Pitch", 1.2f);
            _m.SetFloat("_Depth", 0.35f);
            _m.SetFloat("_Dust", dust);
            _m.SetFloat("_Sheen", sheen);
            _m.SetFloat("_Hue", rx.beatS / 256f);
            var key = Color.Lerp(new Color(1f, 0.92f, 0.82f), Kit.Hue(rx.beatS / 512f), 0.25f) * 1.6f * rx.Gain(0.9f, 1.15f);
            var rim = Kit.Hue(rx.beatS / 512f + 0.4f) * 1.4f;
            _m.SetVector("_Key", new Vector4(-20f, 25f, -12f, 0f)); _m.SetColor("_KeyCol", key);
            _m.SetVector("_Rim", new Vector4(16f, 9f, 22f, 0f)); _m.SetColor("_RimCol", rim);
            _m.SetFloat("_Gain", 1f);

            // Shots: one per 8 bars.
            float b = rx.beatS;
            int sb = Mathf.FloorToInt(b / 32f);
            if (sb != _shot) { _shot = sb; rig.Snap(); }
            float u = Mathf.Repeat(b, 32f) / 32f;
            float sw = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * drift;
            Vector3 pos, look;
            switch (((sb % 4) + 4) % 4)
            {
                default:
                case 0: pos = new Vector3(1.5f * sw, 1.7f, -9f); look = new Vector3(0f, 0.5f, 2f); break;
                case 1: pos = new Vector3(-8.5f, 2.6f, 3f + 1.5f * sw); look = new Vector3(0f, 1.6f, 3f); break;
                case 2: pos = new Vector3(3f * sw, 11f, -7f); look = new Vector3(0f, 0f, 3f); break;
                case 3: pos = new Vector3(1.3f + 0.4f * sw, 0.7f, -3.4f); look = new Vector3(0f, 0.25f, 0f); break;
            }
            rig.Move(cam, pos, look, dt, 3f);
        }
    }
}
