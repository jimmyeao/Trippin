// unity_fluid: a real-time fluid simulation (stable fluids on the GPU,
// Fluid.compute) shown as glossy liquid paint (FluidView.shader). The music
// is the force that stirs it.
//  - Shape: three emitters pour dye and push the fluid, each driven by one
//    band (eased bass, mids, highs), so the liquid's form is the mix; every
//    kick fires a burst that blooms into a mushroom of swirls; the drop
//    floods the centre (eased).
//  - Motion: the emitters orbit on the smooth energy clock and their orbit
//    turns one way then the other with the phrase, so the flow swings
//    direction; vorticity keeps the swirls curling.
//  - Breakdowns: the forces fall away and the dye lingers, drifting.
// Show controls (Trippin's Perform tab): swirl, dye life, flow life, splat
// size, force and shine.
using UnityEngine;

namespace TrippinStage
{
    public sealed class FluidShow : KitShow
    {
        public ComputeShader sim;
        public Material viewMat;

        [TrippinParam("Swirl", 0f, 80f)] public float swirl = 40f;
        [TrippinParam("Dye life", 0.5f, 8f)] public float dyeLife = 1.6f;
        [TrippinParam("Flow life", 0.3f, 6f)] public float flowLife = 1.6f;
        [TrippinParam("Splat size", 0.3f, 2.5f)] public float splatSize = 1f;
        [TrippinParam("Force", 0.2f, 3f)] public float force = 1f;
        [TrippinParam("Shine", 0f, 2f)] public float shine = 0.8f;

        const int VelW = 256, VelH = 144, DyeW = 1280, DyeH = 720, Iterations = 24;

        RenderTexture _velA, _velB, _dyeA, _dyeB, _pA, _pB, _div, _curl;
        int _kSplat, _kAdvect, _kCurl, _kVort, _kDiv, _kJacobi, _kGrad;
        Material _view;
        readonly Vector4[] _splats = new Vector4[8], _vals = new Vector4[8];
        int _nSplat;
        float _kickPrev, _impPrev, _orbit, _hue, _clkPrev = -1f;
        int _burst;

        static RenderTexture Rt(int w, int h, RenderTextureFormat f)
        {
            var rt = new RenderTexture(w, h, 0, f) { enableRandomWrite = true, filterMode = FilterMode.Bilinear, wrapMode = TextureWrapMode.Clamp };
            rt.Create();
            return rt;
        }

        protected override void Build()
        {
            _velA = Rt(VelW, VelH, RenderTextureFormat.ARGBHalf);
            _velB = Rt(VelW, VelH, RenderTextureFormat.ARGBHalf);
            _pA = Rt(VelW, VelH, RenderTextureFormat.ARGBHalf);
            _pB = Rt(VelW, VelH, RenderTextureFormat.ARGBHalf);
            _div = Rt(VelW, VelH, RenderTextureFormat.ARGBHalf);
            _curl = Rt(VelW, VelH, RenderTextureFormat.ARGBHalf);
            _dyeA = Rt(DyeW, DyeH, RenderTextureFormat.ARGBHalf);
            _dyeB = Rt(DyeW, DyeH, RenderTextureFormat.ARGBHalf);
            _kSplat = sim.FindKernel("Splat");
            _kAdvect = sim.FindKernel("Advect");
            _kCurl = sim.FindKernel("Curl");
            _kVort = sim.FindKernel("Vorticity");
            _kDiv = sim.FindKernel("Divergence");
            _kJacobi = sim.FindKernel("Jacobi");
            _kGrad = sim.FindKernel("Gradient");
            _view = Kit.Fullscreen(transform, viewMat, "fluid");
        }

        void OnDestroy()
        {
            foreach (var rt in new[] { _velA, _velB, _dyeA, _dyeB, _pA, _pB, _div, _curl })
                if (rt != null) rt.Release();
        }

        void AddSplat(Vector2 uv, float radius, Vector4 val)
        {
            if (_nSplat >= 8) return;
            _splats[_nSplat] = new Vector4(uv.x, uv.y, radius, 0f);
            _vals[_nSplat] = val;
            _nSplat++;
        }

