// Atmosphere layer: a big soft quad of drifting smoke, tinted by the stage
// colours and brightened toward the stage — layered at several depths it
// gives the space air and catches the beams' colour.
Shader "Trippin/Haze"
{
    Properties
    {
        _Density ("Density", Float) = 0.12
        _Scale ("Noise scale", Float) = 0.05
        _HueOff ("Palette offset", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent-10" "RenderPipeline"="UniversalPipeline" }
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
            float _Density, _Scale, _HueOff;
            CBUFFER_END

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

            float4 frag(V i) : SV_Target
            {
                float3 q = i.wp * _Scale + float3(_TFlow * 0.03, _TFlow * 0.015, 0);
                float n = smoothstep(0.3, 0.9, TFbm3(q));
                float2 e = min(i.uv, 1.0 - i.uv);
                float edge = smoothstep(0.0, 0.25, min(e.x, e.y));
                float lift = smoothstep(0.0, 1.0, i.uv.y) * 0.6 + 0.4;
                float3 c = TPalette(_HueOff + i.uv.x * 0.3) * n * edge * lift * _Density * (0.4 + 0.6 * _TEnergy);
                return float4(c, 0);
            }
            ENDHLSL
        }
    }
}
