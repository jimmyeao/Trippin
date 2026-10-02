// A solid sculpture in a void: a sphere mesh whose vertices are pushed out
// to one of five forms (pebble, urchin, twisted spire, lobed flower,
// dimpled disc), morphing between two. Normals come from finite
// differences of the same form, so it shades as a true solid.
// Look: dark glossy chrome/ceramic with studio softbox reflections (the
// environment stays fixed while the object turns, so highlights slide),
// palette-tinted key lights; colour drifts over the
// surface as an iridescent sheen, and a soft glow rides the kick ripple.
Shader "Trippin/Sculpture"
{
    Properties
    {
        _LineGain ("Line glow", Float) = 1.2
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" "RenderPipeline"="UniversalPipeline" }
        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma target 4.5
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "TrippinCommon.hlsl"

            CBUFFER_START(UnityPerMaterial)
            float _LineGain, _Mirror;
            CBUFFER_END
            float _FormA, _FormB, _Morph, _Swell, _Twist, _WaveFront, _WaveAmp, _LineT, _FloorY;

            static const float3 ICO[6] = {
                float3(0, 0.5257, 0.8507), float3(0, 0.5257, -0.8507),
                float3(0.5257, 0.8507, 0), float3(-0.5257, 0.8507, 0),
                float3(0.8507, 0, 0.5257), float3(-0.8507, 0, 0.5257) };

            float3 form(float3 d, int f)
            {
                float az = atan2(d.z, d.x);
                if (f == 0) // pebble: soft organic lumps
                {
                    float r = 1.0 + 0.2 * sin(3.0 * d.x + 1.3) * sin(2.5 * d.y + 0.4) * sin(3.5 * d.z + 2.0)
                                  + 0.1 * sin(5.0 * d.y + 3.0 * d.x);
                    return d * r * float3(1.0, 0.85, 1.0);
                }
                if (f == 1) // urchin: twelve rounded spikes (icosahedron vertices, both signs)
                {
                    // Smooth max over the spike axes: a hard max() leaves a
                    // crease between neighbouring spikes that aliases into a
                    // jagged seam.
                    float acc = 0.0;
                    [unroll] for (int k = 0; k < 6; k++) acc += exp(18.0 * (abs(dot(d, ICO[k])) - 1.0));
                    float m = 1.0 + log(acc) / 18.0;
                    float sp = saturate((m - 0.8) / 0.2);
                    return d * (0.82 + 0.7 * sp * sp * sp);
                }
                if (f == 2) // twisted spire: tall, fluted, tapering
                {
                    float y = d.y;
                    float flute = 1.0 + 0.16 * cos(7.0 * az + y * 5.0);
                    float taper = 0.95 - 0.35 * y;
                    return float3(d.x * taper * flute * 0.8, y * 1.3, d.z * taper * flute * 0.8);
                }
                if (f == 3) // lobed flower
                {
                    float s2 = 1.0 - d.y * d.y;
                    float r = 1.0 + 0.3 * cos(5.0 * az) * s2 + 0.18 * cos(3.0 * acos(clamp(d.y, -1.0, 1.0)));
                    return d * r;
                }
                // dimpled disc: flattened, the poles pressed in
                float rho = length(d.xz);
                float3 p = float3(d.x * 1.25, d.y * (0.18 + 0.4 * rho), d.z * 1.25);
                return p;
            }

            float3 shape(float3 d)
            {
                float3 p = lerp(form(d, (int)_FormA), form(d, (int)_FormB), _Morph);
                // Twist about the vertical, swinging with the phrase.
                float a = _Twist * p.y;
                float c = cos(a), s = sin(a);
                p.xz = float2(c * p.x - s * p.z, s * p.x + c * p.z);
                // Swell (bass) and the kick ripple running top to bottom.
                float ripple = exp(-(d.y - _WaveFront) * (d.y - _WaveFront) * 25.0) * _WaveAmp;
                return p * (1.0 + _Swell + 0.14 * ripple);
            }

            struct A { float4 pos : POSITION; };
            struct V { float4 pos : SV_POSITION; float3 n : TEXCOORD0; float3 wp : TEXCOORD1; float3 op : TEXCOORD2; float3 d : TEXCOORD3; };

            V vert(A i)
            {
                V o;
                float3 d = normalize(i.pos.xyz);
                // Tangents with cross(t1, t2) = d, so the normal faces out.
                float3 t1 = normalize(cross(abs(d.y) < 0.99 ? float3(0, 1, 0) : float3(1, 0, 0), d));
                float3 t2 = cross(d, t1);
                const float e = 0.01;
                float3 p0 = shape(d);
                float3 p1 = shape(normalize(d + t1 * e));
                float3 p2 = shape(normalize(d + t2 * e));
                float3 n = normalize(cross(p1 - p0, p2 - p0));
                o.wp = TransformObjectToWorld(p0);
                o.pos = TransformWorldToHClip(o.wp);
                o.n = TransformObjectToWorldNormal(n);
                o.op = p0;
                o.d = d;
                return o;
            }

            // Studio environment: two softboxes, a top light and a faint
            // horizon band, fixed in the world.
            float3 env(float3 r)
            {
                float3 c = 0;
                // Big near-white softboxes (a touch of palette) make it read
                // as polished metal; the palette lives in the rims and lines.
                c += pow(saturate(dot(r, normalize(float3(0.8, 0.35, 0.5)))), 10.0) * 3.0 * lerp(1.0, TPalette(0.05), 0.35);
                c += pow(saturate(dot(r, normalize(float3(-0.7, 0.15, -0.6)))), 8.0) * 1.8 * lerp(1.0, TPalette(0.55), 0.5);
                c += pow(saturate(r.y), 3.0) * 1.4;
                c += exp(-r.y * r.y * 30.0) * 0.25 * TPalette(0.3);
                return c;
            }

            float4 frag(V i) : SV_Target
            {
                float3 n = normalize(i.n);
                float3 v = normalize(GetCameraPositionWS() - i.wp);
                float3 r = reflect(-v, n);
                float nv = saturate(dot(n, v));
                float fr = 0.06 + 0.94 * pow(1.0 - nv, 5.0);
                float3 tint = lerp(float3(0.75, 0.75, 0.78), TPalette(0.15), 0.35);
                float3 col = env(r) * lerp(0.35, 1.0, fr) * tint;
                // Coloured rim so the silhouette separates from the void.
                col += pow(1.0 - nv, 3.0) * TPalette(0.45) * 0.8;
                // Key lights (palette), a little diffuse on the dark body.
                float3 l1 = normalize(float3(0.6, 0.7, -0.4)), l2 = normalize(float3(-0.8, -0.2, 0.5));
                float3 base = float3(0.02, 0.02, 0.025);
                col += base * (saturate(dot(n, l1)) * TPalette(0.1) * 3.0 + saturate(dot(n, l2)) * TPalette(0.6) * 2.0);
                // Iridescent sheen at grazing angles, its hue drifting on the
                // flow clock (no stripes: lines read as dividers).
                col += pow(1.0 - nv, 2.0) * TPalette(0.2 + 0.6 * (1.0 - nv) + _LineT * 0.05) * 0.35;
                // The kick ripple: a soft wide glow travelling down the form.
                float ripple = exp(-(i.d.y - _WaveFront) * (i.d.y - _WaveFront) * 9.0) * _WaveAmp;
                col += TPalette(0.4 + i.op.y * 0.15) * ripple * _LineGain * 0.45 * (0.4 + 0.6 * pow(1.0 - nv, 1.5)) * (1.0 - 0.5 * _TCalm);
                // Hat glints: sparse surface cells catch the hits.
                // The floor reflection copy fades into the dark below the floor.
                if (_Mirror > 0.5)
                {
                    clip(_FloorY - i.wp.y);
                    col *= 0.3 * exp(-(_FloorY - i.wp.y) * 0.7);
                }
                return float4(col, 1);
            }
            ENDHLSL
        }
    }
}
