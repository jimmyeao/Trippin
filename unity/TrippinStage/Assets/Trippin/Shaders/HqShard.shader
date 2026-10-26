// unity_kaleido's glass, the light on it: two moving glints catching the
// faces as a shard tumbles (added). The
// glass's colour is HqShardGlass.shader, a second material on the same
// renderer: URP draws only one pass of a multi-pass shader, so the two
// halves are separate shaders.
Shader "Trippin/HqShard"
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
        Tags { "RenderType"="Transparent" "Queue"="Transparent+1" "RenderPipeline"="UniversalPipeline" }
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

        // Pass 2, the light on it: a Fresnel rim and two moving glints, added.
        Pass
        {
            Blend One One
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            float4 frag(V i, bool front : SV_IsFrontFace) : SV_Target
            {
                float3 n = normalize(front ? i.n : -i.n);
                float nv = saturate(dot(n, i.v));
                float3 glass = SAMPLE_TEXTURE2D(_MainTex, sampler_MainTex, i.uv).rgb;
                float rim = pow(1.0 - nv, 3.0);
                float t = _Time.y * 0.4;
                float3 l1 = normalize(float3(cos(t), 0.6, sin(t)));
                float3 l2 = normalize(float3(-sin(t * 1.3), -0.4, cos(t * 1.3)));
                float3 h1 = normalize(l1 + i.v), h2 = normalize(l2 + i.v);
                float spec = pow(saturate(dot(n, h1)), 90.0) + 0.6 * pow(saturate(dot(n, h2)), 60.0);
                // Only the glints: a rim term lit whole faces and washed the glass out.
                spec = pow(saturate(dot(n, h1)), 400.0) + 0.6 * pow(saturate(dot(n, h2)), 250.0);
                float3 c = spec * 1.2 * _Glint * lerp(float3(1, 1, 1), _Tint.rgb, 0.3);
                return float4(c * _Gain, 0);
            }
            ENDHLSL
        }
    }
}
