// unity_fluid's picture: the simulated dye (Fluid.compute) shown full-screen
// as a glossy liquid. The dye's density is read as a height field, so its
// gradient gives a surface normal: a soft key light from the upper left and
// a tight specular streak make the swirls read as thick, wet paint rather
// than a flat texture. The background is a dark palette-tinted gradient.
//  _Dye      the dye texture (rgb colour, a = density)
//  _Shine    specular strength (show control)
//  _Gain     luminance (eased loudness, build dip, drop flare)
Shader "Trippin/FluidView"
{
    Properties
    {
        _Dye ("Dye", 2D) = "black" {}
        _Shine ("Shine", Float) = 0.8
        _Gain ("Gain", Float) = 1
        _Hue ("Palette offset", Float) = 0.5
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
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"
            #include "TrippinFull.hlsl"

            TEXTURE2D(_Dye);
            SAMPLER(sampler_Dye);
            float4 _Dye_TexelSize;
            CBUFFER_START(UnityPerMaterial)
            float _Shine, _Gain, _Hue;
            CBUFFER_END

            float Height(float2 uv)
            {
                float4 d = SAMPLE_TEXTURE2D_LOD(_Dye, sampler_Dye, uv, 0);
                return d.a;
            }

            float4 frag(FsV i) : SV_Target
            {
                float2 uv = i.ndc * 0.5 + 0.5;
                float4 d = SAMPLE_TEXTURE2D_LOD(_Dye, sampler_Dye, uv, 0);
                float2 e = _Dye_TexelSize.xy * 1.5;
                float hx = Height(uv + float2(e.x, 0)) - Height(uv - float2(e.x, 0));
                float hy = Height(uv + float2(0, e.y)) - Height(uv - float2(0, e.y));
                float3 n = normalize(float3(-hx * 3.0, -hy * 3.0, 1.0));
                float3 l = normalize(float3(-0.5, 0.6, 0.65));
                float diff = saturate(dot(n, l));
                float3 h = normalize(l + float3(0, 0, 1));
                float spec = pow(saturate(dot(n, h)), 60.0) * _Shine;

                // Background: near-black, a breath of palette toward the bottom.
                float3 p = TPalette(_Hue);
                float3 bg = lerp(float3(0.004, 0.004, 0.008), p * p * 0.03, saturate(1.0 - uv.y) * 0.8);

                float dens = saturate(d.a);
                float3 col = d.rgb / max(d.a, 1e-3);            // the dye's own colour
                float3 lit = col * (0.35 + 0.75 * diff) * smoothstep(0.0, 0.6, dens);
                float3 c = lerp(bg, lit, smoothstep(0.02, 0.35, dens));
                c += spec * smoothstep(0.05, 0.3, dens) * float3(1.0, 0.97, 0.92);
                return float4(c * _Gain, 1.0);
            }
            ENDHLSL
        }
    }
}
