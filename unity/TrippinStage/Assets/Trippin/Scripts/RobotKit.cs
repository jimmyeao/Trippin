// Procedural androids for the robot shows: a handful of primitive parts (balls,
// cylinders, boxes) placed every frame from joint positions, with two-bone IK
// for arms and legs, shaded by Robot.shader (ceramic, chrome, emissive). No
// meshes to import, no skeleton: the pose is data (pelvis, chest, head, hands,
// feet), so a show can drive it from the music. Parts are drawn twice (the
// second, dimmer, copy mirrored under the floor) when `mirror` is set.
using UnityEngine;

namespace TrippinStage
{
    /// A fixed pool of one primitive kind sharing one material.
    public sealed class PartPool
    {
        public readonly int Count;
        readonly Transform[] _t, _m;
        readonly Renderer[] _r, _mr;
        readonly bool _mirror;
        readonly MaterialPropertyBlock _mpb = new MaterialPropertyBlock();
        static Mesh _cube, _sphere, _cyl;

        public enum Kind { Cube, Sphere, Cylinder }

        static Mesh Prim(Kind k)
        {
            if (_cube == null)
            {
                _cube = Take(PrimitiveType.Cube);
                _sphere = Take(PrimitiveType.Sphere);
                _cyl = Take(PrimitiveType.Cylinder);
            }
            return k == Kind.Cube ? _cube : (k == Kind.Sphere ? _sphere : _cyl);
        }

        static Mesh Take(PrimitiveType t)
        {
            var g = GameObject.CreatePrimitive(t);
            var m = g.GetComponent<MeshFilter>().sharedMesh;
            Object.Destroy(g);
            return m;
        }

        public PartPool(Transform parent, Material mat, Kind kind, int n, string name, bool mirror)
        {
            Count = n;
            _mirror = mirror;
            _t = new Transform[n]; _r = new Renderer[n];
            if (mirror) { _m = new Transform[n]; _mr = new Renderer[n]; }
            Mesh mesh = Prim(kind);
            for (int i = 0; i < n; i++)
            {
                _t[i] = Make(parent, mat, mesh, name + " " + i, out _r[i]);
                if (mirror)
                {
                    _m[i] = Make(parent, mat, mesh, name + " reflection " + i, out _mr[i]);
                    _mr[i].sharedMaterial = mat;
                }
            }
        }

        static Transform Make(Transform parent, Material mat, Mesh mesh, string name, out Renderer r)
        {
            var g = new GameObject(name);
            g.transform.SetParent(parent, false);
            g.AddComponent<MeshFilter>().sharedMesh = mesh;
            var mr = g.AddComponent<MeshRenderer>();
            mr.sharedMaterial = mat;
            mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            r = mr;
            return g.transform;
        }

        void Place(int i, Vector3 pos, Quaternion rot, Vector3 scale)
        {
            _r[i].enabled = true;
            _t[i].localPosition = pos;
            _t[i].localRotation = rot;
            _t[i].localScale = scale;
            if (_mirror)
            {
                _mr[i].enabled = true;
                _m[i].localPosition = new Vector3(pos.x, -pos.y, pos.z);
                _m[i].localRotation = new Quaternion(-rot.x, rot.y, -rot.z, rot.w);
                _m[i].localScale = scale;
            }
        }

        /// A cylinder of radius r from a to b (Unity's cylinder is 2 m tall, 0.5 m radius).
        public void Cyl(int i, Vector3 a, Vector3 b, float r)
        {
            Vector3 d = b - a;
            float len = d.magnitude;
            Quaternion q = len > 1e-4f ? Quaternion.FromToRotation(Vector3.up, d / len) : Quaternion.identity;
            Place(i, (a + b) * 0.5f, q, new Vector3(2f * r, Mathf.Max(len * 0.5f, 1e-3f), 2f * r));
        }

        /// A ball (or ellipsoid with `size` the full diameters) at c.
        public void Ball(int i, Vector3 c, float r) { Place(i, c, Quaternion.identity, new Vector3(2f * r, 2f * r, 2f * r)); }
        public void Ellipsoid(int i, Vector3 c, Quaternion q, Vector3 size) { Place(i, c, q, size); }
        public void Box(int i, Vector3 c, Quaternion q, Vector3 size) { Place(i, c, q, size); }

