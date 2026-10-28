// unity_paint: a macro of wet paint dancing on a woofer lying face up on
// the booth table (HqPaint.shader for the painted membrane, GearKit +
// HqGear for the surround, frame and the flying droplets). 1 unit = 10 cm.
//  - Shape: the membrane pumps with the bass, ripples run out from the
//    centre with the mids, and each kick sends a ring across it and throws
//    droplets of paint up off it (falling back in slow motion); a build
//    stands the paint up in fingers, and the drop flings the most.
//  - Motion: the marbling drifts and folds on the smooth energy clock, the
//    paint colours walk the palette with the phrase, and the camera drifts
//    through four macro shots, cutting every 8 bars. A breakdown stills the
//    membrane to slow ripples.
// Controls: pump, splash, swirl, booth lamp, club lights, drift.
using UnityEngine;

namespace TrippinStage
{
    public sealed class PaintShow : KitShow
    {
        public Material gearMat, clubMat, paintMat;

        [TrippinParam("Pump", 0.2f, 2f)] public float pump = 1f;
        [TrippinParam("Splash", 0f, 2f)] public float splash = 1f;
        [TrippinParam("Swirl", 0f, 2f)] public float swirl = 1f;
        [TrippinParam("Booth lamp", 0.3f, 2f)] public float lamp = 1f;
        [TrippinParam("Club lights", 0f, 2f)] public float club = 1f;
        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;

        const float R = 1.5f, Y0 = 0.32f, G = 22f;
        const int ND = 64;

        GearMats _m;
        Booth _booth;
        Material _paint, _surround, _frame, _drop;
        Mesh _surroundM, _frameM, _boltM, _dropM;
        readonly Vector3[] _dp = new Vector3[ND], _dv = new Vector3[ND];
        readonly float[] _dlife = new float[ND], _dhue = new float[ND], _dsize = new float[ND];
        readonly Vector4[] _rings = new Vector4[4], _ringOut = new Vector4[4];
        readonly Vector4[] _lp = new Vector4[4], _lc = new Vector4[4];
        int _shot = -1, _next;
        float _t, _paintT, _exc, _mid, _spike, _kickPrev, _imp;

        protected override void Build()
        {
            _m = new GearMats(gearMat);
            _booth = new Booth(transform, clubMat, _m);
            _paint = new Material(paintMat);
            _paint.SetTexture("_Vol", HqNoise.Volume);
            _paint.SetFloat("_R", R);
            Kit.Part(transform, "membrane", Kit.GridMesh(256, 256, "membrane"), _paint, new Vector3(0f, Y0, 0f), Vector3.one);
            _surround = _m.M(0.018f, 0.018f, 0.02f, 0f, 0.6f);
            _frame = _m.M(0.1f, 0.1f, 0.11f, 1f, 0.35f, 0.4f);
            _drop = _m.M(0.5f, 0.5f, 0.5f, 0f, 0.08f);
            // The rubber roll surround, from the membrane's edge out over the frame.
            var sr = new Vector2[13];
            for (int i = 0; i <= 12; i++)
            {
                float a = i / 12f * Mathf.PI;
                sr[i] = new Vector2(R + 0.13f - Mathf.Cos(a) * 0.13f, Y0 - 0.2f + Mathf.Sin(a) * 0.11f);
            }
            _surroundM = GearMesh.Lathe(sr, 64, "surround");
            _frameM = GearMesh.Lathe(new[]
            {
                new Vector2(R + 0.26f, Y0 - 0.18f), new Vector2(R + 0.62f, Y0 - 0.18f), new Vector2(R + 0.62f, Y0 - 0.18f),
                new Vector2(R + 0.62f, 0f), new Vector2(R + 0.62f, 0f), new Vector2(R + 0.26f, 0f),
            }, 64, "frame");
            _boltM = GearMesh.Lathe(new[] { new Vector2(0f, 0.05f), new Vector2(0.05f, 0.04f), new Vector2(0.06f, 0f), new Vector2(0f, 0f) }, 12, "bolt");
            var dp = new Vector2[11];
            for (int i = 0; i <= 10; i++)
            {
                float a = i / 10f * Mathf.PI;
                dp[i] = new Vector2(Mathf.Sin(a), -Mathf.Cos(a));
            }
            _dropM = GearMesh.Lathe(dp, 16, "droplet");
            for (int i = 0; i < 4; i++) _rings[i] = new Vector4(-99f, 0f, 0f, 0f);
        }

