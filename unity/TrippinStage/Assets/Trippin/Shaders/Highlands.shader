// unity_highlands' valley: a shaded dawn heightfield, a port of Trippin's
// shaders/scenes/highlands.wgsl to a displaced grid (like Land.shader). A
// carved corridor along x ~ 0 is the valley the camera glides down; the
// pattern scrolls on the energy clock (_Scroll) so the flight rides it.
// Ridges sharpen with slow bass presence (_RxLvl against the eased band);
// mist pools on the valley floor and warms toward the low sun ahead.
// Fogs into the shared sky (TrippinSky.hlsl, sun mode) so no edge shows.
Shader "Trippin/Highlands"
{
    Properties
    {
        _SizeX ("Width (m)", Float) = 340
        _SizeZ ("Depth (m)", Float) = 900
        _Amp ("Height amplitude (m)", Float) = 62
        _Scroll ("Pattern scroll (m)", Float) = 0
        _Gain ("Gain", Float) = 1
        _Fade ("Fog distance (m)", Float) = 380
        _Ridge ("Ridge sharpness", Float) = 0.5
        _Mist ("Floor mist", Float) = 0.5
        _SkyMode ("0 sun, 1 moon", Float) = 0
        _SkyHue ("Palette offset", Float) = 0.1
        _SunH ("Sun elevation", Float) = 0.10
        _SunSize ("Sun radius (rad)", Float) = 0.10
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
            float _SizeX, _SizeZ, _Amp, _Scroll, _Gain, _Fade, _Ridge, _Mist;
            float _SkyMode, _SkyHue, _SunH, _SunSize, _SkyGain, _SkyClk;
            CBUFFER_END

            #include "TrippinSky.hlsl"

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float3 wp : TEXCOORD0; float h : TEXCOORD1; float fe : TEXCOORD2; };

            float Fbm(float2 p)
            {
                float s = 0.0, a = 0.5;
                for (int i = 0; i < 4; i++) { s += a * TNoise(p); p = p * 2.03 + 11.7; a *= 0.5; }
                return s;
            }

            // wgsl terrain_h(), world metres -> its noise units (x6.7 height,
            // x0.15 horizontally: the wgsl corridor was |x| < ~6 units).
            float Height(float2 w)
            {
                float2 p = float2(w.x, w.y + _Scroll) * 0.15;
                float broad = Fbm(p * 0.09);
                float ridged = 1.0 - abs(2.0 * TNoise(p * 0.22) - 1.0);
                float h = broad * 5.5 + ridged * ridged * (1.6 + 2.8 * _Ridge)
                        + Fbm(p * 0.5) * 0.6;
                float wall = smoothstep(0.8, 6.0, abs(p.x) + 1.2 * sin(p.y * 0.05));
                float hn = h * wall + 0.35 * Fbm(float2(p.x * 0.4, p.y * 0.18)) - 0.2;
                return hn * (_Amp / 12.0); // wgsl peaks ~ +-12 units
            }

            V vert(A i)
            {
                V o;
                float2 xz = (i.uv - 0.5) * float2(_SizeX, _SizeZ);
                float3 wp = TransformObjectToWorld(float3(xz.x, 0.0, xz.y));
                float edge = smoothstep(0.0, 0.10, min(i.uv.x, 1.0 - i.uv.x)) * smoothstep(0.0, 0.10, 1.0 - i.uv.y);
                float h = Height(wp.xz) * edge;
                wp.y += h;
                o.pos = TransformWorldToHClip(wp);
                o.wp = wp;
                o.h = h;
                o.fe = max(smoothstep(0.80, 1.0, i.uv.y), smoothstep(0.82, 1.0, abs(i.uv.x * 2.0 - 1.0)));
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float3 wp = i.wp;
                float3 cv = _WorldSpaceCameraPos - wp;
                float dist = length(cv);
                float3 vv = cv / max(dist, 1e-3);
                // Normals from the field (its noise unit is 1/0.15 m).
                float e = 0.5;
                float hx = Height(wp.xz + float2(e, 0.0)) - Height(wp.xz - float2(e, 0.0));
                float hz = Height(wp.xz + float2(0.0, e)) - Height(wp.xz - float2(0.0, e));
                float3 n = normalize(float3(-hx, 2.0 * e, -hz));

                // Rock and scree, greener low down, snow on high flat tops.
                float2 np = wp.xz * 0.15;
                float3 rock = lerp(float3(0.05, 0.045, 0.05), float3(0.14, 0.12, 0.11), Fbm(np * 0.8));
                float snow = smoothstep(2.6, 4.2, i.h / (_Amp / 12.0)) * smoothstep(0.55, 0.8, n.y);
                float3 alb = lerp(rock, float3(0.5, 0.55, 0.6), snow);
                alb = lerp(alb, float3(0.06, 0.09, 0.05), smoothstep(1.2, 0.2, i.h / (_Amp / 12.0)) * smoothstep(0.5, 0.85, n.y) * 0.7);

                // Low warm sun ahead (+z), cool ambient from the dawn sky.
                float3 sun = normalize(float3(0.15, _SunH, 1.0));
                float dif = saturate(dot(n, sun));
                float warm = 1.0 + 0.5 * _TIntensity;
                float3 col = alb * (float3(1.0, 0.6, 0.4) * dif * warm
                          + float3(0.1, 0.14, 0.22) * (0.5 + 0.5 * n.y));

                // Valley mist: thick on the floor, thinning with height and
                // range; tinted warm toward the sun.
                float mist = (1.0 - exp(-dist * 0.028 * 0.15)) * (0.7 + 0.3 * exp(-max(wp.y, 0.0) * 0.5));
                mist = saturate(mist * (0.55 + 0.9 * _Mist));
                float sunw = pow(saturate(dot(-vv, sun)), 3.0);
                float3 mistc = lerp(float3(0.5, 0.5, 0.62), float3(0.95, 0.6, 0.4), sunw);
                col = lerp(col, mistc * (0.55 + 0.3 * _TEnergy) * _SkyGain, mist);

                // Far ridge-line haze into the sky.
                float fog = max(1.0 - exp(-pow(abs(dist / _Fade), 1.4)), i.fe);
                float3 fogc = SkyCol(normalize(float3(-vv.x, 0.02, -vv.z)));
                col = lerp(col, fogc, fog);
                col *= _Gain;
                col = col / (1.0 + 0.3 * col);
                return float4(col, 1.0);
            }
            ENDHLSL
        }
    }
}
