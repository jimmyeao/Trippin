// unity_marble: paper marbling, live. A full canvas of thick glossy paint
// (Marble.compute, MarbleView.shader) worked the way a marbler works it, by
// the music:
//  - Shape: every kick drops paint at the bar's spot, so a bar lays down
//    rings of colour (black and gold between the palette's colours); their
//    size follows the eased bass, and the relief swells with it. The drop
//    opens a ring of big drops round the centre.
//  - Motion: a vortex stirs the rings into spirals. It turns on the smooth
//    energy clock (so it surges on drops and rests in breakdowns) and its
//    direction swings through zero with the phrase; a second, smaller one
//    turns against it. Every 8 bars a comb is pulled through the paint over
//    four beats, across one way, then the other.
//  - Breakdowns: no kicks, so one big slow drop every two bars blooms while
//    the swirl drifts.
// Show controls: swirl, drop size, comb, shine, relief.
using UnityEngine;

namespace TrippinStage
{
    public sealed class MarbleShow : KitShow
    {
        public override bool UsesCamera => false;

        public ComputeShader sim;       // Fluid.compute: the velocity solver
        public ComputeShader marble;    // Marble.compute: the paint
        public Material viewMat;

        [TrippinParam("Swirl", 0f, 3f)] public float swirl = 1f;
        [TrippinParam("Drop size", 0.4f, 2f)] public float dropSize = 1f;
        [TrippinParam("Comb", 0f, 2f)] public float comb = 0.6f;
        [TrippinParam("Shine", 0f, 2f)] public float shine = 1f;
        [TrippinParam("Relief", 0f, 3f)] public float relief = 1f;

        const int VelW = 256, VelH = 144, PaintW = 1920, PaintH = 1080, Iterations = 20;
        const float FlowLife = 2f;

        RenderTexture _velA, _velB, _pA, _pB, _div, _paintA, _paintB, _propsA, _propsB;
        int _kAdvect, _kDiv, _kJacobi, _kGrad, _kInit, _kPaint, _kVortex;
        Material _view;

        struct Drop
        {
            public Vector2 c;
            public float area, tau, t;
            public Color col;
            public float height, metal;
        }
        readonly Drop[] _drops = new Drop[4];
        int _nDrops, _nPaint;
        readonly Vector4[] _dropV = new Vector4[4], _dropC = new Vector4[4], _vort = new Vector4[2];

        float _kickPrev, _clkPrev = -1f, _clkRate = 2f, _angle, _ridge = 1f;
        bool _laid;
        float _beatPrev = -1f, _phasePrev = -1f, _lastFrame = -1f;
        int _bar = int.MinValue;
        Vector2 _site = new Vector2(0.5f, 0.5f);

        static RenderTexture Rt(int w, int h)
        {
            var rt = new RenderTexture(w, h, 0, RenderTextureFormat.ARGBHalf) { enableRandomWrite = true, filterMode = FilterMode.Bilinear, wrapMode = TextureWrapMode.Clamp };
            rt.Create();
            return rt;
        }

        protected override void Build()
        {
            _velA = Rt(VelW, VelH); _velB = Rt(VelW, VelH);
            _pA = Rt(VelW, VelH); _pB = Rt(VelW, VelH); _div = Rt(VelW, VelH);
            _paintA = Rt(PaintW, PaintH); _paintB = Rt(PaintW, PaintH);
            _propsA = Rt(PaintW, PaintH); _propsB = Rt(PaintW, PaintH);
            _kAdvect = sim.FindKernel("Advect");
            _kDiv = sim.FindKernel("Divergence");
            _kJacobi = sim.FindKernel("Jacobi");
            _kGrad = sim.FindKernel("Gradient");
            _kInit = marble.FindKernel("Init");
            _kPaint = marble.FindKernel("Advect");
            _kVortex = marble.FindKernel("Vortex");
            _view = Kit.Fullscreen(transform, viewMat, "marble");
            _view.SetTexture("_Paint", _paintA);
            _view.SetTexture("_Props", _propsA);
        }

        void OnDestroy()
        {
            foreach (var rt in new[] { _velA, _velB, _pA, _pB, _div, _paintA, _paintB, _propsA, _propsB })
                if (rt != null) rt.Release();
        }

