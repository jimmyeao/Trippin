// unity_jellyfish: the HQ version of the 2D deep-sea scenes: glowing
// jellyfish drifting up through dark water (HqJelly.shader bells, tentacles
// and frilled oral arms as TubeRibbons trailing behind them), marine snow
// drifting past (HqSnow.shader) and shafts of light from the surface far
// above (HqDeep.shader).
//  - Shape: each kick makes the next jellyfish contract its bell, which
//    thrusts it upward and sends a wave down its tentacles; the bass makes
//    every bell breathe; the drop pulses them all at once.
//  - Motion: the jellies swim in pulses and drift on a current that swings
//    with the phrase; the tentacles trail and sway behind them; the camera
//    sinks slowly alongside.
// Controls: jellies, size, pulse strength, tentacle length, glow, snow.
using UnityEngine;

namespace TrippinStage
{
    public sealed class JellyfishShow : KitShow
    {
        public Material jellyMat, waterMat, snowMat;

        [TrippinParam("Jellies", 2f, 8f)] public float count = 7f;
        [TrippinParam("Size", 0.5f, 1.8f)] public float size = 1f;
        [TrippinParam("Pulse", 0.2f, 2f)] public float pulse = 1f;
        [TrippinParam("Tentacles", 0.3f, 2f)] public float tentacle = 1f;
        [TrippinParam("Glow", 0.3f, 2.5f)] public float glow = 1f;
        [TrippinParam("Snow", 0f, 1f)] public float snow = 0f;   // off: the specks read as stars (owner)

        public override bool UsesCamera => true;

        const int Max = 8, Tent = 6, Arms = 3, Pts = 24;
        const float CamY = 9f;
        Mesh _bell;
        Material _deep, _snow;
        RenderParams _snowRp;
        readonly RenderParams[] _rp = new RenderParams[Max];
        readonly MaterialPropertyBlock[] _mpb = new MaterialPropertyBlock[Max];
        readonly TubeRibbon[,] _tube = new TubeRibbon[Max, Tent + Arms];
        readonly Vector3[] _p = new Vector3[Max], _v = new Vector3[Max];
        readonly float[] _age = new float[Max], _size = new float[Max], _hue = new float[Max], _seed = new float[Max], _wave = new float[Max];
        readonly ClkAcc _clk = new ClkAcc();
        float _kickPrev, _breath, _t;
        int _next, _spawned;

        protected override void Build()
        {
            _deep = Kit.Fullscreen(transform, waterMat, "deep water");
            _bell = new Mesh { name = "bell" };
            const int U = 48, Vn = 24;
            var verts = new Vector3[(U + 1) * (Vn + 1)];
            var uv = new Vector2[verts.Length];
            var tris = new int[U * Vn * 6];
            for (int y = 0; y <= Vn; y++)
                for (int x = 0; x <= U; x++)
                    uv[y * (U + 1) + x] = new Vector2(x / (float)U, y / (float)Vn);
            int k = 0;
            for (int y = 0; y < Vn; y++)
                for (int x = 0; x < U; x++)
                {
                    int a = y * (U + 1) + x, b = a + 1, c = a + U + 1, d = c + 1;
                    tris[k++] = a; tris[k++] = c; tris[k++] = b; tris[k++] = b; tris[k++] = c; tris[k++] = d;
                }
            _bell.vertices = verts;
            _bell.uv = uv;
            _bell.triangles = tris;
            _bell.bounds = new Bounds(Vector3.zero, Vector3.one * 4f);
            for (int i = 0; i < Max; i++)
            {
                _mpb[i] = new MaterialPropertyBlock();
                _rp[i] = new RenderParams(jellyMat) { matProps = _mpb[i] };
                for (int j = 0; j < Tent + Arms; j++)
                {
                    var tb = new TubeRibbon(transform, ribbonMat, Pts, j < Tent ? 4 : 6, "tentacle " + i + "." + j);
                    tb.Mat.SetFloat("_PulseAmt", 0.5f);
                    tb.Mat.SetFloat("_PulseFreq", 3f);
                    tb.Mat.SetFloat("_Core", j < Tent ? 4f : 2.5f);
                    tb.Mat.SetFloat("_White", 0.25f);
                    _tube[i, j] = tb;
                }
                Spawn(i, true);
            }
            _snow = new Material(snowMat);
            _snowRp = new RenderParams(_snow) { worldBounds = new Bounds(new Vector3(0f, 9f, 30f), Vector3.one * 140f) };
        }

