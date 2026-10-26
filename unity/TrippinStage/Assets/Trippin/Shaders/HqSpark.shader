// HQ particles (HqKit.cs ParticleField, HqParticles.compute): each spark is
// a camera-facing quad stretched along its screen-space velocity, so fast
// stars draw as streaks (a long-exposure look) and slow ones as round dots.
// Hot white cores fade to the spark's colour, then dim and twinkle as they
// die (crackle, stronger with _Crackle). Additive HDR, so bloom and depth of
// field turn them into soft discs when they're out of focus.
//  _Mirror   1: draw the reflection in water at y = _WaterY (flipped,
//            dimmed, shimmering sideways with _Shimmer)
Shader "Trippin/HqSpark"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
        _Streak ("Streak", Float) = 0.045
        _Crackle ("Crackle", Float) = 0
        _Mirror ("Mirror", Float) = 0
        _WaterY ("Water Y", Float) = 0
        _Shimmer ("Shimmer", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" }
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
            float _Gain, _Streak, _Crackle, _Mirror, _WaterY, _Shimmer;

            struct V { float4 pos : SV_POSITION; float2 q : TEXCOORD0; float3 col : TEXCOORD1; float hot : TEXCOORD2; };

            V vert(uint vid : SV_VertexID)
            {
                V o;
                P p = _P[vid / 6];
                uint c = vid % 6;
                float2 corner[6] = { float2(-1,-1), float2(1,-1), float2(-1,1), float2(-1,1), float2(1,-1), float2(1,1) };
                float2 q = corner[c];
                o.q = q;
                if (p.life <= 0.0 || p.delay > 0.0) { o.pos = float4(0, 0, -10, 1); o.col = 0; o.hot = 0; return o; }

                float3 wp = p.pos, wv = p.vel;
                if (_Mirror > 0.5)
                {
                    wp.y = 2.0 * _WaterY - wp.y;
                    wv.y = -wv.y;
                    // Ripples break the reflection into sideways shimmer, more
                    // the further below the waterline it falls.
                    float depth = max(_WaterY - wp.y, 0.0);
                    wp.x += sin(wp.y * 0.9 + _Time.y * 2.3 + p.seed * 6.0) * depth * 0.035 * _Shimmer;
                }
                float age = 1.0 - p.life / max(p.maxLife, 1e-3);
                float size = p.col.a * (1.0 - 0.45 * age);

                // Stretch along the velocity as seen on screen.
                float3 toCam = normalize(_WorldSpaceCameraPos - wp);
                float3 along = wv - toCam * dot(wv, toCam);
                float sp = length(along);
                float3 dirA = sp > 1e-4 ? along / sp : UNITY_MATRIX_V[0].xyz;
                float3 side = normalize(cross(dirA, toCam));
                float len = size + sp * _Streak;
                // The streak trails behind the spark: its head is the quad's front end.
                float3 w = wp + dirA * (q.y * len - (len - size)) + side * q.x * size;
                o.pos = TransformWorldToHClip(w);

                // Colour: white-hot at birth, the star's colour, then dimming.
                float fade = saturate(p.life / max(p.maxLife * 0.35, 1e-3));
                float twinkle = 1.0;
                if (age > 0.55)
                {
                    float t = _Time.y * (14.0 + 10.0 * p.seed) + p.seed * 40.0;
                    float f = frac(sin(floor(t) * 12.9898 + p.seed * 78.233) * 43758.5453);
                    twinkle = lerp(1.0, step(0.45, f) * 1.8, saturate(_Crackle) * saturate((age - 0.55) * 4.0));
                }
                o.hot = saturate(1.0 - age * 6.0);
                float3 col = lerp(p.col.rgb, float3(1.4, 1.3, 1.15), o.hot * 0.7);
                // A streak spreads its light over its length.
                o.col = col * fade * twinkle * _Gain * (size / len) * (_Mirror > 0.5 ? 0.35 : 1.0);
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float r = saturate(1.0 - dot(i.q, i.q));
                float core = r * r;
                return float4(i.col * (core + 0.6 * pow(r, 8.0) * i.hot), 0);
            }
            ENDHLSL
        }
    }
}
