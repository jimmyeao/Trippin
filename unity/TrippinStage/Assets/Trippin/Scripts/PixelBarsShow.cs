// unity_pixelbars: a stage wall of 32 LED pixel bars (24 pixels each) on
// motorised tilt mounts in a shallow arc, with a row of blinders on the truss
// above, running a lighting desk's chases through the haze (GearKit + HqGear
// for the hardware, HqPixelBar for the pixels). Units are metres.
//  - Shape: the pixels draw the music. A new look every 4 bars, morphing in
//    over a beat: a VU wall (each bar's height is the spectrum, bass in the
//    middle, a peak pixel falling back), a chase running across on the 16ths,
//    a sine wave rolling over the bars, rings bursting out from the centre
//    bar on each kick (four slots, the most-faded taking a new kick), and a
//    falling sparkle the highs thicken.
//  - Motion: the bars lean on a 2-bar figure on Trippin's beat (the bass
//    sets how far), the wave and sparkle travel on the smooth energy clock,
//    the blinders swell through a build and warm on the drop (eased, never
//    flashed), and the camera drifts through four shots, cutting every 8
//    bars. Count fixed: a breakdown dims the outer bars, nothing switches.
// Controls: pixels, lean, blinders, haze, drift.
using UnityEngine;

namespace TrippinStage
{
    public sealed class PixelBarsShow : KitShow
    {
        public Material gearMat, pixelMat;

        [TrippinParam("Pixels", 0.2f, 2f)] public float pixels = 1f;
        [TrippinParam("Lean", 0f, 2f)] public float lean = 1f;
        [TrippinParam("Blinders", 0f, 2f)] public float blinders = 1f;
        [TrippinParam("Haze", 0f, 2f)] public float hazeAmt = 1f;
        [TrippinParam("Drift", 0f, 2f)] public float drift = 1f;

        const int NB = 32, NP = 24, NBl = 8;
        const float BarH = 2.2f, ArcR = 14f, ArcZ = 6f, TrussY = 6.4f;

        GearMats _m;
        Material _body, _riser, _blind, _px;
        Mesh _barM, _faceM, _mountM, _riserM, _blindM, _lampM, _trussM;
        readonly float[] _v = new float[NB * NP], _v0 = new float[NB * NP], _peak = new float[NB];
        readonly Vector4[] _rings = new Vector4[4];
        readonly Vector4[] _row = new Vector4[6];
        readonly Vector4[] _lp = new Vector4[4], _lc = new Vector4[4];
        int _form = -1, _shot = -1;
        float _t, _bass, _high, _kickPrev, _flat, _imp, _build;

        protected override void Build()
        {
            Env(120f, 0.1f, true);
            _m = new GearMats(gearMat);
            _body = _m.M(0.03f, 0.03f, 0.033f, 0.5f, 0.4f);
            _riser = _m.M(0.02f, 0.02f, 0.022f, 0f, 0.7f);
            _blind = _m.M(0.04f, 0.04f, 0.045f, 0.6f, 0.35f);
            _px = new Material(pixelMat);
            _barM = GearMesh.RoundedBox(new Vector3(0.2f, BarH + 0.08f, 0.12f), 0.02f, 2, "pixel bar");
            _faceM = GearMesh.Quad(0.16f, BarH, "pixel face");
            _mountM = GearMesh.RoundedBox(new Vector3(0.22f, 0.12f, 0.2f), 0.02f, 2, "tilt mount");
            _riserM = GearMesh.RoundedBox(new Vector3(0.34f, 1.2f, 0.34f), 0.02f, 2, "plinth");
            _blindM = GearMesh.RoundedBox(new Vector3(0.62f, 0.34f, 0.18f), 0.03f, 2, "blinder");
            _lampM = GearMesh.Lathe(new[] { new Vector2(0f, 0.02f), new Vector2(0.11f, 0.01f), new Vector2(0.12f, 0f), new Vector2(0f, 0f) }, 20, "blinder lamp");
            _trussM = GearMesh.Truss(18f, 0.29f, "pixel truss");
            for (int i = 0; i < 4; i++) _rings[i] = new Vector4(-99f, 0f, 0f, 0f);
        }

