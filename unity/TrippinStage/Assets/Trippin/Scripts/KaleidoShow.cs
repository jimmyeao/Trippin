// unity_kaleido: the HQ version of the 2D kaleido scene, built the way a
// real kaleidoscope works. A hidden camera looks into a chamber of glass
// shards (HqShard.shader, Alice-generated dichroic and stained glass) lit
// from behind by a glowing glass panel; HqKaleido.shader folds that live
// image through the mirrors.
//  - Shape: the bass pushes the shards apart (eased) and the zoom breathes
//    with it; each kick gives the tumble a spin; the highs make the glints
//    flare.
//  - Motion: the shards tumble and the mirrors turn on the smooth energy
//    clock, turning back the other way with the phrase. Every 8 bars the
//    mirrors change between a mandala and a three-mirror triangle, over a
//    beat.
//  - Breakdowns: the chamber slows and gathers, the image goes soft.
// Controls: segments, spin, tumble, shard spread, zoom, glints, colour split, colour.
using UnityEngine;

namespace TrippinStage
{
    public sealed class KaleidoShow : KitShow
    {
        public Material kaleidoMat, shardMat, glassMat, backMat;
        public Texture glassA, glassB;

        [TrippinParam("Segments", 4f, 16f)] public float segments = 8f;
        [TrippinParam("Spin", 0f, 3f)] public float spin = 1f;
        [TrippinParam("Tumble", 0f, 3f)] public float tumble = 1f;
        [TrippinParam("Shard spread", 0.5f, 2f)] public float spreadAmt = 1f;
        [TrippinParam("Zoom", 0.5f, 2f)] public float zoom2 = 1f;
        [TrippinParam("Glints", 0f, 3f)] public float glints = 1f;
        [TrippinParam("Colour split", 0f, 3f)] public float split = 1f;
        [TrippinParam("Colour", 0f, 1f)] public float hue = 0f;

        public override bool UsesCamera => false;

        const int Shards = 110;
        static readonly Vector3 Chamber = new Vector3(0f, -5000f, 0f);
        Material _k, _back;
        Camera _kcam;
        RenderTexture _rt;
        readonly Transform[] _shard = new Transform[Shards];
        readonly Material[] _sm = new Material[Shards], _gm = new Material[Shards];
        readonly Vector3[] _home = new Vector3[Shards], _axis = new Vector3[Shards];
        readonly ClkAcc _roll = new ClkAcc();
        float _kickPrev, _spinKick, _mode, _push;

        protected override void Build()
        {
            _k = Kit.Fullscreen(transform, kaleidoMat, "kaleidoscope");
            _rt = new RenderTexture(1024, 1024, 24, RenderTextureFormat.ARGBHalf) { name = "kaleido chamber" };
            _rt.Create();
            _k.SetTexture("_Src", _rt);
            // `-kaleidoRaw`: show the chamber camera's own view (checking the source).
            if (System.Array.IndexOf(System.Environment.GetCommandLineArgs(), "-kaleidoRaw") >= 0) _k.SetFloat("_Raw", 1f);
            // The full-screen pass draws in every camera: keep it out of the
            // chamber camera's view (it would film its own output).
            const int PassLayer = 30;
            transform.Find("kaleidoscope").gameObject.layer = PassLayer;

            var cg = new GameObject("kaleido camera");
            cg.transform.SetParent(transform, false);
            cg.transform.position = Chamber + new Vector3(0f, 0f, -14f);
            _kcam = cg.AddComponent<Camera>();
            _kcam.targetTexture = _rt;
            _kcam.clearFlags = CameraClearFlags.SolidColor;
            _kcam.backgroundColor = Color.black;
            _kcam.fieldOfView = 40f;
            _kcam.nearClipPlane = 0.5f;
            _kcam.farClipPlane = 60f;
            _kcam.enabled = false;   // rendered by hand each frame (headless)
            _kcam.cullingMask = ~(1 << PassLayer);
            var acd = cg.AddComponent<UnityEngine.Rendering.Universal.UniversalAdditionalCameraData>();
            acd.renderPostProcessing = false;

            // The light box behind the shards: a glowing glass panel.
            var back = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(back.GetComponent<Collider>());
            back.name = "light box";
            back.transform.SetParent(transform, false);
            back.transform.position = Chamber + new Vector3(0f, 0f, 9f);
            back.transform.localScale = new Vector3(26f, 26f, 1f);
            _back = new Material(backMat);
            _back.SetTexture("_MainTex", glassA);
            back.GetComponent<Renderer>().sharedMaterial = _back;

            for (int i = 0; i < Shards; i++)
            {
                var g = new GameObject("shard " + i);
                g.transform.SetParent(transform, false);
                g.AddComponent<MeshFilter>().sharedMesh = ShardMesh(i);
                var mr = g.AddComponent<MeshRenderer>();
                _sm[i] = new Material(shardMat);
                _sm[i].SetTexture("_MainTex", i % 3 == 0 ? glassA : glassB);
                _sm[i].SetTextureOffset("_MainTex", new Vector2(Kit.H(i, 4), Kit.H(i, 5)));
                _sm[i].SetTextureScale("_MainTex", Vector2.one * (0.25f + 0.3f * Kit.H(i, 6)));
                _gm[i] = new Material(glassMat);
                _gm[i].SetTexture("_MainTex", _sm[i].GetTexture("_MainTex"));
                _gm[i].SetTextureOffset("_MainTex", _sm[i].GetTextureOffset("_MainTex"));
                _gm[i].SetTextureScale("_MainTex", _sm[i].GetTextureScale("_MainTex"));
                // Two materials on one submesh: Unity draws the mesh with each.
                mr.sharedMaterials = new[] { _gm[i], _sm[i] };
                mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
                _shard[i] = g.transform;
                float a = Kit.H(i, 1) * Mathf.PI * 2f, r = Mathf.Sqrt(Kit.H(i, 2)) * 5.5f;
                _home[i] = new Vector3(Mathf.Cos(a) * r, Mathf.Sin(a) * r, (Kit.H(i, 3) - 0.5f) * 6f);
                _axis[i] = new Vector3(Kit.H(i, 7) - 0.5f, Kit.H(i, 8) - 0.5f, Kit.H(i, 9) - 0.5f).normalized;
            }
        }

