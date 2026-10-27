// unity_bubbles' bubbles: soap bubbles as camera-facing impostors. Each
// fragment intersects its view ray with the bubble (an ellipsoid, so it can
// wobble) and shades both films it crosses: thin-film interference from the
// soap's thickness (thinner at the top, where it drains, swirling with its
// own currents) gives the rainbow, Fresnel-weighted over reflections of a
// studio of softboxes and neon (equirect _Env/_Env2). Additive, so bubbles
// overlap freely.
//  _B[i]   xyz centre, w radius (w <= 0: none)
//  _BS[i]  x wobble amount, y wobble phase, z swirl seed, w pop (0 whole .. 1 gone)
Shader "Trippin/HqBubble"
{
    Properties
    {
        _Env ("Environment (equirect)", 2D) = "black" {}
        _Env2 ("Second environment", 2D) = "black" {}
        _EnvMix ("Environment mix", Float) = 0.4
        _Gain ("Gain", Float) = 1
        _Film ("Film", Float) = 1
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

            TEXTURE2D(_Env); SAMPLER(sampler_Env);
            TEXTURE2D(_Env2);
            float _EnvMix, _Gain, _Film, _T;
            float4 _B[64], _BS[64];

            struct V { float4 pos : SV_POSITION; float3 wp : TEXCOORD0; nointerpolation uint id : TEXCOORD1; };

            V vert(uint vid : SV_VertexID)
            {
                V o;
                uint id = vid / 6, c = vid % 6;
                float2 corner[6] = { float2(-1,-1), float2(1,-1), float2(-1,1), float2(-1,1), float2(1,-1), float2(1,1) };
                float2 q = corner[c];
                float4 b = _B[id];
                o.id = id;
                if (b.w <= 0.0 || _BS[id].w >= 1.0) { o.pos = float4(0, 0, -10, 1); o.wp = 0; return o; }
                // A quad facing the camera, big enough for the wobble and the pop.
                float3 toCam = normalize(_WorldSpaceCameraPos - b.xyz);
                float3 right = normalize(cross(float3(0, 1, 0), toCam));
                float3 up = cross(toCam, right);
                float s = b.w * 1.45;
                o.wp = b.xyz + (right * q.x + up * q.y) * s + toCam * b.w;
                o.pos = TransformWorldToHClip(o.wp);
                return o;
            }

            float3 Env(float3 r)
            {
                float yaw = atan2(r.x, r.z) / 6.2831853 + 0.5 + _T * 0.004;
                float pitch = acos(clamp(r.y, -1.0, 1.0)) / 3.14159265;
                float2 uv = float2(yaw, 1.0 - pitch);
                float3 c = lerp(SAMPLE_TEXTURE2D_LOD(_Env, sampler_Env, uv, 3.5).rgb, SAMPLE_TEXTURE2D_LOD(_Env2, sampler_Env, uv, 3.5).rgb, _EnvMix);
                float l = dot(c, float3(0.299, 0.587, 0.114));
                return c * (0.3 + 2.2 * l * l);
            }

            // Thin-film interference: reflectance of a soap film d nm thick at
            // red, green and blue, seen at cosT.
            float3 Film(float d, float cosT)
            {
                float n = 1.33;
                float cosI = sqrt(saturate(1.0 - (1.0 - cosT * cosT) / (n * n)));
                float3 lambda = float3(650.0, 532.0, 450.0);
                float3 ph = 6.2831853 * 2.0 * n * d * cosI / lambda;
                float3 s = sin(ph * 0.5);
                return s * s;
            }

            float3 Shade(float3 p, float3 n, float3 rd, float4 bs, float r, float3 c)
            {
                float cosT = abs(dot(rd, n));
                float F = 0.04 + 0.96 * pow(1.0 - cosT, 5.0);
                float3 l = (p - c) / r;
                // Thickness: drains toward the top; currents swirl it round.
                float sw = sin(l.x * 5.0 + _T * 0.7 + bs.z * 6.0) * cos(l.z * 4.0 - _T * 0.5 + bs.z * 3.0) + 0.5 * sin(l.y * 9.0 + atan2(l.z, l.x) * 2.0 + _T * 1.1);
                float d = 240.0 + 420.0 * (1.0 - l.y) * 0.5 + 230.0 * sw;
                float3 film = Film(d, cosT) * 3.0;
                float3 refl = Env(reflect(rd, n));
                float3 col = (refl + 0.06) * lerp(float3(F, F, F), film * (0.18 + 0.82 * F), _Film);
                // A key light glint.
                float3 h = normalize(normalize(float3(-0.5, 0.8, -0.4)) - rd);
                col += pow(saturate(dot(n, h)), 300.0) * 6.0;
                return col;
            }

            float4 frag(V i) : SV_Target
            {
                float4 b = _B[i.id], bs = _BS[i.id];
                float3 ro = _WorldSpaceCameraPos;
                float3 rd = normalize(i.wp - ro);
                // Wobble: squash and stretch as an ellipsoid.
                float w = bs.x * sin(_T * 7.0 + bs.y);
                float3 sc = float3(1.0 + w, 1.0 - 0.8 * w, 1.0 + w * 0.6) * b.w * (1.0 + 0.25 * bs.w);
                float3 o = (ro - b.xyz) / sc, dd = rd / sc;
                float A = dot(dd, dd), B = dot(o, dd), C = dot(o, o) - 1.0;
                float disc = B * B - A * C;
                if (disc <= 0.0) discard;
                float sq = sqrt(disc);
                float t0 = (-B - sq) / A, t1 = (-B + sq) / A;
                if (t1 <= 0.0) discard;
                float3 col = 0;
                // Each film: the normal of the ellipsoid there.
                float3 p0 = ro + rd * t0, p1 = ro + rd * t1;
                float3 n0 = normalize((p0 - b.xyz) / (sc * sc)), n1 = normalize((p1 - b.xyz) / (sc * sc));
                if (t0 > 0.0) col += Shade(p0, n0, rd, bs, b.w, b.xyz);
                col += Shade(p1, -n1, rd, bs, b.w, b.xyz) * 0.75;
                // Popping: the film tears from one side and fades.
                float pop = bs.w;
                if (pop > 0.0)
                {
                    float3 l = (p0 - b.xyz) / b.w;
                    float tear = step(pop * 2.4 - 1.2, -l.x * 0.6 - l.y * 0.8);
                    col *= tear * (1.0 - pop);
                }
                return float4(col * _Gain, 0);
            }
            ENDHLSL
        }
    }
}
