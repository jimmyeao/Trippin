// unity_lasers: a laser show the way the owner's rules have it: thin,
// saturated beams cutting through fog-machine haze, patchy along their
// length where the smoke is (Kit BeamPool). Six projectors (four on a truss
// over the stage, two on the stage front) each fire a fixed set of 14 beams.
// Units are metres.
//  - Shape: a new formation every 4 bars, morphing in over a beat: fan,
//    rotating tunnel, a sheet rolling over the crowd, crossing beams, a
//    scanning fan, a starburst. The bass widens the fans, the mids open the
//    tunnels, the highs roll the sheet's wave.
//  - Motion: tunnels turn, sheets roll and scans sweep on the beat clock;
//    the camera drifts, cutting every 8 bars. The beam count never changes:
//    a breakdown fades the outer beams and the floor projectors.
// Controls: beams, spread, haze, colour, drift.
using UnityEngine;

namespace TrippinStage
{
    public sealed class LasersShow : KitShow
    {
        public Material gearMat;

        [TrippinParam("Beams", 0.2f, 2f)] public float beams = 1f;
        [TrippinParam("Spread", 0.4f, 1.6f)] public float spread = 1f;
        [TrippinParam("Haze", 0f, 2f)] public float hazeAmt = 1f;
        [TrippinParam("Colour", 0f, 1f)] public float colour = 0.3f;
        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;

        const int P = 6, B = 14;
        const float TrussY = 6.8f, TrussZ = 2f;

        GearMats _m;
        Material _body, _fin;
        Mesh _boxM, _finM, _aperture, _trussM, _legM;
        BeamPool _beams;
        readonly Vector3[] _pos = new Vector3[P];
        readonly Vector3[] _dir = new Vector3[P * B], _dir0 = new Vector3[P * B];
        readonly Vector4[] _lp = new Vector4[4], _lc = new Vector4[4];
        int _form = -1, _shot = -1;
        float _bass, _mid, _high, _imp;

        protected override void Build()
        {
            Env(160f, 0.11f, true);
            _m = new GearMats(gearMat);
            _body = _m.M(0.035f, 0.035f, 0.038f, 0.3f, 0.4f);
            _fin = _m.M(0.25f, 0.25f, 0.27f, 1f, 0.35f, 0.5f);
            // Lasers show the smoke in smaller, harder patches than the wide shafts do.
            var laserMat = new Material(beamMat);
            laserMat.SetFloat("_SmokeScale", 0.6f);
            laserMat.SetVector("_SmokeEdge", new Vector4(0.42f, 0.58f, 0f, 0f));
            _beams = new BeamPool(transform, laserMat, P * B, "laser", false, 0.010f, 0.0015f, 70f, 0.95f, 0.9f, 0.55f);
            _boxM = GearMesh.RoundedBox(new Vector3(0.36f, 0.2f, 0.42f), 0.03f, 3, "laser projector");
            _finM = GearMesh.RoundedBox(new Vector3(0.01f, 0.16f, 0.30f), 0.004f, 1, "fin");
            _aperture = GearMesh.RoundedBox(new Vector3(0.14f, 0.07f, 0.01f), 0.01f, 1, "aperture");
            _trussM = GearMesh.Truss(13.2f, 0.29f, "laser truss");
            _legM = GearMesh.Truss(TrussY + 0.3f, 0.29f, "laser truss leg");
            for (int i = 0; i < 4; i++) _pos[i] = new Vector3(-4.5f + i * 3f, TrussY - 0.32f, TrussZ - 0.1f);
            _pos[4] = new Vector3(-3f, 0.12f, -0.8f); _pos[5] = new Vector3(3f, 0.12f, -0.8f);
        }

