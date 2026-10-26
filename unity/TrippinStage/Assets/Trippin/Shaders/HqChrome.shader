// unity_chrome's picture: liquid-chrome metaballs, ray traced full screen.
// Up to 10 blobs (_Blobs: xyz centre, w radius) melt together through a
// smooth union; the surface is a polished metal that reflects a studio
// (_Env: an equirectangular panorama of softboxes, _Env2 a neon showroom,
// both generated with Alice, mixed by _EnvMix),
// each other (one bounce), and a black glossy floor that mirrors them. A
// thin-film tint (_Film) shifts the reflections' colour with the viewing
// angle, like anodised titanium; each blob carries its own palette colour
// (_BlobCol) into the metal where it dominates. Writes depth, so the
// show's depth of field focuses on the blobs and softens the studio.
Shader "Trippin/HqChrome"
{
    Properties
    {
        _Env ("Environment (equirect)", 2D) = "black" {}
        _Env2 ("Second environment", 2D) = "black" {}
        _EnvMix ("Environment mix", Float) = 1
        _EnvGain ("Environment gain", Float) = 1.4
        _EnvYaw ("Environment yaw", Float) = 0
        _Film ("Thin film", Float) = 0.5
        _Tint ("Colour tint", Float) = 0.6
        _Smooth ("Melt", Float) = 2.4
        _Gain ("Gain", Float) = 1
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Background" "RenderPipeline"="UniversalPipeline" }
        ZTest Always
        ZWrite On
        Cull Off
        Pass
        {
            HLSLPROGRAM
            #pragma vertex FsVert
            #pragma fragment frag
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"
            #include "TrippinFull.hlsl"

            TEXTURE2D(_Env);
            SAMPLER(sampler_Env);
            TEXTURE2D(_Env2);
            CBUFFER_START(UnityPerMaterial)
            float _EnvGain, _EnvYaw, _Film, _Tint, _Smooth, _Gain, _EnvMix;
            CBUFFER_END
            float4 _Blobs[10];
            float4 _BlobCol[10];
            int _BlobCount;

            static const float FloorY = 0.0;

            float SMin(float a, float b, float k)
            {
                float h = max(k - abs(a - b), 0.0) / k;
                return min(a, b) - h * h * k * 0.25;
            }

            float Blobs(float3 p)
            {
                float d = 1e5;
                for (int i = 0; i < _BlobCount; i++)
                    d = SMin(d, length(p - _Blobs[i].xyz) - _Blobs[i].w, _Smooth);
                return d;
            }

            // Each blob's share of the surface at p (for its colour).
            float3 BlobTint(float3 p)
            {
                float3 c = 0; float wsum = 0;
                for (int i = 0; i < _BlobCount; i++)
                {
                    float d = length(p - _Blobs[i].xyz) - _Blobs[i].w;
                    float w = exp(-max(d, 0.0) * 1.5) + 1e-4;
                    c += _BlobCol[i].rgb * w; wsum += w;
                }
                return c / wsum;
            }

            float3 Normal(float3 p)
            {
                const float2 e = float2(0.002, -0.002);
                return normalize(e.xyy * Blobs(p + e.xyy) + e.yyx * Blobs(p + e.yyx) + e.yxy * Blobs(p + e.yxy) + e.xxx * Blobs(p + e.xxx));
            }

            float March(float3 ro, float3 rd, float maxT, int steps)
            {
                float t = 0.0;
                for (int i = 0; i < steps; i++)
                {
                    float d = Blobs(ro + rd * t);
                    if (d < 0.002 * max(t, 1.0)) return t;
                    t += d * 0.9;
                    if (t > maxT) break;
                }
                return -1.0;
            }

            float3 Env(float3 r)
            {
                float yaw = atan2(r.x, r.z) / 6.2831853 + 0.5 + _EnvYaw;
                float pitch = acos(clamp(r.y, -1.0, 1.0)) / 3.14159265;
                float2 uv = float2(yaw, 1.0 - pitch);
                float3 c = lerp(SAMPLE_TEXTURE2D_LOD(_Env, sampler_Env, uv, 0).rgb, SAMPLE_TEXTURE2D_LOD(_Env2, sampler_Env, uv, 0).rgb, _EnvMix);
                // Lift the softboxes into HDR so they bloom in the metal.
                float l = dot(c, float3(0.299, 0.587, 0.114));
                return c * (0.35 + 2.6 * l * l) * _EnvGain;
            }

            // Thin-film interference, cheaply: a hue rotation with the angle.
            float3 Film(float cosT)
            {
                float t = (1.0 - cosT) * 2.2 + _RxClk.x * 0.02;
                return 0.5 + 0.5 * cos(6.2831853 * (t + float3(0.0, 0.33, 0.67)));
            }

            float3 Metal(float3 p, float3 n, float3 rd, int bounce)
            {
                float3 r = reflect(rd, n);
                float cosT = saturate(dot(-rd, n));
                float3 tint = lerp(float3(1, 1, 1), BlobTint(p) * 1.6, _Tint);
                tint = lerp(tint, tint * Film(cosT) * 1.8, _Film * (1.0 - cosT));
                float3 F = lerp(tint * 0.75, float3(1, 1, 1), pow(1.0 - cosT, 5.0));
                float3 c = Env(r);
                // The floor in the reflection: dark, catching the blobs' glow.
                if (r.y < 0.0) c = lerp(c * 0.08, c, saturate(-p.y * 0.02));
                return c * F;
            }

            struct FOut { float4 col : SV_Target; float depth : SV_Depth; };

            float Depth(float3 wp)
            {
                float4 cp = TransformWorldToHClip(wp);
                return cp.z / cp.w;
            }

            FOut frag(FsV i)
            {
                FOut o;
                float3 ro = _WorldSpaceCameraPos;
                float3 rd = normalize(i.rd);
                float tFloor = rd.y < -1e-4 ? (FloorY - ro.y) / rd.y : 1e5;
                float t = March(ro, rd, min(tFloor, 120.0), 110);
                float3 c;
                float3 hitP;
                if (t > 0.0)
                {
                    float3 p = ro + rd * t;
                    float3 n = Normal(p);
                    c = Metal(p, n, rd, 0);
                    // One bounce: the blobs see each other.
                    float3 r = reflect(rd, n);
                    float t2 = March(p + n * 0.02, r, 40.0, 48);
                    if (t2 > 0.0)
                    {
                        float3 p2 = p + n * 0.02 + r * t2;
                        float3 n2 = Normal(p2);
                        float cosT = saturate(dot(-rd, n));
                        float3 F = lerp(lerp(1.0, BlobTint(p) * 1.6, _Tint) * 0.75, 1.0, pow(1.0 - cosT, 5.0));
                        c = Metal(p2, n2, r, 1) * F * 0.85;
                    }
                    hitP = p;
                }
                else if (tFloor < 1e4)
                {
                    // Black glossy floor mirroring the blobs.
                    float3 p = ro + rd * tFloor;
                    float3 rr = reflect(rd, float3(0, 1, 0));
                    float tr = March(p + float3(0, 0.01, 0), rr, 60.0, 64);
                    float fres = 0.04 + 0.5 * pow(1.0 - saturate(-rd.y), 4.0);
                    c = Env(rr) * fres * 0.25;
                    if (tr > 0.0)
                    {
                        float3 q = p + rr * tr;
                        c = Metal(q, Normal(q), rr, 1) * (0.25 + fres);
                    }
                    // Soft contact shadow where the blobs sit low.
                    float near = Blobs(p + float3(0, 0.4, 0));
                    c *= lerp(0.25, 1.0, saturate(near / 3.0));
                    c *= exp(-tFloor * 0.012);
                    hitP = p;
                }
                else
                {
                    c = Env(rd) * 0.35;
                    hitP = ro + rd * 200.0;
                }
                o.col = float4(c * _Gain, 1.0);
                o.depth = Depth(hitP);
                return o;
            }
            ENDHLSL
        }
    }
}
