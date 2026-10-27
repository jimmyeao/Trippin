// unity_bubbles' background: a dark studio, a few large soft glows of the
// palette drifting far behind the bubbles (out-of-focus coloured lights),
// and a faint floor gradient. _Glow[i]: xy screen position, z size, w hue.
Shader "Trippin/HqBubbleBg"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Background" "RenderPipeline"="UniversalPipeline" }
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

            float _Gain;
            float4 _Glow[4];

            float4 frag(FsV i) : SV_Target
            {
                float2 p = i.ndc * float2(_ScreenParams.x / _ScreenParams.y, 1.0);
                float3 col = lerp(float3(0.006, 0.006, 0.01), float3(0.02, 0.018, 0.03), saturate(i.ndc.y * 0.5 + 0.5));
                [unroll] for (int k = 0; k < 4; k++)
                {
                    float2 d = p - _Glow[k].xy;
                    float3 c = TPalette(_Glow[k].w);
                    col += c * c * exp(-dot(d, d) / (_Glow[k].z * _Glow[k].z)) * 0.22;
                }
                return float4(col * _Gain, 1.0);
            }
            ENDHLSL
        }
    }
}
