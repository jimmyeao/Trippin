// A vertical light curtain (aurora ribbon): a grid mesh (u along the ribbon,
// v up) displaced in the vertex shader into slow folds, shaded with a
// bright lower edge that fades up, drifting vertical rays and the palette.
// Additive. The show drives the folds (_Fold from slow bass), the travel
// (_Phase from the smooth energy clock) and the height; nothing here reads
// raw audio. _Mirror flips it under the floor for the reflection.
Shader "Trippin/Aurora"
{
    Properties
    {
        _Hue ("Palette offset", Float) = 0.4
        _Intensity ("Intensity", Float) = 0.4
        _Phase ("Fold travel", Float) = 0
        _Fold ("Fold amount", Float) = 1
        _Height ("Curtain height (m)", Float) = 20
        _SpecAmt ("Spectrum lift of the curtain top (x height)", Float) = 0
        _WaveR ("Kick wave: distance from the centre (m)", Float) = 999
        _WaveA ("Kick wave: depth (m)", Float) = 0
        _Base ("Base height (m)", Float) = 6
        _Z ("Depth (m)", Float) = 0
        _Seed ("Seed", Float) = 0
        _Mirror ("Mirror under floor", Float) = 0
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
            float _Hue, _Intensity, _Phase, _Fold, _Height, _Base, _Z, _Seed, _Mirror, _SpecAmt, _WaveR, _WaveA;
            CBUFFER_END

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; };

            V vert(A i)
            {
                V o;
                float u = i.uv.x, v = i.uv.y;
                float w1 = sin(u * 6.0 + _Phase * 0.7 + _Seed * 5.0);
                float w2 = sin(u * 13.0 - _Phase * 1.1 + _Seed * 9.0);
                float x = (u - 0.5) * 150.0 + sin(v * 2.5 + _Phase * 0.5 + _Seed * 7.0) * _Fold * 2.0;
                float z = _Z + _Fold * (w1 * 9.0 + w2 * 3.5);
                // The curtain's top follows the eased spectrum along its length, mirrored about the
                // centre (bass in the middle, highs at the ends; no seam), so the music reshapes it.
                float sx = abs(u - 0.5) * 2.0;
                float sp = RxSpec(0.05 + 0.85 * sx);
                float y = _Base + v * _Height * (0.6 + _SpecAmt * sp);
                // A kick sends a fold out from the centre along the curtain.
                float dw = sx * 75.0 - _WaveR;
                z += _WaveA * sin(dw * 0.3) * exp(-dw * dw / 150.0) * (0.4 + 0.6 * v);
                if (_Mirror > 0.5) y = -y;
                o.pos = TransformWorldToHClip(TransformObjectToWorld(float3(x, y, z)));
                o.uv = i.uv;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float u = i.uv.x, v = i.uv.y;
                // Drifting vertical rays: noise along the ribbon only, so they
                // stay vertical streaks; two scales.
                float r1 = TNoise(float2(u * 55.0 + _Seed * 13.0, _Phase * 0.15));
                float r2 = TNoise(float2(u * 140.0 - _Seed * 7.0, _Phase * 0.3 + 4.0));
                float rays = smoothstep(0.22, 0.95, r1) * (0.65 + 0.35 * r2);
                float vert = pow(saturate(1.0 - v), 1.6) * smoothstep(0.0, 0.06, v);
                float edge = smoothstep(0.0, 0.1, u) * smoothstep(1.0, 0.9, u);
                float g = (0.2 + 0.8 * rays) * vert * edge;
                float3 c = TPalette(_Hue + u * 0.25 + v * 0.12);
                return float4(c * c * g * _Intensity, 0);
            }
            ENDHLSL
        }
    }
}
