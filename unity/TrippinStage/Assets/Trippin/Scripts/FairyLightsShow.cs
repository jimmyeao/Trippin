// unity_bokeh: the HQ version of the 2D bokeh lights scene, done with a
// real lens. Strings of fairy lights hang at different depths in the dark
// with a far city glittering behind; the camera's depth of field turns
// every bulb that isn't in focus into a soft bokeh disc, and the focus
// racks from string to string with the music.
//  - Shape: each bulb brightens with the band of the spectrum under it (the
//    strings read left to right as an EQ); each kick sends a chase of light
//    running along the strings.
//  - Motion: every 8 bars the focus pulls to another string (over a second
//    or so); the strings sway; breakdowns let everything melt into bokeh.
// Controls: focus pull, blur, twinkle, chase, warmth, sway.
using UnityEngine;

namespace TrippinStage
{
    public sealed class FairyLightsShow : KitShow
    {
        public Material bulbMat;

        [TrippinParam("Focus pull", 0f, 1f)] public float pull = 1f;
        [TrippinParam("Blur", 0.3f, 2f)] public float blur = 1f;
        [TrippinParam("Twinkle", 0f, 2f)] public float twinkle = 1f;
        [TrippinParam("Chase", 0f, 2f)] public float chase = 1f;
        [TrippinParam("Warmth", 0f, 1f)] public float warmth = 0.5f;
        [TrippinParam("Sway", 0f, 3f)] public float sway = 1f;

        const int Strands = 7, PerStrand = 60, City = 90, Bulbs = Strands * PerStrand + City;
        static readonly float[] Depth = { 2.2f, 4.5f, 7f, 10.5f, 15f, 22f, 32f };
        // Each string's height at its two ends: draped across the frame at
        // different heights and tilts, not stacked in one band.
        static readonly Vector2[] Ends = { new Vector2(4.6f, 2.4f), new Vector2(0.4f, 3.2f), new Vector2(6.5f, 4.8f), new Vector2(1.2f, 0.2f), new Vector2(8.5f, 9.5f), new Vector2(3.5f, 6.5f), new Vector2(11f, 8f) };
        Material _m;
        readonly Vector4[] _col = new Vector4[512];
        readonly float[] _u = new float[Bulbs];        // position along its string (0..1), -1 for the city
        readonly float[] _strand = new float[Bulbs];
        float _t, _kickPrev, _focus = 9.5f, _chaseT = 99f;
        int _target = 2;

        protected override void Build()
        {
            _m = new Material(bulbMat);
            var ico = SculptureShow.IcoSphere(1);
            var iv = ico.vertices; var it = ico.triangles;
            var v = new System.Collections.Generic.List<Vector3>();
            var n = new System.Collections.Generic.List<Vector3>();
            var uv = new System.Collections.Generic.List<Vector2>();
            var t = new System.Collections.Generic.List<int>();
            int b = 0;
            for (int s = 0; s < Strands; s++)
            {
                float z = Depth[s], half = 2.5f + z * 0.95f;
                float sag = 0.5f + 0.1f * z;
                for (int k = 0; k < PerStrand; k++, b++)
                {
                    float u = (k + 0.5f) / PerStrand;
                    float x = (u - 0.5f) * 2f * half;
                    float y = Mathf.Lerp(Ends[s].x, Ends[s].y, u) * (0.5f + z * 0.05f) - sag * (1f - (2f * u - 1f) * (2f * u - 1f));
                    _u[b] = u; _strand[b] = s;
                    Add(v, n, uv, t, iv, it, new Vector3(x, y, z), 0.05f, b, s * 1.7f + u * 3f);
                }
            }
            for (int k = 0; k < City; k++, b++)
            {
                // The far city: a band of lights low on the horizon.
                var p = new Vector3((Kit.H(k, 3) - 0.5f) * 260f, 0.5f + Kit.H(k, 4) * Kit.H(k, 5) * 18f, 120f + Kit.H(k, 6) * 90f);
                _u[b] = -1f; _strand[b] = -1f;
                Add(v, n, uv, t, iv, it, p, 0.35f + 0.3f * Kit.H(k, 7), b, 0f);
            }
            var mesh = new Mesh { name = "fairy lights", indexFormat = UnityEngine.Rendering.IndexFormat.UInt32 };
            mesh.SetVertices(v); mesh.SetNormals(n); mesh.SetUVs(0, uv); mesh.SetTriangles(t, 0);
            mesh.bounds = new Bounds(new Vector3(0, 0, 100), Vector3.one * 600f);
            Kit.Part(transform, "fairy lights", mesh, _m, Vector3.zero, Vector3.one);
        }

