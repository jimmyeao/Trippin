// unity_jellyfish's bells: a translucent glowing jellyfish bell. The mesh
// is only a uv grid (x round the bell, y apex 0 .. rim 1); the vertex
// shader lathes the bell from it, squeezing the lower bell in as it
// contracts (_Contract) and rippling the frilled rim. Drawn additively and
// two-sided: a faint body that brightens toward its silhouette (light
// scattering through the thin edges), radial canals, four glowing gonads
// round the top and a ring of lights on the rim.
Shader "Trippin/HqJelly"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
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

            float _Gain, _Contract, _Hue, _T, _Seed;

            struct A { float4 pos : POSITION; float2 uv : TEXCOORD0; };
            struct V { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; float3 n : TEXCOORD1; float3 v : TEXCOORD2; };

            float3 Bell(float u, float v)
            {
                float th = v * 1.3;
                float a = u * 6.2831853;
                float squeeze = 1.0 - 0.3 * _Contract * smoothstep(0.25, 1.0, v);
                float frill = 1.0 + 0.05 * sin(a * 16.0 + _T * 3.0 + _Seed * 9.0) * v * v * v * v;
                float r = sin(th) * squeeze * frill;
                float y = cos(th) * (1.0 + 0.2 * _Contract) - 0.25 * v * v * _Contract;
                return float3(cos(a) * r, y, sin(a) * r);
            }

            V vert(A i)
            {
                V o;
                float3 p = Bell(i.uv.x, i.uv.y);
                float3 pu = Bell(i.uv.x + 0.01, i.uv.y), pv = Bell(i.uv.x, i.uv.y + 0.01);
                float3 n = normalize(cross(pv - p, pu - p));
                float3 wp = TransformObjectToWorld(p);
                o.n = normalize(TransformObjectToWorldDir(n));
                o.v = normalize(_WorldSpaceCameraPos - wp);
                o.uv = i.uv;
                o.pos = TransformWorldToHClip(wp);
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float u = i.uv.x, v = i.uv.y;
                float nv = abs(dot(normalize(i.n), i.v));
                float3 c = TPalette(_Hue);
                c *= c;
                float3 c2 = TPalette(_Hue + 0.12);
                c2 *= c2;
                // The body: faint inside, bright toward the silhouette.
                float edge = pow(1.0 - nv, 2.5);
                float3 col = c * (0.05 + 0.75 * edge);
                // Radial canals.
                float canal = pow(abs(cos(u * 3.14159 * 8.0)), 60.0) * smoothstep(0.1, 0.4, v);
                col += c * canal * 0.35;
                // Four gonads: soft horseshoes round the top.
                float g = abs(cos(u * 3.14159 * 4.0 + 0.4));
                col += c2 * smoothstep(0.75, 1.0, g) * exp(-pow((v - 0.38) / 0.09, 2.0)) * 0.6;
                // The rim: a band of lights, twinkling round it.
                float rim = smoothstep(0.86, 0.97, v) * (1.0 - smoothstep(0.985, 1.0, v));
                float dots = pow(abs(sin(u * 3.14159 * 24.0)), 8.0);
                col += lerp(c, float3(1, 1, 1), 0.3) * rim * (0.4 + 1.6 * dots * (0.6 + 0.4 * sin(_T * 4.0 + u * 30.0)));
                return float4(col * _Gain, 0);
            }
            ENDHLSL
        }
    }
}
