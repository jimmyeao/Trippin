// A flat LED screen or LED dance floor (a quad, additive): a grid of round LED
// dots lit by a live spectrum (mode 0: mirrored equaliser columns over a slow
// plasma, with sweeps on the energy clock) or by rings spreading from the
// middle (mode 1: the floor, tiles lit by distance, with the spectrum round
// the edge). The show passes everything in, eased: _Gain luminance, _Clk the
// smooth energy clock, _Hue the palette position; the spectrum comes from the
// Rx globals. uv (0..1) lives on the quad; world size is the quad's scale.
Shader "Trippin/Screen"
{
    Properties
    {
        _Mode ("0 wall, 1 floor", Float) = 0
        _Cols ("LED columns", Float) = 64
        _Rows ("LED rows", Float) = 36
        _Hue ("Palette offset", Float) = 0.5
        _Gain ("Gain", Float) = 1
        _Clk ("Clock", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent-10" "RenderPipeline"="UniversalPipeline" }
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
            float _Mode, _Cols, _Rows, _Hue, _Gain, _Clk;
            CBUFFER_END

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; };

            V vert(A i)
            {
                V o;
                o.pos = TransformObjectToHClip(float3(i.uv.x - 0.5, i.uv.y - 0.5, 0.0));
                o.uv = i.uv;
                return o;
            }

            float3 Pal(float t) { float3 c = TPalette(t); return c * c; }

            float4 frag(V i) : SV_Target
            {
                float2 g = float2(i.uv.x * _Cols, i.uv.y * _Rows);
                float2 cell = floor(g);
                float2 f = frac(g) - 0.5;
                float dotMask = smoothstep(0.46, 0.3, length(f));
                float2 c = (cell + 0.5) / float2(_Cols, _Rows);       // the dot's own centre, 0..1
                float bass = _RxLvl.x, lum = _RxMisc.y, impact = _RxMisc.w;
                float3 col;
                if (_Mode < 0.5)
                {
                    float sym = abs(c.x * 2.0 - 1.0);
                    float lvl = RxSpec(sym);
                    float h = 0.12 + 0.85 * lvl;
                    float bar = step(c.y, h);
                    float top = smoothstep(0.05, 0.0, abs(c.y - h));
                    float plasma = 0.5 + 0.5 * sin(c.x * 9.0 + _Clk * 0.05 + sin(c.y * 7.0 - _Clk * 0.04) * 2.0);
                    float sweep = pow(0.5 + 0.5 * sin((c.x + c.y * 0.4) * 6.0 - _Clk * 0.2), 6.0);
                    col = Pal(_Hue + c.y * 0.3 + lvl * 0.2) * (bar * (0.25 + 0.9 * lvl) + top * 1.4)
                        + Pal(_Hue + 0.5 + c.x * 0.2) * (plasma * 0.05 + sweep * 0.08) * (0.5 + lum);
                }
                else
                {
                    float2 p = (c - 0.5) * 2.0;
                    float r = length(p);
                    float ring = 0.5 + 0.5 * sin(r * 14.0 - _Clk * 0.35);
                    float ang = atan2(p.y, p.x);
                    float lvl = RxSpec(abs(ang) / 3.14159);
                    float edge = smoothstep(0.45, 1.0, r) * lvl;
                    col = Pal(_Hue + r * 0.4) * (pow(ring, 4.0) * (0.12 + 0.5 * bass) * (1.0 - r * 0.6) + edge * 1.1)
                        + Pal(_Hue + 0.5) * smoothstep(0.1, 0.0, r) * (0.3 + 1.4 * bass);
                }
                col *= dotMask * (1.0 + 0.8 * impact);
                col *= smoothstep(0.0, 0.03, i.uv.x) * smoothstep(0.0, 0.03, 1.0 - i.uv.x) * smoothstep(0.0, 0.03, i.uv.y) * smoothstep(0.0, 0.03, 1.0 - i.uv.y);
                return float4(col * _Gain, 0.0);
            }
            ENDHLSL
        }
    }
}
