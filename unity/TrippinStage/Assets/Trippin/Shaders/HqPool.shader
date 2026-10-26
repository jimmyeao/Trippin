// unity_pool's picture: a swimming pool at night, ray traced full screen.
// The water's surface is a sum of travelling waves plus ripple rings
// (_Rip: xy centre, z radius, w height) dropped in by the show; each view
// ray reflects off it (a night sky and the deck's lights, by Fresnel) and
// refracts into the pool, down to a mosaic-tiled floor and walls (_Tiles,
// Alice-generated). The floor carries caustics: for each floor point the
// light falling through the surface above is bent by the waves, and the
// change in the area it lands on (finite differences) gives how much it's
// concentrated, the bright dancing net of a real pool. Underwater lights in
// the walls (_LightCol) wash it in the palette's colours.
Shader "Trippin/HqPool"
{
    Properties
    {
        _Tiles ("Tiles", 2D) = "white" {}
        _Waves ("Waves", Float) = 1
        _Caustic ("Caustics", Float) = 1
        _Depth ("Depth", Float) = 2.2
        _T ("Time", Float) = 0
        _Glint ("Glints", Float) = 1
        _Gain ("Gain", Float) = 1
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
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"
            #include "TrippinFull.hlsl"

            TEXTURE2D(_Tiles);
            SAMPLER(sampler_Tiles);
            CBUFFER_START(UnityPerMaterial)
            float _Waves, _Caustic, _Depth, _T, _Glint, _Gain;
            CBUFFER_END
            float4 _Rip[6];
            float4 _LightPos[4], _LightCol[4];

            static const float W = 6.0, L = 12.5;      // the pool's half width and half length

            float WaveH(float2 p)
            {
                float t = _T;
                float h = 0.0;
                h += sin(dot(p, float2(0.8, 0.6)) * 1.1 + t * 1.3) * 0.045;
                h += sin(dot(p, float2(-0.5, 0.86)) * 1.9 + t * 1.7) * 0.03;
                h += sin(dot(p, float2(0.95, -0.3)) * 3.3 + t * 2.4) * 0.016;
                h += sin(dot(p, float2(-0.2, -0.98)) * 5.1 + t * 3.1) * 0.009;
                h *= _Waves;
                for (int i = 0; i < 6; i++)
                {
                    float d = length(p - _Rip[i].xy);
                    float r = _Rip[i].z;
                    float env = exp(-pow((d - r) * 1.6, 2.0)) * _Rip[i].w;
                    h += sin((d - r) * 7.0) * env * 0.12;
                }
                return h;
            }

            float3 WaveN(float2 p)
            {
                const float e = 0.02;
                float h = WaveH(p);
                return normalize(float3(-(WaveH(p + float2(e, 0)) - h) / e, 1.0, -(WaveH(p + float2(0, e)) - h) / e));
            }

            // The surface as the light sees it: the waves plus fine ripples,
            // which focus it into the bright net (too fine to see on the surface).
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
            float CausH(float2 p)
            {
                float t = _T;
                // Two layers of drifting noise: irregular, never a lattice.
                float n = VN(p * 1.1 + float2(t * 0.35, t * 0.2)) + VN(p * 2.3 - float2(t * 0.25, -t * 0.4)) * 0.5;
                return WaveH(p) + n * 0.055 * _Waves;
            }

            // Where light falling straight down through the surface at p lands on the floor.
            float2 Land(float2 p)
            {
                const float e = 0.02;
                float h = CausH(p);
                float3 n = normalize(float3(-(CausH(p + float2(e, 0)) - h) / e, 1.0, -(CausH(p + float2(0, e)) - h) / e));
                float3 r = refract(float3(0, -1, 0), n, 1.0 / 1.33);
                return p + r.xz * (_Depth / max(-r.y, 0.2));
            }

            float Caustic(float2 q)
            {
                const float e = 0.02;
                float2 a = Land(q), b = Land(q + float2(e, 0)), c = Land(q + float2(0, e));
                float area = abs((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)) / (e * e);
                float k = 1.0 / max(area, 0.03);
                // Only where the light is focused hard: thin bright lines.
                return saturate((k - 0.95) * 0.6) * 2.2;
            }

            float3 Lights(float3 q)
            {
                float3 c = 0;
                for (int i = 0; i < 4; i++)
                {
                    float d = length(q - _LightPos[i].xyz);
                    c += _LightCol[i].rgb / (1.0 + d * d * 0.12);
                }
                return c;
            }

            float3 Sky(float3 r)
            {
                float3 c = lerp(float3(0.01, 0.012, 0.03), float3(0.03, 0.04, 0.09), saturate(1.0 - r.y));
                // A few deck lamps catching the surface.
                c += TPalette(0.15) * pow(saturate(dot(r, normalize(float3(0.6, 0.35, 0.7)))), 300.0) * 6.0;
                c += TPalette(0.55) * pow(saturate(dot(r, normalize(float3(-0.7, 0.3, 0.6)))), 300.0) * 6.0;
                return c;
            }

            // Inside the pool: from q along d to the floor or a wall; returns the colour there.
            float3 Underwater(float3 q, float3 d)
            {
                float tFloor = (-_Depth - q.y) / min(d.y, -1e-4);
                float tx = d.x > 0 ? (W - q.x) / d.x : (-W - q.x) / min(d.x, -1e-6);
                float tz = d.z > 0 ? (L - q.z) / d.z : (-L - q.z) / min(d.z, -1e-6);
                float t = min(tFloor, min(tx, tz));
                float3 h = q + d * t;
                float2 tuv;
                float3 base;
                bool floorHit = t == tFloor;
                if (floorHit) tuv = h.xz * 0.16;
                else if (t == tx) tuv = float2(h.z, h.y) * 0.16;
                else tuv = float2(h.x, h.y) * 0.16;
                base = SAMPLE_TEXTURE2D(_Tiles, sampler_Tiles, tuv).rgb;
                float3 light = Lights(h) * 0.9 + float3(0.02, 0.05, 0.07);
                float caus = floorHit ? Caustic(h.xz) : Caustic(float2(h.x, h.z)) * saturate(1.0 + h.y / _Depth) * 0.6;
                float3 c = base * light + (base * 0.6 + 0.4) * caus * _Caustic * (Lights(float3(h.x, 0, h.z)) * 0.6 + 0.35);
                // Water absorbs red first: the further the light travels, the bluer.
                float3 absorb = exp(-t * float3(0.35, 0.09, 0.06));
                return c * absorb;
            }

            float4 frag(FsV i) : SV_Target
            {
                float3 ro = _WorldSpaceCameraPos;
                float3 rd = normalize(i.rd);
                float3 c;
                float t0 = rd.y < -1e-4 ? -ro.y / rd.y : -1.0;
                float3 p = ro + rd * max(t0, 0.0);
                bool inPool = t0 > 0.0 && abs(p.x) < W && abs(p.z) < L;
                if (inPool)
                {
                    float3 n = WaveN(p.xz);
                    float3 rf = refract(rd, n, 1.0 / 1.33);
                    float fres = 0.02 + 0.98 * pow(1.0 - saturate(dot(-rd, n)), 5.0);
                    float3 under = Underwater(p, rf);
                    float3 refl = Sky(reflect(rd, n));
                    c = lerp(under, refl, fres);
                    // Glints: the lamps' highlights dancing on the wave crests.
                    float3 h = normalize(normalize(float3(0.4, 1.0, 0.8)) - rd);
                    c += pow(saturate(dot(n, h)), 600.0) * 3.0 * _Glint;
                }
                else if (t0 > 0.0)
                {
                    // The deck: dark stone, lit by the pool's glow near the edge.
                    float edge = max(abs(p.x) - W, abs(p.z) - L);
                    float2 tuv = p.xz * 0.12;
                    float3 stone = SAMPLE_TEXTURE2D(_Tiles, sampler_Tiles, tuv).rgb;
                    stone = dot(stone, float3(0.3, 0.4, 0.3)) * float3(0.14, 0.13, 0.12);
                    float glow = exp(-max(edge, 0.0) * 0.6);
                    c = stone * (0.25 + Lights(float3(clamp(p.x, -W, W), -0.5, clamp(p.z, -L, L))) * glow * 0.8);
                    c *= exp(-t0 * 0.02);
                }
                else c = Sky(rd);
                return float4(c * _Gain, 1.0);
            }
            ENDHLSL
        }
    }
}
