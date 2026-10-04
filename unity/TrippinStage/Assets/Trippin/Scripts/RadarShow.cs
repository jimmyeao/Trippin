// unity_radar: a radar scope. A sweep with phosphor persistence turns across a
// disc of range rings; a polar spectrum plot around it lights up as the sweep
// passes, contacts glow where it has just been, rings expand from the hub on
// kicks, and a wall of light bars stands round the rim as a 3D spectrum.
// (The first version only drew a faint grid; this one is the Scope shader.)
//  - Shape: the polar plot's bars and the wall of light round the rim rise and
//    fall with the eased spectrum; each contact swells with its own band; the
//    hub swells with the eased bass.
//  - Motion: the sweep is locked to the bars (one revolution per four bars, back
//    at north on the downbeat); the camera orbits above and swings
//    direction with the phrase; kick rings expand outward at a fixed speed.
//  - Luminance: everything follows the music's loudness.
//  - Drops: a build dims the scope and shrinks the wall; the drop sweeps it
//    bright and throws the wall up (eased); a drop also launches a ring.
using UnityEngine;

namespace TrippinStage
{
    public sealed class RadarShow : KitShow
    {
        const int Bars = 72;
        const float Radius = 24f;
        Material _m;
        BeamPool _bars;
        readonly float[] _ringR = { -1f, -1f, -1f, -1f };
        readonly float[] _ringK = new float[4];
        bool _armed = true, _wasDrop;

        protected override void Build()
        {
            Env(140f, 0.04f, false);
            _m = Kit.Part(transform, "scope", Kit.GridMesh(1, 1, "scope"), new Material(scopeMat), Vector3.zero, Vector3.one)
                .GetComponent<Renderer>().sharedMaterial;
            _m.SetFloat("_Radius", Radius);
            _m.SetFloat("_Extent", Radius * 1.12f);
            _bars = new BeamPool(transform, beamMat, Bars, "bar", false, 0.16f, 0f, 14f, 0.35f, 0.97f, 0.25f);
        }

        // Launch a ring into the most-faded slot (so a busy track still shows new ones).
        void Spawn()
        {
            int best = 0; float oldest = -2f;
            for (int i = 0; i < 4; i++)
            {
                float age = _ringR[i] < 0f ? 9f : _ringR[i];
                if (age > oldest) { oldest = age; best = i; }
            }
            _ringR[best] = 0.04f;
            _ringK[best] = 1f;
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.6f, 1.5f);
            if (rx.kick > 0.55f && _armed) { Spawn(); _armed = false; }
            else if (rx.kick < 0.3f) _armed = true;
            if (rx.dropped && !_wasDrop) Spawn();
            _wasDrop = rx.dropped;
            for (int i = 0; i < 4; i++)
            {
                if (_ringR[i] < 0f) { _ringK[i] = 0f; continue; }
                _ringR[i] += dt * 0.5f;
                if (_ringR[i] > 1f) { _ringR[i] = -1f; _ringK[i] = 0f; continue; }
                _ringK[i] = Mathf.Pow(1f - _ringR[i], 1.5f);
            }
            _m.SetVector("_RingR", new Vector4(_ringR[0], _ringR[1], _ringR[2], _ringR[3]));
            _m.SetVector("_RingK", new Vector4(_ringK[0], _ringK[1], _ringK[2], _ringK[3]));
            _m.SetFloat("_Gain", gain * 1.2f);
            _m.SetFloat("_Hue", 0.4f + 0.08f * Mathf.Sin(rx.phrase));
            // One revolution per four bars, at north on the downbeat (Scope.shader measures the angle from east, atan2(y, x)).
            _m.SetFloat("_Sweep", rx.beatS * (Mathf.PI * 2f / 16f) + Mathf.PI * 0.5f);
            _m.SetFloat("_Drift", rx.clk);

            // A wall of light bars round the rim: the same 96-bin plot, in 3D.
            float lift = (1f - 0.35f * rx.tension) * (1f + 0.6f * rx.impact);
            for (int i = 0; i < Bars; i++)
            {
                float a = i / (float)Bars * Mathf.PI * 2f;
                float sym = Mathf.Abs(i / (float)Bars * 2f - 1f);
                float lvl = rx.Spec(sym);
                float h = (0.8f + 11f * lvl) * lift;
                Vector3 foot = new Vector3(Mathf.Cos(a) * Radius * 1.06f, 0.1f, Mathf.Sin(a) * Radius * 1.06f);
                _bars.Set(i, foot, foot + new Vector3(0f, h, 0f), Kit.Hue(0.4f + 0.3f * sym), gain * (0.3f + 1.1f * lvl));
            }
            rig.Orbit(cam, rx, 34f, 26f, 0f, dt);
        }
    }
}
