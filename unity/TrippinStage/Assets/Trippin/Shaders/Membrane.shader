// A circular wire membrane (a sub-bass plate): a polar grid mesh (uv.x =
// angle 0..1, uv.y = radius 0..1) displaced in the vertex shader by a slow
// radial ripple plus up to four travelling ring pulses, drawn as bright
// grid lines (rings and spokes) with a faint fill. Additive. The show
// drives amplitude (slow bass), ripple travel (smooth energy clock) and the
// ring pulses (_Rings radii, _RingAmp amplitudes); nothing here reads raw
// audio. _Mirror is unused by the shader: the show mirrors the object.
Shader "Trippin/Membrane"
{
    Properties
    {
        _Radius ("Radius (m)", Float) = 16
        _Amp ("Ripple amplitude (m)", Float) = 1
        _Phase ("Ripple travel", Float) = 0
        _Rings ("Ring radii 0..1", Vector) = (-1,-1,-1,-1)
        _RingAmp ("Ring amplitudes (m)", Vector) = (0,0,0,0)
        _Hue ("Palette offset", Float) = 0.0
        _Intensity ("Intensity", Float) = 0.5
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
            float _Radius, _Amp, _Phase, _Hue, _Intensity;
            float4 _Rings, _RingAmp;
            CBUFFER_END

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float h : TEXCOORD1; };

            float Height(float r)
            {
                float h = _Amp * sin(r * 14.0 - _Phase) * exp(-r * 2.2);
                h += _Amp * 0.5 * exp(-r * 5.0);
                [unroll] for (int k = 0; k < 4; k++)
                {
                    float d = (r - _Rings[k]) * 9.0;
                    h += _RingAmp[k] * exp(-d * d);
                }
                return h;
            }

            V vert(A i)
            {
                V o;
                float r = i.uv.y;
                float th = i.uv.x * 6.2831853;
                float h = Height(r);
                float3 p = float3(cos(th) * r * _Radius, h, sin(th) * r * _Radius);
                o.pos = TransformWorldToHClip(TransformObjectToWorld(p));
                o.uv = i.uv;
                o.h = h;
                return o;
            }

            // Anti-aliased line at integer values of x.
            float Line(float x)
            {
                float d = abs(frac(x + 0.5) - 0.5);
                return 1.0 - smoothstep(0.0, max(fwidth(x), 1e-4) * 1.5, d);
            }

            float4 frag(V i) : SV_Target
            {
                float r = i.uv.y;
                float rings = Line(r * 24.0);
                float spokes = Line(i.uv.x * 96.0) * smoothstep(0.04, 0.2, r);
                float glow = saturate(abs(i.h) * 0.35);
                float edge = 1.0 - smoothstep(0.82, 1.0, r);
                float g = (rings + spokes * 0.8) * (0.25 + 1.5 * glow) + 0.05 * (0.5 + glow);
                float3 c = TPalette(_Hue + r * 0.3 + i.h * 0.02);
                return float4(c * c * g * edge * _Intensity, 0);
            }
            ENDHLSL
        }
    }
}