        void Spawn(int i, bool anywhere)
        {
            _spawned++;
            float z = Mathf.Lerp(3f, 26f, Kit.H(_spawned, 1));
            float half = (z + 12f) * 0.48f;
            float x = (Kit.H(_spawned, 2) - 0.5f) * 2f * half * 1.5f;
            float y = anywhere ? CamY + (Kit.H(_spawned, 3) - 0.5f) * 2f * half : CamY - half * 0.85f - 1.5f;
            _p[i] = new Vector3(x, y, z);
            _v[i] = Vector3.up * 0.3f;
            _size[i] = Mathf.Lerp(1.1f, 2.3f, Kit.H(_spawned, 4));
            _hue[i] = Kit.H(_spawned, 5);
            _seed[i] = Kit.H(_spawned, 6);
            _age[i] = 5f;
        }

        static float Contract(float age) => age < 0.22f ? Mathf.SmoothStep(0f, 1f, age / 0.22f) : Mathf.Exp(-(age - 0.22f) * 3f);

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            bool calm = rx.calm > 0.5f;
            float c = _clk.Step(rx.clk, 1f);
            _t = c * 0.5f;
            _breath = Eased.Follow(_breath, rx.bassFast, 12f, 3f, dt);
            int n = Mathf.Clamp(Mathf.RoundToInt(count), 1, Max);
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && !calm;
            _kickPrev = rx.kick;
            if (kick)
            {
                for (int tries = 0; tries < n; tries++)
                {
                    int i = _next++ % n;
                    if (_age[i] > 0.8f) { _age[i] = 0f; break; }
                }
            }
            if (rx.dropped) for (int i = 0; i < n; i++) _age[i] = 0f;
            float swing = Mathf.Sin(rx.phrase * 0.5f);
            float gain = rx.Gain(0.85f, 1.2f);

