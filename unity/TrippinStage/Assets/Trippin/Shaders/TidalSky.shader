Shader "Trippin/TidalSky"
{
    Properties { }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Background" "RenderPipeline"="UniversalPipeline" }
        Cull Front
        ZWrite Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            float _Drift;
            struct A { float4 pos : POSITION; };
            struct V { float4 pos : SV_POSITION; float3 d : TEXCOORD0; };
            V vert(A i) { V o; o.pos = TransformObjectToHClip(i.pos.xyz); o.d = normalize(i.pos.xyz); return o; }
            float4 frag(V i) : SV_Target
            {
                float3 d = normalize(i.d);
                float h = d.y;
                float3 deep = float3(0.002, 0.006, 0.021);
                float3 twilight = float3(0.008, 0.059, 0.075);
                float3 c = lerp(twilight, deep, smoothstep(0.0, 0.42, h));
                float facing = pow(saturate(d.z * 0.5 + 0.5), 6.0);
                c += float3(0.02, 0.15, 0.18) * exp(-h * h * 72.0) * facing;
                float aurora = sin(d.x * 45.0 + sin(h * 19.0 + _Drift * 0.17) * 3.0 + _Drift * 0.1);
                aurora = pow(saturate(aurora), 9.0) * smoothstep(0.02, 0.23, h) * (1.0 - smoothstep(0.4, 0.72, h));
                c += float3(0.005, 0.03, 0.045) * aurora * facing;
                float2 cell = float2(atan2(d.z, d.x) * 180.0, h * 290.0);
                float star = step(0.997, THash(floor(cell))) * smoothstep(0.32, 0.02, length(frac(cell) - 0.5)) * smoothstep(0.13, 0.37, h);
                c += star * float3(0.55, 0.8, 0.85);
                return float4(c, 1);
            }
            ENDHLSL
        }
    }
}