        void OnDestroy() { if (_rt != null) _rt.Release(); }

        // An irregular flat shard: a random convex polygon, extruded thin.
        static Mesh ShardMesh(int seed)
        {
            int sides = 4 + (int)(Kit.H(seed, 11) * 4f);
            float size = 1.0f + 1.8f * Kit.H(seed, 12);
            float thick = 0.06f + 0.06f * Kit.H(seed, 13);
            var ring = new Vector3[sides];
            float a0 = Kit.H(seed, 14) * 6.28f;
            for (int k = 0; k < sides; k++)
            {
                float a = a0 + (k + 0.6f * (Kit.H(seed * 31 + k, 15) - 0.5f)) / sides * Mathf.PI * 2f;
                float r = size * (0.55f + 0.45f * Kit.H(seed * 17 + k, 16)) * (k % 2 == 0 ? 1f : 0.8f + 0.4f * Kit.H(seed, 17));
                ring[k] = new Vector3(Mathf.Cos(a) * r, Mathf.Sin(a) * r * (0.5f + Kit.H(seed, 18)), 0f);
            }
            var v = new System.Collections.Generic.List<Vector3>();
            var n = new System.Collections.Generic.List<Vector3>();
            var uv = new System.Collections.Generic.List<Vector2>();
            var t = new System.Collections.Generic.List<int>();
            foreach (float side in new[] { 1f, -1f })
            {
                int b = v.Count;
                v.Add(new Vector3(0, 0, side * thick)); n.Add(new Vector3(0, 0, side)); uv.Add(new Vector2(0.5f, 0.5f));
                for (int k = 0; k < sides; k++)
                {
                    v.Add(ring[k] + new Vector3(0, 0, side * thick)); n.Add(new Vector3(0.15f * ring[k].x, 0.15f * ring[k].y, side).normalized);
                    uv.Add(new Vector2(0.5f + ring[k].x / 5f, 0.5f + ring[k].y / 5f));
                }
                for (int k = 0; k < sides; k++)
                {
                    int a = b + 1 + k, c = b + 1 + (k + 1) % sides;
                    if (side > 0) { t.Add(b); t.Add(c); t.Add(a); } else { t.Add(b); t.Add(a); t.Add(c); }
                }
            }
            for (int k = 0; k < sides; k++)   // the cut edges
            {
                var p0 = ring[k]; var p1 = ring[(k + 1) % sides];
                var en = Vector3.Cross(p1 - p0, Vector3.forward).normalized;
                int b = v.Count;
                v.Add(p0 + new Vector3(0, 0, thick)); v.Add(p1 + new Vector3(0, 0, thick)); v.Add(p0 - new Vector3(0, 0, thick)); v.Add(p1 - new Vector3(0, 0, thick));
                for (int q = 0; q < 4; q++) { n.Add(-en); uv.Add(new Vector2(0.5f, 0.5f)); }
                t.Add(b); t.Add(b + 2); t.Add(b + 1); t.Add(b + 1); t.Add(b + 2); t.Add(b + 3);
            }
            var m = new Mesh { name = "shard" };
            m.SetVertices(v); m.SetNormals(n); m.SetUVs(0, uv); m.SetTriangles(t, 0);
            m.RecalculateBounds();
            return m;
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && rx.calm < 0.6f;
            _kickPrev = rx.kick;
            if (kick) _spinKick += 0.8f + 0.8f * rx.bassFast;
            _spinKick *= Mathf.Exp(-dt * 2.5f);
            _push = Eased.Follow(_push, rx.bassFast, 10f, 3f, dt);
            float swing = Mathf.Sin(rx.phrase * 0.5f);
            float roll = _roll.Step(rx.clk, tumble) * 0.12f + _spinKick * 0.15f;
            float gather = 1f - 0.45f * rx.calm;
            float spread = spreadAmt * gather * (0.8f + 0.5f * _push + 0.4f * rx.impact);

            for (int i = 0; i < Shards; i++)
            {
                var h = _home[i];
                float a = roll * (0.4f + 0.6f * Kit.H(i, 20)) * (i % 2 == 0 ? 1f : -1f) + swing * 0.4f;
                var p = new Vector3(h.x * Mathf.Cos(a) - h.y * Mathf.Sin(a), h.x * Mathf.Sin(a) + h.y * Mathf.Cos(a), h.z) * spread;
                _shard[i].position = Chamber + p;
                _shard[i].rotation = Quaternion.AngleAxis((roll * 60f + i * 37f) * (0.5f + Kit.H(i, 21)), _axis[i]);
                Color c = Kit.Hue(hue + Kit.H(i, 22) * 0.7f + 0.1f * swing);
                _sm[i].SetColor("_Tint", Color.Lerp(Color.white, c, 0.3f));
                _sm[i].SetFloat("_Gain", 1f);
                _gm[i].SetColor("_Tint", Color.Lerp(Color.white, c, 0.3f));
                _gm[i].SetFloat("_Filter", 0.82f);
                _gm[i].SetFloat("_Gain", 0.9f * (0.8f + 0.4f * rx.lum));
                _sm[i].SetFloat("_Glint", glints * (0.6f + 1.4f * rx.highFast));
            }
            // The light box: a heap of backlit dichroic glass (Alice), jewel
            // colours on black, drifting slowly; the shards filter it.
            _back.SetFloat("_Haze", 0f);
            _back.SetFloat("_Lit", 0.5f + 0.4f * rx.highFast);
            _back.SetFloat("_Gain", 1.25f * (0.75f + 0.35f * rx.lum));
            _back.SetTextureOffset("_MainTex", new Vector2(Mathf.Sin(roll * 0.05f) * 0.2f, Mathf.Cos(roll * 0.04f) * 0.2f));
            _kcam.Render();

            // Mandala or triangle: a new mirror every 8 bars, over a beat.
            int block = Mathf.FloorToInt(rx.beatS / 32f);
            float want = (block & 1) == 0 ? 0f : 1f;
            _mode = Mathf.MoveTowards(_mode, want, dt * Mathf.Clamp(s.bpm, 60f, 200f) / 60f);
            _k.SetFloat("_Mode", Mathf.SmoothStep(0f, 1f, _mode));
            _k.SetFloat("_Seg", Mathf.Round(segments));
            _k.SetFloat("_Rot", TurnAngle(dt, swing));
            _k.SetFloat("_Zoom", zoom2 * (0.9f + 0.18f * _push) * (1f + 0.25f * rx.calm));
            _k.SetFloat("_Split", split);
            _k.SetFloat("_Gain", rx.Gain(0.8f, 1.25f));
        }

        // The mirrors' angle: turning with the energy clock at Spin, the
        // direction swinging through zero with the phrase.
        float _angle, _clkPrev = -1f;
        float TurnAngle(float dt, float swing)
        {
            float d = _clkPrev < 0f ? 0f : rx.clk - _clkPrev;
            _clkPrev = rx.clk;
            if (d < -2048f) d += 4096f;
            if (d < 0f || d > 4f) d = 0f;
            _angle += d * 0.06f * spin * swing;
            return _angle;
        }
    }
}