        // Paint colours: the palette's, pushed to full saturation; without a
        // feed (no palette yet) a magenta and teal.
        static Color Paint(float t)
        {
            if (TrippinLink.State.palette == null || TrippinLink.State.palette.Length < 24)
                return Mathf.Repeat(t, 1f) < 0.5f ? new Color(0.85f, 0.05f, 0.45f) : new Color(0.02f, 0.55f, 0.6f);
            // Marbling inks are rich: lift a pastel palette's saturation.
            Color.RGBToHSV(Kit.Hue(t), out float h, out float sat, out float v);
            return Color.HSVToRGB(h, Mathf.Max(sat, 0.85f), v * 0.9f);
        }

        static readonly Color Gold = new Color(1f, 0.66f, 0.26f), Black = new Color(0.012f, 0.012f, 0.016f);

        // The k-th paint of a sequence: palette colours with black and gold
        // between them, as a marbler would load the brushes.
        static void Pick(int k, float hue, out Color c, out float height, out float metal)
        {
            metal = 0f;
            switch (((k % 6) + 6) % 6)
            {
                case 0: c = Paint(hue); height = 0.8f; break;
                case 1: c = Black; height = 0.45f; break;
                case 2: c = Paint(hue + 0.33f); height = 0.95f; break;
                case 3: c = Gold; height = 0.7f; metal = 1f; break;
                case 4: c = Black; height = 0.5f; break;
                default: c = Paint(hue + 0.66f); height = 0.85f; break;
            }
        }

        /// A first canvas of wavy bands.
        void Lay()
        {
            var bands = new Vector4[8];
            var metal = new Vector4[2];
            for (int i = 0; i < 8; i++)
            {
                Pick(i, 0.1f, out var c, out var h, out var m);
                bands[i] = new Vector4(c.r, c.g, c.b, h);
                if (i < 4) metal[0][i] = m; else metal[1][i - 4] = m;
            }
            marble.SetVectorArray("_Bands", bands);
            marble.SetVectorArray("_BandMetal", metal);
            marble.SetFloat("_Seed", Random.value * 6.28f);
            marble.SetInts("_Size", PaintW, PaintH);
            marble.SetTexture(_kInit, "_PaintOut", _paintA);
            marble.SetTexture(_kInit, "_PropsOut", _propsA);
            marble.Dispatch(_kInit, (PaintW + 7) / 8, (PaintH + 7) / 8, 1);
        }

        void AddDrop(Vector2 c, float radius, float tau, Color col, float height, float metal)
        {
            if (_nDrops == _drops.Length)                        // full: the oldest makes way
            {
                for (int j = 1; j < _drops.Length; j++) _drops[j - 1] = _drops[j];
                _nDrops--;
            }
            // A drop on one still spreading takes over the rest of its
            // spread: both growing at once would lay thin rings of each.
            float area = radius * radius;
            for (int j = 0; j < _nDrops; j++)
            {
                var o = _drops[j];
                if ((o.c - c).sqrMagnitude > 1e-6f || o.t > o.tau * 6f) continue;
                area += o.area * (1f - Grow(o.t, o.tau));
                o.area *= Grow(o.t, o.tau);
                o.t = o.tau * 7f;
                _drops[j] = o;
            }
            _drops[_nDrops++] = new Drop { c = c, area = area, tau = tau, t = 0f, col = col, height = height, metal = metal };
        }

        static float Grow(float t, float tau) => 1f - Mathf.Exp(-t / tau);

        void Dispatch(int k, int w, int h)
        {
            sim.SetInts("_Size", w, h);
            sim.Dispatch(k, (w + 7) / 8, (h + 7) / 8, 1);
        }

        void Swap(ref RenderTexture a, ref RenderTexture b) { var t = a; a = b; b = t; }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 30f);
            float aspect = (float)PaintW / PaintH;
            float live = 1f - 0.8f * rx.calm;
            // The first canvas, once the feed has brought the palette.
            if (!_laid) { Lay(); _laid = true; }

