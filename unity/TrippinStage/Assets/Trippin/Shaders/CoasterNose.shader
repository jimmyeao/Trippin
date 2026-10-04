// The coaster car's nose: the screen-space foreground hood and its neon trim,
// a port of the tail of shaders/scenes/neon_coaster.wgsl. Drawn as a
// full-screen pass (uv -> clip space) so it rides every camera bank for free.
// The cyan trim pulses on the kick (calm-aware); the lap stripe is magenta.
Shader "Trippin/CoasterNose"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent+50" "RenderPipeline"="UniversalPipeline" }
        Blend SrcAlpha OneMinusSrcAlpha
        ZWrite Off
        ZTest Always
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

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; };

            V vert(A i)
            {
                V o;
                o.pos = float4(i.uv * 2.0 - 1.0, 0.5, 1.0);
                o.uv = i.uv;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                // wgsl centred() space: p.y +down, ~[-1,1]; p.x aspect-wide.
                float px = (i.uv.x - 0.5) * 3.6;
                float py = (0.5 - i.uv.y) * 2.0;
                float ny = 0.68 + 0.05 * px * px;
                float nose = smoothstep(ny, ny + 0.015, py);
                float trim = exp(-abs(py - ny) * 60.0) * smoothstep(1.5, 0.3, abs(px));
                float lap = smoothstep(0.012, 0.0, abs(py - (0.88 + 0.02 * px * px))) * step(abs(px), 0.9);
                float pulse = 0.6 + 1.6 * TBeatPulse(6.0);
                float3 glow = float3(0.2, 0.9, 1.0) * trim * pulse
                            + float3(0.8, 0.2, 0.7) * lap * 0.5;
                // Dark hood occludes; the glow rides on top of it.
                float a = saturate(nose * 0.97 + trim * 0.8 + lap * 0.4);
                float3 col = lerp(glow * _Gain, float3(0.006, 0.006, 0.01) + glow * 0.08, nose * step(trim + lap, 0.01));
                col += glow * nose * _Gain;
                return float4(col, a);
            }
            ENDHLSL
        }
    }
}