        /// Formation k's direction for beam j of projector p at beat b.
        Vector3 Aim(int k, int p, int j, float b)
        {
            bool floor = p >= 4;
            float x = (j - (B - 1) * 0.5f) / ((B - 1) * 0.5f);           // -1..1 across the beams
            float px = (p % 4 - 1.5f) / 1.5f;
            float sp = spread * (0.8f + 0.5f * _bass);
            Vector3 d;
            switch (k)
            {
                default:
                case 0:   // fan: a flat spread over the crowd, tipping slowly with the phrase
                {
                    float a = x * 0.75f * sp, el = floor ? 0.32f : -0.16f + 0.06f * Mathf.Sin(rx.phrase * 0.5f);
                    d = new Vector3(Mathf.Sin(a), el, -Mathf.Cos(a));
                    break;
                }
                case 1:   // tunnel: a cone round an axis toward the crowd, turning a bar round
                {
                    float r = (0.16f + 0.12f * _mid) * sp;
                    float a = j / (float)B * Mathf.PI * 2f + b / 4f * Mathf.PI * 2f * (p % 2 == 0 ? 1f : -1f);
                    var axis = new Vector3(-px * 0.15f, floor ? 0.25f : -0.18f, -1f).normalized;
                    var u = Vector3.Cross(axis, Vector3.up).normalized; var v = Vector3.Cross(u, axis);
                    d = axis + (u * Mathf.Cos(a) + v * Mathf.Sin(a)) * r;
                    break;
                }
                case 2:   // sheet: nearly level, a wave rolling across it
                {
                    float a = x * 0.9f * sp;
                    float el = (floor ? 0.08f : -0.06f) + (0.02f + 0.05f * _high) * Mathf.Sin(x * 4f + b * Mathf.PI * 0.5f + p);
                    d = new Vector3(Mathf.Sin(a), el, -Mathf.Cos(a));
                    break;
                }
                case 3:   // crossing: each projector throws its beams across to the far side
                {
                    float a = (-px * 0.5f + x * 0.18f) * sp;
                    d = new Vector3(Mathf.Sin(a), floor ? 0.4f : -0.2f + 0.05f * x, -Mathf.Cos(a));
                    break;
                }
                case 4:   // scan: a narrow fan sweeping across, two bars over and back
                {
                    float a = (x * 0.22f + 0.7f * Mathf.Sin(b / 8f * Mathf.PI * 2f + p * 0.4f)) * sp;
                    d = new Vector3(Mathf.Sin(a), floor ? 0.3f : -0.12f, -Mathf.Cos(a));
                    break;
                }
                case 5:   // starburst: spraying out in every direction toward the crowd
                {
                    float a = j / (float)B * Mathf.PI * 2f + p * 0.7f + b * 0.05f;
                    float r = 0.55f * sp;
                    d = new Vector3(Mathf.Cos(a) * r, Mathf.Sin(a) * r * 0.7f + (floor ? 0.3f : -0.1f), -1f);
                    break;
                }
            }
            return d.normalized;
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            GearDraw.BeginFrame();
            float b = rx.beatS;
            _bass = Eased.Follow(_bass, rx.bassFast, 14f, 3.5f, dt);
            _mid = Eased.Follow(_mid, rx.midFast, 14f, 3.5f, dt);
            _high = Eased.Follow(_high, rx.highFast, 14f, 3.5f, dt);
            _imp = rx.impact > _imp ? Mathf.Lerp(_imp, rx.impact, 1f - Mathf.Exp(-dt * 4f)) : rx.impact;
            int blk = Mathf.FloorToInt(b / 16f);
            int[] order = { 0, 1, 2, 4, 3, 1, 5, 2, 0, 4 };
            int form = order[((blk % order.Length) + order.Length) % order.Length];
            if (form != _form)
            {
                for (int i = 0; i < P * B; i++) _dir0[i] = _form < 0 ? Aim(form, i / B, i % B, b) : _dir[i];
                _form = form;
            }
            float into = MixClock.Ease(Mathf.Repeat(b, 16f));                    // a beat to morph in
            // Pure laser colours: two per formation, fully saturated.
            float h0 = Mathf.Repeat(blk * 0.17f + rx.beatS / 2048f, 1f);
            var c1 = Color.HSVToRGB(Mathf.Repeat(h0, 1f), 1f, 1f);
            var c2 = Color.HSVToRGB(Mathf.Repeat(h0 + 0.33f * colour + 0.1f, 1f), 1f, 1f);
            float gain = rx.Gain(0.85f, 1.2f) * beams;
            float calm = rx.calm;

            // The projectors themselves, lit dimly.
            _lp[0] = new Vector4(0f, 5f, -9f, 14f); _lc[0] = (Vector4)(new Color(0.7f, 0.72f, 0.8f) * 0.5f);
            _lp[1] = new Vector4(-6f, 9f, 6f, 12f); _lc[1] = (Vector4)(c1 * 0.8f);
            _lp[2] = new Vector4(6f, 9f, 6f, 12f); _lc[2] = (Vector4)(c2 * 0.8f);
            _lp[3] = new Vector4(0f, 1f, -3f, 6f); _lc[3] = (Vector4)(new Color(0.4f, 0.45f, 0.6f) * 0.4f);
            Shader.SetGlobalVectorArray("_GLPos", _lp);
            Shader.SetGlobalVectorArray("_GLCol", _lc);
            GearDraw.Draw(_trussM, _m.alu, GearDraw.T(new Vector3(0f, TrussY, TrussZ)));
            foreach (var x in new[] { -6.75f, 6.75f })
                GearDraw.Draw(_legM, _m.alu, GearDraw.TRS(new Vector3(x, (TrussY + 0.15f) * 0.5f, TrussZ), Quaternion.Euler(0f, 0f, 90f)));

            var camPos = cam.transform.position;
            for (int p = 0; p < P; p++)
            {
                var at = _pos[p];
                GearDraw.Draw(_boxM, _body, GearDraw.T(at));
                for (int f = 0; f < 6; f++) GearDraw.Draw(_finM, _fin, GearDraw.T(at + new Vector3(-0.15f + f * 0.06f, 0.18f, 0.03f)));
                var ap = at + new Vector3(0f, 0f, -0.212f);
                var pr = GearDraw.Props; pr.SetColor("_Emit", Color.Lerp(c1, c2, 0.5f) * (1.5f + 2f * gain));
                GearDraw.Draw(_aperture, _m.led, GearDraw.T(ap), pr);
                bool floor = p >= 4;
                for (int j = 0; j < B; j++)
                {
                    int i = p * B + j;
                    var want = Aim(form, p, j, b);
                    _dir[i] = Vector3.Slerp(_dir0[i], want, into).normalized;
                    if (into >= 1f) _dir0[i] = _dir[i];
                    float x = Mathf.Abs(j - (B - 1) * 0.5f) / ((B - 1) * 0.5f);
                    float fade = (1f - calm * Mathf.Lerp(0.3f, 0.95f, x)) * (floor ? 1f - 0.8f * calm : 1f);
                    // Beams passing close to the camera fade (AGENTS: a near beam fills the lens).
                    var cp = camPos - ap;
                    float along = Mathf.Clamp(Vector3.Dot(cp, _dir[i]), 0f, 40f);
                    float near = (cp - _dir[i] * along).magnitude;
                    fade *= Mathf.SmoothStep(0.1f, 1f, Mathf.InverseLerp(1f, 4f, near));
                    var col = (j % 2 == 0) ? c1 : c2;
                    col = Color.Lerp(col, Color.white, 0.12f * _imp);
                    _beams.Set(i, ap, ap + _dir[i] * 40f, col, 1.7f * gain * fade);
                }
            }
            hazeScale = hazeAmt * 1.3f;

            // Shots: one per 8 bars.
            int sb = Mathf.FloorToInt(b / 32f);
            if (sb != _shot) { _shot = sb; rig.Snap(); }
            float u = Mathf.Repeat(b, 32f) / 32f;
            float sw = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * drift;
            Vector3 pos, look;
            switch (((sb % 4) + 4) % 4)
            {
                default:
                case 0: pos = new Vector3(sw * 2.5f, 1.7f, -16f); look = new Vector3(0f, 3.8f, 0f); break;
                case 1: pos = new Vector3(-13f, 3f, -9f + 2f * sw); look = new Vector3(0f, 3.5f, -3f); break;
                case 2: pos = new Vector3(1f + sw, 0.9f, -10f); look = new Vector3(0f, 6.5f, -4f); break;
                case 3: pos = _pos[1] + new Vector3(1.6f + 0.4f * sw, -0.6f, -2.2f); look = _pos[1] + new Vector3(0f, -0.2f, -0.5f); break;
            }
            // No depth of field: the beams write no depth, so it would blur them by what's behind.
            rig.Move(cam, pos, look, dt, 3f);
        }
    }
}
