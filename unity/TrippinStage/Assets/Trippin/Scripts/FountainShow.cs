// unity_fountain: three fountains of light throwing 360 glowing drops in
// arcs, reflected in a wet floor.
//  - Shape: each drop's launch speed (so the fountain's height and spread)
//    follows the eased bass and mids at the moment it leaves the nozzle; the
//    drop then flies a plain ballistic arc, so the spray is smooth by
//    construction. The nozzles lean with the phrase.
//  - Motion: gravity and flight time are real time; the lean direction
//    swings with the phrase.
//  - Luminance: drop glow follows the music's loudness.
//  - Drops: a build lowers the fountains dim; the drop sends a surge of drops
//    much higher (eased).
using UnityEngine;

namespace TrippinStage
{
    public sealed class FountainShow : KitShow
    {
        const int N = 360;
        const float G = 12f;
        GlowPool _drops;
        readonly Vector3[] _p0 = new Vector3[N], _v = new Vector3[N];
        readonly float[] _age = new float[N], _life = new float[N], _size = new float[N];
        readonly bool[] _live = new bool[N];
        uint _seed = 2463534242u;

        float R()
        {
            _seed ^= _seed << 13; _seed ^= _seed >> 17; _seed ^= _seed << 5;
            return (_seed & 0xffffffu) / 16777216f;
        }

        protected override void Build()
        {
            Env(150f, 0.06f);
            _drops = new GlowPool(transform, glowMat, N, "drop", true);
            for (int i = 0; i < N; i++) { _life[i] = 2.2f + 0.8f * R(); _age[i] = -R() * _life[i]; _size[i] = 0.5f + 0.6f * R(); }
        }

        void Respawn(int i)
        {
            int nozzle = i % 3;
            _p0[i] = new Vector3((nozzle - 1) * 9f, 0.3f, 0f);
            float az = R() * Mathf.PI * 2f;
            float elev = (68f + 17f * R()) * Mathf.Deg2Rad;
            float speed = (10f + 9f * rx.bassFast + 6f * rx.midFast + 7f * rx.impact) * (0.8f + 0.4f * R());
            float lean = 5f * Mathf.Sin(rx.phrase + nozzle * 1.4f);
            _v[i] = new Vector3(Mathf.Cos(az) * Mathf.Cos(elev) * speed * 0.5f + lean, Mathf.Sin(elev) * speed, Mathf.Sin(az) * Mathf.Cos(elev) * speed * 0.5f);
            _life[i] = 2.2f + 0.8f * R();
            _age[i] = 0f;
            _size[i] = 0.5f + 0.6f * R();
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.55f, 1.4f);
            Quaternion face = cam.transform.rotation;
            float scale = 1f - 0.4f * rx.tension;
            for (int i = 0; i < N; i++)
            {
                _age[i] += dt;
                if (!_live[i])
                {
                    // Still waiting to launch (staggered start): hidden until its time comes.
                    if (_age[i] < 0f) { _drops.Set(i, Vector3.zero, 0f, 0f, 0f, face); continue; }
                    Respawn(i);
                    _live[i] = true;
                }
                else if (_age[i] > _life[i]) Respawn(i);
                float t = _age[i];
                Vector3 pos = _p0[i] + _v[i] * (t * scale) + new Vector3(0f, -0.5f * G * t * t * scale, 0f);
                float f = t / _life[i];
                if (pos.y < 0.1f) { _drops.Set(i, Vector3.zero, 0f, 0f, 0f, face); continue; }
                float fade = Mathf.Clamp01(f * 8f) * (1f - f * f);
                _drops.Set(i, pos, _size[i] * (0.6f + 0.8f * rx.lum) * (1f - 0.4f * f), 0.1f + (i % 3) * 0.15f + f * 0.2f, gain * fade * 1.2f, face);
            }
            rig.Orbit(cam, rx, 32f, 9f, 9f, dt);
        }
    }
}
