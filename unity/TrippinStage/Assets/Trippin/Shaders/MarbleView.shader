// unity_marble's picture: the marbled paint (Marble.compute) as a thick,
// wet, glossy surface seen from above. Each paint has its own thickness, so
// the band edges are ridges: their gradient gives the normal. A studio
// environment of two soft boxes is reflected in the gloss (Fresnel), metal
// paint reflects in its own colour with a fine glitter, and the rest is lit
// by a soft key from the upper left.
//  _Paint    rgb paint colour
//  _Props    x thickness, y metal
//  _Shine    gloss strength (show control)
//  _Gain     luminance (eased loudness, build dip, drop flare)
//  _Ridge    relief depth (eased bass: the paint swells on the low end)
Shader "Trippin/MarbleView"
{
    Properties
    {
        _Paint ("Paint", 2D) = "black" {}
        _Props ("Props", 2D) = "black" {}
        _Shine ("Shine", Float) = 1
        _Gain ("Gain", Float) = 1
        _Ridge ("Ridge", Float) = 1
        _Glint ("Glint", Float) = 0
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

            TEXTURE2D(_Paint);
            SAMPLER(sampler_Paint);
            TEXTURE2D(_Props);
            SAMPLER(sampler_Props);
            float4 _Props_TexelSize;
            CBUFFER_START(UnityPerMaterial)
            float _Shine, _Gain, _Ridge, _Glint;
            CBUFFER_END

            float H(float2 uv) { return SAMPLE_TEXTURE2D_LOD(_Props, sampler_Props, uv, 0).x; }

            // A soft-edged rectangle of light on the plane z = 1 the reflection
            // ray reaches: the studio's soft boxes.
            float Box(float2 q, float2 c, float2 half, float soft)
            {
                float2 d = abs(q - c) - half;
                float e = length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
                return 1.0 - smoothstep(-soft, soft, e);
            }

            float3 Env(float3 r)
            {
                float2 q = r.xy / max(r.z, 0.05);
                // A dim studio, brighter overhead, with a big soft box up
                // left and a strip light to the right. Flat paint reflects
                // q ~ -ndc * (0.35, 0.2), so the boxes sit where the frame's
                // flat areas see them and every bulge carries them across.
                float3 c = float3(0.02, 0.02, 0.025) * (0.3 + 0.7 * saturate(r.z));
                c += 1.5 * Box(q, float2(-0.2, 0.1), float2(0.14, 0.07), 0.07) * float3(1.0, 0.98, 0.95);
                c += 0.8 * Box(q, float2(0.28, -0.05), float2(0.03, 0.16), 0.03) * float3(0.95, 0.97, 1.0);
                return c;
            }

            float Hash(float2 p)
            {
                uint2 u = (uint2)(int2)p;
                uint x = u.x * 1664525u + u.y * 1013904223u;
                x ^= x >> 16; x *= 0x7feb352du; x ^= x >> 15; x *= 0x846ca68bu; x ^= x >> 16;
                return (x & 0xffffffu) / 16777216.0;
            }

            float4 frag(FsV i) : SV_Target
            {
                float2 uv = i.ndc * 0.5 + 0.5;
                float3 alb = SAMPLE_TEXTURE2D_LOD(_Paint, sampler_Paint, uv, 0).rgb;
                float2 pr = SAMPLE_TEXTURE2D_LOD(_Props, sampler_Props, uv, 0).xy;
                float metal = saturate(pr.y);

                // Relief: band edges at two scales (crisp ridges + the soft
                // swell of thick paint), in height units per pixel.
                float2 e = _Props_TexelSize.xy;
                float2 g1 = float2(H(uv + float2(e.x, 0)) - H(uv - float2(e.x, 0)), H(uv + float2(0, e.y)) - H(uv - float2(0, e.y)));
                float2 g4 = float2(H(uv + float2(4 * e.x, 0)) - H(uv - float2(4 * e.x, 0)), H(uv + float2(0, 4 * e.y)) - H(uv - float2(0, 4 * e.y)));
                // The pillow of thick paint: a broad slope inside each band.
                float2 g12 = float2(H(uv + float2(12 * e.x, 0)) - H(uv - float2(12 * e.x, 0)), H(uv + float2(0, 12 * e.y)) - H(uv - float2(0, 12 * e.y)));
                float2 g = (g1 * 1.0 + g4 * 0.7 + g12 * 0.45) * _Ridge;
                float3 n = normalize(float3(-g, 1.0));

                // An overhead camera with a little perspective: the view ray
                // leans out toward the frame's edges.
                float3 v = normalize(float3(-i.ndc * float2(0.35, 0.2), 1.0));
                float3 l = normalize(float3(-0.45, 0.55, 0.7));
                float diff = saturate(dot(n, l));
                float3 r = reflect(-v, n);
                float nv = saturate(dot(n, v));
                float fres = 0.04 + 0.96 * pow(1.0 - nv, 5.0);
                float3 env = Env(r);

                // Paint: a dielectric with a clear wet coat; metal reflects
                // in its own colour.
                float3 dielectric = alb * (0.22 + 0.85 * diff) + env * fres * _Shine;
                float3 h = normalize(l + v);
                float spec = pow(saturate(dot(n, h)), 80.0) * 2.5 * _Shine;
                dielectric += spec;
                float3 metallic = alb * (0.16 + 0.4 * diff) + alb * env * (0.8 + 0.6 * _Shine) + spec * lerp(alb, 1.0, 0.3);
                // Glitter in the metal: fine flakes catching the key light.
                float2 cell = floor(uv * float2(_Props_TexelSize.z, _Props_TexelSize.w) * 0.5);
                float fl = Hash(cell);
                float3 fn = normalize(n + 0.6 * float3(Hash(cell + 17.0) - 0.5, Hash(cell + 31.0) - 0.5, 0.0));
                float glint = pow(saturate(dot(fn, h)), 400.0) * step(0.6, fl) * (3.0 + 4.0 * _Glint);
                metallic += glint * lerp(alb, 1.0, 0.5);
                float3 c = lerp(dielectric, metallic, metal);
                return float4(c * _Gain, 1.0);
            }
            ENDHLSL
        }
    }
}
