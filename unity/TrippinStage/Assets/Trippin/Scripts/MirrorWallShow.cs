// unity_mirrors: a kinetic mirror wall: 16 x 9 mirror panels on their own
// pivots, tilting in waves, with two spotlights sweeping across them. Each
// panel throws its own reflected beam back into the haze and flashes a glint
// when it turns its light toward the camera (GearKit + HqGear chrome, Kit
// BeamPool). Units are metres.
//  - Shape: the wall's tilt field is the music. A new figure every 4 bars,
//    morphing in over a beat: the spectrum across the columns (bass in the
//    middle), a wave rolling across, rings from the centre, and a
//    checkerboard flip on Trippin's 2-beat figure; the bass sets the depth of
//    every figure and each kick sends a ring of tilt out from the centre.
//    The reflected beams fan and fold as the panels turn.
//  - Motion: the waves and rings travel on the smooth energy clock, the
//    spotlights sweep across the wall every 2 bars (one each way), and the
//    camera drifts through four shots, cutting every 8 bars. Count fixed:
//    a breakdown calms the tilt and dims the reflections.
// Controls: tilt, beams, glints, haze, drift.
using UnityEngine;

namespace TrippinStage
{
    public sealed class MirrorWallShow : KitShow
    {
        public Material gearMat;

        [TrippinParam("Tilt", 0.2f, 2f)] public float tiltAmt = 1f;
        [TrippinParam("Beams", 0f, 2f)] public float beams = 1f;
        [TrippinParam("Glints", 0f, 2f)] public float glints = 1f;
        [TrippinParam("Haze", 0f, 2f)] public float hazeAmt = 1f;
        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;

        const int NX = 16, NY = 9, N = NX * NY, S = 2;
        const float Pitch = 0.56f, Panel = 0.5f, WallZ = 4f, WallY = 3.4f;

        GearMats _m;
        Material _mirror, _back, _frame, _spotBody;
        Mesh _panelM, _backM, _frameM, _spotM;
        BeamPool _refl, _spots;
        readonly Vector2[] _tilt = new Vector2[N], _tilt0 = new Vector2[N];
        readonly Vector4[] _rings = new Vector4[4];
        readonly Vector4[] _lp = new Vector4[4], _lc = new Vector4[4];
        readonly Vector3[] _src = new Vector3[S];
        readonly Color[] _srcCol = new Color[S];
        int _form = -1, _shot = -1;
        float _t, _bass, _kickPrev, _flat, _imp;

        protected override void Build()
        {
            Env(120f, 0.06f, true);
            _m = new GearMats(gearMat);
            _mirror = _m.M(0.95f, 0.95f, 0.97f, 1f, 0.05f);
            _back = _m.M(0.03f, 0.03f, 0.032f, 0.2f, 0.5f);
            _frame = _m.M(0.05f, 0.05f, 0.055f, 0.9f, 0.35f);
            _spotBody = _m.M(0.03f, 0.03f, 0.033f, 0.2f, 0.45f);
            _panelM = GearMesh.RoundedBox(new Vector3(Panel, Panel, 0.012f), 0.006f, 1, "mirror");
            _backM = GearMesh.RoundedBox(new Vector3(Panel * 0.9f, Panel * 0.9f, 0.03f), 0.01f, 1, "mirror back");
            _frameM = GearMesh.RoundedBox(new Vector3(NX * Pitch + 0.5f, NY * Pitch + 0.5f, 0.12f), 0.04f, 2, "mirror frame");
            _spotM = GearMesh.Lathe(new[] { new Vector2(0f, -0.25f), new Vector2(0.12f, -0.25f), new Vector2(0.16f, 0.2f), new Vector2(0.16f, 0.2f), new Vector2(0.14f, 0.22f) }, 24, "spot");
            _refl = new BeamPool(transform, beamMat, N, "reflection", false, 0.012f, 0.006f, 40f, 0.85f, 0.7f, 0.45f);
            _spots = new BeamPool(transform, beamMat, S, "spot", false, 0.07f, 0.03f, 20f, 0.7f, 0.98f, 0.3f);
            for (int i = 0; i < 4; i++) _rings[i] = new Vector4(-99f, 0f, 0f, 0f);
        }

        Vector3 PanelPos(int ix, int iy) =>
            new Vector3((ix - (NX - 1) * 0.5f) * Pitch, WallY + (iy - (NY - 1) * 0.5f) * Pitch, WallZ);