        static void Add(System.Collections.Generic.List<Vector3> v, System.Collections.Generic.List<Vector3> n,
                        System.Collections.Generic.List<Vector2> uv, System.Collections.Generic.List<int> t,
                        Vector3[] iv, int[] it, Vector3 at, float r, int bulb, float phase)
        {
            int start = v.Count;
            foreach (var p in iv) { v.Add(at + p.normalized * r); n.Add(p.normalized); uv.Add(new Vector2(bulb, phase)); }
            foreach (var i in it) t.Add(start + i);
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            _t += dt * (0.6f + 0.6f * rx.lum);
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && rx.calm < 0.6f;
            _kickPrev = rx.kick;
            if (kick) _chaseT = 0f;
            _chaseT += dt;
            float gain = rx.Gain(0.8f, 1.25f);
            Color warm = new Color(1f, 0.62f, 0.3f);
            for (int b = 0; b < Bulbs; b++)
            {
                Color c;
                float lvl;
                if (_u[b] < 0f)
                {
                    c = Color.Lerp(warm, Kit.Hue(Kit.H(b, 9)), 0.4f);
                    lvl = 0.9f + 0.5f * Mathf.Sin(_t * (0.5f + Kit.H(b, 8)) + b);
                }
                else
                {
                    c = Color.Lerp(Kit.Hue(_strand[b] * 0.13f + _u[b] * 0.3f), warm, warmth);
                    float band = rx.Spec(_u[b] * 0.9f);
                    float tw = 0.5f + 0.5f * Mathf.Sin(_t * (2f + 3f * Kit.H(b, 2)) + b * 1.3f);
                    float wave = (_u[b] * 1.6f - _chaseT * 2.2f) / 0.08f;
                    lvl = 0.9f + twinkle * (1.2f * band + 0.25f * tw) + chase * 2.5f * Mathf.Exp(-wave * wave);
                }
                c *= 2.1f * lvl * gain;
                _col[b] = new Vector4(c.r, c.g, c.b, 1f);
            }
            _m.SetVectorArray("_BulbCol", _col);
            _m.SetFloat("_T", _t);
            _m.SetFloat("_Sway", sway);

            // The focus pull: another string every 8 bars, racked over a second or so.
            int block = Mathf.FloorToInt(rx.beatS / 32f);
            int[] order = { 3, 5, 2, 4, 6, 3 };
            _target = order[((block % order.Length) + order.Length) % order.Length];
            var camPos = new Vector3(Mathf.Sin(rx.phrase * 0.5f) * 1.2f, 2.4f, -1f);
            float want = Mathf.Lerp(9.5f, Depth[_target] + 1f, pull);
            if (rx.calm > 0.5f) want = 1.2f;     // a breakdown lets it all melt into bokeh
            _focus = Mathf.Lerp(_focus, want, 1f - Mathf.Exp(-dt * 1.6f));
            rig.Move(cam, camPos, camPos + new Vector3(0f, 1.6f, 12f), dt, 0.6f);
            // 85 mm at about f/2.8: the string in focus is crisp, the others
            // soften into bokeh (220 mm wide open left nothing sharp).
            HqPost.Focus(_focus, Mathf.Lerp(4f, 1.8f, Mathf.Clamp01(blur * 0.5f)), 85f);
        }
    }
}
