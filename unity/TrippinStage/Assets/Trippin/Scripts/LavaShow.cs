// unity_lava: the HQ version of the 2D lava scenes: three lava lamps on a
// glossy table in a dark room (HqLava.shader, ray marched), blobs of wax
// rising, stretching, sinking and melting back into the pool, glowing
// through their own thickness in a coloured liquid.
//  - Shape: the bass swells the wax; each kick sends a blob up faster in
//    the next lamp along; the drop melts every lamp's wax together (the
//    union loosens) and boils it.
//  - Motion: the blobs rise and sink on a heat clock that runs on the
//    smooth energy clock, each at its own pace; the camera sways slowly.
// Controls: heat, swell, melt, glow, wax colour, lamps.
using UnityEngine;

namespace TrippinStage
{
    public sealed class LavaShow : KitShow
    {
        public Material lavaMat;

        [TrippinParam("Heat", 0.2f, 3f)] public float heat = 1f;
        [TrippinParam("Swell", 0f, 2f)] public float swell = 1f;
        [TrippinParam("Melt", 0f, 1f)] public float melt = 0.25f;
        [TrippinParam("Glow", 0.3f, 2.5f)] public float glow = 1f;
        [TrippinParam("Wax colour", 0f, 1f)] public float hue = 0f;
        [TrippinParam("Kick boost", 0f, 2f)] public float boost = 1f;

        public override bool UsesCamera => true;

        const float BaseH = 0.38f;
        static readonly Vector3[] Pos = { new Vector3(-2.3f, 0f, 0.7f), new Vector3(0f, 0f, -0.2f), new Vector3(2.3f, 0f, 0.7f) };
        static readonly float[] Height = { 1.6f, 1.85f, 1.6f };
        Material _m;
        readonly ClkAcc _clk = new ClkAcc();
        readonly float[] _ph = new float[24], _w = new float[24], _boost = new float[24], _r = new float[24], _seed = new float[24];
        readonly Vector4[] _blob = new Vector4[24], _lamp = new Vector4[3], _wax = new Vector4[3], _liq = new Vector4[3];
        float _prevClk = -1f, _kickPrev, _swell, _meltBoost;
        int _nextLamp;

        protected override void Build()
        {
            _m = Kit.Fullscreen(transform, lavaMat, "lamps");
            for (int i = 0; i < 24; i++)
            {
                _seed[i] = Kit.H(i, 1);
                _ph[i] = Kit.H(i, 2) * Mathf.PI * 2f;
                _w[i] = 0.55f + 0.6f * Kit.H(i, 3);
                _r[i] = 0.08f + 0.08f * Kit.H(i, 4);
            }
        }

        static float GlassR(float u) => 0.2f + 0.3f * Mathf.Sin(Mathf.PI * (Mathf.Clamp01(u) * 0.8f + 0.1f));

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            bool calm = rx.calm > 0.5f;
            float c = _clk.Step(rx.clk, 1f);
            float dc = _prevClk < 0f ? 0f : Mathf.Max(0f, c - _prevClk);
            _prevClk = c;
            _swell = Eased.Follow(_swell, rx.bassFast, 12f, 2.5f, dt);
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && !calm;
            _kickPrev = rx.kick;
            if (kick)
            {
                // The lowest blob in the next lamp along gets a push upward.
                int l = _nextLamp++ % 3, best = l * 8;
                float lowest = 99f;
                for (int k = 0; k < 7; k++) { int i = l * 8 + k; float y = 0.5f - 0.5f * Mathf.Cos(_ph[i]); if (y < lowest && Mathf.Sin(_ph[i]) > -0.2f) { lowest = y; best = i; } }
                _boost[best] = Mathf.Max(_boost[best], boost * 2.5f);
            }
            if (rx.dropped) { _meltBoost = 1f; for (int i = 0; i < 24; i++) _boost[i] = Mathf.Max(_boost[i], boost * 1.2f); }
            _meltBoost = Mathf.Max(0f, _meltBoost - dt * 0.12f);

            float gain = rx.Gain(0.85f, 1.2f);
            for (int l = 0; l < 3; l++)
            {
                float H = Height[l];
                _lamp[l] = new Vector4(Pos[l].x, Pos[l].y, Pos[l].z, H);
                var w = Kit.Hue(hue + l * 0.33f + rx.beatS / 1024f);
                Color.RGBToHSV(w, out float hh, out float ss, out float vv);
                var wax = Color.HSVToRGB(hh, Mathf.Max(ss, 0.8f), 1f);
                var liq = Color.HSVToRGB(Mathf.Repeat(hh + 0.55f, 1f), 0.7f, 1f) * 0.15f;
                _wax[l] = new Vector4(wax.r, wax.g, wax.b, 0f);
                _liq[l] = new Vector4(liq.r, liq.g, liq.b, 0f);
                for (int k = 0; k < 8; k++)
                {
                    int i = l * 8 + k;
                    if (k == 7) { _blob[i] = new Vector4(0f, -9f, 0f, 0f); continue; }
                    _boost[i] = Mathf.Max(0f, _boost[i] - dt * 1.2f);
                    _ph[i] += dc * heat * 0.22f * _w[i] * (1f + _boost[i]) * (calm ? 0.6f : 1f);
                    float y01 = 0.5f - 0.5f * Mathf.Cos(_ph[i]);
                    float y = BaseH + 0.12f + (H - 0.42f) * y01;
                    float u = (y - BaseH) / H;
                    float rad = _r[i] * (1f + 0.35f * swell * _swell) * (1f + 0.5f * _meltBoost);
                    float room = Mathf.Max(0f, GlassR(u) - rad - 0.05f);
                    float a = _ph[i] * 0.37f + _seed[i] * 6.28f;
                    _blob[i] = new Vector4(Mathf.Sin(a) * room * 0.6f, y, Mathf.Cos(a * 1.3f) * room * 0.6f, rad);
                }
            }
            _m.SetVectorArray("_Lamp", _lamp);
            _m.SetVectorArray("_Wax", _wax);
            _m.SetVectorArray("_Liquid", _liq);
            _m.SetVectorArray("_Blob", _blob);
            _m.SetFloat("_Melt", Mathf.Clamp01(melt + 0.6f * _meltBoost));
            _m.SetFloat("_Glow", glow * (0.85f + 0.3f * _swell));
            _m.SetFloat("_Gain", gain);

            float sway = Mathf.Sin(rx.phrase * 0.25f);
            var camPos = new Vector3(sway * 0.7f, 1.3f, -4.7f);
            rig.Move(cam, camPos, new Vector3(sway * 0.2f, 1.2f, 0.3f), dt, 0.5f);
        }
    }
}