        public void Hide(int i)
        {
            _r[i].enabled = false;
            if (_mirror) _mr[i].enabled = false;
        }

        /// Per-part emission (palette position and strength) on top of the pool's material.
        public void Glow(int i, float hue, float emit)
        {
            _mpb.Clear();
            _mpb.SetFloat("_EmitHue", hue);
            _mpb.SetFloat("_Emit", emit);
            _r[i].SetPropertyBlock(_mpb);
            if (_mirror)
            {
                _mpb.SetFloat("_Emit", emit * 0.3f);
                _mr[i].SetPropertyBlock(_mpb);
            }
        }
    }

    /// The three materials every android shares.
    public sealed class RobotMats
    {
        public Material ceramic, chrome, glow, dark;
        readonly Material[] _all;

        public RobotMats(Material robot)
        {
            ceramic = new Material(robot); chrome = new Material(robot); glow = new Material(robot); dark = new Material(robot);
            dark.SetColor("_Base", new Color(0.035f, 0.037f, 0.045f, 1f));
            dark.SetFloat("_Metal", 0.75f); dark.SetFloat("_Rough", 0.3f);
            ceramic.SetColor("_Base", new Color(0.86f, 0.88f, 0.92f, 1f));
            ceramic.SetFloat("_Metal", 0.08f); ceramic.SetFloat("_Rough", 0.2f);
            chrome.SetColor("_Base", new Color(0.55f, 0.58f, 0.66f, 1f));
            chrome.SetFloat("_Metal", 0.95f); chrome.SetFloat("_Rough", 0.12f);
            glow.SetColor("_Base", new Color(0.05f, 0.05f, 0.06f, 1f));
            glow.SetFloat("_Metal", 0.5f); glow.SetFloat("_Emit", 1.2f);
            _all = new[] { ceramic, chrome, glow, dark };
        }

        public void Tune(float hue, float gain)
        {
            foreach (var m in _all) { m.SetFloat("_Hue", hue); m.SetFloat("_Gain", gain); }
        }
    }

    /// Joint targets for one android, in world space. Angles in degrees.
    public struct AndroidPose
    {
        public Vector3 pelvis;               // world position of the pelvis
        public float yaw;                    // body yaw about +y (0 = facing the pose's `facing`)
        public Vector3 chest;                // pitch (forward lean), yaw (twist), roll (side lean)
        public Vector3 head;                 // pitch (nod), yaw (turn), roll (tilt) relative to the chest
        public Vector3 handL, handR;         // world targets for the wrists
        public Vector3 footL, footR;         // world targets for the ankles
        public float eye, core;              // emission strength of the visor and the chest core
    }

    /// A humanoid of height `H`, built from primitives and posed from an AndroidPose.
    public sealed class Android
    {
        public readonly float H;
        readonly Quaternion _facing;         // maps the android's local +z (forward) to its world facing
        readonly PartPool _limb, _joint, _shell, _visor, _core, _bars, _acc;
        readonly bool _phones;
        public float Hue = 0.5f;
        const int Bars = 7;
        readonly PartPool _neck;

        // Limbs: 0/1 upper arm, 2/3 forearm, 4/5 thigh, 6/7 shin, 8 abdomen, 9 neck.
        // Joints: 0 pelvis, 1/2 shoulder, 3/4 elbow, 5/6 hip, 7/8 knee, 9/10 wrist, 11/12 ankle, 13/14 hand, 15/16 foot.
        // Shells: 0 chest, 1 head, 2/3 shoulder pad.

        public Android(Transform parent, RobotMats mats, float height, Quaternion facing, bool mirror, string name, bool headphones = false)
        {
            _phones = headphones;
            H = height;
            _facing = facing;
            _limb = new PartPool(parent, mats.ceramic, PartPool.Kind.Cylinder, 9, name + " limb", mirror);
            _neck = new PartPool(parent, mats.chrome, PartPool.Kind.Cylinder, 1, name + " neck", mirror);
            _acc = headphones ? new PartPool(parent, mats.dark, PartPool.Kind.Cylinder, 6, name + " headphones", mirror) : null;
            _joint = new PartPool(parent, mats.chrome, PartPool.Kind.Sphere, 17, name + " joint", mirror);
            _shell = new PartPool(parent, mats.ceramic, PartPool.Kind.Sphere, 4, name + " shell", mirror);
            _visor = new PartPool(parent, mats.glow, PartPool.Kind.Cube, 1, name + " visor", mirror);
            _core = new PartPool(parent, mats.glow, PartPool.Kind.Sphere, 1, name + " core", mirror);
            _bars = new PartPool(parent, mats.glow, PartPool.Kind.Cube, Bars, name + " visor bar", mirror);
        }

