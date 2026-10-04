// A radar / sonar scope on a flat disc (a quad on the XZ plane, drawn
// additively): range rings, compass ticks, a sweep with phosphor persistence,
// a polar spectrum plot that the sweep lights up as it passes, contacts that
// glow where the sweep has just been, and expanding rings launched by kicks.
// The show passes everything in (eased); nothing reads raw audio here.
//  _Sweep  sweep angle (the smooth energy clock)      _Gain  luminance
//  _RingR  four expanding ring radii 0..1 (< 0 = off)  _RingK  their strengths
Shader "Trippin/Scope"
{
    Properties
    {
        _Radius ("Scope radius (m)", Float) = 24
        _Extent ("Quad half size (m)", Float) = 27
        _Hue ("Palette offset", Float) = 0.4
        _Gain ("Gain", Float) = 1
        _Sweep ("Sweep angle", Float) = 0
        _Drift ("Contact drift", Float) = 0
        _RingR ("Ring radii", Vector) = (-1, -1, -1, -1)
        _RingK ("Ring strengths", Vector) = (0, 0, 0, 0)
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" }
        Blend One One
        ZWrite Off
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Radius, _Extent, _Hue, _Gain, _Sweep, _Drift;
            float4 _RingR, _RingK;
            CBUFFER_END

            static const float kTau = 6.2831853;

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 q : TEXCOORD0; };

            V vert(A i)
            {
                V o;
                float2 xz = (i.uv - 0.5) * 2.0 * _Extent;
                o.pos = TransformWorldToHClip(TransformObjectToWorld(float3(xz.x, 0.0, xz.y)));
                o.q = xz / _Radius;
                return o;
            }

            float3 Pal(float t) { float3 c = TPalette(t); return c * c; }
            float Mod(float x) { return x - kTau * floor(x / kTau); }

            float4 frag(V i) : SV_Target
            {
                float2 q = i.q;
                float r = length(q);
                float th = atan2(q.y, q.x);
                float fw = fwidth(r) + 1e-4;
                float inside = smoothstep(1.03, 1.0, r);
                float bass = _RxLvl.x, lum = _RxMisc.y;
                float3 base = Pal(_Hue);
                float3 col = base * 0.025 * inside * (0.5 + 0.5 * (1.0 - r));

                // Range rings and rim.
                float ringL = 1.0 - smoothstep(0.0, 1.5 * fw * 6.0, abs(frac(r * 6.0 + 0.5) - 0.5));
                col += base * ringL * 0.1 * inside;
                col += Pal(_Hue + 0.1) * (1.0 - smoothstep(0.0, 2.0 * fw, abs(r - 1.0))) * 0.55;

                // Cross hairs.
                float2 aw = fwidth(q) + 1e-4;
                float cross_ = max(1.0 - smoothstep(0.0, 1.5 * aw.x, abs(q.x)), 1.0 - smoothstep(0.0, 1.5 * aw.y, abs(q.y)));
                col += base * cross_ * 0.07 * inside;

                // Compass ticks: 72 round the rim, every sixth long.
                float tu = (th / kTau + 0.5) * 72.0;
                float ta = abs(frac(tu + 0.5) - 0.5);
                float isLong = step(abs(frac(tu / 6.0 + 0.5) - 0.5) * 6.0, 0.25);
                float tickR = lerp(0.955, 0.915, isLong);
                float tick = (1.0 - smoothstep(0.03, 0.08, ta)) * step(tickR, r) * step(r, 0.99);
                col += Pal(_Hue + 0.1) * tick * 0.5;

                // Sweep: a bright leading edge and a phosphor wedge behind it.
                float delta = Mod(_Sweep - th);
                col += Pal(_Hue + 0.05) * (exp(-delta * 2.4) * 0.3 + exp(-delta * 70.0) * 1.2) * inside;

                // Polar spectrum plot: 96 bars, each lit by the sweep's persistence at its own angle.
                float u = (th / kTau + 0.5) * 96.0;
                float ib = floor(u), fb = frac(u);
                float sym = abs((ib + 0.5) / 96.0 * 2.0 - 1.0);
                float lvl = RxSpec(sym);
                float top = 0.58 + 0.36 * lvl;
                float angC = ((ib + 0.5) / 96.0 - 0.5) * kTau;
                float persC = exp(-Mod(_Sweep - angC) * 1.3);
                float barW = smoothstep(0.06, 0.16, fb) * smoothstep(0.06, 0.16, 1.0 - fb);
                float bar = step(0.58, r) * step(r, top) * barW;
                float cap = (1.0 - smoothstep(0.0, 0.012, abs(r - top))) * barW;
                col += Pal(_Hue + 0.1 + 0.3 * (r - 0.58)) * bar * (0.18 + 1.0 * persC) * (0.4 + 0.9 * lvl);
                col += Pal(_Hue + 0.4) * cap * (0.35 + 1.2 * persC) * (0.5 + lvl);

                // Rings launched by kicks.
                [unroll]
                for (int k = 0; k < 4; k++)
                {
                    float rr = _RingR[k];
                    float on = step(0.0, rr);
                    float w = 0.014 + 0.01 * rr;
                    col += Pal(_Hue + 0.2) * exp(-pow((r - rr) / w, 2.0)) * _RingK[k] * on * 1.4;
                }

                // Contacts: glow where the sweep has just passed, swelling with their own band.
                [unroll]
                for (int c = 0; c < 20; c++)
                {
                    float fc = (float)c;
                    float a = THash(float2(fc, 1.3)) * kTau + _Drift * 0.02 * (THash(float2(fc, 2.7)) - 0.5);
                    float rho = 0.16 + 0.68 * THash(float2(fc, 3.9));
                    float2 pos = rho * float2(cos(a), sin(a));
                    float dist = length(q - pos);
                    float pers = exp(-Mod(_Sweep - a) * 1.1);
                    float l = RxSpec(THash(float2(fc, 4.4)));
                    float size = 0.012 + 0.024 * l;
                    float blip = smoothstep(size * 1.6, 0.0, dist);
                    float box = exp(-pow((dist - size * 2.8) / 0.005, 2.0));
                    col += Pal(_Hue + 0.5 + 0.3 * THash(float2(fc, 5.1))) * (blip * (0.4 + 1.6 * l) + box * 0.6) * (0.08 + pers);
                }

                // Hub.
                col += Pal(_Hue) * exp(-r * r * 90.0) * (0.3 + 1.4 * bass);

                col *= _Gain * inside;
                return float4(col, 0.0);
            }
            ENDHLSL
        }
    }
}
