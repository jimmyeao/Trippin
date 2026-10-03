// A plasma orb: a unit icosphere displaced in the vertex shader by two
// octaves of noise that drift with _Phase, drawn additively with a fresnel
// rim and hot spots where the noise peaks. Both faces add, so it reads as a
// translucent ball of energy. The show drives _Amp (eased bass), _Freq (eased
// highs), _Phase (the smooth energy clock) and _Intensity.
Shader "Trippin/Orb"
{
    Properties
    {
        _Radius ("Radius (m)", Float) = 6
        _Amp ("Displacement", Float) = 1
        _Freq ("Noise frequency", Float) = 1.5
        _Phase ("Noise drift", Float) = 0
        _Hue ("Palette offset", Float) = 0.5
        _Intensity ("Intensity", Float) = 0.5
        _Rim ("Rim sharpness", Float) = 2.5
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
            float _Radius, _Amp, _Freq, _Phase, _Hue, _Intensity, _Rim;
            CBUFFER_END

            struct A { float4 pos : POSITION; };
            struct V { float4 pos : SV_POSITION; float3 nw : TEXCOORD0; float d : TEXCOORD1; float3 wp : TEXCOORD2; };

            V vert(A i)
            {
                V o;
                float3 n = normalize(i.pos.xyz);
                float3 q = n * _Freq + float3(_Phase * 0.3, _Phase * 0.17, -_Phase * 0.23);
                float d = TNoise3(q * 2.0) * 0.65 + TNoise3(q * 4.6 + 4.1) * 0.35 - 0.5;
                float3 p = n * _Radius * (1.0 + _Amp * d * 0.5);
                float3 wp = TransformObjectToWorld(p);
                o.pos = TransformWorldToHClip(wp);
                o.nw = normalize(TransformObjectToWorldNormal(n));
                o.d = d;
                o.wp = wp;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float3 v = normalize(_WorldSpaceCameraPos - i.wp);
                float fres = pow(1.0 - saturate(abs(dot(normalize(i.nw), v))), _Rim);
                float3 c = TPalette(_Hue + i.d * 0.8 + i.nw.y * 0.15);
                float hot = smoothstep(0.05, 0.45, i.d);
                float em = 0.18 + 1.4 * fres + 1.2 * hot * hot;
                return float4(c * c * em * _Intensity, 0);
            }
            ENDHLSL
        }
    }
}