        void Dispatch(int k, int w, int h)
        {
            sim.SetInts("_Size", w, h);
            sim.Dispatch(k, (w + 7) / 8, (h + 7) / 8, 1);
        }

        void Swap(ref RenderTexture a, ref RenderTexture b) { var t = a; a = b; b = t; }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Min(dt, 1f / 30f);
            float aspect = (float)VelW / VelH;
            float live = 1f - 0.75f * rx.calm;

            // --- the music's forces ------------------------------------------
            // Orbit: advances with the smooth energy clock (so it surges on
            // drops and follows Trippin's Speed control), its direction
            // swinging through zero each phrase.
            float dClk = _clkPrev < 0f ? 0f : rx.clk - _clkPrev;
            if (dClk < -2048f) dClk += 4096f;                 // the clock wraps at 4096
            if (dClk < 0f || dClk > 4f) dClk = 0f;            // a jump, not travel
            _clkPrev = rx.clk;
            _orbit += dClk * 0.3f * Mathf.Sin(rx.phrase * 0.5f + 0.4f);
            _hue = rx.beat / 128f;
            float rad = 0.03f * splatSize;
            float[] band = { rx.bassFast, rx.midFast, rx.highFast };
            for (int e = 0; e < 3; e++)
            {
                float a = _orbit + e * Mathf.PI * 2f / 3f;
                float r = 0.26f + 0.06f * Mathf.Sin(rx.clk * 0.07f + e * 2f);
                var p = new Vector2(0.5f + Mathf.Cos(a) * r / aspect, 0.5f + Mathf.Sin(a) * r);
                // Push along the orbit (tangent), a little inward.
                var tan = new Vector2(-Mathf.Sin(a) / aspect, Mathf.Cos(a)) * Mathf.Sign(Mathf.Sin(rx.phrase * 0.5f + 0.4f) + 1e-3f);
                var inward = (new Vector2(0.5f, 0.5f) - p).normalized * 0.3f;
                float amt = (0.25f + 1.1f * band[e]) * live * force;
                // A steady push (per second, not per frame).
                var v = (tan + inward) * (7f * amt * dt);
                AddSplat(p, rad * (1f + band[e]), new Vector4(v.x, v.y, 0, 0));
                Color c = Kit.Hue(_hue + e * 0.33f);
                float dye = (0.05f + 0.3f * band[e]) * live * dt * 60f;
                _dyeQueue[e] = (p, rad * 0.8f * (1f + 0.6f * band[e]), new Vector4(c.r * dye, c.g * dye, c.b * dye, dye));
            }
            // Kick: a burst from a fresh spot near the middle, shooting outward.
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && rx.calm < 0.6f;
            _kickPrev = rx.kick;
            if (kick)
            {
                _burst++;
                var p = new Vector2(0.3f + 0.4f * Kit.H(_burst, 1), 0.3f + 0.4f * Kit.H(_burst, 2));
                float ang = Kit.H(_burst, 3) * Mathf.PI * 2f;
                float f = (1.2f + 2f * rx.bassFast) * force;
                AddSplat(p, rad * 1.6f, new Vector4(Mathf.Cos(ang) * f / aspect, Mathf.Sin(ang) * f, 0, 0));
                _kickDye = (p, rad * 1.2f, Kit.Hue(_hue + 0.5f + 0.2f * Kit.H(_burst, 4)));
                _kickDyeAmt = 0.9f;
            }
            // The drop: a flood from the centre (eased by rx.impact).
            if (rx.impact > 0.05f)
                AddSplat(new Vector2(0.5f, 0.5f), rad * 4f, new Vector4(0, 0.9f * rx.impact * force * dt * 60f * 0.1f, 0, 0));

