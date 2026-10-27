// unity_movingheads: a stage lighting rig of real moving-head beam fixtures:
// eight hang from a box truss over the stage, six stand on the stage front.
// Each fixture is a base, a yoke that pans and a head that tilts (built in
// GearKit, shaded by HqGear), with its beam cutting through the haze (Kit
// BeamPool). Units are metres.
//  - Shape: the rig runs a new formation every 4 bars (fan, parallel, cross,
//    focus, wave, circles, sweep), the heads panning and tilting into it over
//    a beat, the way a lighting desk crossfades cues; the zoom breathes with
//    the bass (beams widen and narrow); the drop opens them wide.
//  - Motion: wave, circle and sweep formations move on the beat clock; the
//    camera drifts slowly, cutting every 8 bars. Count is fixed: in a
//    breakdown the floor row dims and the outer heads fade, nothing switches.
// Controls: beams, zoom, haze, colour spread, drift.
using UnityEngine;

namespace TrippinStage
{
    public sealed class MovingHeadsShow : KitShow
    {
        public Material gearMat;

        [TrippinParam("Beams", 0.2f, 2f)] public float beams = 1f;
        [TrippinParam("Zoom", 0.3f, 2f)] public float zoomBeam = 1f;
        [TrippinParam("Haze", 0f, 2f)] public float hazeAmt = 1f;
        [TrippinParam("Colour spread", 0f, 1f)] public float spread = 0.5f;
        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;

        const int Hang = 8, Floor = 6, N = Hang + Floor;
        const float TrussY = 7.2f, TrussZ = 1.5f, TrussHalf = 6.6f;

        GearMats _m;
        Material _body, _lensM, _truss, _screen;
        Mesh _base, _arm, _bridge, _head, _lens, _clamp, _trussM, _legM, _disp;
        BeamPool _beams;
        readonly Vector3[] _root = new Vector3[N];
        readonly Quaternion[] _rootQ = new Quaternion[N];
        readonly float[] _pan = new float[N], _tilt = new float[N], _pan0 = new float[N], _tilt0 = new float[N];
        readonly Vector4[] _lp = new Vector4[4], _lc = new Vector4[4];
        int _form = -1, _shot = -1;
        float _bass, _imp, _zoomS = 1f;

        protected override void Build()
        {
            Env(160f, 0.09f, true);
            _m = new GearMats(gearMat);
            _body = _m.M(0.03f, 0.03f, 0.033f, 0.2f, 0.45f);
            _lensM = _m.M(0.02f, 0.02f, 0.02f, 0f, 0.1f);
            _truss = _m.alu;
            _screen = _m.M(0f, 0f, 0f, 0f, 0.5f);
            _beams = new BeamPool(transform, beamMat, N, "moving head beam", false, 0.07f, 0.035f, 30f, 0.6f, 0.92f, 0.35f);

            // Fixture parts, standing (base on y 0, head pivot at y 0.40, head axis +y).
            _base = GearMesh.RoundedBox(new Vector3(0.42f, 0.16f, 0.34f), 0.035f, 3, "mh base");
            _arm = GearMesh.RoundedBox(new Vector3(0.055f, 0.34f, 0.17f), 0.025f, 2, "mh arm");
            _bridge = GearMesh.RoundedBox(new Vector3(0.40f, 0.06f, 0.17f), 0.025f, 2, "mh bridge");
            _head = GearMesh.Lathe(new[]
            {
                new Vector2(0f, -0.20f), new Vector2(0.09f, -0.20f), new Vector2(0.11f, -0.17f), new Vector2(0.12f, -0.12f),
                new Vector2(0.125f, 0.05f), new Vector2(0.125f, 0.05f), new Vector2(0.115f, 0.16f), new Vector2(0.115f, 0.16f),
                new Vector2(0.12f, 0.17f), new Vector2(0.12f, 0.19f), new Vector2(0.12f, 0.19f), new Vector2(0.10f, 0.19f),
            }, 40, "mh head");
            _lens = GearMesh.Lathe(new[] { new Vector2(0f, 0.192f), new Vector2(0.10f, 0.186f), new Vector2(0.10f, 0.186f), new Vector2(0.10f, 0.18f) }, 40, "mh lens");
            _clamp = GearMesh.RoundedBox(new Vector3(0.08f, 0.16f, 0.08f), 0.02f, 2, "clamp");
            _disp = GearMesh.RoundedBox(new Vector3(0.10f, 0.06f, 0.005f), 0.004f, 1, "mh display");
            _trussM = GearMesh.Truss(TrussHalf * 2f, 0.29f, "truss");
            _legM = GearMesh.Truss(TrussY + 0.3f, 0.29f, "truss leg");

            for (int i = 0; i < Hang; i++)
            {
                _root[i] = new Vector3(-5.25f + i * 1.5f, TrussY - 0.145f - 0.1f, TrussZ);
                _rootQ[i] = Quaternion.Euler(0f, 0f, 180f);          // hanging: base up, head below
            }
            for (int i = 0; i < Floor; i++)
            {
                _root[Hang + i] = new Vector3(-4.5f + i * 1.8f, 0.02f, -1.2f);
                _rootQ[Hang + i] = Quaternion.identity;
            }
        }