            for (int i = 0; i < Max; i++)
            {
                bool on = i < n;
                float sz = _size[i] * size;
                float prevAge = _age[i];
                _age[i] += dt;
                // Pulse on its own when the music leaves it alone.
                if (_age[i] > (calm ? 2.6f : 4.5f) + 2f * _seed[i]) _age[i] = 0f;
                if (prevAge < 0.22f && _age[i] >= 0.22f) _wave[i] = 0f;
                _wave[i] += dt;
                float con = Mathf.Clamp01(Contract(_age[i]) * pulse + 0.15f * _breath);
                // The bell leans into its swim; the current carries it sideways.
                var axis = (Vector3.up + new Vector3(Mathf.Sin(_t * 0.3f + _seed[i] * 6f) * 0.25f + swing * 0.15f, 0f, Mathf.Cos(_t * 0.23f + _seed[i] * 4f) * 0.15f)).normalized;
                if (_age[i] < 0.22f) _v[i] += axis * (sz * 1.3f * pulse * dt / 0.22f);
                _v[i] += (new Vector3(swing * 0.25f, -0.05f, 0f) - _v[i]) * (1f - Mathf.Exp(-dt * 0.9f));
                _p[i] += _v[i] * dt;
                float half = (_p[i].z + 12f) * 0.48f;
                if (_p[i].y > CamY + half + 3f + sz * 2f || Mathf.Abs(_p[i].x) > half * 2.2f) Spawn(i, false);

                var rot = Quaternion.FromToRotation(Vector3.up, axis);
                _mpb[i].SetFloat("_Contract", con);
                _mpb[i].SetFloat("_Hue", _hue[i] + rx.beatS / 1024f);
                _mpb[i].SetFloat("_T", _t + _seed[i] * 10f);
                _mpb[i].SetFloat("_Seed", _seed[i]);
                _mpb[i].SetFloat("_Gain", on ? gain * glow * 1.3f : 0f);
                if (on) Graphics.RenderMesh(_rp[i], _bell, 0, Matrix4x4.TRS(_p[i], rot, Vector3.one * sz));

                // Tentacles from the rim, arms from the middle, trailing behind the swim.
                float rimR = Mathf.Sin(1.3f) * (1f - 0.3f * con) * sz * 0.92f;
                float rimY = (Mathf.Cos(1.3f) * (1f + 0.2f * con) - 0.25f * con) * sz;
                var side1 = rot * Vector3.right;
                var side2 = rot * Vector3.forward;
                for (int j = 0; j < Tent + Arms; j++)
                {
                    var tb = _tube[i, j];
                    bool arm = j >= Tent;
                    float a = arm ? (j - Tent) * 2.0944f + 0.5f : j * 1.0472f;
                    float rr = arm ? rimR * 0.18f : rimR;
                    var anchor = _p[i] + rot * new Vector3(Mathf.Cos(a) * rr, arm ? rimY + 0.15f * sz : rimY, Mathf.Sin(a) * rr);
                    float len = sz * tentacle * (arm ? 2.2f : 4.2f + 1.2f * Kit.H(i * 16 + j, 3));
                    for (int k = 0; k < Pts; k++)
                    {
                        float t = k / (Pts - 1f);
                        // A wave runs down from the bell after each pulse.
                        float wv = Mathf.Exp(-Mathf.Pow((t - _wave[i] * 0.9f) / 0.15f, 2f)) * 0.25f;
                        float sway = Mathf.Sin(_t * 1.6f - t * 5f + j * 1.3f + _seed[i] * 7f) * (0.08f + wv) * len * t;
                        float sway2 = Mathf.Cos(_t * 1.2f - t * 4f + j * 2.1f) * 0.06f * len * t;
                        var pt = anchor - axis * len * t - _v[i] * len * t * t * 0.35f + side1 * sway + side2 * sway2;
                        if (arm) pt += side1 * Mathf.Sin(t * 14f + j) * 0.04f * sz;
                        tb.P[k] = pt;
                        tb.R[k] = (arm ? 0.05f * (1f - 0.6f * t) * (1f + 0.4f * Mathf.Sin(t * 30f + j)) : 0.012f * (1f - 0.5f * t)) * sz;
                        tb.K[k] = on ? (1f - t * 0.85f) * (arm ? 0.55f : 0.8f) * glow : 0f;
                        tb.Hu[k] = t * 0.08f;
                    }
                    tb.Mat.SetFloat("_Hue", _hue[i] + rx.beatS / 1024f + (arm ? 0.12f : 0f));
                    tb.Mat.SetFloat("_Phase", -_t * 0.8f + i);
                    tb.Apply(on ? gain * 0.9f : 0f);
                }
            }

            _deep.SetFloat("_T", c);
            _deep.SetFloat("_Sway", swing);
            _deep.SetFloat("_Hue", rx.beatS / 1024f);
            _deep.SetFloat("_Gain", 0.9f + 0.2f * rx.lum);
            if (snow > 0f)
            {
                _snow.SetFloat("_T", c * 0.15f);
                _snow.SetFloat("_Wind", swing * 2f + c * 0.05f);
                _snow.SetVectorArray("_Gust", new[] { Vector4.zero, Vector4.zero });
                _snow.SetFloat("_Warmth", 0f);
                _snow.SetFloat("_Size", 0.7f);
                _snow.SetFloat("_Gain", 0.22f * gain);
                int flakes = Mathf.RoundToInt(5000 * snow);
                _snow.SetFloat("_Flakes", flakes);
                _snow.SetFloat("_Bulbs", 0f);
                Graphics.RenderPrimitives(_snowRp, MeshTopology.Triangles, flakes * 6);
            }

            var camPos = new Vector3(Mathf.Sin(rx.phrase * 0.25f) * 1.5f, CamY, -12f);
            rig.Move(cam, camPos, camPos + new Vector3(0f, 1.5f, 20f), dt, 0.4f);
        }
    }
}
