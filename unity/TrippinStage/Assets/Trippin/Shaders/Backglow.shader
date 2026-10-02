// A soft radial glow on a big quad behind a hero object, so its silhouette
// separates from the void. Additive; colour from the palette, size and
// strength from the show (_Glow).
Shader "Trippin/Backglow"
{
    Properties
    {
        _Glow ("Glow", Float) = 0.3
        _Hue ("Hue", Float) = 0.4
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
            float _Glow, _Hue;
            CBUFFER_END

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; };
            V vert(A i) { V o; o.pos = TransformObjectToHClip(i.pos.xyz); o.uv = i.uv; return o; }

            float4 frag(V i) : SV_Target
            {
                float2 p = (i.uv - 0.5) * 2.0;
                float r = length(p * float2(1.0, 0.8));
                // Fades to nothing well inside the quad, so no edge shows.
                float g = (exp(-r * r * 4.0) * 0.8 + exp(-r * 7.0) * 0.5) * smoothstep(0.95, 0.5, r);
                float3 c = TPalette(_Hue);
                return float4(c * c * g * _Glow, 0);
            }
            ENDHLSL
        }
    }
}
