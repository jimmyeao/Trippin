// unity_jellyfish's water: the deep sea seen from below the light, a dim
// teal glow from the surface fading to black in the depths, with shafts of
// light fanning down from above (swaying with _Sway, flickering slowly as
// the surface moves) and a faint haze tinted by the palette.
Shader "Trippin/HqDeep"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
        _Rays ("Rays", Float) = 1
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

            float _Gain, _Rays, _T, _Sway, _Hue;

            float4 frag(FsV i) : SV_Target
            {
                float3 rd = normalize(i.rd);
                float up = rd.y;
                float3 deep = float3(0.0, 0.004, 0.012);
                float3 shallow = float3(0.01, 0.07, 0.11);
                float3 col = lerp(deep, shallow, saturate(up * 0.9 + 0.35));
                // Shafts of light fanning down from the surface above the frame,
                // swaying, flickering, fading with depth.
                float2 sc = float2(i.ndc.x, -i.ndc.y);     // ndc y runs down here
                float2 q = sc - float2(0.25 + _Sway * 0.15, 2.6);
                float az = atan2(q.x, -q.y);
                float s = sin(az * 26.0 + sin(az * 9.0 + _T * 0.15) * 1.2) * 0.5 + 0.5;
                s *= sin(az * 47.0 - _T * 0.11) * 0.5 + 0.5;
                float fall = saturate(1.0 - (2.6 - sc.y) / 4.2);
                float shaft = pow(s, 4.0) * fall * fall * (0.7 + 0.3 * sin(_T * 0.6 + az * 13.0));
                col += float3(0.05, 0.16, 0.2) * shaft * _Rays;
                // A glow where the surface would be.
                col += float3(0.05, 0.14, 0.18) * pow(saturate(up), 4.0);
                // Palette haze low down.
                float3 p = TPalette(_Hue);
                col += p * p * 0.012 * saturate(0.6 - up);
                return float4(col * _Gain, 1.0);
            }
            ENDHLSL
        }
    }
}
