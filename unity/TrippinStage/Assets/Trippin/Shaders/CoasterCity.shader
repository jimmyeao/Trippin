// The neon megacity round the coaster: instanced towers (one draw call) whose
// facades carry lit window grids, corner neon strips, horizontal neon bands
// and giant spectrum billboards near the roofs — the night-city part of
// shaders/scenes/neon_coaster.wgsl ported from pixel work to boxes.
// Towers never move; only their lights respond: windows ride the spectrum,
// a colour ring ripples out on each kick, more windows light as it builds.
Shader "Trippin/CoasterCity"
{
    Properties
    {
        _Block ("City block size (m)", Float) = 40
        _Mirror ("Mirror copy", Float) = 0
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
            #pragma multi_compile_instancing
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Block, _Mirror;
            CBUFFER_END
            float _Wave, _WaveAmp, _Lit, _FogDist;

            struct A { float4 pos : POSITION; float3 n : NORMAL; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct V { float4 pos : SV_POSITION; float3 wp : TEXCOORD0; float3 n : TEXCOORD1; float3 op : TEXCOORD2; float seed : TEXCOORD3; float top : TEXCOORD4; };

            float h21(float2 p)
            {
                uint2 q = (uint2)(int2)(p + 32768.0);
                q = q * uint2(1597334673u, 3812015801u);
                uint n = (q.x ^ q.y) * 1597334673u;
                return n * (1.0 / 4294967296.0);
            }

            V vert(A i)
            {
                V o;
                UNITY_SETUP_INSTANCE_ID(i);
                o.wp = TransformObjectToWorld(i.pos.xyz);
                o.pos = TransformWorldToHClip(o.wp);
                o.n = TransformObjectToWorldNormal(i.n);
                o.op = i.pos.xyz;
                float3 origin = TransformObjectToWorld(float3(0, 0, 0));
                o.seed = h21(origin.xz * 0.37);
                o.top = TransformObjectToWorld(float3(0, 0.5, 0)).y;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float3 n = normalize(i.n);
                float3 wp = i.wp;
                // The mirror copy is below the floor; real heights are y >= 0.
                float wy = _Mirror > 0.5 ? -wp.y : wp.y;
                float top = abs(i.top);
                float3 col = float3(0.01, 0.01, 0.018);
                if (abs(n.y) < 0.5)
                {
                    float uu = abs(n.x) > 0.5 ? wp.z : wp.x;
                    // Window grid, spectrum-loud blocks more likely lit.
                    float2 g = float2(uu * 0.7, wy * 0.45);
                    float2 wc = floor(g);
                    float2 f = frac(g);
                    float pane = smoothstep(0.42, 0.32, abs(f.x - 0.5)) * smoothstep(0.4, 0.28, abs(f.y - 0.5));
                    float r = h21(wc + i.seed * 73.0 + n.xz * 3.0);
                    float lit = step(0.8, r);
                    float wave = smoothstep(0.7, 1.0, sin(length(wp.xz) * 0.02 - _TClock.z * 0.5) * 0.5 + 0.5);
                    lit = max(lit, step(0.45, r) * wave * saturate(RxSpec(frac(uu / _Block + 0.5)) * 1.2) * (0.3 + _Lit));
                    float3 tone = lerp(float3(0.3, 0.9, 1.0), float3(1.0, 0.3, 0.8), step(0.5, h21(wc + 3.3)));
                    col += tone * lit * pane * (0.4 + 0.5 * _Lit);
                    // Vertical neon edge strips on the tower corners.
                    float corner = abs(n.x) > 0.5 ? abs(i.op.z) : abs(i.op.x);
                    col += smoothstep(0.47, 0.5, corner) * TPalette(i.seed) * 0.3;
                    // Horizontal neon bands on some towers (~0.6 m every 24 m).
                    if (frac(i.seed * 7.31) < 0.4)
                    {
                        float band = smoothstep(0.025, 0.0, abs(frac(wy / 24.0) - 0.5));
                        col += TPalette(i.seed + _TBeat * 0.001) * band * (0.7 + 0.7 * _TEnergy);
                    }
                    // Giant LED billboards (the spectrum) high on tall towers.
                    if (frac(i.seed * 3.77) < 0.3 && top > 70.0)
                    {
                        float bw = abs(n.x) > 0.5 ? i.op.z + 0.5 : i.op.x + 0.5;
                        if (wy > top - 30.0 && wy < top && abs(bw - 0.5) < 0.4)
                        {
                            float x = abs(bw - 0.5) / 0.4;
                            float lvl = RxSpec(x * 0.8);
                            float yy = (wy - (top - 30.0)) / 30.0;
                            float bar = step(yy, lvl * 0.95) * smoothstep(0.45, 0.3, abs(frac(x * 16.0) - 0.5));
                            float3 pal = TPalette(x * 0.7 + i.seed * 0.5);
                            col = pal * (0.06 + bar * 2.0) * (0.6 + 0.8 * _TIntensity);
                        }
                    }
                    // The kick wave: a ring of colour expanding over the city.
                    float dist = length(wp.xz);
                    float ring = exp(-(dist - _Wave) * (dist - _Wave) * 0.0015) * _WaveAmp;
                    col += ring * TPalette(0.4 + dist * 0.002) * 0.8;
                }
                else if (n.y > 0.5)
                {
                    // Roof: a blinking red aircraft light near the centre.
                    float blink = step(0.85, frac(_Time.y * 0.5 + i.seed * 3.0));
                    float d = length(i.op.xz);
                    col += float3(1.0, 0.1, 0.05) * smoothstep(0.06, 0.0, d) * (0.3 + 3.0 * blink);
                }
                // Smog: magenta haze with distance, thicker low down.
                float dist2 = length(wp - GetCameraPositionWS());
                float fog = (1.0 - exp(-dist2 / _FogDist)) * (0.65 + 0.55 * smoothstep(80.0, 0.0, wy));
                float3 fogCol = lerp(float3(0.12, 0.035, 0.1), float3(0.02, 0.02, 0.03), saturate(wy / 60.0));
                col = lerp(col, fogCol, saturate(fog));
                if (_Mirror > 0.5)
                {
                    clip(-i.wp.y);
                    col *= 0.35;
                }
                return float4(col, 1);
            }
            ENDHLSL
        }
    }
}