        public static Vector3 Ik(Vector3 a, Vector3 t, float l1, float l2, Vector3 pole, out Vector3 end)
        {
            Vector3 d = t - a;
            float dist = d.magnitude;
            float reach = l1 + l2 - 1e-3f;
            Vector3 dir = dist > 1e-5f ? d / dist : Vector3.down;
            if (dist > reach) { dist = reach; t = a + dir * reach; }
            dist = Mathf.Max(dist, Mathf.Abs(l1 - l2) + 1e-3f);
            float cosA = Mathf.Clamp((l1 * l1 + dist * dist - l2 * l2) / (2f * l1 * dist), -1f, 1f);
            float sinA = Mathf.Sqrt(1f - cosA * cosA);
            Vector3 po = pole - dir * Vector3.Dot(pole, dir);
            po = po.sqrMagnitude > 1e-8f ? po.normalized : Vector3.up;
            end = t;
            return a + dir * (l1 * cosA) + po * (l1 * sinA);
        }

        /// World-space reach of an arm (shoulder to wrist) and a leg (hip to ankle).
        public float ArmReach => (0.165f + 0.155f) * H;
        public float LegReach => 0.49f * H;

        /// Pose the android. `rx` (optional) lights the visor's spectrum bars.
        public void Apply(AndroidPose p, float hue, Rx rx)
        {
            Hue = hue;
            Quaternion body = _facing * Quaternion.AngleAxis(p.yaw, Vector3.up);
            Quaternion chestQ = body * Quaternion.Euler(p.chest.x, p.chest.y, p.chest.z);
            Quaternion headQ = chestQ * Quaternion.Euler(p.head.x, p.head.y, p.head.z);
            float h = H;
            Vector3 P = p.pelvis;
            Vector3 waist = P + chestQ * new Vector3(0f, 0.1f * h, 0f);
            Vector3 chest = P + chestQ * new Vector3(0f, 0.2f * h, 0f);
            Vector3 neck = P + chestQ * new Vector3(0f, 0.33f * h, 0f);
            Vector3 head = neck + headQ * new Vector3(0f, 0.085f * h, 0f);
            Vector3 shL = P + chestQ * new Vector3(-0.118f * h, 0.28f * h, 0f);
            Vector3 shR = P + chestQ * new Vector3(0.118f * h, 0.28f * h, 0f);
            Vector3 hipL = P + body * new Vector3(-0.055f * h, -0.02f * h, 0f);
            Vector3 hipR = P + body * new Vector3(0.055f * h, -0.02f * h, 0f);
            float a1 = 0.165f * h, a2 = 0.155f * h, l1 = 0.245f * h, l2 = 0.245f * h;
            Vector3 wrL, wrR, ankL, ankR;
            Vector3 elL = Ik(shL, p.handL, a1, a2, body * new Vector3(-0.6f, -1f, -0.4f), out wrL);
            Vector3 elR = Ik(shR, p.handR, a1, a2, body * new Vector3(0.6f, -1f, -0.4f), out wrR);
            Vector3 knL = Ik(hipL, p.footL, l1, l2, body * new Vector3(-0.1f, 0f, 1f), out ankL);
            Vector3 knR = Ik(hipR, p.footR, l1, l2, body * new Vector3(0.1f, 0f, 1f), out ankR);

            float rl = 0.034f * h, rs = 0.043f * h;
            _limb.Cyl(0, shL, elL, rl); _limb.Cyl(1, shR, elR, rl);
            _limb.Cyl(2, elL, wrL, rl * 0.88f); _limb.Cyl(3, elR, wrR, rl * 0.88f);
            _limb.Cyl(4, hipL, knL, rs); _limb.Cyl(5, hipR, knR, rs);
            _limb.Cyl(6, knL, ankL, rs * 0.85f); _limb.Cyl(7, knR, ankR, rs * 0.85f);
            _limb.Cyl(8, P, waist + chestQ * new Vector3(0f, 0.05f * h, 0f), 0.048f * h);

            _shell.Ellipsoid(0, chest, chestQ, new Vector3(0.235f * h, 0.2f * h, 0.13f * h));
            _shell.Ellipsoid(1, head, headQ, new Vector3(0.125f * h, 0.15f * h, 0.135f * h));
            _shell.Ellipsoid(2, shL + chestQ * new Vector3(-0.02f * h, 0.015f * h, 0f), chestQ, new Vector3(0.085f * h, 0.07f * h, 0.085f * h));
            _shell.Ellipsoid(3, shR + chestQ * new Vector3(0.02f * h, 0.015f * h, 0f), chestQ, new Vector3(0.085f * h, 0.07f * h, 0.085f * h));

            _joint.Ball(0, P, 0.052f * h);
            _joint.Ball(1, shL, 0.042f * h); _joint.Ball(2, shR, 0.042f * h);
            _joint.Ball(3, elL, 0.037f * h); _joint.Ball(4, elR, 0.037f * h);
            _joint.Ball(5, hipL, 0.05f * h); _joint.Ball(6, hipR, 0.05f * h);
            _joint.Ball(7, knL, 0.043f * h); _joint.Ball(8, knR, 0.043f * h);
            _joint.Ball(9, wrL, 0.03f * h); _joint.Ball(10, wrR, 0.03f * h);
            _joint.Ball(11, ankL, 0.034f * h); _joint.Ball(12, ankR, 0.034f * h);
            Vector3 dL = (wrL - elL).normalized, dR = (wrR - elR).normalized;
            _joint.Ball(13, wrL + dL * 0.035f * h, 0.036f * h); _joint.Ball(14, wrR + dR * 0.035f * h, 0.036f * h);
            _joint.Ball(15, ankL + body * new Vector3(0f, -0.015f * h, 0.045f * h), 0.034f * h);
            _joint.Ball(16, ankR + body * new Vector3(0f, -0.015f * h, 0.045f * h), 0.034f * h);

            _neck.Cyl(0, neck - chestQ * new Vector3(0f, 0.035f * h, 0f), neck + headQ * new Vector3(0f, 0.03f * h, 0f), 0.022f * h);
            if (_phones)
            {
                Vector3 cl = head + headQ * new Vector3(-0.072f * h, 0f, 0f), cr = head + headQ * new Vector3(0.072f * h, 0f, 0f);
                _acc.Cyl(0, cl - headQ * new Vector3(0.012f * h, 0f, 0f), cl + headQ * new Vector3(0.016f * h, 0f, 0f), 0.04f * h);
                _acc.Cyl(1, cr - headQ * new Vector3(0.016f * h, 0f, 0f), cr + headQ * new Vector3(0.012f * h, 0f, 0f), 0.04f * h);
                Vector3[] arc = { new Vector3(-0.076f, 0.0f, 0f), new Vector3(-0.066f, 0.07f, 0f), new Vector3(0f, 0.098f, 0f), new Vector3(0.066f, 0.07f, 0f), new Vector3(0.076f, 0.0f, 0f) };
                for (int i = 0; i < 4; i++)
                    _acc.Cyl(2 + i, head + headQ * (arc[i] * h), head + headQ * (arc[i + 1] * h), 0.0085f * h);
            }
            Vector3 visorC = head + headQ * new Vector3(0f, 0.014f * h, 0.062f * h);
            _visor.Box(0, visorC, headQ, new Vector3(0.1f * h, 0.04f * h, 0.012f * h));
            _visor.Glow(0, hue + 0.08f, 0.25f + 1.4f * p.eye);
            _core.Ball(0, chest + chestQ * new Vector3(0f, 0.025f * h, 0.072f * h), 0.032f * h);
            _core.Glow(0, hue, 0.3f + 2.2f * p.core);
            for (int i = 0; i < Bars; i++)
            {
                float lvl = rx != null ? rx.Spec(0.05f + 0.15f * i) : 0.3f;
                float bh = 0.005f * h + 0.03f * h * lvl;
                float x = (i - (Bars - 1) * 0.5f) * 0.0135f * h;
                _bars.Box(i, visorC + headQ * new Vector3(x, 0f, 0.008f * h), headQ, new Vector3(0.008f * h, bh, 0.006f * h));
                _bars.Glow(i, hue + 0.3f + 0.05f * i, 1.2f + 2.5f * lvl);
            }
        }
    }
}
