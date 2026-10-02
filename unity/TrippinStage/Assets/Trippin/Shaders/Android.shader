// The colossus: a generated person (tools/android_mesh.py) re-skinned as a
// giant android. Object space is unit height, feet at y=0, facing +z.
//  - Armour: 3D Voronoi panels in object space; most white ceramic, some
//    dark chrome, with glowing palette seams between them.
//  - Kicks push the panels out along the normal (each panel its own
//    amount), staggered from the feet up like a wave; a drop bursts them.
//  - Light flows up the seams on the energy clock; the visor band on the
//    helmet glows with the bass. The head turns (vertices near the head
//    rotate about the neck), swinging with the phrase.
Shader "Trippin/Android"
{
    Properties
    {
        _SeamGain ("Seam glow", Float) = 1.0
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _SeamGain, _Mirror;
            CBUFFER_END
            float4 _Head;          // xyz head centre (object space), w head yaw (radians)
            float _Panel, _PanelWave, _Flow, _Visor, _FloorY, _Calm;

            #define CELLS 7.0

            float3 hash3(float3 c)
            {
                uint3 q = (uint3)(int3)(c + 4096.0);
                q = q * uint3(1597334673u, 3812015801u, 2798796415u);
                q = (q.x ^ q.y ^ q.z) * uint3(1597334673u, 3812015801u, 2798796415u);
                return float3(q) * (1.0 / 4294967296.0);
            }

            // Nearest and second-nearest feature distance, and the nearest cell.
            void voronoi(float3 p, out float f1, out float f2, out float3 cell)
            {
                float3 b = floor(p);
                f1 = 9.0; f2 = 9.0; cell = b;
                [unroll] for (int z = -1; z <= 1; z++)
                [unroll] for (int y = -1; y <= 1; y++)
                [unroll] for (int x = -1; x <= 1; x++)
                {
                    float3 c = b + float3(x, y, z);
                    float3 fp = c + 0.15 + 0.7 * hash3(c);
                    float d = length(p - fp);
                    if (d < f1) { f2 = f1; f1 = d; cell = c; }
                    else if (d < f2) f2 = d;
                }
            }

            // Head turn: rotate about the neck, fading out below it.
            float3 turnHead(float3 p)
            {
                float3 neck = _Head.xyz - float3(0, 0.075, 0);
                float w = saturate(1.0 - (length(p - _Head.xyz) - 0.07) / 0.04) * saturate((p.y - neck.y + 0.02) / 0.04);
                float a = _Head.w * w;
                float c = cos(a), s = sin(a);
                float3 q = p - neck;
                q.xz = float2(c * q.x + s * q.z, -s * q.x + c * q.z);
                return neck + q;
            }

            struct A { float4 pos : POSITION; float3 n : NORMAL; };
            struct V { float4 pos : SV_POSITION; float3 n : TEXCOORD0; float3 wp : TEXCOORD1; float3 op : TEXCOORD2; float push : TEXCOORD3; };

            V vert(A i)
            {
                V o;
                float3 p = i.pos.xyz;
                float f1, f2; float3 cell;
                voronoi(p * CELLS, f1, f2, cell);
                float h = hash3(cell * 1.7).x;
                // The kick wave climbs the body: panels near its front push out.
                float front = exp(-(p.y - _PanelWave) * (p.y - _PanelWave) * 30.0);
                float push = _Panel * (0.35 + 0.65 * h) * (0.4 + 0.6 * front);
                p += i.n * push * 0.012;
                float3 n = i.n;
                float3 pt = turnHead(p);
                n = normalize(turnHead(p + n * 0.01) - pt);
                o.wp = TransformObjectToWorld(pt);
                o.pos = TransformWorldToHClip(o.wp);
                o.n = TransformObjectToWorldNormal(n);
                o.op = i.pos.xyz;
                o.push = push;
                return o;
            }

            float3 env(float3 r)
            {
                float3 c = 0;
                c += pow(saturate(dot(r, normalize(float3(0.7, 0.4, 0.6)))), 10.0) * 2.6;
                c += pow(saturate(dot(r, normalize(float3(-0.8, 0.2, 0.3)))), 8.0) * 1.4 * lerp(1.0, TPalette(0.55), 0.5);
                c += pow(saturate(r.y), 3.0) * 1.2;
                return c;
            }

            float4 frag(V i) : SV_Target
            {
                float3 n = normalize(i.n);
                float3 v = normalize(GetCameraPositionWS() - i.wp);
                float nv = saturate(dot(n, v));
                float3 r = reflect(-v, n);
                float f1, f2; float3 cell;
                voronoi(i.op * CELLS, f1, f2, cell);
                float h = hash3(cell * 3.1).y;
                bool chrome = h > 0.72;
                float fr = 0.04 + 0.96 * pow(1.0 - nv, 5.0);
                float3 col;
                if (chrome)
                    col = env(r) * lerp(0.4, 1.0, fr) * lerp(float3(0.7, 0.7, 0.75), TPalette(0.2), 0.3) + 0.01;
                else
                {
                    // White ceramic: soft diffuse plus a clear-coat reflection.
                    float3 alb = float3(0.62, 0.63, 0.66);
                    float key = saturate(dot(n, normalize(float3(0.5, 0.6, 0.6))));
                    float fill = saturate(dot(n, normalize(float3(-0.7, 0.1, 0.4))));
                    col = alb * (0.02 + key * key * 1.1 + fill * 0.3 * TPalette(0.6)) + env(r) * fr * 0.8;
                }
                // Rim against the back glow.
                col += pow(1.0 - nv, 3.0) * TPalette(0.45) * 0.6;
                // Seams: light flowing up the body, brighter where panels open.
                float seam = smoothstep(0.035, 0.0, f2 - f1);
                float flow = 0.5 + 0.5 * sin(i.op.y * 40.0 - _Flow * 6.2831853);
                float glow = (0.6 + 1.0 * flow * flow) * (1.0 - 0.35 * _Calm) + 30.0 * i.push;
                col += TPalette(0.35 + i.op.y * 0.3) * seam * glow * _SeamGain;
                // Visor: a band across the front of the helmet.
                float3 hp = i.op - _Head.xyz;
                float onHead = step(length(hp * float3(1.0, 0.8, 1.0)), 0.075) * step(0.0, hp.z);
                // Low cameras see the helmet from below: sit the band a little
                // under the equator so it reads as eyes, not a cap.
                float band = smoothstep(0.016, 0.006, abs(hp.y + 0.012));
                col += TPalette(0.05) * onHead * band * (1.0 + 2.5 * _Visor);
                if (_Mirror > 0.5)
                {
                    clip(_FloorY - i.wp.y);
                    col *= 0.28 * exp(-(_FloorY - i.wp.y) * 0.12);
                }
                return float4(col, 1);
            }
            ENDHLSL
        }
    }
}