        static float Ease(float x) => MixClock.Ease(x);

        /// Formation k's aim for fixture i (world direction), at beat b.
        Vector3 Aim(int k, int i, float b)
        {
            bool hang = i < Hang;
            int j = hang ? i : i - Hang;
            int n = hang ? Hang : Floor;
            float x = (j - (n - 1) * 0.5f) / ((n - 1) * 0.5f);      // -1..1 across the row
            Vector3 d;
            switch (k)
            {
                default:
                case 0:   // fan out toward the crowd / up and out
                    d = hang ? new Vector3(x * 1.4f, -1f, -1.1f) : new Vector3(x * 0.9f, 1.4f, -0.2f);
                    break;
                case 1:   // parallel
                    d = hang ? new Vector3(0.25f, -1f, -0.8f) : new Vector3(-0.15f, 1f, 0.2f);
                    break;
                case 2:   // crossing pairs
                    d = hang ? new Vector3(((j % 2 == 0) ? 1f : -1f) * 0.9f, -1f, -0.7f) : new Vector3(-x * 1.1f, 1.2f, 0.1f);
                    break;
                case 3:   // focus on one point over the crowd
                {
                    var target = hang ? new Vector3(0f, 2.5f, -7f) : new Vector3(0f, 9f, -3f);
                    d = target - _root[i];
                    break;
                }
                case 4:   // a wave along the row, one bar round
                {
                    float w = Mathf.Sin((b / 4f + j / (float)n) * Mathf.PI * 2f);
                    d = hang ? new Vector3(x * 0.4f, -1f, -0.4f - 0.9f * w) : new Vector3(x * 0.3f, 1f, 0.6f * w);
                    break;
                }
                case 5:   // circles, two beats round, offset along the row
                {
                    float a = (b / 2f + j * 0.25f) * Mathf.PI * 2f;
                    d = hang ? new Vector3(x * 0.6f + 0.45f * Mathf.Cos(a), -1f, -0.7f + 0.45f * Mathf.Sin(a))
                             : new Vector3(x * 0.4f + 0.35f * Mathf.Cos(a), 1.2f, 0.35f * Mathf.Sin(a));
                    break;
                }
                case 6:   // all sweep together, two bars across and back
                {
                    float s = Mathf.Sin(b / 8f * Mathf.PI * 2f);
                    d = hang ? new Vector3(s * 1.2f, -1f, -0.9f) : new Vector3(-s * 0.8f, 1.2f, -0.1f);
                    break;
                }
            }
            return d.normalized;
        }

