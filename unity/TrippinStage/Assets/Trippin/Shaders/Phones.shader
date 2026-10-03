// Crowd phone lights: instanced soft dots held above the crowd, fading in
// with the breakdown (calm) and swaying slowly — the "lighters up" moment.
// Some show a screen (cool white), some a torch (warm, brighter).
Shader "Trippin/Phones"
{
    Properties { _Gain ("Gain", Float) = 2.5 }
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
            #pragma multi_compile_instancing
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Gain;
            CBUFFER_END

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float seed : TEXCOORD1; };

            V vert(A i)
            {
                V o;
                UNITY_SETUP_INSTANCE_ID(i);
                float3 origin = TransformObjectToWorld(float3(0, 0, 0));
                float seed = THash(origin.xz * 3.7 + 11.0);
                // Slow sway, each phone on its own phase.
                float3 p = i.pos.xyz;
                p.x += sin(_TFlow * 0.35 + seed * 30.0) * 0.6;
                float3 w = TransformObjectToWorld(p);
                o.pos = TransformWorldToHClip(w);
                o.uv = i.uv;
                o.seed = seed;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float2 q = i.uv * 2 - 1;
                float dot2 = q.x * q.x + q.y * q.y;
                float m = exp(-dot2 * 9.0) + 0.25 * exp(-dot2 * 2.0);
                bool torch = i.seed > 0.7;
                float3 c = torch ? float3(1.0, 0.85, 0.6) * 1.6 : float3(0.7, 0.85, 1.0);
                float vis = smoothstep(0.3, 0.85, _TCalm) * step(0.35, i.seed); // ~2/3 of the crowd
                float twinkle = 0.85 + 0.15 * sin(_Time.y * 1.7 + i.seed * 50.0);
                return float4(c * m * vis * twinkle * _Gain, 0);
            }
            ENDHLSL
        }
    }
}
