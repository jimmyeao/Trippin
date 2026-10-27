// unity_halloween's bats: a flock worked out from each bat's index in the
// vertex shader (no stored state). Each circles the flock's centre (_Flock:
// xyz, w spread) on its own orbit, banking and bobbing, and flaps fast; it
// is drawn as a silhouette (body, ears, scalloped wings whose tips rise and
// fall with the flap) on a camera-facing quad, near-black with a faint rim
// of moonlight.
Shader "Trippin/HqBat"
{
    Properties
    {
        _Size ("Size", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Transparent" "Queue"="Transparent" "RenderPipeline"="UniversalPipeline" }
        Blend SrcAlpha OneMinusSrcAlpha
        ZWrite Off
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            float _Size, _T, _Flap;
            float4 _Flock, _Scatter;

            struct V { float4 pos : SV_POSITION; float2 q : TEXCOORD0; float flap : TEXCOORD1; float fade : TEXCOORD2; };

            uint BHash(uint x)
            {
                x ^= x >> 16; x *= 0x7feb352du; x ^= x >> 15; x *= 0x846ca68bu; x ^= x >> 16;
                return x;
            }
            float R(uint i, uint salt) { return (BHash(i * 9781u + salt * 6271u) & 0xffffff) / 16777216.0; }

            V vert(uint vid : SV_VertexID)
            {
                V o;
                uint id = vid / 6, c = vid % 6;
                float2 corner[6] = { float2(-1,-1), float2(1,-1), float2(-1,1), float2(-1,1), float2(1,-1), float2(1,1) };
                float2 q = corner[c];
                o.q = q;
                float u1 = R(id, 1), u2 = R(id, 2), u3 = R(id, 3), u4 = R(id, 4), u5 = R(id, 5);
                // Its own orbit round the flock centre: radius, tilt, speed, direction.
                float rad = _Flock.w * (0.35 + 0.65 * u1);
                float sp = (0.5 + 0.6 * u2) * (u3 < 0.5 ? 1.0 : -1.0);
                float a = _T * sp + u4 * 6.2831853;
                float3 off = float3(cos(a) * rad, sin(a * 1.7 + u5 * 6.0) * rad * 0.35, sin(a) * rad * 0.6);
                // The scatter: a kick blows the flock outward, then it regroups.
                off *= 1.0 + _Scatter.x * (0.6 + 0.8 * u5);
                float3 p = _Flock.xyz + off;
                float size = 0.45 * _Size * (0.7 + 0.6 * u3);
                o.flap = sin(_T * _Flap * (5.5 + 2.0 * u2) + u4 * 40.0);
                // Fade bats that swing round behind the camera's near plane.
                o.fade = saturate((p.z - 4.0) / 3.0);
                float3 right = UNITY_MATRIX_V[0].xyz, up = UNITY_MATRIX_V[1].xyz;
                // Bank into the turn.
                float bank = sp * 0.35;
                float2 qr = float2(q.x * cos(bank) - q.y * sin(bank), q.x * sin(bank) + q.y * cos(bank));
                o.pos = TransformWorldToHClip(p + (right * qr.x + up * qr.y * 0.6) * size);
                return o;
            }

            // A bat silhouette in quad space (x -1..1 wingspan, y -1..1).
            float Bat(float2 p, float flap)
            {
                float ax = abs(p.x);
                // Body and head.
                float body = length(float2(p.x / 0.11, (p.y + 0.05) / 0.32)) - 1.0;
                float head = length(float2(p.x / 0.09, (p.y - 0.28) / 0.09)) - 1.0;
                float ear = max(abs(ax - 0.05) - 0.03 * (0.45 - p.y) / 0.1, max(p.y - 0.45, 0.33 - p.y));
                // Wings: the leading edge rises and falls with the flap; the
                // trailing edge is scalloped between the finger bones.
                float lift = flap * 0.55;
                float top = 0.12 + lift * ax - 0.15 * ax * ax;
                float bottom = top - 0.42 * (1.0 - ax * ax) + 0.09 * abs(sin(ax * 3.14159 * 3.0)) + 0.05;
                float wing = max(max(p.y - top, bottom - p.y), ax - 1.0);
                float d = min(min(body, head) * 0.12, min(wing, ear));
                return d;
            }

            float4 frag(V i) : SV_Target
            {
                float d = Bat(i.q, i.flap);
                float a = saturate(-d / 0.03 + 0.5);
                if (a <= 0.0) discard;
                // Near-black with a faint moonlit rim.
                float rim = saturate(1.0 + d / 0.05) * 0.06;
                return float4(float3(0.01, 0.01, 0.018) + rim * float3(0.55, 0.6, 0.8), a * i.fade);
            }
            ENDHLSL
        }
    }
}
