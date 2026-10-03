// Deep-space backdrop for the void shows (flow, crystals): an inverted
// sphere round the camera with a sparse starfield and slow nebula wisps in
// the palette, kept dim so the additive content reads over it. Wisps
// drift on the flow clock; a soft band glows along the "horizon" of the
// dome so there's always a faint floor of light.
Shader "Trippin/Nebula"
{
    Properties
    {
        _Wisp ("Wisp brightness", Float) = 0.5
        _Hue ("Palette offset", Float) = 0.6
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Background" "RenderPipeline"="UniversalPipeline" }
        Cull Front
        ZWrite Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Wisp, _Hue;
            CBUFFER_END

            struct A { float4 pos : POSITION; };
            struct V { float4 pos : SV_POSITION; float3 d : TEXCOORD0; };
            V vert(A i) { V o; o.pos = TransformObjectToHClip(i.pos.xyz); o.d = normalize(i.pos.xyz); return o; }

            float4 frag(V i) : SV_Target
            {
                float3 d = normalize(i.d);
                // Wisps: two fbm layers at different scales, crawling on the
                // flow clock so the sky itself breathes slowly.
                float3 q = d * 3.5 + float3(_TFlow * 0.004, _TFlow * 0.002, -_TFlow * 0.003);
                float w1 = TFbm3(q);
                float w2 = TFbm3(d * 9.0 - float3(0, _TFlow * 0.005, 0));
                float w = smoothstep(0.62, 0.95, w1 * 0.65 + w2 * 0.45);
                // Wisps keep a cosmic blue-violet lean: the raw palette can
                // turn swampy stretched across a whole sky.
                float3 wc = lerp(TPalette(_Hue + w1 * 0.3), float3(0.10, 0.11, 0.28), 0.55);
                float3 c = wc * w * w * _Wisp * 0.16;
                // A faint equatorial band so there's a floor of light.
                c += TPalette(_Hue + 0.15) * exp(-d.y * d.y * 10.0) * 0.06 * (0.6 + _TEnergy);
                // Stars: hashed cells on an azimuth/elevation grid.
                float az = atan2(d.z, d.x);
                float2 g = float2(az * 160.0, d.y * 320.0);
                float2 cell = floor(g);
                uint2 uq = (uint2)(int2)(cell + 32768.0);
                uq = uq * uint2(1597334673u, 3812015801u);
                float r = ((uq.x ^ uq.y) * 1597334673u) * (1.0 / 4294967296.0);
                float star = step(0.995, r) * smoothstep(0.4, 0.05, length(frac(g) - 0.5));
                // A few stars twinkle gently on the beat clock.
                float tw = 0.55 + 0.45 * sin(_TBeat * 3.14159 * r + r * 40.0);
                c += star * tw * lerp(float3(0.9, 0.92, 1.0), TPalette(r * 5.0), 0.4) * 0.5;
                return float4(c, 1);
            }
            ENDHLSL
        }
    }
}
