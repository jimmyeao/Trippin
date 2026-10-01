// Crowd silhouettes, drawn instanced: each quad is one person, shaped
// procedurally (head, shoulders, torso, sometimes raised arms) from a seed
// taken from its position. Near-black with a thin rim of stage light, so
// the bright stage behind makes them pop. They bounce on the kick only
// while drums play (calm-aware).
Shader "Trippin/Crowd"
{
    Properties
    {
        _Rim ("Rim light", Float) = 1.2
    }
    SubShader
    {
        Tags { "RenderType"="TransparentCutout" "Queue"="AlphaTest" "RenderPipeline"="UniversalPipeline" }
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma multi_compile_instancing
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _Rim;
            CBUFFER_END

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; UNITY_VERTEX_INPUT_INSTANCE_ID };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float seed : TEXCOORD1; float3 wp : TEXCOORD2; };

            V vert(A i)
            {
                V o;
                UNITY_SETUP_INSTANCE_ID(i);
                float3 origin = TransformObjectToWorld(float3(0, 0, 0));
                float seed = THash(origin.xz * 7.31);
                // Bounce: each person on the kick with their own lag/height.
                float lag = seed * 0.25;
                float ph = frac(_TBeatPhase - lag);
                float bounce = exp(-ph * 6.0) * (1.0 - _TCalm) * (0.08 + 0.12 * seed) * _TIntensity;
                float3 p = i.pos.xyz;
                p.y += bounce / max(length(TransformObjectToWorld(float3(0, 1, 0)) - origin), 1e-3);
                o.wp = TransformObjectToWorld(p);
                o.pos = TransformWorldToHClip(o.wp);
                o.uv = i.uv;
                o.seed = seed;
                return o;
            }

            float sdBox(float2 p, float2 b) { float2 d = abs(p) - b; return length(max(d, 0)) + min(max(d.x, d.y), 0); }

            float4 frag(V i) : SV_Target
            {
                // Quad is ~0.45 wide x 1.0 tall; work in metres-ish units.
                float2 p = (i.uv - float2(0.5, 0.0)) * float2(0.46, 1.0);
                float s = i.seed;
                float lean = (s - 0.5) * 0.03;
                // Head (slightly forward), neck, shoulders tapering to the waist.
                float head = length((p - float2(lean, 0.83)) * float2(1.0, 0.85)) - 0.062;
                float y = saturate((p.y - 0.25) / 0.52);
                float halfW = lerp(0.09, 0.15, smoothstep(0.0, 0.85, y)) * (1.0 - 0.6 * smoothstep(0.9, 1.0, y));
                float torso = max(abs(p.x - lean * y) - halfW, abs(p.y - 0.5) - 0.27);
                float neck = sdBox(p - float2(lean, 0.755), float2(0.028, 0.04));
                float d = min(min(head, torso), neck);
                // Raised arms (more when it's loud): upper arm out from the
                // shoulder, forearm up, swaying on the flow clock.
                float up = step(0.6 - 0.35 * _TIntensity * (1.0 - _TCalm), frac(s * 13.7));
                if (up > 0.5) {
                    float sway = sin(_TFlow * 0.8 + s * 20.0) * 0.025;
                    for (int k = -1; k <= 1; k += 2) {
                        float2 sh = float2(k * 0.13 + lean, 0.72);
                        float2 el = float2(k * 0.2 + sway, 0.84);
                        float2 hd = float2(k * 0.17 + sway * 2.0, 0.965);
                        float2 a1 = p - sh, b1 = el - sh;
                        float t1 = saturate(dot(a1, b1) / dot(b1, b1));
                        float2 a2 = p - el, b2 = hd - el;
                        float t2 = saturate(dot(a2, b2) / dot(b2, b2));
                        d = min(d, length(a1 - b1 * t1) - 0.024);
                        d = min(d, length(a2 - b2 * t2) - 0.02);
                    }
                }
                clip(-d);
                float rim = smoothstep(-0.012, 0.0, d) * _Rim * step(0.5, p.y + 0.3);
                float3 c = TPalette(0.4 + s * 0.2) * rim * (0.3 + 0.7 * _TEnergy) + 0.004;
                return float4(c, 1);
            }
            ENDHLSL
        }
    }
}
