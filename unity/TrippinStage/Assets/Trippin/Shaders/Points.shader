// Draws the flow particles: 6 vertices per particle (a camera-facing quad
// built from SV_VertexID), soft additive dots coloured by palette and
// brightened by speed, so turbulent streams glow hotter.
Shader "Trippin/Points"
{
    Properties
    {
        _Size ("Size", Float) = 0.07
        _Gain ("Gain", Float) = 0.5
        _Spark ("Hit Flare", Float) = 1.0
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
            #include "TrippinCommon.hlsl"

            StructuredBuffer<float4> _Pos;
            float _Size, _Gain, _Spark;

            struct V { float4 pos : SV_POSITION; float2 q : TEXCOORD0; float3 col : TEXCOORD1; };

            V vert(uint vid : SV_VertexID)
            {
                V o;
                uint i = vid / 6;
                uint c = vid % 6;
                float2 corner[6] = { float2(-1,-1), float2(1,-1), float2(-1,1), float2(-1,1), float2(1,-1), float2(1,1) };
                float2 q = corner[c];
                float4 pd = _Pos[i];
                // Integer hash: frac(i * 0.618) loses precision at i ~ 260k.
                uint hs = i * 747796405u + 2891336453u;
                hs = ((hs >> ((hs >> 28u) + 4u)) ^ hs) * 277803737u;
                float h = ((hs >> 22u) ^ hs) * (1.0 / 4294967296.0);
                float3 right = UNITY_MATRIX_V[0].xyz;
                float3 up = UNITY_MATRIX_V[1].xyz;
                // Hats: a scattered subset of particles flares on each hit.
                float spark = step(0.82, frac(h * 7.0 + floor(_TBeat * 2.0) * 0.37)) * _THits.w * _Spark;
                float size = _Size * (0.6 + 0.8 * h) * (1.0 + 2.0 * spark);
                float3 w = pd.xyz + (right * q.x + up * q.y) * size;
                o.pos = TransformWorldToHClip(w);
                o.q = q;
                float speed = pd.w;
                o.col = TPalette(h * 0.9 + speed * 0.02 + pd.y * 0.015) * (0.35 + speed * 0.06 + 1.5 * spark) * _Gain;
                // Distance falloff so the far side of the cloud sinks back —
                // without it every particle is the same brightness and the
                // shape reads flat. Also fade the few cm nearest the lens so
                // a mote crossing the camera plane doesn't pop as a blob.
                float pdist = length(pd.xyz - _WorldSpaceCameraPos);
                o.col *= exp(-pdist * 0.014) * smoothstep(0.15, 1.2, pdist);
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float m = saturate(1.0 - dot(i.q, i.q));
                return float4(i.col * m * m, 0);
            }
            ENDHLSL
        }
    }
}