            // --- the music ---------------------------------------------------
            // The energy clock's rate (beats per second), eased: the swirl's speed.
            // Cut back in after a while away: the last frame's clocks are stale.
            if (Time.time - _lastFrame > 0.5f) { _clkPrev = -1f; _phasePrev = -1f; _beatPrev = -1f; }
            _lastFrame = Time.time;
            float dClk = _clkPrev < 0f ? 0f : rx.clk - _clkPrev;
            if (dClk < -2048f) dClk += 4096f;                 // the clock wraps at 4096
            if (dClk < 0f || dClk > 4f) dClk = 0f;            // a jump, not travel
            _clkPrev = rx.clk;
            _clkRate += (dClk / dt - _clkRate) * (1f - Mathf.Exp(-dt * 2f));

            int bar = Mathf.FloorToInt(rx.beatS / 4f);
            if (bar != _bar)
            {
                _bar = bar;
                _site = new Vector2(0.22f + 0.56f * Kit.H(bar, 1), 0.25f + 0.5f * Kit.H(bar, 2));
                _nPaint = 0;
            }
            float hue = Mathf.Floor(rx.beatS / 32f) * 0.17f;

            // Kick: paint at the bar's spot, a ring per kick.
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && rx.calm < 0.6f;
            _kickPrev = rx.kick;
            if (kick)
            {
                Pick(bar * 4 + _nPaint, hue, out var c, out var h, out var m);
                _nPaint++;
                AddDrop(_site, dropSize * (0.04f + 0.05f * rx.bassFast), 0.12f, c, h, m);
            }
            // Breakdown: one big slow drop every two bars.
            float b8 = rx.beatS / 8f;
            if (_beatPrev >= 0f && Mathf.Floor(b8) != Mathf.Floor(_beatPrev) && rx.calm >= 0.6f)
            {
                int k = Mathf.FloorToInt(b8);
                Pick(k, hue, out var c, out var h, out var m);
                AddDrop(new Vector2(0.25f + 0.5f * Kit.H(k, 5), 0.3f + 0.4f * Kit.H(k, 6)), dropSize * 0.13f, 1.2f, c, h, m);
            }
            _beatPrev = b8;
            // The drop: a ring of big drops round the centre.
            if (rx.dropped)
            {
                for (int j = 0; j < 4; j++)
                {
                    float a = j * Mathf.PI * 0.5f + Kit.H(bar, 7) * 6.28f;
                    Pick(j * 2 + bar, hue, out var c, out var h, out var m);
                    AddDrop(new Vector2(0.5f + 0.2f * Mathf.Cos(a) / aspect, 0.5f + 0.2f * Mathf.Sin(a)), dropSize * 0.1f, 0.25f, c, h, m);
                }
            }

            // Drops grow over their tau; each frame adds the area grown since the last.
            int n = 0;
            float dropMetal0 = 0, dropMetal1 = 0, dropMetal2 = 0, dropMetal3 = 0;
            for (int i = 0; i < _nDrops; i++)
            {
                var d = _drops[i];
                float grown = d.area * (Grow(d.t + dt, d.tau) - Grow(d.t, d.tau));
                d.t += dt;
                _drops[i] = d;
                _dropV[n] = new Vector4(d.c.x, d.c.y, grown, 0f);
                _dropC[n] = new Vector4(d.col.r, d.col.g, d.col.b, d.height);
                switch (n) { case 0: dropMetal0 = d.metal; break; case 1: dropMetal1 = d.metal; break; case 2: dropMetal2 = d.metal; break; default: dropMetal3 = d.metal; break; }
                n++;
            }
            // Retire grown drops.
            int w = 0;
            for (int i = 0; i < _nDrops; i++)
                if (_drops[i].t < _drops[i].tau * 6f) _drops[w++] = _drops[i];
            _nDrops = w;

            // The comb: every 8 bars, pulled through over four beats (twice as
            // slow in a breakdown), across then down, alternating sides.
            float span = rx.calm > 0.5f ? 8f : 4f;
            float phase = Mathf.Repeat(rx.beatS, 32f);
            float u1 = Mathf.Clamp01(phase / span), u0 = _phasePrev < 0f ? u1 : phase >= _phasePrev ? Mathf.Clamp01(_phasePrev / span) : 0f;
            _phasePrev = phase;
            float pull = comb * 0.12f * (Mathf.SmoothStep(0f, 1f, u1) - Mathf.SmoothStep(0f, 1f, u0));
            int stroke = Mathf.FloorToInt(rx.beatS / 32f);
            float sgn = (stroke & 2) == 0 ? 1f : -1f;
            Vector2 dir = (stroke & 1) == 0 ? new Vector2(sgn, 0f) : new Vector2(0f, sgn);

