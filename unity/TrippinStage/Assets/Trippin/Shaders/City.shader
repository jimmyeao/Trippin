// The night city round the colossus: instanced boxes (one per tower) with
// lit window grids worked out from world position, rooftop aircraft lights,
// and height fog that swallows the far towers. The towers never move or
// change shape: only their lights respond. A ring of window colour ripples
// out from the plaza on each kick (_Wave = its radius), and more windows
// come on as the track builds (_Lit).
Shader "Trippin/City"
{
    Properties
    {
        _Mirror ("Mirror copy", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Cull Off // the mirrored copy flips the winding
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
            float _Mirror;
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
                // The tower's world-space height (the cube is centred on 0).
                o.top = TransformObjectToWorld(float3(0, 0.5, 0)).y;
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float3 n = normalize(i.n);
                // The mirror copy is below the floor; real positions are y >= 0.
                float3 wp = i.wp;
                float y = _Mirror > 0.5 ? -wp.y : wp.y;
                float top = abs(i.top);
                float3 col = float3(0.012, 0.014, 0.02);
                if (abs(n.y) < 0.5)
                {
                    // Facade: a window grid in world space along the face.
                    float u = abs(n.x) > 0.5 ? wp.z : wp.x;
                    float2 cell = float2(floor(u / 1.5), floor(y / 3.2));
                    float2 f = float2(frac(u / 1.5), frac(y / 3.2));
                    float win = smoothstep(0.24, 0.3, f.x) * smoothstep(0.76, 0.7, f.x) * smoothstep(0.3, 0.36, f.y) * smoothstep(0.74, 0.68, f.y);
                    float r = h21(cell + i.seed * 917.0);
                    float on = step(1.0 - (0.08 + 0.26 * _Lit), r) * (0.35 + 0.65 * frac(r * 13.7));
                    // Warm white mostly, some in the palette.
                    float3 wc = lerp(float3(1.0, 0.82, 0.55), TPalette(r * 3.0 + i.seed), step(0.7, frac(r * 7.3)));
                    // The kick wave: a ring of colour expanding over the city.
                    float dist = length(wp.xz);
                    float ring = exp(-(dist - _Wave) * (dist - _Wave) * 0.0015) * _WaveAmp;
                    col += win * (on * wc * 0.32 + ring * TPalette(0.4 + dist * 0.002) * 1.1);
                    // Faint vertical light strips on the tower corners.
                    float corner = abs(n.x) > 0.5 ? abs(i.op.z) : abs(i.op.x);
                    col += smoothstep(0.47, 0.5, corner) * TPalette(i.seed) * 0.15;
                }
                else if (n.y > 0.5)
                {
                    // Roof: a blinking red aircraft light near the centre.
                    float blink = step(0.85, frac(_Time.y * 0.5 + i.seed * 3.0));
                    float d = length(i.op.xz);
                    col += float3(1.0, 0.1, 0.05) * smoothstep(0.06, 0.0, d) * (0.3 + 3.0 * blink);
                }
                // Height fog: distance and low altitude both thicken it.
                float dist = length(wp - GetCameraPositionWS());
                float fog = 1.0 - exp(-dist / _FogDist) * (0.55 + 0.45 * smoothstep(0.0, 60.0, y));
                float3 fogCol = lerp(float3(0.02, 0.02, 0.03), TPalette(0.45) * 0.12, 0.6);
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
