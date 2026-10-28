// unity_paint: wet paint on a woofer membrane, lying face up. A fine grid
// (Kit.GridMesh, uv 0..1 across the membrane) displaced in the vertex
// shader: the cone pumps with the bass (_Exc), ripples run out from the
// centre (_Rip, travelling on _T), each kick sends a ring (_Ring[i]: front
// radius, amplitude) and where the excursion is high the paint stands up in
// fingers (_Spike) from the baked noise. The fragment shades the same height
// field: marbled bands of the palette (domain-warped noise, drifting on _T),
// a wet clear-coat highlight from the two key lights and the room glow.
// Units are the gear shows' (1 = 10 cm); radius _R.
Shader "Trippin/HqPaint"
{
    Properties
    {
        _R ("Membrane radius", Float) = 1.5
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
            #include "TrippinCommon.hlsl"

            TEXTURE3D(_Vol); SAMPLER(sampler_Vol);
            float _R, _Gain, _Exc, _Rip, _Spike, _T, _Hue, _Mix;
            float4 _Ring[4];
            float4 _GLPos[4], _GLCol[4];
            float4 _GEnvA, _GEnvB, _GEnvC;

            float Vol(float3 p) { return SAMPLE_TEXTURE3D_LOD(_Vol, sampler_Vol, p, 0).r; }

            // Height (units) of the painted surface at membrane point p (xz, units).
            float Hgt(float2 p)
            {
                float r = length(p) / _R;
                float cone = saturate(1.0 - r * r);
                float h = -0.16 * r * r * _R;                                   // the cone's dish
                h += _Exc * cone;
                h += _Rip * cos(r * 18.0 - _T * 4.0) * cone * 0.05;
                [unroll] for (int k = 0; k < 4; k++)
                {
                    float d = r * _R - _Ring[k].x;
                    h += _Ring[k].y * exp(-d * d * 10.0) * cone;
                }
                float n = Vol(float3(p * 0.11, _T * 0.03));
                h += pow(saturate((n - 0.52) * 3.2), 2.0) * _Spike * cone;
                h += 0.035 * cone;                                              // the paint's own depth
                return h;
            }

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 p : TEXCOORD0; float3 wp : TEXCOORD1; };

            V vert(A i)
            {
                V o;
                float2 p = (i.uv - 0.5) * 2.0 * _R;
                // Square grid to disc, so the rim is round and the cells even.
                float2 q = i.uv * 2.0 - 1.0;
                p = float2(q.x * sqrt(1.0 - q.y * q.y * 0.5), q.y * sqrt(1.0 - q.x * q.x * 0.5)) * _R;
                float3 lp = float3(p.x, Hgt(p), p.y);
                o.wp = TransformObjectToWorld(lp);
                o.pos = TransformWorldToHClip(o.wp);
                o.p = p;
                return o;
            }

            float3 Paint(float2 p)
            {
                // Marbled bands: warped noise, the warp drifting slowly.
                float3 q = float3(p * 0.09, _T * 0.012);
                float wx = Vol(q + float3(0.3, 0.1, 0.0)) - 0.5, wy = Vol(q + float3(0.7, 0.5, 0.2)) - 0.5;
                float m = Vol(float3(p * 0.06 + float2(wx, wy) * 0.9 * _Mix, _T * 0.01 + 0.4));
                float band = frac(m * 3.5 + _Hue);
                float3 a = TPalette(floor(band * 4.0) / 4.0 + _Hue);
                float3 b = TPalette(floor(band * 4.0 + 1.0) / 4.0 + _Hue);
                float e = smoothstep(0.75, 1.0, frac(band * 4.0));
                float3 c = lerp(a, b, e);
                // White and black paint streaks through it.
                float s = Vol(float3(p * 0.15 + 3.0, _T * 0.02));
                c = lerp(c, float3(0.9, 0.9, 0.88), smoothstep(0.72, 0.8, s) * 0.8);
                c = lerp(c, float3(0.01, 0.01, 0.012), smoothstep(0.24, 0.17, s) * 0.85);
                return c * c * 0.9;                                             // squared for saturation
            }

            float4 frag(V i) : SV_Target
            {
                float e = 0.02;
                float h0 = Hgt(i.p);
                float hx = Hgt(i.p + float2(e, 0)), hz = Hgt(i.p + float2(0, e));
                float3 n = normalize(TransformObjectToWorldNormal(normalize(float3(h0 - hx, e, h0 - hz))));
                float3 v = normalize(_WorldSpaceCameraPos - i.wp);
                float3 alb = Paint(i.p);
                float r = length(i.p) / _R;
                float3 col = alb * (_GEnvA.rgb * 0.6 + 0.02);
                [unroll] for (int k = 0; k < 4; k++)
                {
                    float3 L = _GLPos[k].xyz - i.wp;
                    float dist = length(L); L /= dist;
                    float att = saturate(1.0 - dist / max(_GLPos[k].w, 1e-3));
                    att *= att;
                    float nl = saturate(dot(n, L));
                    float3 hv = normalize(L + v);
                    float sp = pow(saturate(dot(n, hv)), 180.0) * 6.0 + pow(saturate(dot(n, hv)), 24.0) * 0.25;
                    col += _GLCol[k].rgb * att * (alb * nl + sp * nl);
                }
                // Room glow in the wet coat: the two low glows wrap round the surface.
                float f = pow(1.0 - saturate(dot(n, v)), 4.0);
                float3 rf = reflect(-v, n);
                col += (_GEnvB.rgb * saturate(rf.x * 0.5 + 0.5) + _GEnvC.rgb * saturate(-rf.x * 0.5 + 0.5)) * (0.08 + 0.6 * f);
                col *= smoothstep(1.02, 0.96, r);
                return float4(col * _Gain, 1);
            }
            ENDHLSL
        }
    }
}
