// HQ smoke (HqKit.cs ParticleField with this material): soft billowing
// puffs left hanging where shells burst, drifting and spreading as they
// age. They're lit by the show's bursts: _Flash (rgb colour x intensity)
// at _FlashPos, falling off with distance, over a faint ambient grey, so
// each new burst lights up the smoke of the last ones the way real display
// smoke glows. Additive (it never darkens the sky).
Shader "Trippin/HqSmoke"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
        _Ambient ("Ambient", Color) = (0.02, 0.018, 0.022, 1)
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent-5" "RenderPipeline"="UniversalPipeline" }
        Blend One One
        ZWrite Off
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            struct P
            {
                float3 pos; float life;
                float3 vel; float maxLife;
                float4 col;
                float drag, grav, seed, delay;
            };
            StructuredBuffer<P> _P;
            float _Gain;
            float4 _Ambient;
            float4 _Flash[4];      // rgb colour x intensity
            float4 _FlashPos[4];   // xyz, w radius

            struct V { float4 pos : SV_POSITION; float2 q : TEXCOORD0; float3 col : TEXCOORD1; float seed : TEXCOORD2; float dens : TEXCOORD3; };

            V vert(uint vid : SV_VertexID)
            {
                V o;
                P p = _P[vid / 6];
                uint c = vid % 6;
                float2 corner[6] = { float2(-1,-1), float2(1,-1), float2(-1,1), float2(-1,1), float2(1,-1), float2(1,1) };
                float2 q = corner[c];
                o.q = q;
                o.seed = p.seed;
                if (p.life <= 0.0 || p.delay > 0.0) { o.pos = float4(0, 0, -10, 1); o.col = 0; o.dens = 0; return o; }
                float age = 1.0 - p.life / max(p.maxLife, 1e-3);
                float size = p.col.a * (0.5 + 1.6 * sqrt(age));      // puffs spread as they age
                float a = p.seed * 6.2831853 + age * (p.seed - 0.5) * 2.0;
                float2 rq = float2(q.x * cos(a) - q.y * sin(a), q.x * sin(a) + q.y * cos(a));
                float3 w = p.pos + (UNITY_MATRIX_V[0].xyz * rq.x + UNITY_MATRIX_V[1].xyz * rq.y) * size;
                o.pos = TransformWorldToHClip(w);
                float3 light = _Ambient.rgb;
                for (int i = 0; i < 4; i++)
                {
                    float d = length(p.pos - _FlashPos[i].xyz) / max(_FlashPos[i].w, 1.0);
                    light += _Flash[i].rgb / (1.0 + d * d * 4.0);
                }
                o.col = light * _Gain;
                // Fade in fast, thin out slowly.
                o.dens = saturate(age * 12.0) * (1.0 - age) * (1.0 - age);
                return o;
            }

            float N(float2 x)
            {
                float2 i = floor(x), f = frac(x);
                float2 u = f * f * (3.0 - 2.0 * f);
                float a = frac(sin(dot(i, float2(127.1, 311.7))) * 43758.5453);
                float b = frac(sin(dot(i + float2(1, 0), float2(127.1, 311.7))) * 43758.5453);
                float c = frac(sin(dot(i + float2(0, 1), float2(127.1, 311.7))) * 43758.5453);
                float d = frac(sin(dot(i + float2(1, 1), float2(127.1, 311.7))) * 43758.5453);
                return lerp(lerp(a, b, u.x), lerp(c, d, u.x), u.y);
            }

            float4 frag(V i) : SV_Target
            {
                float r = saturate(1.0 - dot(i.q, i.q));
                float2 x = i.q * 2.2 + i.seed * 17.0;
                float n = N(x) * 0.5 + N(x * 2.1 + 3.1) * 0.3 + N(x * 4.3 - 1.7) * 0.2;
                // Billows: the noise carves the puff into lumps with clear gaps.
                float m = saturate(r * (1.9 * n - 0.55)) * r;
                return float4(i.col * m * i.dens, 0);
            }
            ENDHLSL
        }
    }
}
