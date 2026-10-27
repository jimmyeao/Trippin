// unity_gold: molten gold pouring into a pool in a dark crucible (HqGold.shader,
// one ray-marched full-screen pass), a mirror of a warm studio glowing with
// its own heat.
//  - Shape: each kick drops a droplet that sends a ring rippling out across
//    the pool (eight slots, a new one taking the most-faded); the bass
//    thickens the pour and the mids make it wobble; a build swells the pool.
//  - Motion: the stream wanders slowly with the phrase; the heat breathes
//    with the music's level (the glow, the least of it); the camera drifts
//    through four angles, cutting every 8 bars.
// Controls: pour, ripples, heat, drift.
using UnityEngine;

namespace TrippinStage
{
    public sealed class GoldShow : KitShow
    {
        public Material goldMat;

        [TrippinParam("Pour", 0.3f, 2f)] public float pour = 1f;
        [TrippinParam("Ripples", 0f, 2f)] public float ripples = 1f;
        [TrippinParam("Heat", 0.3f, 2f)] public float heat = 1f;
        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;

        Material _m;
        readonly Vector4[] _rings = new Vector4[8];
        float _t, _bass, _mid, _kickPrev;
        int _shot = -1, _drops;

        protected override void Build()
        {
            _m = Kit.Fullscreen(transform, goldMat, "molten gold");
            for (int i = 0; i < 8; i++) _rings[i] = new Vector4(0f, 0f, -99f, 0f);
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            _t += dt;
            _bass = Eased.Follow(_bass, rx.bassFast, 12f, 3f, dt);
            _mid = Eased.Follow(_mid, rx.midFast, 12f, 3f, dt);
            float sw = Mathf.Sin(rx.phrase * 0.5f);
            var stream = new Vector2(-1.2f + 1.4f * sw, 0.8f + 0.6f * Mathf.Cos(rx.phrase * 0.35f));
            // A droplet on each kick: a ring from a spot near the stream, into the most-faded slot.
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && rx.calm < 0.5f;
            _kickPrev = rx.kick;
            if (kick && ripples > 0f)
            {
                int best = 0; float oldest = float.MaxValue;
                for (int i = 0; i < 8; i++) if (_rings[i].z < oldest) { oldest = _rings[i].z; best = i; }
                _drops++;
                float a = Kit.H(_drops, 1) * Mathf.PI * 2f, r = 1.5f + 4.5f * Kit.H(_drops, 2);
                _rings[best] = new Vector4(stream.x + Mathf.Cos(a) * r, stream.y + Mathf.Sin(a) * r, _t, ripples * (0.6f + 0.6f * rx.kick));
            }
            _m.SetVectorArray("_Ring", _rings);
            _m.SetVector("_Stream", new Vector4(stream.x, stream.y, (0.22f + 0.16f * _bass) * pour, _mid));
            _m.SetFloat("_T", _t);
            _m.SetFloat("_Swell", 0.4f + 1.6f * rx.tension);
            _m.SetFloat("_Heat", heat * (0.85f + 0.3f * rx.lum));
            _m.SetFloat("_Hue", rx.beatS / 512f);
            _m.SetFloat("_Gain", rx.Gain(0.9f, 1.15f));

            // Shots: one per 8 bars.
            float b = rx.beatS;
            int sb = Mathf.FloorToInt(b / 32f);
            if (sb != _shot) { _shot = sb; rig.Snap(); }
            float u = Mathf.Repeat(b, 32f) / 32f;
            float d = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * drift;
            var land = new Vector3(stream.x, 0.2f, stream.y);
            Vector3 pos, look;
            switch (((sb % 4) + 4) % 4)
            {
                default:
                case 0: look = land + new Vector3(0f, 1.2f, 0f); pos = land + new Vector3(2.5f * d - 1f, 2.2f, -7.5f); break;
                case 1: look = land; pos = land + new Vector3(-3.2f - d, 0.7f, -3.2f); break;
                case 2: look = new Vector3(0f, 0f, 1f); pos = new Vector3(2f * d, 13f, -6f); break;
                case 3: look = land + new Vector3(0.6f, 0.3f, 0f); pos = land + new Vector3(4.5f + d, 1.0f, -4.5f); break;
            }
            rig.Move(cam, pos, look, dt, 3f);
        }
    }
}
