// unity_synthwave's world: a neon-gridded landscape flown through at night
// under a retro sunset (the sky is HqBackdrop with an Alice-generated
// striped sun). A grid mesh displaced in the vertex shader: a flat road
// down the middle with dashed lane lines, valleys rising into ridged hills
// either side. The surface is dark glass: glowing grid lines (anti-aliased
// with fwidth), the sunset reflected in it by Fresnel, and fog toward the
// horizon in the sky's colour. Light pulses (_Pulse, metres ahead) roll
// down the grid toward the camera on the kicks; the hills never move.
Shader "Trippin/HqSynth"
{
    Properties
    {
        _SizeX ("Width (m)", Float) = 600
        _SizeZ ("Depth (m)", Float) = 700
        _Scroll ("Scroll (m)", Float) = 0
        _Hills ("Hill height", Float) = 1
        _Grid ("Grid brightness", Float) = 1
        _Cell ("Grid cell (m)", Float) = 8
        _FogCol ("Fog colour", Color) = (0.6, 0.15, 0.4, 1)
        _SunCol ("Sun colour", Color) = (1.2, 0.5, 0.3, 1)
        _LineA ("Line colour", Color) = (1, 0.2, 0.8, 1)
        _LineB ("Line colour far", Color) = (0.2, 0.8, 1, 1)
        _Gain ("Gain", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _SizeX, _SizeZ, _Scroll, _Hills, _Grid, _Cell, _Gain;
            float4 _FogCol, _SunCol, _LineA, _LineB;
            CBUFFER_END
            float4 _Pulse;     // four pulse fronts, metres ahead of the camera

            float VN(float2 x)
            {
                float2 i = floor(x), f = frac(x);
                float2 u = f * f * (3.0 - 2.0 * f);
                float a = frac(sin(dot(i, float2(127.1, 311.7))) * 43758.5453);
                float b = frac(sin(dot(i + float2(1, 0), float2(127.1, 311.7))) * 43758.5453);
                float c = frac(sin(dot(i + float2(0, 1), float2(127.1, 311.7))) * 43758.5453);
                float d = frac(sin(dot(i + float2(1, 1), float2(127.1, 311.7))) * 43758.5453);
                return lerp(lerp(a, b, u.x), lerp(c, d, u.x), u.y);
            }

            // The land: flat road, ridged hills rising either side (pattern coordinates).
            float Height(float x, float wz)
            {
                float side = smoothstep(16.0, 70.0, abs(x));
                float2 q = float2(x, wz) * 0.012;
                float r = 0.0, a = 1.0;
                for (int i = 0; i < 4; i++)
                {
                    float n = 1.0 - abs(VN(q) * 2.0 - 1.0);      // ridged
                    r += n * n * a;
                    q = q * 2.03 + 3.1; a *= 0.48;
                }
                float far = 1.0 + smoothstep(120.0, 300.0, abs(x)) * 1.5;
                return side * r * 38.0 * far * _Hills;
            }

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float3 wp : TEXCOORD0; float2 g : TEXCOORD1; float h : TEXCOORD2; };

            V vert(A i)
            {
                V o;
                float x = (i.uv.x - 0.5) * _SizeX;
                // Rows packed closer near the camera, where the detail shows.
                float z = pow(i.uv.y, 1.6) * _SizeZ;
                float wz = z + _Scroll;
                float h = Height(x, wz);
                float3 wp = TransformObjectToWorld(float3(x, h, z));
                o.wp = wp;
                o.g = float2(x, wz);
                o.h = h;
                o.pos = TransformWorldToHClip(wp);
                return o;
            }

            float Line(float coord, float cell, float width)
            {
                float d = abs(frac(coord / cell + 0.5) - 0.5) * cell;
                float w = fwidth(coord) * 1.2 + width;
                return 1.0 - smoothstep(width, w, d);
            }

            float4 frag(V i) : SV_Target
            {
                float3 v = normalize(_WorldSpaceCameraPos - i.wp);
                float3 n = normalize(cross(ddy(i.wp), ddx(i.wp)));
                if (n.y < 0.0) n = -n;
                float dist = length(_WorldSpaceCameraPos - i.wp);
                float ahead = i.wp.z - _WorldSpaceCameraPos.z;

                bool road = abs(i.g.x) < 14.0;
                float grid = max(Line(i.g.x, _Cell, 0.05), Line(i.g.y, _Cell, 0.05));
                // Lines thin out with distance instead of turning to mush.
                grid *= saturate(1.2 - dist / 450.0);
                float3 lineCol = lerp(_LineA.rgb, _LineB.rgb, saturate(i.h / 40.0));
                // Kick pulses roll toward the camera along the grid.
                float pulse = 0.0;
                for (int k = 0; k < 4; k++)
                {
                    float d = (ahead - _Pulse[k]) / 6.0;
                    pulse += exp(-d * d);
                }
                float3 c = float3(0.012, 0.004, 0.025);
                // Dark glass: the sunset reflected, stronger at grazing angles.
                float fres = 0.04 + 0.96 * pow(1.0 - saturate(dot(n, v)), 5.0);
                float3 r = reflect(-v, n);
                float sunRef = pow(saturate(dot(r, normalize(float3(0.0, 0.08, 1.0)))), 40.0);
                c += (_FogCol.rgb * 0.25 + _SunCol.rgb * sunRef * 2.0) * fres;
                if (road)
                {
                    c = float3(0.008, 0.006, 0.014) + _SunCol.rgb * sunRef * fres * 1.5;
                    float edge = 1.0 - smoothstep(0.15, 0.15 + fwidth(i.g.x) * 1.5, abs(abs(i.g.x) - 13.0));
                    float dash = step(0.5, frac(i.g.y / 9.0)) * (1.0 - smoothstep(0.12, 0.12 + fwidth(i.g.x) * 1.5, abs(i.g.x)));
                    c += _LineA.rgb * edge * 2.2 * _Grid + _LineB.rgb * dash * 1.4 * _Grid;
                }
                else c += lineCol * grid * (1.3 + 2.0 * pulse) * _Grid;
                // Haze toward the horizon, in the sunset's colour.
                float fog = 1.0 - exp(-max(dist - 60.0, 0.0) * 0.0045);
                c = lerp(c, _FogCol.rgb, fog);
                return float4(c * _Gain, 1.0);
            }
            ENDHLSL
        }
    }
}
