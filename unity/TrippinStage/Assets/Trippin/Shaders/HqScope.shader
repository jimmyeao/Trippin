// unity_scope's CRT face: the phosphor trace (ScopeTrace.compute) glowing
// through a dark green glass with the 10 x 8 division graticule etched on
// it, a faint scanline, a vignette toward the rounded corners and a soft
// sheen from the room. Opaque and depth-writing, so depth of field focuses it.
Shader "Trippin/HqScope"
{
    Properties
    {
        _Trace ("Trace", 2D) = "black" {}
        _Phos ("Phosphor colour", Color) = (0.25, 1, 0.45, 1)
        _Gain ("Gain", Float) = 1
        _Grat ("Graticule", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            TEXTURE2D(_Trace); SAMPLER(sampler_Trace);
            CBUFFER_START(UnityPerMaterial)
            float4 _Phos;
            float _Gain, _Grat;
            CBUFFER_END
            float4 _GEnvA;

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float3 wp : TEXCOORD1; };

            V vert(A i)
            {
                V o;
                o.wp = TransformObjectToWorld(i.pos.xyz);
                o.pos = TransformWorldToHClip(o.wp);
                o.uv = i.uv;
                return o;
            }

            float Line(float x, float w) { return 1.0 - smoothstep(0.0, w, abs(x)); }

            float4 frag(V i) : SV_Target
            {
                float2 uv = i.uv;
                float4 tr = SAMPLE_TEXTURE2D(_Trace, sampler_Trace, uv);
                // Graticule in divisions: major lines every division, ticks on the centre axes.
                float2 d = float2(uv.x * 10.0, uv.y * 8.0);
                float2 f = abs(frac(d + 0.5) - 0.5);
                float g = max(Line(f.x, 0.025), Line(f.y, 0.025)) * 0.5;
                float2 c = abs(d - float2(5.0, 4.0));
                float2 tk = abs(frac(d * 5.0 + 0.5) - 0.5);
                g = max(g, Line(c.x, 0.02) * Line(tk.y, 0.08) * step(c.y, 4.0));
                g = max(g, Line(c.y, 0.02) * Line(tk.x, 0.08) * step(c.x, 5.0));
                // Rounded-rectangle vignette.
                float2 e = abs(uv - 0.5) * 2.0;
                float vig = 1.0 - smoothstep(0.75, 1.02, length(max(e - 0.55, 0.0)) / 0.45);
                float scan = 0.93 + 0.07 * sin(uv.y * 900.0);
                float3 glass = float3(0.008, 0.016, 0.011) + _GEnvA.rgb * 0.04 * (1.0 - uv.y);
                float3 col = glass + float3(0.35, 0.5, 0.42) * g * 0.05 * _Grat;
                col += _Phos.rgb * (tr.r * 1.4 + tr.g * 0.22) * _Gain * scan;
                col += float3(0.9, 1.0, 0.95) * saturate(tr.r - 1.2) * 0.3 * _Gain;   // the hottest parts go white
                return float4(col * vig, 1);
            }
            ENDHLSL
        }
    }
}
