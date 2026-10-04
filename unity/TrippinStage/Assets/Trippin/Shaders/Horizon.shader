// The sky behind the landscape shows (terrain: a huge banded sun at dusk;
// ocean: a moon over a starry night), drawn as a full-screen pass so it has no
// edges. See TrippinSky.hlsl for the sky itself. The sun / moon swells with the
// eased bass, the scan bands and the stars follow the mids and highs.
Shader "Trippin/Horizon"
{
    Properties
    {
        _SkyMode ("0 sun, 1 moon", Float) = 0
        _SkyHue ("Palette offset", Float) = 0.1
        _SunH ("Sun elevation", Float) = 0.07
        _SunSize ("Sun radius (rad)", Float) = 0.2
        _SkyGain ("Gain", Float) = 1
        _SkyClk ("Clock", Float) = 0
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

            CBUFFER_START(UnityPerMaterial)
            float _SkyMode, _SkyHue, _SunH, _SunSize, _SkyGain, _SkyClk;
            CBUFFER_END

            #include "TrippinSky.hlsl"

            float4 frag(FsV i) : SV_Target
            {
                float3 c = SkyCol(normalize(i.rd));
                c = c / (1.0 + 0.3 * c);
                return float4(c, 1.0);
            }
            ENDHLSL
        }
    }
}
