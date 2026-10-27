// unity_halloween's ground mist: a full-screen layer over the backdrop,
// rolling banks of fog (the shared baked noise volume, HqNoise, drifting
// through time) that hug the ground and thin out above _Top (screen height,
// 0 bottom .. 1 top). Moonlit pale blue, tinted by _Tint low down, lifted
// by the glow of the wisps (_Wisp[i]: screen xy, strength).
Shader "Trippin/HqFog"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
        _Top ("Top", Float) = 0.35
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent+5" "RenderPipeline"="UniversalPipeline" }
        Blend SrcAlpha OneMinusSrcAlpha
        ZTest Always
        ZWrite Off
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex FsVert
            #pragma fragment frag
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"
            #include "TrippinFull.hlsl"

            TEXTURE3D(_Vol); SAMPLER(sampler_Vol);
            float _Gain, _Top, _T, _Drift;
            float4 _Tint;
            float4 _Wisp[6];

            float4 frag(FsV i) : SV_Target
            {
                float2 uv = i.ndc * 0.5 + 0.5;
                float asp = _ScreenParams.x / _ScreenParams.y;
                // Perspective-ish: the banks stretch out toward the horizon.
                float depth = 1.0 / max(0.08, 1.0 - uv.y * 1.6);
                float2 p = float2((uv.x - 0.5) * asp * depth * 2.0 + _Drift, depth * 3.0);
                float n = SAMPLE_TEXTURE3D_LOD(_Vol, sampler_Vol, float3(p * 0.11, _T * 0.02), 0).r;
                n += 0.5 * SAMPLE_TEXTURE3D_LOD(_Vol, sampler_Vol, float3(p * 0.27 + 3.1, _T * 0.035), 0).g;
                n += 0.25 * SAMPLE_TEXTURE3D_LOD(_Vol, sampler_Vol, float3(p * 0.6 + 7.7, _T * 0.05), 0).b;
                n /= 1.75;
                float h = smoothstep(_Top, _Top - 0.28, uv.y + (n - 0.5) * 0.18);
                float dens = saturate((n - 0.32) * 2.2) * h;
                float3 col = lerp(float3(0.42, 0.48, 0.62), _Tint.rgb, saturate(1.0 - uv.y / _Top) * 0.6);
                [unroll] for (int k = 0; k < 6; k++)
                {
                    float2 d = (uv - _Wisp[k].xy) * float2(asp, 1.0);
                    col += float3(0.3, 1.0, 0.85) * _Wisp[k].z * exp(-dot(d, d) / 0.006) * 1.5;
                }
                return float4(col * _Gain, dens * 0.85);
            }
            ENDHLSL
        }
    }
}
