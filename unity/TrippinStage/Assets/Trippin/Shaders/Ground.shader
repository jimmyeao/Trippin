// Wet asphalt over the mirrored reflections: a dark, mostly opaque layer
// that thins out in puddles, so the colossus and the city lights reflect
// patchily like a rain-soaked plaza. Fades into the fog with distance.
Shader "Trippin/Ground"
{
    Properties { }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent-20" "RenderPipeline"="UniversalPipeline" }
        Blend SrcAlpha OneMinusSrcAlpha
        ZWrite Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            float _FogDist;

            struct A { float4 pos : POSITION; };
            struct V { float4 pos : SV_POSITION; float3 wp : TEXCOORD0; };
            V vert(A i) { V o; o.wp = TransformObjectToWorld(i.pos.xyz); o.pos = TransformWorldToHClip(o.wp); return o; }

            float4 frag(V i) : SV_Target
            {
                float2 p = i.wp.xz;
                // Puddles: low-frequency noise from the shared fbm.
                float wet = smoothstep(0.35, 0.65, TFbm3(float3(p * 0.045, 1.7)));
                float a = lerp(0.88, 0.35, wet);
                float3 col = float3(0.008, 0.009, 0.012);
                // Paving lines on a 6 m grid, faint.
                float2 g = abs(frac(p / 6.0) - 0.5);
                col += smoothstep(0.485, 0.5, max(g.x, g.y)) * 0.006;
                float dist = length(i.wp - GetCameraPositionWS());
                float fog = 1.0 - exp(-dist / _FogDist);
                float3 fogCol = lerp(float3(0.02, 0.02, 0.03), TPalette(0.45) * 0.12, 0.6);
                col = lerp(col, fogCol, fog);
                return float4(col, lerp(a, 1.0, fog));
            }
            ENDHLSL
        }
    }
}