        Vector3 BarPos(int i, out float yaw)
        {
            float a = (i - (NB - 1) * 0.5f) / ((NB - 1) * 0.5f) * 0.55f;      // radians either side
            yaw = a * Mathf.Rad2Deg;
            return new Vector3(Mathf.Sin(a) * ArcR, 1.2f + 0.08f, ArcZ - ArcR + Mathf.Cos(a) * ArcR);
        }

        static float H(int a, int b) => Kit.H(a * 131 + b, 7);

        /// Look k's intensity (0..1) for pixel j of bar i at beat b.
        float Look(int k, int i, int j, float b)
        {
            float x = (i - (NB - 1) * 0.5f) / ((NB - 1) * 0.5f);
            float y = (j + 0.5f) / NP;
            float c = rx.clk;
            switch (k)
            {
                default:
                case 0:   // VU wall: the spectrum from the middle out, a peak pixel on top
                {
                    float lvl = Mathf.Clamp01(rx.Spec(Mathf.Abs(x) * 0.9f + 0.03f) * 1.15f);
                    float on = y < lvl ? 0.55f + 0.45f * y : 0f;
                    float pk = Mathf.Abs(y - _peak[i]) < 0.5f / NP ? 1f : 0f;
                    return Mathf.Max(on, pk);
                }
                case 1:   // a chase across the wall on the 16ths, a tail behind it
                {
                    float head = Mathf.Repeat(b * 4f, NB);
                    float d = Mathf.Repeat(head - i, NB);
                    float tail = Mathf.Exp(-d * 0.45f);
                    return tail * (0.6f + 0.4f * Mathf.Sin(y * Mathf.PI));
                }
                case 2:   // a sine rolling over the bars
                {
                    float mid = 0.5f + 0.36f * Mathf.Sin(x * 6f - c * 1.4f);
                    float w = 0.09f + 0.07f * _bass;
                    return Mathf.Clamp01(1f - Mathf.Abs(y - mid) / w);
                }
                case 3:   // rings out from the centre bar on each kick, a dim base glow
                {
                    float r = Mathf.Abs(x) * 16f;
                    float v = 0.06f;
                    for (int s = 0; s < 4; s++)
                    {
                        float age = _t - _rings[s].x;
                        if (age < 0f || age > 2f) continue;
                        float d = r - age * 22f;
                        v += _rings[s].y * Mathf.Exp(-d * d * 0.2f) * Mathf.Exp(-age * 1.3f) * Mathf.SmoothStep(0f, 1f, age / 0.1f);
                    }
                    return Mathf.Clamp01(v);
                }
                case 4:   // falling sparkle: drops sliding down, more of them with the highs
                {
                    float cy = Mathf.Repeat(y + c * 0.12f * (0.8f + 0.5f * H(i, 1)), 1f);
                    int cell = Mathf.FloorToInt(cy * 6f);
                    float on = H(i, cell + Mathf.FloorToInt(c * 0.12f + H(i, 2)) * 7) < 0.18f + 0.45f * _high ? 1f : 0f;
                    float f = Mathf.Repeat(cy * 6f, 1f);
                    return on * Mathf.Exp(-f * 3.5f);
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
            _high = Eased.Follow(_high, rx.highFast, 14f, 3.5f, dt);
            _flat = Mathf.Lerp(_flat, rx.calm, 1f - Mathf.Exp(-dt * 1.2f));
            _imp = rx.impact > _imp ? Mathf.Lerp(_imp, rx.impact, 1f - Mathf.Exp(-dt * 4f)) : rx.impact;
            _build = Mathf.Lerp(_build, rx.tension, 1f - Mathf.Exp(-dt * 2f));
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && rx.calm < 0.5f;
            _kickPrev = rx.kick;
            if (kick)
            {
                int best = 0;
                for (int i = 1; i < 4; i++) if (_rings[i].x < _rings[best].x) best = i;
                _rings[best] = new Vector4(_t, 0.6f + 0.4f * rx.kick, 0f, 0f);
            }
            for (int i = 0; i < NB; i++)
            {
                float x = (i - (NB - 1) * 0.5f) / ((NB - 1) * 0.5f);
                float lvl = Mathf.Clamp01(rx.Spec(Mathf.Abs(x) * 0.9f + 0.03f) * 1.15f);
                _peak[i] = lvl > _peak[i] ? lvl : Mathf.Max(lvl, _peak[i] - dt * 0.6f);
            }

            int blk = Mathf.FloorToInt(b / 16f);
            int[] order = { 0, 1, 2, 3, 0, 4, 2, 1, 3, 4 };
            int form = order[((blk % order.Length) + order.Length) % order.Length];
            if (form != _form) { System.Array.Copy(_v, _v0, _v.Length); _form = form; }
            float into = MixClock.Ease(Mathf.Repeat(b, 16f));
            float gain = rx.Gain(0.85f, 1.2f) * pixels;
            var ca = Kit.Hue(blk * 0.21f + b / 1024f);
            var cb = Kit.Hue(blk * 0.21f + b / 1024f + 0.18f + 0.2f * Mathf.Sin(rx.phrase * 0.5f));
            _px.SetColor("_ColA", ca); _px.SetColor("_ColB", cb);

            float avg = 0f;
            float leanAmt = lean * (6f + 10f * _bass) * Mathf.Lerp(1f, 0.4f, _flat);
            for (int i = 0; i < NB; i++)
            {
                var p = BarPos(i, out float yaw);
                float x = Mathf.Abs((i - (NB - 1) * 0.5f) / ((NB - 1) * 0.5f));
                float edge = 1f - _flat * Mathf.Lerp(0.2f, 0.85f, x);
                float tilt = leanAmt * Mathf.Sin(b / 8f * Mathf.PI * 2f + i * 0.35f);
                var q = Quaternion.Euler(0f, yaw, 0f) * Quaternion.Euler(0f, 0f, tilt);
                GearDraw.Draw(_riserM, _riser, GearDraw.TRS(new Vector3(p.x, 0.6f, p.z), Quaternion.Euler(0f, yaw, 0f)));
                GearDraw.Draw(_mountM, _body, GearDraw.TRS(p, Quaternion.Euler(0f, yaw, 0f)));
                var pivot = p + new Vector3(0f, 0.06f, 0f);
                var mid = pivot + q * new Vector3(0f, BarH * 0.5f + 0.04f, 0f);
                GearDraw.Draw(_barM, _body, GearDraw.TRS(mid, q));
                for (int j = 0; j < NP; j++)
                {
                    int k = i * NP + j;
                    float want = Look(form, i, j, b);
                    float v = Mathf.Lerp(_v0[k], want, into);
                    if (into >= 1f) _v0[k] = want;
                    _v[k] = v * edge;
                    avg += _v[k];
                }
                for (int r = 0; r < 6; r++)
                    _row[r] = new Vector4(_v[i * NP + r * 4], _v[i * NP + r * 4 + 1], _v[i * NP + r * 4 + 2], _v[i * NP + r * 4 + 3]);
                var pr = GearDraw.Props;
                pr.SetVectorArray("_Px", _row);
                pr.SetFloat("_Gain", gain * 2.5f);
                var face = mid + q * new Vector3(0f, 0f, -0.061f);
                GearDraw.Draw(_faceM, _px, Matrix4x4.TRS(face, q * Quaternion.Euler(-90f, 0f, 0f), Vector3.one), pr);
            }
            avg /= NB * NP;

            // Blinders: they swell through a build and warm on the drop, eased (never flashed).
            float bl = blinders * Mathf.Min(0.5f, 0.02f + 0.3f * _build * _build + 0.4f * _imp);
            var warm = new Color(1f, 0.72f, 0.42f);
            GearDraw.Draw(_trussM, _m.alu, GearDraw.T(new Vector3(0f, TrussY, ArcZ - 0.6f)));
            for (int i = 0; i < NBl; i++)
            {
                var p = new Vector3(-7f + i * 2f, TrussY - 0.35f, ArcZ - 0.8f);
                var q = Quaternion.Euler(18f, 0f, 0f);
                GearDraw.Draw(_blindM, _blind, GearDraw.TRS(p, q));
                for (int l = 0; l < 2; l++)
                {
                    var lp = p + q * new Vector3(-0.15f + 0.3f * l, 0f, -0.095f);
                    var pr = GearDraw.Props; pr.SetColor("_Emit", warm * (0.03f + 2.5f * bl));
                    GearDraw.Draw(_lampM, _m.led, GearDraw.TRS(lp, q * Quaternion.Euler(-90f, 0f, 0f)), pr);
                }
            }

            // The pixels light the room: their average colour from the wall, the blinders from above.
            var wallCol = Color.Lerp(ca, cb, 0.5f) * (0.4f + 3f * avg) * gain;
            _lp[0] = new Vector4(0f, 2f, ArcZ - 1.5f, 12f); _lc[0] = (Vector4)wallCol;
            _lp[1] = new Vector4(-6f, TrussY - 0.5f, ArcZ - 2f, 10f); _lc[1] = (Vector4)(warm * 3f * bl);
            _lp[2] = new Vector4(6f, TrussY - 0.5f, ArcZ - 2f, 10f); _lc[2] = (Vector4)(warm * 3f * bl);
            _lp[3] = new Vector4(0f, 4f, -10f, 14f); _lc[3] = (Vector4)(new Color(0.3f, 0.33f, 0.45f) * 0.25f);
            Shader.SetGlobalVectorArray("_GLPos", _lp);
            Shader.SetGlobalVectorArray("_GLCol", _lc);
            Shader.SetGlobalVector("_GEnvA", (Vector4)(new Color(0.5f, 0.55f, 0.7f) * 0.08f));
            Shader.SetGlobalVector("_GEnvB", new Vector4(ca.r * 0.3f, ca.g * 0.3f, ca.b * 0.3f, 0.4f));
            Shader.SetGlobalVector("_GEnvC", new Vector4(cb.r * 0.3f, cb.g * 0.3f, cb.b * 0.3f, -0.4f));
            hazeScale = hazeAmt * (1f + 0.5f * _build);

            int sb = Mathf.FloorToInt(b / 32f);
            if (sb != _shot) { _shot = sb; rig.Snap(); }
            float u = Mathf.Repeat(b, 32f) / 32f;
            float sw = Mathf.Sin(u * Mathf.PI - Mathf.PI * 0.5f) * drift;
            var wall = new Vector3(0f, 2.4f, ArcZ);
            Vector3 pos, look;
            switch (((sb % 4) + 4) % 4)
            {
                default:
                case 0: pos = new Vector3(2.5f * sw, 1.8f, -4.5f); look = wall + new Vector3(0f, 0.6f, 0f); break;
                case 1: pos = new Vector3(-7f, 2.6f, -1f + sw); look = wall + new Vector3(2f, 0.6f, 0f); break;
                case 2: pos = new Vector3(3.5f + sw, 2.2f, 1.5f); look = new Vector3(-1.5f, 2.4f, ArcZ); break;
                case 3: pos = new Vector3(1.5f * sw, 5.5f, -4f); look = wall + new Vector3(0f, 0.3f, 0f); break;
            }
            rig.Move(cam, pos, look, dt, 3f);
            HqPost.Focus(Vector3.Distance(cam.transform.position, look), 5.6f, 50f);
        }
    }
}