            // The vortices: turning on the energy clock, direction swinging with the phrase.
            float swing = Mathf.Sin(rx.phrase * 0.5f + 0.4f);
            float omega = swirl * swing * (0.35f + 0.5f * rx.bassSlow + 0.8f * rx.impact) * Mathf.Clamp(_clkRate / 2f, 0.2f, 2f) * live;
            _angle += dt * 0.05f * Mathf.Clamp(_clkRate, 0.5f, 4f);
            _vort[0] = new Vector4(0.5f + 0.1f * Mathf.Sin(_angle), 0.5f + 0.08f * Mathf.Cos(_angle * 1.3f), omega, 0.32f);
            _vort[1] = new Vector4(0.5f + 0.3f * Mathf.Cos(_angle * 0.7f + 2f), 0.5f + 0.22f * Mathf.Sin(_angle * 0.9f), -0.6f * omega, 0.18f);

            // --- velocity ----------------------------------------------------
            marble.SetInts("_Size", VelW, VelH);
            marble.SetFloat("_Aspect", (float)VelW / VelH);
            marble.SetFloat("_Dt", dt);
            marble.SetVectorArray("_Vortices", _vort);
            marble.SetFloat("_Pull", 1.5f);
            marble.SetTexture(_kVortex, "_Src", _velA);
            marble.SetTexture(_kVortex, "_Dst", _velB);
            marble.Dispatch(_kVortex, (VelW + 7) / 8, (VelH + 7) / 8, 1);
            Swap(ref _velA, ref _velB);
            sim.SetFloat("_Dt", dt);
            sim.SetVector("_Texel", new Vector2(1f / VelW, 1f / VelH));
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
            sim.SetFloat("_Decay", Mathf.Exp(-dt / FlowLife));
            sim.SetTexture(_kAdvect, "_Vel", _velA);
            sim.SetTexture(_kAdvect, "_Src", _velA);
            sim.SetTexture(_kAdvect, "_Dst", _velB);
            Dispatch(_kAdvect, VelW, VelH);
            Swap(ref _velA, ref _velB);

            // --- paint -------------------------------------------------------
            marble.SetInts("_Size", PaintW, PaintH);
            marble.SetFloat("_Aspect", aspect);
            marble.SetVectorArray("_Drops", _dropV);
            marble.SetVectorArray("_DropCols", _dropC);
            marble.SetVector("_DropMetal", new Vector4(dropMetal0, dropMetal1, dropMetal2, dropMetal3));
            marble.SetInt("_DropCount", n);
            marble.SetVector("_Comb", new Vector4(dir.x, dir.y, pull, 0.16f));
            marble.SetFloat("_CombLambda", 0.05f);
            marble.SetFloat("_CombPhase", Kit.H(stroke, 9));
            marble.SetTexture(_kPaint, "_Vel", _velA);
            marble.SetTexture(_kPaint, "_Paint", _paintA);
            marble.SetTexture(_kPaint, "_Props", _propsA);
            marble.SetTexture(_kPaint, "_PaintOut", _paintB);
            marble.SetTexture(_kPaint, "_PropsOut", _propsB);
            marble.Dispatch(_kPaint, (PaintW + 7) / 8, (PaintH + 7) / 8, 1);
            Swap(ref _paintA, ref _paintB);
            Swap(ref _propsA, ref _propsB);

            // The relief swells with the bass (eased, never a pop).
            _ridge += ((0.8f + 0.9f * rx.bassFast) * relief - _ridge) * (1f - Mathf.Exp(-dt * 6f));
            _view.SetTexture("_Paint", _paintA);
            _view.SetTexture("_Props", _propsA);
            _view.SetFloat("_Shine", shine);
            _view.SetFloat("_Ridge", _ridge);
            _view.SetFloat("_Glint", rx.highFast);
            _view.SetFloat("_Gain", rx.Gain(0.8f, 1.2f));
        }
    }
}
