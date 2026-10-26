// unity_fire: the HQ version of the 2D fire wall: concert flame cannons.
// A row of jets along a dark stage fires volumetric flames (HqFire.shader)
// that throw orange light on the wet floor and the haze, shed rising embers
// (GPU sparks) and leave smoke glowing above them.
//  - Shape: kicks fire the jets in a pattern that walks along the row
//    through the bar (outer pair, inner pair, centre, all); the bass sets
//    how tall they roar; the drop fires every jet full height. Breakdowns
//    leave small pilot flames flickering.
//  - Motion: the flames' turbulence streams faster with the energy; the
//    camera drifts along the front of the stage.
// Controls: flame height, jets, turbulence, heat, embers, smoke.
using UnityEngine;

namespace TrippinStage
{
    public sealed class FireShow : KitShow
    {
        public Material fireMat, sparkMat, smokeMat;
        public ComputeShader particles;

        [TrippinParam("Flame height", 0.4f, 2f)] public float height = 1f;
        [TrippinParam("Jets", 2f, 8f)] public float jets = 8f;
        [TrippinParam("Turbulence", 0.4f, 2.5f)] public float turb = 1f;
        [TrippinParam("Heat", 0.3f, 2.5f)] public float heat = 1f;
        [TrippinParam("Embers", 0f, 3f)] public float embers = 1f;
        [TrippinParam("Smoke", 0f, 3f)] public float smoke = 1f;


        const int Max = 8;
        const float Z = 14f;
        Material _fire;
        ParticleField _sparks, _smoke;

        readonly float[] _h = new float[Max], _want = new float[Max];
        readonly Vector4[] _jet = new Vector4[Max], _flash = new Vector4[4], _flashPos = new Vector4[4];
        float _t, _kickPrev, _emberAcc;

        protected override void Build()
        {
            Env(120f, 0.08f);
            _fire = Kit.Fullscreen(transform, fireMat, "flames");
            _sparks = new ParticleField(particles, sparkMat, 32768, true, 0f);
            _sparks.Gravity = new Vector3(0f, 3f, 0f);      // embers ride the hot air up
            _smoke = new ParticleField(particles, smokeMat, 4096, false);
            _smoke.Gravity = new Vector3(0.4f, 1.2f, 0f);

        }

        void OnDestroy() { _sparks?.Release(); _smoke?.Release(); }

        float X(int i, int n) => n <= 1 ? 0f : Mathf.Lerp(-14f, 14f, i / (n - 1f));

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            int n = Mathf.Clamp(Mathf.RoundToInt(jets), 2, Max);
            _t += dt * (0.8f + 0.9f * rx.lum);
            bool calm = rx.calm > 0.5f;
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && !calm;
            _kickPrev = rx.kick;
            float tall = (9f + 9f * rx.bassFast) * height;
            if (kick)
            {
                // The pattern walks along the row through the bar.
                int beat = Mathf.FloorToInt(Mathf.Repeat(rx.beatS, 4f));
                for (int i = 0; i < n; i++)
                {
                    int fromEdge = Mathf.Min(i, n - 1 - i);
                    bool fire = beat == 3 || (beat == 0 && fromEdge == 0) || (beat == 1 && fromEdge == 1) || (beat == 2 && fromEdge >= 2);
                    if (fire) _want[i] = Mathf.Max(_want[i], tall);
                }
            }
            if (rx.dropped) for (int i = 0; i < n; i++) _want[i] = 20f * height;

            float gain = rx.Gain(0.8f, 1.25f);
            int f = 0;
            for (int i = 0; i < Max; i++)
            {
                // Roar up fast, die back over half a second; a pilot flame idles.
                float pilot = calm ? 0.9f : 0.5f;
                _want[i] = Mathf.Max(pilot, _want[i] * Mathf.Exp(-dt * 2.2f));
                _h[i] = Eased.Follow(_h[i], i < n ? _want[i] : 0f, 18f, 4f, dt);
                float x = X(i, n);
                _jet[i] = i < n ? new Vector4(x, Z, _h[i], 1.6f + 0.08f * _h[i]) : Vector4.zero;
                if (i < n)
                {

                    if (_h[i] > 2.5f && f < 4)
                    {
                        _flash[f] = new Vector4(1.6f, 0.6f, 0.15f, 0f) * Mathf.Clamp01(_h[i] / 10f) * 0.5f;
                        _flashPos[f] = new Vector4(x, _h[i] * 0.9f, Z, 14f);
                        f++;
                    }
                }

            }
            for (; f < 4; f++) { _flash[f] = Vector4.zero; _flashPos[f] = new Vector4(0, -99, 0, 1); }

            // Embers and smoke from the burning jets.
            _emberAcc += dt * embers * 60f;
            for (int i = 0; i < n && _emberAcc >= 1f; i++)
            {
                if (_h[i] < 2f) continue;
                _emberAcc -= 1f;
                _sparks.Burst(new Vector3(X(i, n), _h[i] * 0.6f, Z), Mathf.RoundToInt(10 * embers + _h[i] * 2f), ParticleField.Shape.Sphere, 3f, 0.09f, 2.2f,
                              new Color(2.4f, 0.9f, 0.25f), 0.8f, 1f, new Vector3(0f, 4f + _h[i] * 0.5f, 0f), default, 0.7f);
                if (smoke > 0f && Random.value < 0.25f * smoke)
                    _smoke.Burst(new Vector3(X(i, n), _h[i] * 1.1f, Z), 6, ParticleField.Shape.Sphere, 1.5f, 3.5f, 7f, Color.white, 0.6f, 0.3f, new Vector3(0f, 2f, 0f), default, 0.6f);
            }
            _sparks.Tick(dt, gain, 0.03f, 0.8f);
            _smoke.Mat.SetVectorArray("_Flash", _flash);
            _smoke.Mat.SetVectorArray("_FlashPos", _flashPos);
            _smoke.Tick(dt, 0.1f * smoke * gain);

            _fire.SetVectorArray("_Jet", _jet);
            _fire.SetInt("_JetCount", n);
            _fire.SetFloat("_T", _t);
            _fire.SetFloat("_Turb", turb);
            _fire.SetFloat("_Heat", 0.65f * heat * gain);

            float sway = Mathf.Sin(rx.phrase * 0.5f);
            rig.Move(cam, new Vector3(sway * 6f, 2.5f, -14f), new Vector3(sway * 2f, 7.5f, Z), dt, 0.6f);
        }
    }
}
