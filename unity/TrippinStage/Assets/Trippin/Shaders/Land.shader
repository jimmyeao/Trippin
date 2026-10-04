// A shaded landscape mesh for the terrain and ocean shows: an opaque surface
// (not a wireframe) lit from the sky it sits under (TrippinSky.hlsl), fogged
// into the horizon so it never shows an edge. A grid mesh (uv 0..1) displaced
// in the vertex shader, normals reconstructed per pixel from the height field.
//  _Mode 0 terrain: a flat corridor (_P0.x half-width, _P0.y ramp, _P0.z outer
//                   half-width) between ridged mountains that rise with
//                   distance; the mountains are a live spectrum left and right
//                   (each ridge's height follows its band); glowing contour
//                   lines and grid; rim light on the slopes facing the camera.
//  _Mode 1 ocean:   eight swells, each driven by its own spectrum slice (long
//                   swells = bass, fine chop = highs); expanding ripples from
//                   kicks (_RingR/_RingK); fresnel sky reflection (moon
//                   column), glowing crests, foam on steep faces.
// The show drives _Amp, _Scroll/_Phase (smooth energy clock), _Gain (eased
// loudness) and the sky parameters; the spectrum comes from the Rx globals.
Shader "Trippin/Land"
{
    Properties
    {
        _Mode ("0 terrain 1 ocean", Float) = 0
        _SizeX ("Width (m)", Float) = 300
        _SizeZ ("Depth (m)", Float) = 280
        _Amp ("Height amplitude (m)", Float) = 14
        _Scroll ("Pattern scroll (m)", Float) = 0
        _Phase ("Wave phase", Float) = 0
        _Grid ("Grid lines per metre", Float) = 0.12
        _Gain ("Gain", Float) = 1
        _Fade ("Fog distance (m)", Float) = 110
        _P0 ("Mode params", Vector) = (20, 30, 150, 0)
        _RingR ("Ripple radii 0..1", Vector) = (-1, -1, -1, -1)
        _RingK ("Ripple strengths", Vector) = (0, 0, 0, 0)
        _SkyMode ("0 sun, 1 moon", Float) = 0
        _SkyHue ("Palette offset", Float) = 0.1
        _SunH ("Sun elevation", Float) = 0.07
        _SunSize ("Sun radius (rad)", Float) = 0.2
        _SkyGain ("Sky gain", Float) = 1
        _SkyClk ("Clock", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        ZWrite On
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Mode, _SizeX, _SizeZ, _Amp, _Scroll, _Phase, _Grid, _Gain, _Fade;
            float4 _P0, _RingR, _RingK;
            float _SkyMode, _SkyHue, _SunH, _SunSize, _SkyGain, _SkyClk;
            CBUFFER_END

            #include "TrippinSky.hlsl"

            static const float kPi = 3.14159265;

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float3 wp : TEXCOORD0; float h : TEXCOORD1; float fe : TEXCOORD2; };

            float TerrainH(float2 w)
            {
                float2 pat = float2(w.x, w.y + _Scroll);
                float base = TNoise(pat * 0.035) * 0.55 + TNoise(pat * 0.09 + 7.3) * 0.3 + TNoise(pat * 0.23 + 3.1) * 0.15;
                float ridge = 1.0 - abs(2.0 * TNoise(pat * 0.055 + 11.0) - 1.0);
                ridge *= ridge;
                float valley = smoothstep(_P0.x, _P0.x + _P0.y, abs(w.x));
                float farm = smoothstep(14.0, 90.0, w.y);
                float bin = saturate((abs(w.x) - _P0.x) / max(_P0.z - _P0.x, 1.0));
                float eq = 0.55 + 1.2 * RxSpec(bin);
                float h = _Amp * valley * (0.25 + farm * 0.85) * (base * 0.55 + ridge * 0.8) * eq;
                h += 0.04 * _Amp * base * (1.0 - valley);
                h += _Amp * 0.1 * valley * _RxLvl.x * sin(w.y * 0.12 - _Phase);
                return h;
            }

            float OceanH(float2 w)
            {
                float h = 0.0;
                [unroll]
                for (int k = 0; k < 8; k++)
                {
                    float fk = (float)k;
                    float a = _P0.x + fk * 0.62 * (k % 2 == 0 ? 1.0 : -1.0);
                    float2 dir = float2(cos(a), sin(a));
                    float K = 6.2831853 / (70.0 / (1.0 + 1.55 * fk));
                    float ph = dot(dir, w) * K - _Phase * sqrt(K) * 1.7;
                    float amp = _Amp * (0.55 / (1.0 + fk * 0.9)) * (0.3 + 1.7 * RxSpec((fk + 0.5) / 8.0));
                    float s = pow(0.5 + 0.5 * sin(ph), 1.8);
                    h += amp * (2.0 * s - 1.0);
                }
                [unroll]
                for (int j = 0; j < 4; j++)
                {
                    float dist = length(w - float2(0.0, 55.0));
                    float R = _RingR[j] * 70.0;
                    float on = step(0.0, _RingR[j]);
                    h += on * _RingK[j] * _Amp * 0.9 * exp(-pow((dist - R) / 5.0, 2.0)) * cos((dist - R) * 0.9);
                }
                return h;
            }

            float Height(float2 w) { return _Mode < 0.5 ? TerrainH(w) : OceanH(w); }

            V vert(A i)
            {
                V o;
                float2 xz = (i.uv - 0.5) * float2(_SizeX, _SizeZ);
                float3 wp = TransformObjectToWorld(float3(xz.x, 0.0, xz.y));
                float edge = smoothstep(0.0, 0.14, min(i.uv.x, 1.0 - i.uv.x)) * smoothstep(0.0, 0.12, 1.0 - i.uv.y);
                float h = Height(wp.xz) * edge;
                wp.y += h;
                o.pos = TransformWorldToHClip(wp);
                o.wp = wp;
                o.h = h;
                o.fe = max(smoothstep(0.78, 1.0, i.uv.y), smoothstep(0.8, 1.0, abs(i.uv.x * 2.0 - 1.0)));
                return o;
            }

            float Line(float x)
            {
                float d = abs(frac(x + 0.5) - 0.5);
                return 1.0 - smoothstep(0.0, max(fwidth(x), 1e-4) * 1.5, d);
            }

            float4 frag(V i) : SV_Target
            {
                float3 wp = i.wp;
                float3 cv = _WorldSpaceCameraPos - wp;
                float dist = length(cv);
                float3 vv = cv / max(dist, 1e-3);
                float e = lerp(0.8, 0.4, _Mode);
                float hx = Height(wp.xz + float2(e, 0.0)) - Height(wp.xz - float2(e, 0.0));
                float hz = Height(wp.xz + float2(0.0, e)) - Height(wp.xz - float2(0.0, e));
                float3 n = normalize(float3(-hx, 2.0 * e, -hz));
                float h = i.h;
                float amp = max(_Amp, 1e-3);
                float lum = _RxMisc.y;
                float3 col;
                if (_Mode < 0.5)
                {
                    float hn = saturate(h / amp);
                    float rim = pow(1.0 - saturate(dot(n, vv)), 3.0);
                    float3 alb = lerp(SkyPal(_SkyHue + 0.6) * 0.05, SkyPal(_SkyHue + 0.1) * 0.16, saturate(hn * 1.6));
                    col = alb * (0.3 + 0.7 * saturate(n.y));
                    col += SkyPal(_SkyHue + 0.12) * rim * (0.25 + 0.5 * hn);
                    float2 pat = float2(wp.x, wp.z + _Scroll);
                    float grid = max(Line(pat.x * _Grid), Line(pat.y * _Grid));
                    float contour = Line(h * 0.55);
                    col += SkyPal(_SkyHue + 0.5) * grid * (0.18 + 0.55 * hn + 0.3 * lum);
                    col += SkyPal(_SkyHue + 0.3) * contour * hn * 0.5 * (0.5 + _RxLvl.y);
                }
                else
                {
                    float F = 0.02 + 0.98 * pow(1.0 - saturate(dot(n, vv)), 5.0);
                    float3 R = reflect(-vv, n);
                    R.y = abs(R.y);
                    float3 sky = SkyCol(R);
                    float hn = saturate(h / amp * 0.5 + 0.5);
                    float3 body = SkyPal(_SkyHue) * 0.03 + SkyPal(_SkyHue + 0.1) * pow(hn, 3.0) * 0.12 * (0.5 + 1.5 * _RxLvl.y);
                    float slope = length(n.xz) / max(n.y, 0.2);
                    float foam = smoothstep(0.8, 1.4, slope + (h / amp) * 0.3) * (0.3 + 1.5 * _RxLvl.w);
                    col = body * (1.0 - F) + sky * F + lerp(float3(1, 1, 1), SkyPal(_SkyHue + 0.2), 0.5) * foam * 0.25;
                }
                col *= _Gain;
                float fog = max(1.0 - exp(-pow(dist / _Fade, 1.5)), i.fe);
                float3 fogc = SkyCol(normalize(float3(-vv.x, 0.015, -vv.z)));
                col = lerp(col, fogc, fog);
                col = col / (1.0 + 0.3 * col);
                return float4(col, 1.0);
            }
            ENDHLSL
        }
    }
}
