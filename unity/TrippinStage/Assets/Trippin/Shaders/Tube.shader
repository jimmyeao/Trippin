// A glowing light tube along a polyline, built on the CPU each frame by
// TubeRibbon (Kit.cs): a round tube whose centre facing the camera is a thin
// white-hot core and whose silhouette is a soft coloured halo, so it reads as
// neon / plasma with volume instead of a flat line. uv0 = (length 0..1, around
// 0..1); uv1 = (per-point intensity, per-point hue). Additive. _Phase (the
// smooth energy clock) slides bright packets along the tube; the show drives
// _Gain (luminance, eased) and the per-point intensity/hue/radius (shape).
Shader "Trippin/Tube"
{
    Properties
    {
        _Hue ("Palette offset", Float) = 0.5
        _Gain ("Gain", Float) = 1
        _Phase ("Packet phase", Float) = 0
        _PulseFreq ("Packets along the tube", Float) = 6
        _PulseAmt ("Packet strength", Float) = 0.6
        _PulseSharp ("Packet sharpness", Float) = 3
        _Core ("Core sharpness", Float) = 6
        _White ("White-hot core", Float) = 0.5
        _EndFade ("Fade at the ends 0..1", Float) = 0
    }
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
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Hue, _Gain, _Phase, _PulseFreq, _PulseAmt, _PulseSharp, _Core, _White, _EndFade;
            CBUFFER_END

            struct A { float4 pos : POSITION; float3 nrm : NORMAL; float2 uv : TEXCOORD0; float2 uv1 : TEXCOORD1; };
            struct V { float4 pos : SV_POSITION; float3 n : TEXCOORD0; float2 uv : TEXCOORD1; float2 k : TEXCOORD2; float3 wp : TEXCOORD3; };

            V vert(A i)
            {
                V o;
                float3 wp = TransformObjectToWorld(i.pos.xyz);
                o.pos = TransformWorldToHClip(wp);
                o.n = normalize(TransformObjectToWorldDir(i.nrm));
                o.uv = i.uv;
                o.k = i.uv1;
                o.wp = wp;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float3 v = normalize(_WorldSpaceCameraPos - i.wp);
                float nv = saturate(abs(dot(normalize(i.n), v)));
                float core = pow(nv, _Core);                 // thin hot line down the middle
                float halo = pow(1.0 - nv, 1.5) * 0.35 + nv * 0.25; // soft body + rim
                float packets = pow(0.5 + 0.5 * sin(i.uv.x * 6.2831853 * _PulseFreq - _Phase), _PulseSharp);
                float p = (1.0 - _PulseAmt) + _PulseAmt * 2.2 * packets;
                float3 c = TPalette(_Hue + i.k.y);
                c = c * c;                                    // saturate
                float3 rgb = c * (halo + core * 1.6) + core * _White * (0.5 + 0.5 * p);
                float ends = lerp(1.0, smoothstep(0.0, 0.08, i.uv.x) * smoothstep(0.0, 0.08, 1.0 - i.uv.x), _EndFade);
                float dist = length(i.wp - _WorldSpaceCameraPos);
                float near = smoothstep(0.5, 3.0, dist);
                return float4(rgb * p * i.k.x * _Gain * ends * near, 0);
            }
            ENDHLSL
        }
    }
}
