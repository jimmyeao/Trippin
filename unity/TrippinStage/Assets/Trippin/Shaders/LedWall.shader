// LED screen: content is evaluated at each LED's centre and drawn as a
// rounded dot with dark gaps, so it reads as a real LED wall at a distance
// and as pixels up close. Five content patterns crossfade (_PatA -> _PatB
// by _Blend); all move on the smooth energy clocks and punch on hits. The
// wall's frame never moves — only its content does (house rule).
Shader "Trippin/LedWall"
{
    Properties
    {
        _Cols ("LED columns", Float) = 160
        _Rows ("LED rows", Float) = 80
        _PatA ("Pattern A", Float) = 0
        _PatB ("Pattern B", Float) = 1
        _Blend ("Blend A->B", Float) = 0
        _HueOff ("Palette offset", Float) = 0
        _Bright ("Brightness", Float) = 2.5
        _Seed ("Seed", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Cols, _Rows, _PatA, _PatB, _Blend, _HueOff, _Bright, _Seed;
            CBUFFER_END

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; };

            V vert(A i) { V o; o.pos = TransformObjectToHClip(i.pos.xyz); o.uv = i.uv; return o; }

            // Content at uv (0..1), aspect-corrected p (centred).
            float3 content(int pat, float2 uv, float aspect)
            {
                float2 p = (uv - 0.5) * float2(aspect, 1);
                float r = length(p);
                float a = atan2(p.y, p.x + 1e-5);
                float clk = _TClock.x * 0.25;
                float3 col;
                if (pat == 0) {
                    // Rings expanding from the centre, spacing breathing with bass.
                    float w = frac(r * (3.0 + 2.0 * _TLvl.x) - clk);
                    float ring = smoothstep(0.0, 0.08, w) * (1.0 - smoothstep(0.25, 0.5, w));
                    col = TPalette(r * 0.6 + _HueOff - clk * 0.1) * ring;
                } else if (pat == 1) {
                    // Spectrum bars mirrored from the centre, peaks on hits.
                    float x = abs(uv.x - 0.5) * 2.0;
                    float h = TSpectrum(x) * (0.7 + 0.5 * _THits.y);
                    float bar = step(abs(uv.y - 0.5) * 2.0, h) * step(0.15, frac(x * 24.0));
                    col = TPalette(x * 0.5 + uv.y * 0.3 + _HueOff) * bar;
                } else if (pat == 2) {
                    // Chevrons scrolling toward the crowd.
                    float c = frac((abs(p.x) * 1.2 - p.y) * 3.0 + _TClock.y * 0.35);
                    col = TPalette(c * 0.3 + _HueOff + 0.2) * smoothstep(0.45, 0.5, c) * (1.0 - smoothstep(0.85, 0.9, c));
                } else if (pat == 3) {
                    // Palette plasma, warped by the mids.
                    float n = TNoise(p * 3.0 + float2(clk, -clk * 0.7)) + TNoise(p * 7.0 - clk * (1.0 + _TLvl.y));
                    col = TPalette(n * 0.5 + _HueOff) * smoothstep(0.55, 1.25, n);
                } else {
                    // Radial sun: rays rotating on the flow clock.
                    float rays = pow(saturate(cos(a * 12.0 + _TFlow * 0.4)), 6.0);
                    float core = exp(-r * 5.0);
                    col = TPalette(0.15 + _HueOff + r * 0.4) * (rays * smoothstep(1.2, 0.1, r) + core);
                }
                return col;
            }

            float4 frag(V i) : SV_Target
            {
                float2 grid = float2(_Cols, _Rows);
                float2 cell = floor(i.uv * grid);
                float2 cuv = (cell + 0.5) / grid;
                float2 f = frac(i.uv * grid) - 0.5;
                float led = 1.0 - smoothstep(0.32, 0.45, length(f));
                float aspect = _Cols / max(_Rows, 1.0);
                float3 c = lerp(content((int)_PatA, cuv, aspect), content((int)_PatB, cuv, aspect), saturate(_Blend));
                // Hits punch the whole wall; a dim base keeps it alive in breakdowns.
                float punch = 0.75 + 0.6 * _THits.x * (1.0 - _TCalm) + 0.3 * _TOnset;
                c *= punch * _Bright;
                c += 0.01; // LED black is never quite black
                return float4(c * led, 1);
            }
            ENDHLSL
        }
    }
}
