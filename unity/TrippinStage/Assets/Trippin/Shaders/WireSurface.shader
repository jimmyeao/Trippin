// A rectangular wire surface: a grid mesh (uv 0..1) displaced in the vertex
// shader by one of three height fields, drawn as anti-aliased grid lines with
// a faint fill. Additive. The pattern coordinates scroll (_Scroll) under the
// static mesh, so a flight over terrain or ocean needs no moving geometry.
//  _Mode 0 terrain: a flat corridor (_P0.x half-width, _P0.y ramp) between ridged hills.
//  _Mode 1 ocean:   four crossing swells; _P0.x = wind angle, _Amp = swell height.
//  _Mode 2 chladni: a vibrating plate; _P0.xy = the (n, m) mode numbers (continuous,
//                   so shows can morph between modes); the nodal lines glow like sand.
// Nothing here reads raw audio: the show drives _Amp (eased bands), _Scroll and
// _Phase (the smooth energy clock) and _Intensity.
Shader "Trippin/WireSurface"
{
    Properties
    {
        _Mode ("Mode 0 terrain 1 ocean 2 chladni", Float) = 0
        _SizeX ("Width (m)", Float) = 120
        _SizeZ ("Depth (m)", Float) = 200
        _Amp ("Height amplitude (m)", Float) = 8
        _Scroll ("Pattern scroll (m)", Float) = 0
        _Phase ("Wave phase", Float) = 0
        _Grid ("Grid lines per metre", Float) = 0.2
        _Hue ("Palette offset", Float) = 0.5
        _Intensity ("Intensity", Float) = 0.5
        _Fill ("Fill", Float) = 0.5
        _Fade ("Distance fade (m)", Float) = 120
        _P0 ("Mode params", Vector) = (14, 20, 0, 0)
        _Vib ("Chladni: vibration phase (-1..1, scales the displacement only)", Float) = 1
        _RipR ("Chladni: strike ring radius (m)", Float) = 0
        _RipA ("Chladni: strike ring height (m)", Float) = 0
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
            float _Mode, _SizeX, _SizeZ, _Amp, _Scroll, _Phase, _Grid, _Hue, _Intensity, _Fill, _Fade;
            float4 _P0;
            float _Vib, _RipR, _RipA;
            CBUFFER_END

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float2 pat : TEXCOORD1; float h : TEXCOORD2; float3 wp : TEXCOORD3; };

            float Height(float2 wxz, float2 xz)
            {
                if (_Mode < 0.5)
                {
                    float n = TNoise(wxz * 0.045) * 0.6 + TNoise(wxz * 0.11 + 7.3) * 0.3 + TNoise(wxz * 0.27 + 3.1) * 0.1;
                    float ridge = 1.0 - abs(2.0 * TNoise(wxz * 0.07 + 11.0) - 1.0);
                    float valley = smoothstep(_P0.x, _P0.x + _P0.y, abs(xz.x));
                    return _Amp * (valley * (n * 0.6 + ridge * ridge * 0.8) + 0.03 * n);
                }
                if (_Mode < 1.5)
                {
                    float a = _P0.x;
                    float2 d1 = float2(cos(a), sin(a));
                    float2 d2 = float2(cos(a + 0.9), sin(a + 0.9));
                    float2 d3 = float2(cos(a - 1.3), sin(a - 1.3));
                    float2 d4 = float2(cos(a + 2.1), sin(a + 2.1));
                    return _Amp * (sin(dot(wxz, d1) * 0.21 + _Phase)
                                 + 0.6 * sin(dot(wxz, d2) * 0.37 - _Phase * 1.3)
                                 + 0.35 * sin(dot(wxz, d3) * 0.71 + _Phase * 1.9)
                                 + 0.2 * sin(dot(wxz, d4) * 1.3 - _Phase * 2.3));
                }
                float2 q = xz / float2(_SizeX, _SizeZ) + 0.5;
                float n1 = _P0.x, m1 = _P0.y;
                const float kPi = 3.14159265;
                return _Amp * (cos(n1 * kPi * q.x) * cos(m1 * kPi * q.y) - cos(m1 * kPi * q.x) * cos(n1 * kPi * q.y));
            }

            V vert(A i)
            {
                V o;
                float2 xz = (i.uv - 0.5) * float2(_SizeX, _SizeZ);
                float2 wxz = float2(xz.x, xz.y + _Scroll);
                float h = Height(wxz, xz);
                float dh = h;
                if (_Mode > 1.5)
                {
                    // The plate vibrates: the figure flexes through zero with _Vib (the nodal lines,
                    // computed from the undisplaced figure in frag, stay put), and a kick strike sends
                    // a ring out from the centre.
                    float r = length(xz);
                    float ring = exp(-(r - _RipR) * (r - _RipR) * 0.08);
                    dh = h * _Vib + _RipA * ring;
                }
                float3 p = float3(xz.x, dh, xz.y);
                float3 wp = TransformObjectToWorld(p);
                o.pos = TransformWorldToHClip(wp);
                o.uv = i.uv;
                o.pat = wxz;
                o.h = h;
                o.wp = wp;
                return o;
            }

            // Anti-aliased line at integer values of x.
            float Line(float x)
            {
                float d = abs(frac(x + 0.5) - 0.5);
                return 1.0 - smoothstep(0.0, max(fwidth(x), 1e-4) * 1.5, d);
            }

            float4 frag(V i) : SV_Target
            {
                float amp = max(_Amp, 1e-3);
                float hn = saturate(i.h / amp * 0.5 + 0.5);
                float gridL = max(Line(i.pat.x * _Grid), Line(i.pat.y * _Grid));
                float lines = gridL;
                if (_Mode > 1.5)
                {
                    float nodal = 1.0 - smoothstep(0.0, 0.07, abs(i.h) / amp);
                    lines = nodal + 0.18 * gridL;
                }
                float dist = length(i.wp - _WorldSpaceCameraPos);
                float fade = exp(-dist / _Fade);
                float2 e = min(i.uv, 1.0 - i.uv);
                float edge = smoothstep(0.0, 0.06, min(e.x, e.y));
                float3 c = TPalette(_Hue + hn * 0.35 + dist * 0.0015);
                float glow = lines * (0.35 + 1.2 * hn * hn);
                float fill = _Fill * (0.04 + 0.12 * hn);
                return float4(c * c * (glow + fill) * fade * edge * _Intensity, 0);
            }
            ENDHLSL
        }
    }
}
