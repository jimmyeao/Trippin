// The colossus's sculpted head: a unit sphere mesh pushed out to an android
// face (brow ridge, eye sockets, nose ridge and tip, cheekbones, mouth line,
// chin), with a hinged jaw. Object space: front +z, up +y; ColossusShow
// places it on the neck bone and scales it to the head ellipsoid.
// Look: a polished chrome face mask on a white ceramic skull, a glowing
// seam where they meet, glowing eyes whose pupils follow _Look, and light in
// the mouth when the jaw opens.
// Music: the jaw opens with the vocal presence (_Jaw, smoothed); the eyes
// brighten with the bass (_Eyes); the seam carries the kick pulse (_Pulse).
Shader "Trippin/AndroidHead"
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

            CBUFFER_START(UnityPerMaterial)
            float _Gain, _Mirror;
            CBUFFER_END
            float _Jaw, _Eyes, _Pulse, _FloorY;
            float4 _Look;   // xy: where the pupils look (-1..1)

            static const float3 EYE_L = float3(-0.36, 0.10, 0.927);
            static const float3 EYE_R = float3(0.36, 0.10, 0.927);

            float g(float x, float s) { return exp(-x * x / (s * s)); }

            float angleTo(float3 d, float3 a) { return acos(clamp(dot(d, normalize(a)), -1.0, 1.0)); }

            // Radius of the face surface along direction d (unit sphere = 1).
            float faceR(float3 d)
            {
                float front = smoothstep(0.15, 0.65, d.z);
                float r = 1.0;
                // Eye sockets.
                r -= 0.085 * (g(angleTo(d, EYE_L), 0.17) + g(angleTo(d, EYE_R), 0.17)) * front;
                // Brow ridge.
                r += 0.045 * g(d.y - 0.29, 0.07) * g(d.x, 0.55) * front;
                // Nose ridge and tip.
                r += 0.06 * g(d.x, 0.07) * smoothstep(-0.34, -0.12, d.y) * (1.0 - smoothstep(0.1, 0.24, d.y)) * front;
                r += 0.05 * g(d.x, 0.09) * g(d.y + 0.25, 0.07) * front;
                // Cheekbones.
                r += 0.04 * (g(length(d.xy - float2(-0.5, -0.1)), 0.16) + g(length(d.xy - float2(0.5, -0.1)), 0.16)) * front;
                // Mouth line and chin.
                r -= 0.03 * g(d.y + 0.47, 0.03) * g(d.x, 0.24) * front;
                r += 0.045 * g(length(d.xy - float2(0.0, -0.7)), 0.17) * front;
                // A narrower jaw, a fuller crown.
                r *= 1.0 - 0.12 * saturate(-d.y - 0.2) * saturate(abs(d.x) * 2.0);
                return r;
            }

            // The jaw: everything below the mouth line on the front swings
            // down about a hinge near the ears.
            float jawW(float3 d) { return smoothstep(-0.42, -0.5, d.y) * smoothstep(-0.2, 0.25, d.z); }

            float3 shape(float3 d)
            {
                float3 p = d * faceR(d);
                float w = jawW(d) * _Jaw;
                float3 hinge = float3(0, -0.32, -0.05);
                float a = 0.32 * w;
                float c = cos(a), s = sin(a);
                float3 q = p - hinge;
                q.yz = float2(c * q.y - s * q.z, s * q.y + c * q.z);
                return hinge + q;
            }

            struct A { float4 pos : POSITION; };
            struct V { float4 pos : SV_POSITION; float3 n : TEXCOORD0; float3 wp : TEXCOORD1; float3 d : TEXCOORD2; };

            V vert(A i)
            {
                V o;
                float3 d = normalize(i.pos.xyz);
                float3 t1 = normalize(cross(abs(d.y) < 0.99 ? float3(0, 1, 0) : float3(1, 0, 0), d));
                float3 t2 = cross(d, t1);
                const float e = 0.008;
                float3 p0 = shape(d);
                float3 p1 = shape(normalize(d + t1 * e));
                float3 p2 = shape(normalize(d + t2 * e));
                float3 n = normalize(cross(p1 - p0, p2 - p0));
                o.wp = TransformObjectToWorld(p0);
                o.pos = TransformWorldToHClip(o.wp);
                o.n = TransformObjectToWorldNormal(n);
                o.d = d;
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

            float4 frag(V i, bool front : SV_IsFrontFace) : SV_Target
            {
                float3 d = normalize(i.d);
                float3 n = normalize(front ? i.n : -i.n);
                float3 v = normalize(GetCameraPositionWS() - i.wp);
                float nv = saturate(dot(n, v));
                float3 r = reflect(-v, n);
                float fr = 0.04 + 0.96 * pow(1.0 - nv, 5.0);
                // The face mask: the front, inside an oval.
                float maskD = length(float2(d.x * 1.15, (d.y + 0.12) * 0.95)) - 0.62;
                float mask = smoothstep(0.02, -0.02, maskD) * step(0.0, d.z);
                float3 chrome = env(r) * lerp(0.45, 1.0, fr) * lerp(float3(0.75, 0.75, 0.8), TPalette(0.2), 0.25);
                float key = saturate(dot(n, normalize(float3(0.5, 0.6, 0.6))));
                float fill = saturate(dot(n, normalize(float3(-0.7, 0.1, 0.4))));
                float3 ceramic = float3(0.62, 0.63, 0.66) * (0.02 + key * key * 1.1 + fill * 0.3 * TPalette(0.6)) + env(r) * fr * 0.8;
                float3 col = lerp(ceramic, chrome, mask);
                col += pow(1.0 - nv, 3.0) * TPalette(0.45) * 0.5;
                // The seam round the mask, and one over the crown.
                float seam = g(maskD, 0.012) * step(0.0, d.z) + g(d.x, 0.012) * step(0.25, d.y) * step(d.z, 0.5);
                col += TPalette(0.35) * seam * (0.6 + 2.0 * _Pulse) * _Gain;
                // Eyes: a glowing lens in each socket, the pupil offset by _Look.
                [unroll] for (int k = 0; k < 2; k++)
                {
                    float3 e = normalize(k == 0 ? EYE_L : EYE_R);
                    float a = angleTo(d, e);
                    float lens = smoothstep(0.11, 0.08, a);
                    float3 pc = normalize(e + float3(_Look.x * 0.06, _Look.y * 0.045, 0));
                    float pupil = smoothstep(0.035, 0.022, angleTo(d, pc));
                    float iris = smoothstep(0.075, 0.03, angleTo(d, pc));
                    float3 glow = TPalette(0.05 + 0.1 * k) * (1.2 + 2.5 * _Eyes);
                    col = lerp(col, glow * (0.35 + 0.65 * iris) * (1.0 - 0.85 * pupil), lens);
                }
                // Light inside the mouth as the jaw opens.
                col += TPalette(0.05) * g(d.y + 0.47, 0.035) * g(d.x, 0.2) * step(0.0, d.z) * _Jaw * 2.0 * _Gain;
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
