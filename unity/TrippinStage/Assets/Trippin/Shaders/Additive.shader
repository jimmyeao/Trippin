// Soft additive particle (pyro flames, CO2 plumes, sparks): vertex colour x
// a round falloff, HDR-friendly so bloom picks it up.
Shader "Trippin/Additive"
{
    Properties
    {
        _Gain ("Gain", Float) = 2
        _Soft ("Softness", Float) = 2
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
            float _Gain, _Soft;
            CBUFFER_END

            struct A { float4 pos : POSITION; float4 col : COLOR; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float4 col : COLOR; float2 uv : TEXCOORD0; };

            V vert(A i) { V o; o.pos = TransformObjectToHClip(i.pos.xyz); o.col = i.col; o.uv = i.uv; return o; }

            float4 frag(V i) : SV_Target
            {
                float2 p = i.uv * 2 - 1;
                float r = dot(p, p);
                float m = pow(saturate(1.0 - r), _Soft);
                // A little turbulence so flames aren't perfect discs.
                m *= 0.7 + 0.6 * TNoise(i.uv * 6.0 + i.col.a * 40.0);
                return float4(i.col.rgb * i.col.a * m * _Gain, 0);
            }
            ENDHLSL
        }
    }
}