            // --- simulate ------------------------------------------------------
            sim.SetFloat("_Aspect", aspect);
            sim.SetVector("_Texel", new Vector2(1f / VelW, 1f / VelH));
            sim.SetFloat("_Dt", dt);
            // Velocity splats.
            sim.SetVectorArray("_Splats", _splats);
            sim.SetVectorArray("_SplatVals", _vals);
            sim.SetInt("_SplatCount", _nSplat);
            sim.SetTexture(_kSplat, "_Src", _velA);
            sim.SetTexture(_kSplat, "_Dst", _velB);
            Dispatch(_kSplat, VelW, VelH);
            Swap(ref _velA, ref _velB);
            // Vorticity confinement.
            sim.SetTexture(_kCurl, "_Vel", _velA);
            sim.SetTexture(_kCurl, "_CurlOut", _curl);
            Dispatch(_kCurl, VelW, VelH);
            sim.SetFloat("_Swirl", swirl * (0.6f + 0.6f * rx.midSlow));
            sim.SetTexture(_kVort, "_CurlIn", _curl);
            sim.SetTexture(_kVort, "_Src", _velA);
            sim.SetTexture(_kVort, "_Dst", _velB);
            Dispatch(_kVort, VelW, VelH);
            Swap(ref _velA, ref _velB);
            // Pressure projection.
            sim.SetTexture(_kDiv, "_Vel", _velA);
            sim.SetTexture(_kDiv, "_Dst", _div);
            Dispatch(_kDiv, VelW, VelH);
            sim.SetTexture(_kJacobi, "_Div", _div);
            for (int i = 0; i < Iterations; i++)
            {
                sim.SetTexture(_kJacobi, "_Src", _pA);
                sim.SetTexture(_kJacobi, "_Dst", _pB);
                Dispatch(_kJacobi, VelW, VelH);
                Swap(ref _pA, ref _pB);
            }
            sim.SetTexture(_kGrad, "_Pressure", _pA);
            sim.SetTexture(_kGrad, "_Vel", _velA);
            sim.SetTexture(_kGrad, "_Dst", _velB);
            Dispatch(_kGrad, VelW, VelH);
            Swap(ref _velA, ref _velB);
            // Advect velocity (self) and dye; the lifetimes are e-folding times.
            float calmLinger = 1f + 1.5f * rx.calm;
            sim.SetFloat("_Decay", Mathf.Exp(-dt / flowLife));
            sim.SetTexture(_kAdvect, "_Vel", _velA);
            sim.SetTexture(_kAdvect, "_Src", _velA);
            sim.SetTexture(_kAdvect, "_Dst", _velB);
            Dispatch(_kAdvect, VelW, VelH);
            Swap(ref _velA, ref _velB);
            // Dye splats (emitters + the kick's burst), then advect.
            _nSplat = 0;
            for (int e = 0; e < 3; e++) AddSplat(_dyeQueue[e].p, _dyeQueue[e].r, _dyeQueue[e].v);
            if (_kickDyeAmt > 0.01f)
            {
                float a = _kickDyeAmt * dt * 60f * 0.5f;
                AddSplat(_kickDye.p, _kickDye.r, new Vector4(_kickDye.c.r * a, _kickDye.c.g * a, _kickDye.c.b * a, a));
                _kickDyeAmt *= Mathf.Exp(-dt * 10f);
            }
            sim.SetVectorArray("_Splats", _splats);
            sim.SetVectorArray("_SplatVals", _vals);
            sim.SetInt("_SplatCount", _nSplat);
            sim.SetFloat("_Aspect", (float)DyeW / DyeH);
            sim.SetTexture(_kSplat, "_Src", _dyeA);
            sim.SetTexture(_kSplat, "_Dst", _dyeB);
            Dispatch(_kSplat, DyeW, DyeH);
            Swap(ref _dyeA, ref _dyeB);
            sim.SetFloat("_Decay", Mathf.Exp(-dt / (dyeLife * calmLinger)));
            sim.SetTexture(_kAdvect, "_Vel", _velA);
            sim.SetTexture(_kAdvect, "_Src", _dyeA);
            sim.SetTexture(_kAdvect, "_Dst", _dyeB);
            Dispatch(_kAdvect, DyeW, DyeH);
            Swap(ref _dyeA, ref _dyeB);
            _nSplat = 0;

            _view.SetTexture("_Dye", _dyeA);
            _view.SetFloat("_Shine", shine);
            _view.SetFloat("_Gain", rx.Gain(0.75f, 1.3f));
            _view.SetFloat("_Hue", _hue);
        }

        readonly (Vector2 p, float r, Vector4 v)[] _dyeQueue = new (Vector2, float, Vector4)[3];
        (Vector2 p, float r, Color c) _kickDye;
        float _kickDyeAmt;
    }
}