        /// The membrane's height at radius r (units), a CPU match of HqPaint's dish and pump.
        float Surface(float r)
        {
            float k = r / R, cone = Mathf.Clamp01(1f - k * k);
            return Y0 - 0.16f * k * k * R + _exc * cone + 0.035f * cone;
        }

        void Throw(int n, float power)
        {
            for (int c = 0; c < n; c++)
            {
                int i = _next; _next = (_next + 1) % ND;
                float a = Random.value * Mathf.PI * 2f, r = Mathf.Sqrt(Random.value) * R * 0.8f;
                var p = new Vector3(Mathf.Cos(a) * r, 0f, Mathf.Sin(a) * r);
                p.y = Surface(r);
                _dp[i] = p;
                float up = (3.5f + 4f * Random.value) * power;
                _dv[i] = new Vector3((Random.value - 0.5f) * 1.2f, up, (Random.value - 0.5f) * 1.2f) + p * 0.25f;
                _dlife[i] = 1f;
                _dhue[i] = Random.value;
                _dsize[i] = 0.03f + 0.05f * Random.value * Random.value;
            }
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            GearDraw.BeginFrame();
            _booth.Frame(rx, lamp, club);
            _t += dt;
            float b = rx.beatS;
            float calm = rx.calm;
            _imp = rx.impact > _imp ? Mathf.Lerp(_imp, rx.impact, 1f - Mathf.Exp(-dt * 4f)) : rx.impact;
            // The cone pumps with the bass (fast attack, a softer fall, as a cone does).
            _exc = Eased.Follow(_exc, rx.bassFast * 0.22f * pump * (1f - 0.7f * calm), 22f, 6f, dt);
            _mid = Eased.Follow(_mid, rx.midFast, 10f, 3f, dt);
            _spike = Eased.Follow(_spike, (0.12f + 0.3f * rx.tension + 0.35f * _imp) * (0.4f + 0.8f * rx.bassFast) * (1f - 0.8f * calm), 8f, 2f, dt);
            _paintT = _clk.Step(rx.clk, 0.3f * swirl);

            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && calm < 0.5f;
            _kickPrev = rx.kick;
            if (kick)
            {
                int best = 0;
                for (int i = 1; i < 4; i++) if (_rings[i].x < _rings[best].x) best = i;
                _rings[best] = new Vector4(_t, 0.05f + 0.06f * rx.kick, 0f, 0f);
                if (splash > 0f) Throw(Mathf.RoundToInt((4 + 6 * rx.kick + 10 * _imp) * splash), 0.8f + 0.5f * rx.kick + 0.5f * _imp);
            }
            for (int i = 0; i < 4; i++)
            {
                float age = _t - _rings[i].x;
                _ringOut[i] = age < 0f || age > 1.6f ? Vector4.zero
                    : new Vector4(age * 3.2f, _rings[i].y * Mathf.Exp(-age * 2f) * Mathf.SmoothStep(0f, 1f, age / 0.06f), 0f, 0f);
            }
            _paint.SetFloat("_Exc", _exc);
            _paint.SetFloat("_Rip", 0.3f + 1.2f * _mid);
            _paint.SetFloat("_Spike", _spike);
            _paint.SetFloat("_T", _paintT);
            _paint.SetFloat("_Mix", 0.6f + 0.6f * swirl);
            _paint.SetFloat("_Hue", b / 512f + 0.1f * Mathf.Sin(rx.phrase * 0.25f));
            _paint.SetVectorArray("_Ring", _ringOut);
            _paint.SetFloat("_Gain", 1f);

            // Lights for the macro: a warm key and a cool rim, plus the booth's two palette glows.
            var p1 = Kit.Hue(b / 512f); var p2 = Kit.Hue(b / 512f + 0.4f);
            _lp[0] = new Vector4(-2.5f, 4.5f, -3.5f, 10f); _lc[0] = (Vector4)(new Color(1f, 0.9f, 0.78f) * 1.6f * lamp);
            _lp[1] = new Vector4(3.5f, 2.2f, 3f, 9f); _lc[1] = (Vector4)(new Color(0.6f, 0.75f, 1f) * 1.1f);
            _lp[2] = new Vector4(-4f, 1.2f, 2.5f, 8f); _lc[2] = (Vector4)(p1 * 0.9f * club);
            _lp[3] = new Vector4(4f, 1.2f, -2.5f, 8f); _lc[3] = (Vector4)(p2 * 0.9f * club);
            Shader.SetGlobalVectorArray("_GLPos", _lp);
            Shader.SetGlobalVectorArray("_GLCol", _lc);

            GearDraw.Draw(_surroundM, _surround, Matrix4x4.identity);
            GearDraw.Draw(_frameM, _frame, Matrix4x4.identity);
            for (int i = 0; i < 8; i++)
            {
                float a = i / 8f * Mathf.PI * 2f + 0.39f;
                GearDraw.Draw(_boltM, _m.chrome, GearDraw.T(new Vector3(Mathf.Cos(a) * (R + 0.45f), Y0 - 0.18f, Mathf.Sin(a) * (R + 0.45f))));
            }
            // Droplets: thrown up, falling back in slow motion, stretched along their flight.
            for (int i = 0; i < ND; i++)
            {
                if (_dlife[i] <= 0f) continue;
                _dv[i] += Vector3.down * G * dt;
                _dp[i] += _dv[i] * dt;
                float r = new Vector2(_dp[i].x, _dp[i].z).magnitude;
                if (_dv[i].y < 0f && _dp[i].y < Surface(Mathf.Min(r, R))) { _dlife[i] = 0f; continue; }
                float sp = _dv[i].magnitude;
                var q = Quaternion.FromToRotation(Vector3.up, sp > 1e-3f ? _dv[i] / sp : Vector3.up);
                float st = 1f + Mathf.Min(sp * 0.06f, 1.4f);
                var pr = GearDraw.Props;
                var col = TrippinLink.Palette(b / 512f + _dhue[i]);
                pr.SetColor("_Tint", col * col); pr.SetColor("_Side", col * col);
                GearDraw.Draw(_dropM, _drop, Matrix4x4.TRS(_dp[i], q, new Vector3(_dsize[i], _dsize[i] * st, _dsize[i])), pr);
            }

            // Shots: one per 8 bars, macro distances, focused on the membrane.
            int sb = Mathf.FloorToInt(b / 32f);
            if (sb != _shot) { _shot = sb; rig.Snap(); }
            float u = Mathf.Repeat(b, 32f) / 32f;
            float sw = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * drift;
            Vector3 pos, look;
            switch (((sb % 4) + 4) % 4)
            {
                default:
                case 0: look = new Vector3(0f, Y0, 0.2f); pos = new Vector3(0.6f * sw, 1.6f, -3.8f); break;
                case 1: look = new Vector3(0f, Y0, 0f); pos = new Vector3(0.4f * sw, 4.8f, -1.6f); break;
                case 2: look = new Vector3(-0.2f, Y0 + 0.1f, 0.3f); pos = new Vector3(3f + 0.4f * sw, 0.75f, -2.2f); break;
                case 3: look = new Vector3(0.3f, Y0, -0.2f); pos = new Vector3(-2.6f, 2.2f, -2.8f + 0.5f * sw); break;
            }
            rig.Move(cam, pos, look, dt, 3f);
            HqPost.Focus(Vector3.Distance(cam.transform.position, look), calm > 0.5f ? 4f : 5.6f, 50f);
        }

        readonly ClkAcc _clk = new ClkAcc();
    }
}
