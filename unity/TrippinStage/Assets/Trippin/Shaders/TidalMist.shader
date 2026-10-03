Shader "Trippin/TidalMist"
{
    Properties { }
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

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float3 wp : TEXCOORD1; };
            V vert(A i) { V o; o.wp = TransformObjectToWorld(i.pos.xyz); o.pos = TransformWorldToHClip(o.wp); o.uv = i.uv; return o; }
            float4 frag(V i) : SV_Target
            {
                float2 edge = min(i.uv, 1.0 - i.uv);
                float fade = smoothstep(0.0, 0.26, min(edge.x, edge.y));
                float2 q = i.wp.xz * 0.055 + _TFlow * float2(0.009, -0.006);
                float patch = smoothstep(0.36, 0.68, TNoise(q));
                float3 c = lerp(float3(0.025, 0.1, 0.13), TPalette(0.58), 0.24);
                return float4(c * patch * fade * 0.1, 0);
            }
            ENDHLSL
        }
    }
}
