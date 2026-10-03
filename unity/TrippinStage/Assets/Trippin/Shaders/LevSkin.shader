// The leviathan's skin: the rib lattice from Leviathan.compute (SPINE rings
// of RIB_PTS points, last = first) drawn as a lit surface via an index
// buffer. Normals come from the neighbouring lattice points.
// Look: a dark dorsal side with a pale belly, cellular spots and gill slits,
// sunlight from above broken into moving caustics, a wet highlight, a
// translucent palette rim, glowing wing margins, and bioluminescent spots
// that light as the kick pulse runs head to tail.
Shader "Trippin/LevSkin"
{
    Properties
    {
        _Gain ("Glow gain", Float) = 1.0
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

            StructuredBuffer<float4> _P;      // body lattice (xyz, glow)
            StructuredBuffer<float4> _Spine;  // per segment xyz, w = kick pulse
            uint _Rings, _RingPts;            // SPINE, RIB_PTS
            float _Gain, _Time2;

            struct V
            {
                float4 pos : SV_POSITION;
                float3 n : TEXCOORD0;
                float3 wp : TEXCOORD1;
                float2 st : TEXCOORD2;   // s along the body, u round the ring
                float pulse : TEXCOORD3;
            };

            float3 P(uint ring, uint k)
            {
                ring = min(ring, _Rings - 1);
                k = k % (_RingPts - 1);
                return _P[ring * _RingPts + k].xyz;
            }

            V vert(uint vid : SV_VertexID)
            {
                V o;
                uint ring = vid / _RingPts, k = vid % _RingPts;
                float3 p = _P[vid].xyz;
                uint kk = k % (_RingPts - 1);
                float3 du = P(ring, kk + 1) - P(ring, kk + _RingPts - 2);
                float3 ds = P(ring + 1, kk) - P(ring > 0 ? ring - 1 : 0, kk);
                float3 n = cross(ds, du);
                float l = length(n);
                // Degenerate (a ring collapsed at the nose): point away from the spine.
                o.n = l > 1e-6 ? n / l : normalize(p - _Spine[min(ring, _Rings - 1)].xyz + 1e-4);
                o.wp = p;
                o.pos = TransformWorldToHClip(p);
                o.st = float2(ring / (float)(_Rings - 1), kk / (float)(_RingPts - 1));
                o.pulse = _Spine[min(ring, _Rings - 1)].w;
                return o;
            }

            float2 hash2(float2 c)
            {
                uint2 q = (uint2)(int2)(c + 1024.0);
                q = q * uint2(1597334673u, 3812015801u);
                q = (q.x ^ q.y) * uint2(1597334673u, 3812015801u);
                return float2(q) * (1.0 / 4294967296.0);
            }

            // Cellular spots in (s, u) surface space, wrapping round the ring.
            float spots(float2 st, float2 cells, out float id)
            {
                float2 p = st * cells;
                float2 b = floor(p);
                float best = 9.0; id = 0;
                [unroll] for (int y = -1; y <= 1; y++)
                [unroll] for (int x = -1; x <= 1; x++)
                {
                    float2 c = b + float2(x, y);
                    float2 cw = float2(c.x, fmod(c.y + cells.y, cells.y));
                    float2 h = hash2(cw);
                    float2 fp = c + 0.2 + 0.6 * h;
                    float d = length(p - fp) / (0.25 + 0.3 * h.x);
                    if (d < best) { best = d; id = h.y; }
                }
                return best;
            }

            // Sunlight through water: two drifting wave fields, sharpened.
            float caustic(float2 xz, float t)
            {
                float2 p = xz * 0.18;
                float c = 0.0;
                c += abs(sin(p.x * 1.7 + sin(p.y * 1.3 + t) * 1.6 + t * 0.7));
                c += abs(sin(p.y * 1.9 + sin(p.x * 1.1 - t * 0.8) * 1.4 - t * 0.5));
                return pow(saturate(1.0 - c * 0.5), 6.0);
            }

            float4 frag(V i, bool front : SV_IsFrontFace) : SV_Target
            {
                float3 n = normalize(front ? i.n : -i.n);
                float3 v = normalize(GetCameraPositionWS() - i.wp);
                float nv = saturate(dot(n, v));
                float th = i.st.y * 6.2831853;
                float top = smoothstep(-0.25, 0.25, sin(th));   // 1 dorsal, 0 belly
                float edge = 1.0 - abs(sin(th));                 // wing margins (th ~ 0, pi)
                // Albedo: slate back, pale belly; darker spots on the back.
                float sid;
                float sp = spots(i.st, float2(46.0, 26.0), sid);
                float spot = smoothstep(1.0, 0.7, sp);
                float3 back = lerp(float3(0.035, 0.05, 0.07), TPalette(0.6) * 0.06, 0.4);
                float3 belly = float3(0.5, 0.55, 0.6);
                float3 alb = lerp(belly, back * (1.0 - 0.5 * spot), top);
                // Gill slits on the belly behind the head.
                float gills = (1.0 - top) * step(0.1, i.st.x) * step(i.st.x, 0.2)
                            * smoothstep(0.5, 0.95, abs(sin(i.st.x * 3.14159 * 50.0)))
                            * smoothstep(0.35, 0.1, abs(i.st.y - 0.75));
                alb *= 1.0 - 0.7 * gills;
                // Light: sun from above through water, broken into caustics.
                float3 sun = normalize(float3(0.2, 1.0, 0.3));
                float diff = saturate(dot(n, sun));
                float ca = caustic(i.wp.xz, _Time2) * saturate(n.y);
                float3 col = alb * (0.06 + diff * (0.55 + 1.6 * ca)) * lerp(float3(0.75, 0.9, 1.0), TPalette(0.55), 0.25);
                // Under-light from below, cool.
                col += alb * saturate(-n.y) * 0.18 * TPalette(0.7);
                // Wet highlight.
                float3 r = reflect(-v, n);
                col += pow(saturate(dot(r, sun)), 60.0) * 0.8;
                // Translucent rim.
                col += pow(1.0 - nv, 3.0) * TPalette(0.45) * 0.55;
                // Glowing wing margins.
                col += smoothstep(0.75, 1.0, edge) * TPalette(0.35 + i.st.x * 0.3) * (0.35 + 1.4 * i.pulse) * _Gain;
                // Bioluminescent spots: a sparse subset lights with the kick pulse.
                float glowSpots = spot * step(0.72, sid) * top;
                col += glowSpots * TPalette(0.15 + sid * 0.5) * (0.12 + 2.2 * i.pulse) * _Gain;
                return float4(col, 1);
            }
            ENDHLSL
        }
    }
}