        /// Figure k's tilt (degrees about x, about y) for panel (ix, iy) at beat b.
        Vector2 Figure(int k, int ix, int iy, float b)
        {
            float x = (ix - (NX - 1) * 0.5f) / ((NX - 1) * 0.5f), y = (iy - (NY - 1) * 0.5f) / ((NY - 1) * 0.5f);
            float c = rx.clk;
            switch (k)
            {
                default:
                case 0:   // the spectrum across the columns, bass in the middle; rows lean with it
                {
                    float sp = rx.Spec(Mathf.Abs(x) * 0.95f + 0.02f);
                    return new Vector2((sp - 0.35f) * 50f * (0.6f + 0.4f * (1f - Mathf.Abs(y))), x * 8f);
                }
                case 1:   // a wave rolling across the wall
                    return new Vector2(6f * Mathf.Sin(y * 2f + c * 0.6f), 26f * Mathf.Sin(x * 5f - c * 1.5f));
                case 2:   // rings from the centre, each panel leaning out from it
                {
                    float r = Mathf.Sqrt(x * x + y * y * 0.4f);
                    float w = Mathf.Cos(r * 8f - c * 1.4f) * 24f;
                    float a = Mathf.Atan2(y, x);
                    return new Vector2(-Mathf.Sin(a) * w, Mathf.Cos(a) * w);
                }
                case 3:   // a checkerboard flipping on Trippin's 2-beat figure
                {
                    float f = ((ix + iy) % 2 == 0 ? 1f : -1f) * Mathf.Sin(b * Mathf.PI * 0.5f);
                    return new Vector2(f * 16f, f * 22f);
                }
            }
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            GearDraw.BeginFrame();
            _t += dt;
            float b = rx.beatS;
            _bass = Eased.Follow(_bass, rx.bassFast, 14f, 3.5f, dt);
            _flat = Mathf.Lerp(_flat, rx.calm, 1f - Mathf.Exp(-dt * 1.2f));
            _imp = rx.impact > _imp ? Mathf.Lerp(_imp, rx.impact, 1f - Mathf.Exp(-dt * 4f)) : rx.impact;
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && rx.calm < 0.5f;
            _kickPrev = rx.kick;
            if (kick)
            {
                int best = 0;
                for (int i = 1; i < 4; i++) if (_rings[i].x < _rings[best].x) best = i;
                _rings[best] = new Vector4(_t, 12f + 14f * rx.kick, 0f, 0f);
            }

            int blk = Mathf.FloorToInt(b / 16f);
            int[] order = { 0, 1, 2, 0, 3, 1, 2, 3 };
            int form = order[((blk % order.Length) + order.Length) % order.Length];
            if (form != _form) { System.Array.Copy(_tilt, _tilt0, N); _form = form; }
            float into = MixClock.Ease(Mathf.Repeat(b, 16f));
            float amp = tiltAmt * (0.6f + 0.6f * _bass) * Mathf.Lerp(1f, 0.4f, _flat);
            float gain = rx.Gain(0.85f, 1.2f);
            float hue0 = b / 512f;

            // Spotlights: one each side, sweeping across the wall and back every 2 bars, opposite ways.
            for (int k = 0; k < S; k++)
            {
                float dir = k == 0 ? 1f : -1f;
                _src[k] = new Vector3(dir * -6f, 6.5f, -5f);
                _srcCol[k] = Color.Lerp(Color.white, Kit.Hue(hue0 + k * 0.45f), 0.55f);
            }
            var cp = cam.transform.position;

            _lp[0] = new Vector4(0f, 5f, -8f, 14f); _lc[0] = (Vector4)(new Color(0.6f, 0.65f, 0.8f) * 0.35f);
            _lp[1] = new Vector4(_src[0].x, _src[0].y, _src[0].z, 14f); _lc[1] = (Vector4)(_srcCol[0] * 1.4f * gain);
            _lp[2] = new Vector4(_src[1].x, _src[1].y, _src[1].z, 14f); _lc[2] = (Vector4)(_srcCol[1] * 1.4f * gain);
            _lp[3] = new Vector4(0f, 1f, 2f, 6f); _lc[3] = (Vector4)(new Color(0.25f, 0.28f, 0.4f) * 0.3f);
            Shader.SetGlobalVectorArray("_GLPos", _lp);
            Shader.SetGlobalVectorArray("_GLCol", _lc);
            var e1 = Kit.Hue(hue0) * 0.35f; var e2 = Kit.Hue(hue0 + 0.45f) * 0.35f;
            Shader.SetGlobalVector("_GEnvA", (Vector4)(new Color(0.55f, 0.6f, 0.75f) * 0.35f));
            Shader.SetGlobalVector("_GEnvB", new Vector4(e1.r, e1.g, e1.b, 0.5f));
            Shader.SetGlobalVector("_GEnvC", new Vector4(e2.r, e2.g, e2.b, -0.5f));

            // The spots aim where they sweep on the wall.
            var aim = new Vector3[S];
            for (int k = 0; k < S; k++)
            {
                float dir = k == 0 ? 1f : -1f;
                float sx = dir * 3.8f * Mathf.Sin(b / 8f * Mathf.PI * 2f + k * 1.2f);
                float sy = WallY + 1.2f * Mathf.Sin(b / 16f * Mathf.PI * 2f + k);
                aim[k] = new Vector3(sx, sy, WallZ);
                var d = (aim[k] - _src[k]).normalized;
                GearDraw.Draw(_spotM, _spotBody, GearDraw.TRS(_src[k], Quaternion.FromToRotation(Vector3.up, d)));
                _spots.Set(k, _src[k] + d * 0.22f, aim[k], _srcCol[k], 0.12f * gain * beams);
            }

            GearDraw.Draw(_frameM, _frame, GearDraw.T(new Vector3(0f, WallY, WallZ + 0.12f)));
            for (int iy = 0; iy < NY; iy++)
            for (int ix = 0; ix < NX; ix++)
            {
                int i = iy * NX + ix;
                var want = Figure(form, ix, iy, b);
                var f = Vector2.Lerp(_tilt0[i], want, into);
                if (into >= 1f) _tilt0[i] = want;
                _tilt[i] = f;
                var p = PanelPos(ix, iy);
                float r = new Vector2(p.x, p.y - WallY).magnitude;
                float ring = 0f;
                for (int k = 0; k < 4; k++)
                {
                    float age = _t - _rings[k].x;
                    if (age < 0f || age > 2.5f) continue;
                    float d = r - age * 4f;
                    ring += _rings[k].y * Mathf.Exp(-d * d * 3f) * Mathf.Exp(-age * 1.2f) * Mathf.SmoothStep(0f, 1f, age / 0.15f);
                }
                var q = Quaternion.Euler(f.x * amp + ring, f.y * amp, 0f);
                var n = q * Vector3.back;                                    // the mirror faces -z
                // Each panel bounces the spot whose pool it sits in: a glint when that
                // light turns toward the camera, and one reflected beam into the haze.
                Color gcol = Color.black;
                int bestK = 0; float bestPool = -1f;
                var toCam = (cp - p).normalized;
                for (int k = 0; k < S; k++)
                {
                    float pool = Mathf.Exp(-(p - aim[k]).sqrMagnitude / 9f);
                    var refl = Vector3.Reflect((p - _src[k]).normalized, n);
                    gcol += _srcCol[k] * (Mathf.Pow(Mathf.Max(0f, Vector3.Dot(refl, toCam)), 24f) * pool + 0.12f * pool);
                    if (pool > bestPool) { bestPool = pool; bestK = k; }
                }
                {
                    var refl = Vector3.Reflect((p - _src[bestK]).normalized, n);
                    var cpv = cp - p;
                    float along = Mathf.Clamp(Vector3.Dot(cpv, refl), 0f, 30f);
                    float near = (cpv - refl * along).magnitude;
                    float lens = Mathf.SmoothStep(0.05f, 1f, Mathf.InverseLerp(0.8f, 3.5f, near));
                    float inten = bestPool * bestPool * 0.14f * gain * beams * Mathf.Lerp(1f, 0.45f, _flat) * lens;
                    if (refl.z > 0.2f || inten < 0.01f) _refl.Hide(i);
                    else _refl.Set(i, p + refl * 0.03f, p + refl * 10f, _srcCol[bestK], inten);
                }
                var pr = GearDraw.Props;
                pr.SetColor("_Emit", gcol * 3f * glints * (1f + 0.6f * _imp));
                GearDraw.Draw(_panelM, _mirror, GearDraw.TRS(p, q), pr);
                GearDraw.Draw(_backM, _back, GearDraw.TRS(p + new Vector3(0f, 0f, 0.05f), Quaternion.identity));
            }
            hazeScale = hazeAmt;

            int sb = Mathf.FloorToInt(b / 32f);
            if (sb != _shot) { _shot = sb; rig.Snap(); }
            float u = Mathf.Repeat(b, 32f) / 32f;
            float sw = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * drift;
            var wall = new Vector3(0f, WallY, WallZ);
            Vector3 pos, look;
            switch (((sb % 4) + 4) % 4)
            {
                default:
                case 0: pos = new Vector3(2f * sw, 2.6f, -5.5f); look = wall; break;
                case 1: pos = new Vector3(-6f, 3.4f, -3f + sw); look = wall + new Vector3(1f, 0f, 0f); break;
                case 2: pos = new Vector3(5f + sw, 1.5f, -4f); look = wall + new Vector3(-0.5f, 0.6f, 0f); break;
                case 3: pos = new Vector3(1.5f + sw, 4.6f, -2f); look = wall + new Vector3(1.2f, -0.4f, 0f); break;
            }
            rig.Move(cam, pos, look, dt, 3f);
            // No depth of field: the beams write no depth, so it blurred them into wide soft slabs.
        }
    }
}
