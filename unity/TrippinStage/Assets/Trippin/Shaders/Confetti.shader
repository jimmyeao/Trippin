// Confetti / streamer flakes: opaque, double-sided, vertex colour from the
// particle system. Each flake glints as it tumbles (brightness swings with
// a per-flake phase), so a cloud of them sparkles in the stage light.
Shader "Trippin/Confetti"
{
    Properties { _Gain ("Gain", Float) = 1.6 }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Gain;
            CBUFFER_END

            struct A { float4 pos : POSITION; float4 col : COLOR; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float4 col : COLOR; float2 uv : TEXCOORD0; float ph : TEXCOORD1; };

            V vert(A i)
            {
                V o;
                float3 w = TransformObjectToWorld(i.pos.xyz);
                o.pos = TransformWorldToHClip(w);
                o.col = i.col;
                o.uv = i.uv;
                o.ph = dot(floor(w * 2.0), float3(1.7, 2.3, 3.1));
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                // Glint: tumbling flakes catch the light a few times a second.
                float glint = 0.35 + 0.65 * pow(abs(sin(_Time.y * 6.0 + i.ph)), 6.0);
                return float4(i.col.rgb * glint * _Gain * (0.7 + 0.5 * _TEnergy), 1);
            }
            ENDHLSL
        }
    }
}