        void PanTilt(int i, Vector3 dir, out float pan, out float tilt)
        {
            var dl = Quaternion.Inverse(_rootQ[i]) * dir;
            pan = Mathf.Atan2(dl.x, dl.z) * Mathf.Rad2Deg;
            tilt = Mathf.Acos(Mathf.Clamp(dl.y, -1f, 1f)) * Mathf.Rad2Deg;
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            GearDraw.BeginFrame();
            float b = rx.beatS;
            int blk = Mathf.FloorToInt(b / 16f);
            int[] order = { 0, 4, 2, 5, 1, 6, 3, 4, 0, 5, 2, 6 };
            int form = order[((blk % order.Length) + order.Length) % order.Length];
            if (form != _form)
            {
                // Start the crossfade from wherever the heads are now.
                for (int i = 0; i < N; i++) { _pan0[i] = _pan[i]; _tilt0[i] = _tilt[i]; }
                if (_form < 0) for (int i = 0; i < N; i++) { PanTilt(i, Aim(form, i, b), out _pan0[i], out _tilt0[i]); }
                _form = form;
            }
            float into = Ease(Mathf.Repeat(b, 16f) / 1f);            // a beat to move into the new cue
            _bass = Eased.Follow(_bass, rx.bassFast, 14f, 3.5f, dt);
            _imp = rx.impact > _imp ? Mathf.Lerp(_imp, rx.impact, 1f - Mathf.Exp(-dt * 4f)) : rx.impact;
            float calm = rx.calm;
            float gain = rx.Gain(0.85f, 1.2f) * beams;
            _zoomS = Mathf.Lerp(_zoomS, zoomBeam, 1f - Mathf.Exp(-dt * 3f));

            // Lights on the fixtures themselves: a dim front wash and the palette from behind.
            var p1 = Kit.Hue(blk * 0.13f); var p2 = Kit.Hue(blk * 0.13f + 0.5f * spread);
            _lp[0] = new Vector4(0f, 5f, -9f, 14f); _lc[0] = (Vector4)(new Color(0.8f, 0.82f, 0.9f) * 0.6f);
            _lp[1] = new Vector4(-6f, 9f, 6f, 12f); _lc[1] = (Vector4)(p1 * 1.6f);
            _lp[2] = new Vector4(6f, 9f, 6f, 12f); _lc[2] = (Vector4)(p2 * 1.6f);
            _lp[3] = new Vector4(0f, 1f, -3f, 6f); _lc[3] = (Vector4)(new Color(0.4f, 0.45f, 0.6f) * 0.5f);
            Shader.SetGlobalVectorArray("_GLPos", _lp);
            Shader.SetGlobalVectorArray("_GLCol", _lc);
            Shader.SetGlobalVector("_GEnvA", (Vector4)(new Color(0.5f, 0.5f, 0.6f) * 0.2f));
            Shader.SetGlobalVector("_GEnvB", new Vector4(p1.r * 0.4f, p1.g * 0.4f, p1.b * 0.4f, 0.6f));
            Shader.SetGlobalVector("_GEnvC", new Vector4(p2.r * 0.4f, p2.g * 0.4f, p2.b * 0.4f, -0.6f));

            // Truss and legs.
            GearDraw.Draw(_trussM, _truss, GearDraw.T(new Vector3(0f, TrussY, TrussZ)));
            foreach (var x in new[] { -TrussHalf - 0.15f, TrussHalf + 0.15f })
                GearDraw.Draw(_legM, _truss, GearDraw.TRS(new Vector3(x, (TrussY + 0.15f) * 0.5f, TrussZ), Quaternion.Euler(0f, 0f, 90f)));

            for (int i = 0; i < N; i++)
            {
                bool hang = i < Hang;
                PanTilt(i, Aim(form, i, b), out float pw, out float tw);
                _pan[i] = Mathf.LerpAngle(_pan0[i], pw, into);
                _tilt[i] = Mathf.Lerp(_tilt0[i], tw, into);
                if (into >= 1f) { _pan0[i] = _pan[i]; _tilt0[i] = _tilt[i]; }
                var root = Matrix4x4.TRS(_root[i], _rootQ[i], Vector3.one);
                // Base (with its little display), clamp to the truss when hanging.
                GearDraw.Draw(_base, _body, root * GearDraw.T(new Vector3(0f, 0.08f, 0f)));
                {
                    var pr = GearDraw.Props; pr.SetColor("_Emit", new Color(0.3f, 0.8f, 2f) * 0.6f);
                    GearDraw.Draw(_disp, _m.led, root * GearDraw.T(new Vector3(0f, 0.08f, -0.172f)), pr);
                }
                if (hang) GearDraw.Draw(_clamp, _truss, root * GearDraw.T(new Vector3(0f, -0.06f, 0f)));
                // Yoke pans; the head tilts between its arms.
                var yoke = root * GearDraw.TRS(new Vector3(0f, 0.16f, 0f), Quaternion.Euler(0f, _pan[i], 0f));
                GearDraw.Draw(_bridge, _body, yoke * GearDraw.T(new Vector3(0f, 0.03f, 0f)));
                GearDraw.Draw(_arm, _body, yoke * GearDraw.T(new Vector3(-0.17f, 0.22f, 0f)));
                GearDraw.Draw(_arm, _body, yoke * GearDraw.T(new Vector3(0.17f, 0.22f, 0f)));
                var head = yoke * GearDraw.TRS(new Vector3(0f, 0.30f, 0f), Quaternion.Euler(_tilt[i], 0f, 0f));
                GearDraw.Draw(_head, _body, head);
                // Beam: colour by side, the outer heads and the floor row fade in a breakdown.
                int j = hang ? i : i - Hang;
                int n = hang ? Hang : Floor;
                float edge = Mathf.Abs(j - (n - 1) * 0.5f) / ((n - 1) * 0.5f);
                float fade = (1f - calm * Mathf.Lerp(0.2f, 0.9f, edge)) * (hang ? 1f : 1f - 0.75f * calm);
                var col = (j % 2 == 0) ? p1 : p2;
                col = Color.Lerp(col, Color.white, 0.15f + 0.25f * _imp);
                float inten = 2.6f * gain * fade;
                var lensW = head.MultiplyPoint3x4(new Vector3(0f, 0.19f, 0f));
                var dirW = head.MultiplyVector(Vector3.up).normalized;
                {
                    var pr = GearDraw.Props; pr.SetColor("_Emit", col * (2.5f + 3f * inten));
                    GearDraw.Draw(_lens, _lensM, head, pr);
                }
                // A beam passing close to the lens would fill the frame with one blurred slab:
                // fade it by its nearest distance to the camera (AGENTS: 5 m .. 1.5 m).
                var cp = cam.transform.position - lensW;
                float along = Mathf.Clamp(Vector3.Dot(cp, dirW), 0f, 32f);
                float near = (cp - dirW * along).magnitude;
                inten *= Mathf.SmoothStep(0.15f, 1f, Mathf.InverseLerp(1.5f, 5f, near));
                float width = (0.05f + 0.10f * _bass + 0.12f * _imp) * _zoomS;
                _beams.Set(i, lensW, lensW + dirW * 32f, col, inten, width);
            }
            hazeScale = hazeAmt;

            // Shots: one per 8 bars.
            int sb = Mathf.FloorToInt(b / 32f);
            if (sb != _shot) { _shot = sb; rig.Snap(); }
            float u = Mathf.Repeat(b, 32f) / 32f;
            float sw = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * drift;
            Vector3 pos, look; float fstop = 8f;
            switch (((sb % 5) + 5) % 5)
            {
                default:
                case 0: pos = new Vector3(sw * 3f, 1.7f, -15f); look = new Vector3(0f, 4.5f, 0f); break;
                case 1: pos = new Vector3(2f + sw, 1.2f, -4.5f); look = new Vector3(0f, 6.5f, 1.5f); break;
                case 2:
                {
                    var f = _root[3];
                    look = f + new Vector3(0f, -0.45f, 0f);
                    pos = look + new Vector3(1.0f + 0.4f * sw, -0.5f, -1.6f);
                    fstop = 5.6f;
                    break;
                }
                case 3: pos = new Vector3(-14f, 3.5f, -7f + 2f * sw); look = new Vector3(1f, 4f, 0f); break;
                case 4:
                    look = _root[Hang + 2] + new Vector3(0.9f, 0.4f, 0f);
                    pos = look + new Vector3(0.6f * sw, 0.5f, -3.2f);
                    fstop = 5.6f;
                    break;
            }
            rig.Move(cam, pos, look, dt, 3f);
            HqPost.Focus(Vector3.Distance(cam.transform.position, look), fstop, 50f);
        }
    }
}
