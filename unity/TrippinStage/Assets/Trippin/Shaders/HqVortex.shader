// unity_vortex's particles: a tornado of light, several hundred thousand
// sparks with no stored state. Each particle is worked out in the vertex
// shader from its index: a height up the funnel (rising, wrapping at the
// top), a radius (narrow at the ground, flaring toward the sky), an angle
// that turns faster the nearer the core (so the swirl shears into spiral
// spirals); most of them ride eight thin strands that wind up the funnel
// and turn together; a few are debris skimming round the base. Each is a
// quad stretched along its motion, so fast inner sparks draw as streaks.
//  _Clk      the swirl's angle clock (integrated on the CPU, never jumps)
//  _ClkRate  its rate now (streak length)
//  _Rise     the climb clock
//  _Width    funnel width (breathes with the bass)
//  _Sway     x phase, y amount: the funnel's axis bends, more higher up
//  _Pulse[4] x height (0..1), y amount: kick rings bulging up the funnel
//  _Spec     how much the spectrum (bass at the base, highs at the top) bulges it
//  _Hue      palette offset; _Gain brightness; _Size spark size
//  _Mirror   1: draw its reflection in the wet ground
Shader "Trippin/HqVortex"
{
    Properties
    {
        _Gain ("Gain", Float) = 1
        _Size ("Size", Float) = 1
        _Height ("Height", Float) = 24
        _Width ("Width", Float) = 1
        _Twist ("Twist", Float) = 3
        _Spec ("Spectrum", Float) = 1
        _Hue ("Hue", Float) = 0
        _Mirror ("Mirror", Float) = 0
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

            float _Gain, _Size, _Height, _Width, _Twist, _Spec, _Hue, _Mirror;
            float _Clk, _ClkRate, _Rise;
            float4 _Sway;
            float4 _Pulse[4];

            struct V { float4 pos : SV_POSITION; float2 q : TEXCOORD0; float3 col : TEXCOORD1; };

            uint VHash(uint x)
            {
                x ^= x >> 16; x *= 0x7feb352du; x ^= x >> 15; x *= 0x846ca68bu; x ^= x >> 16;
                return x;
            }
            float R(uint i, uint salt) { return (VHash(i * 9781u + salt * 6271u) & 0xffffff) / 16777216.0; }

            V vert(uint vid : SV_VertexID)
            {
                V o;
                uint id = vid / 6, c = vid % 6;
                float2 corner[6] = { float2(-1,-1), float2(1,-1), float2(-1,1), float2(-1,1), float2(1,-1), float2(1,1) };
                float2 q = corner[c];
                o.q = q;

                float u1 = R(id, 1), u2 = R(id, 2), u3 = R(id, 3), u4 = R(id, 4), u5 = R(id, 5), u6 = R(id, 6), u7 = R(id, 7);
                bool debris = u5 < 0.08;
                bool strand = u5 >= 0.08 && u5 < 0.72;
                float arm = floor(u6 * 8.0);
                float y, rb, h, ang, w;
                if (debris)
                {
                    // Debris skimming round the base in loose spiral arms.
                    y = 0.0;
                    rb = 1.2 + 12.0 * pow(u3, 1.3);
                    h = 0.05 + 0.6 * u1 * u1 * exp(-rb * 0.15);
                    w = 1.6 / (rb * 0.3 + 0.6);
                    ang = floor(u6 * 3.0) * 2.0943951 + (u2 - 0.5) * 0.9 + log(rb) * 1.8 + _Clk * w;
                }
                else
                {
                    y = frac(u1 + _Rise * (strand ? 1.0 : 0.6 + 0.8 * u4));
                    float rf = 0.5 + 10.0 * pow(y, 1.4);
                    if (strand)
                    {
                        // Eight strands wind up the funnel and turn together, each a
                        // thin fuzzy filament round its helix.
                        rb = rf * (1.0 + (u3 - 0.5) * 0.14);
                        w = 2.0;
                        ang = arm * 0.7853982 + (u2 - 0.5) * 0.22 + y * _Twist + _Clk * w;
                        h = y * _Height + (u7 - 0.5) * 0.35;
                    }
                    else
                    {
                        // The rest fill the funnel, the core turning faster than the
                        // rim so it shears into fine spirals.
                        rb = rf * (0.75 + 0.5 * u3);
                        w = 2.6 / (rb * 0.4 + 0.6);
                        ang = u2 * 6.2831853 + _Clk * w;
                        h = y * _Height;
                    }
                }
                // The visible radius: width, the spectrum and the kick rings bulge it.
                float bulge = 1.0 + _Spec * 0.35 * RxSpec(saturate(y * 0.85));
                [unroll] for (int k = 0; k < 4; k++)
                {
                    float d = (y - _Pulse[k].x) / 0.07;
                    bulge += _Pulse[k].y * exp(-d * d);
                }
                float r = rb * _Width * (debris ? 1.0 : bulge);
                float2 axis = float2(sin(_Sway.x + y * 2.2), cos(_Sway.x * 0.7 + y * 1.7)) * _Sway.y * y * y * 5.0;
                float2 cs = float2(cos(ang), sin(ang));
                float3 wp = float3(axis.x + r * cs.x, h, axis.y + r * cs.y);
                float3 wv = float3(-cs.y, 0.0, cs.x) * r * w * _ClkRate + float3(0, _Height * 0.08, 0);
                if (_Mirror > 0.5)
                {
                    // Its reflection in the wet ground, broken sideways by ripples.
                    wp.y = -wp.y;
                    wv.y = -wv.y;
                    wp.x += sin(wp.y * 1.3 + _Time.y * 1.7 + u2 * 6.0) * -wp.y * 0.03;
                }

                // Fade in at the ground, out at the top of the funnel.
                float fade = debris ? 0.5 * saturate(1.0 - rb / 13.0) : saturate(y / 0.05) * saturate((1.0 - y) / 0.2) * (strand ? 0.8 : 0.9);
                float size = 0.035 * _Size * (0.6 + 0.8 * u3) * (debris ? 0.8 : 1.0 + y);

                float3 toCam = normalize(_WorldSpaceCameraPos - wp);
                float3 along = wv - toCam * dot(wv, toCam);
                float sp = length(along);
                float3 dirA = sp > 1e-4 ? along / sp : UNITY_MATRIX_V[0].xyz;
                float3 side = normalize(cross(dirA, toCam));
                float len = size + sp * 0.03;
                float3 pw = wp + dirA * (q.y * len - (len - size)) + side * q.x * size;
                o.pos = TransformWorldToHClip(pw);

                // Colour: the palette up the funnel, each arm its own shade, a
                // white-hot core.
                float3 pal = TPalette(_Hue + y * 0.45 + arm * 0.07 + (debris ? 0.5 : 0.0));
                pal *= pal;
                float core = exp(-rb * 2.5);
                float3 col = lerp(pal * 1.6, float3(1.3, 1.25, 1.2), core * 0.7);
                o.col = col * fade * _Gain * (size / len) * (0.5 + u4) * (_Mirror > 0.5 ? 0.12 * exp(wp.y * 0.1) : 1.0);
                return o;
            }

            float4 frag(V i) : SV_Target
            {
                float r = saturate(1.0 - dot(i.q, i.q));
                return float4(i.col * r * r, 0);
            }
            ENDHLSL
        }
    }
}
