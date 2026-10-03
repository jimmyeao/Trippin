Shader "Trippin/TidalCore"
{
    Properties { _RoseTex ("Rose glass", 2D) = "black" {} }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent-10" "RenderPipeline"="UniversalPipeline" }
        Cull Off
        Blend SrcAlpha OneMinusSrcAlpha
        ZWrite Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            TEXTURE2D(_RoseTex);
            SAMPLER(sampler_RoseTex);
            float _Current, _Pulse;
            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; };
            V vert(A i) { V o; o.pos = TransformObjectToHClip(i.pos.xyz); o.uv = 0.5 + (i.uv - 0.5) * (1.0 - _Pulse * 0.07); return o; }
            float4 frag(V i) : SV_Target
            {
                float4 rose = SAMPLE_TEXTURE2D(_RoseTex, sampler_RoseTex, i.uv);
                float2 p = (i.uv - 0.5) * 2.0;
                float shimmer = 0.5 + 0.5 * sin(p.x * 6.0 + p.y * 10.0 - _Current * 0.65);
                float3 c = rose.rgb * (1.25 + shimmer * 0.2 + _Pulse * 0.5) + rose.rgb * rose.rgb * (0.3 + _Pulse * 0.5);
                return float4(c, rose.a);
            }
            ENDHLSL
        }
    }
}
