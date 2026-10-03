Shader "Trippin/TidalFrame"
{
    Properties { }
    SubShader
    {
        Tags { "RenderType"="Opaque" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            struct A { float4 pos : POSITION; float3 normal : NORMAL; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float3 normal : TEXCOORD0; float3 wp : TEXCOORD1; float2 uv : TEXCOORD2; };
            V vert(A i)
            {
                V o;
                o.wp = TransformObjectToWorld(i.pos.xyz);
                o.normal = TransformObjectToWorldNormal(i.normal);
                o.pos = TransformWorldToHClip(o.wp);
                o.uv = i.uv;
                return o;
            }
            float4 frag(V i) : SV_Target
            {
                float3 n = normalize(i.normal);
                float3 v = normalize(GetCameraPositionWS() - i.wp);
                float3 r = reflect(-v, n);
                float rim = pow(1.0 - abs(dot(n, v)), 2.0);
                float spec = pow(saturate(dot(r, normalize(float3(-0.4, 0.75, -0.52)))), 12.0);
                float crest = pow(saturate(n.y), 2.0);
                float inlay = pow(saturate(cos(i.uv.y * 6.2831853)), 22.0);
                float3 c = float3(0.035, 0.16, 0.21) + float3(0.035, 0.15, 0.19) * crest;
                c += float3(0.17, 0.74, 0.87) * (spec * 0.7 + rim * 0.32 + inlay * 0.13);
                c += TPalette(0.63) * (0.035 + 0.035 * sin(i.uv.x * 37.0 - _TFlow * 0.045)) * crest;
                return float4(c, 1);
            }
            ENDHLSL
        }
    }
}
