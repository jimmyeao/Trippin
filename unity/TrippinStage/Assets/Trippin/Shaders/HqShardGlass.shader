// unity_kaleido's glass, its colour: a backlit pane of coloured glass,
// glowing in the shard's texture (Alice-generated stained or dichroic
// glass) deepened and tinted by the palette, mostly covering what's behind
// it (_Filter is its opacity). A multiply blend was tried first: the panes
// came out flat white, so they're drawn as glowing panes instead.
// The glints are HqShard.shader, a second material on the same renderer.
Shader "Trippin/HqShardGlass"
{
    Properties
    {
        _MainTex ("Glass", 2D) = "white" {}
        _Tint ("Tint", Color) = (1, 1, 1, 1)
        _Gain ("Gain", Float) = 1
        _Glint ("Glint", Float) = 1
        _Filter ("Filter", Float) = 0.9
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" }
        ZWrite Off
        Cull Off
        HLSLINCLUDE
        #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
        TEXTURE2D(_MainTex);
        SAMPLER(sampler_MainTex);
        CBUFFER_START(UnityPerMaterial)
        float4 _MainTex_ST;
        float4 _Tint;
        float _Gain, _Glint, _Filter;
        CBUFFER_END

        struct A { float4 pos : POSITION; float3 n : NORMAL; float2 uv : TEXCOORD0; };
        struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float3 n : TEXCOORD1; float3 v : TEXCOORD2; };

        V vert(A i)
        {
            V o;
            float3 wp = TransformObjectToWorld(i.pos.xyz);
            o.pos = TransformWorldToHClip(wp);
            o.uv = TRANSFORM_TEX(i.uv, _MainTex);
            o.n = TransformObjectToWorldNormal(i.n);
            o.v = normalize(_WorldSpaceCameraPos - wp);
            return o;
        }
        ENDHLSL

        Pass
        {
            Blend SrcAlpha OneMinusSrcAlpha
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            float4 frag(V i) : SV_Target
            {
                float3 glass = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, i.uv).rgb;
                // Jewel colours: deepen and saturate the glass.
                float3 g = pow(saturate(glass * 1.3), 1.8);
                // Backlit: the pane glows in its own colour (HDR, so it blooms).
                float3 t = g * _Tint.rgb * _Gain;
                return float4(t, _Filter);
            }
            ENDHLSL
        }
    }
}
