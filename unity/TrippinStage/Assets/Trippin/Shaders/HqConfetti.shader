// unity_confetti's paper and foil (HqKit.cs ParticleField with this
// material): each particle is a small rectangle tumbling about its own
// axis as it falls and flutters sideways. It's lit by the stage's lights
// (_LightDir/_LightCol, up to 3): matt colour on one side, metallic foil on
// a share of the pieces, which flash white-hot whenever a face turns to
// catch a light. Opaque with depth, so depth of field can turn the near
// pieces into bokeh.
Shader "Trippin/HqConfetti"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
        _Foil ("Foil share", Float) = 0.4
        _Glint ("Glint", Float) = 1
        _Flutter ("Flutter", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Cull Off
        ZWrite On
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
            float _Gain, _Foil, _Glint, _Flutter;
            float4 _LightDir[3], _LightCol[3];
            float4 _Ambient;

            struct V { float4 pos : SV_POSITION; float3 n : TEXCOORD0; float3 wp : TEXCOORD1; float3 col : TEXCOORD2; float foil : TEXCOORD3; float2 q : TEXCOORD4; };

            float3 Rot(float3 v, float3 k, float a)
            {
                return v * cos(a) + cross(k, v) * sin(a) + k * dot(k, v) * (1.0 - cos(a));
            }

            V vert(uint vid : SV_VertexID)
            {
                V o;
                P p = _P[vid / 6];
                uint c = vid % 6;
                float2 corner[6] = { float2(-1,-1), float2(1,-1), float2(-1,1), float2(-1,1), float2(1,-1), float2(1,1) };
                float2 q = corner[c];
                o.q = q;
                if (p.life <= 0.0 || p.delay > 0.0) { o.pos = float4(0, 0, -10, 1); o.n = 0; o.wp = 0; o.col = 0; o.foil = 0; return o; }
                float age = p.maxLife - p.life;
                float s = p.seed;
                // Tumble: each piece spins about its own axis at its own rate.
                float3 axis = normalize(float3(sin(s * 91.0), cos(s * 57.0), sin(s * 23.0 + 1.0)));
                float spin = (3.0 + 6.0 * frac(s * 13.7)) * (frac(s * 7.1) > 0.5 ? 1.0 : -1.0);
                float a = s * 6.2831853 + age * spin;
                float3 u = Rot(float3(1, 0, 0), axis, a), v = Rot(float3(0, 1, 0), axis, a * 0.7 + 1.0);
                v = normalize(v - u * dot(u, v));
                float size = p.col.a;
                float3 wp = p.pos + (u * q.x * 0.6 + v * q.y) * size;
                // Flutter: a sideways sway as it falls (stronger for slow pieces).
                float fl = saturate(1.0 - length(p.vel) * 0.15) * _Flutter;
                wp.x += sin(age * (2.0 + 3.0 * s) + s * 40.0) * 0.35 * fl;
                wp.z += cos(age * (1.7 + 2.0 * s) + s * 17.0) * 0.25 * fl;
                o.wp = wp;
                o.pos = TransformWorldToHClip(wp);
                o.n = normalize(cross(u, v));
                o.col = p.col.rgb;
                o.foil = step(1.0 - _Foil, frac(s * 31.7));
                return o;
            }

            float4 frag(V i, bool front : SV_IsFrontFace) : SV_Target
            {
                float3 n = normalize(front ? i.n : -i.n);
                float3 v = normalize(_WorldSpaceCameraPos - i.wp);
                float3 base = front ? i.col : lerp(i.col, float3(1, 1, 1), 0.55);
                float3 c = base * _Ambient.rgb;
                for (int k = 0; k < 3; k++)
                {
                    float3 l = normalize(_LightDir[k].xyz);
                    float diff = abs(dot(n, l));                     // paper is lit from either side
                    float3 h = normalize(l + v);
                    float sp = pow(saturate(abs(dot(n, h))), lerp(12.0, 160.0, i.foil));
                    float3 lc = _LightCol[k].rgb;
                    c += base * lc * diff * (1.0 - 0.6 * i.foil);
                    c += lc * sp * lerp(0.15, 6.0, i.foil) * _Glint * lerp(float3(1, 1, 1), base, 0.5 * i.foil);
                }
                // Slightly rounded corners so pieces don't read as perfect squares.
                float r = max(abs(i.q.x), abs(i.q.y));
                clip(1.02 - r);
                return float4(c * _Gain, 1);
            }
            ENDHLSL
        }
    }
}
