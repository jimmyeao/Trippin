// Screen-content show: a towering android on a glossy floor in a void with
// a soft glow behind it, seen from low down. The body is a generated person
// (Alice + Hunyuan3D, tools/android_mesh.py) re-skinned as white ceramic and
// dark chrome armour with glowing seams, posed by an 11-bone skeleton, with a
// sculpted face (AndroidHead.shader) on the neck.
//  - Motion: fists pump on a smooth two-beat wave that grows with the
//    energy; the torso twists over the phrase; the knees dip on the kick
//    envelope; the head looks around and nods; in a breakdown the arms come
//    down to the sides and the body settles. Nothing reads raw audio.
//  - Shape: kicks push the armour panels out in a wave climbing from feet
//    to head; a drop bursts them open; the jaw opens with the vocal
//    presence; the eyes brighten with the bass and follow the camera.

using System.Collections.Generic;
using System.Globalization;
using System.Text.RegularExpressions;
using UnityEngine;

namespace TrippinStage
{
    public class ColossusShow : MonoBehaviour
    {
        public Camera cam;
        public Material androidMat, headMat, glowMat;

        const float Height = 24f;
        // Bone order: tools/android_mesh.py BONES.
        const int Pelvis = 0, Chest = 1, Neck = 2, UpperL = 3, ForeL = 4, UpperR = 5, ForeR = 6,
                  ThighL = 7, ShinL = 8, ThighR = 9, ShinR = 10, BoneCount = 11;

        Transform _body, _mirror, _glow;
        Material _mat, _mirMat, _headM, _headMirM, _glowM;
        Mesh _headMesh;
        RenderParams _headRp, _headMirRp;
        readonly Dictionary<string, Vector3> _j = new Dictionary<string, Vector3>();
        Vector3 _headR = new Vector3(0.056f, 0.072f, 0.064f);
        readonly Matrix4x4[] _w = new Matrix4x4[BoneCount];
        float _bass, _energy, _kick, _panel, _burst, _wave = -1f, _flow, _orbit, _calmLong;
        float _dip, _armsDown, _jaw, _vocal, _lookT;
        bool _drumsWas = true, _ready;

        void Awake()
        {
            var t = Resources.Load<TextAsset>("Android/android");
            var rig = Resources.Load<TextAsset>("Android/android_rig");
            var mesh = t != null ? LoadSkinned(t) : null;
            if (mesh == null || rig == null || !ParseRig(rig.text))
            {
                Debug.LogError("[Colossus] Resources/Android/android.bytes or android_rig.json missing (tools/android_mesh.py)");
                return;
            }
            mesh.bounds = new Bounds(new Vector3(0, 0.5f, 0), new Vector3(2f, 2f, 2f));
            _mat = new Material(androidMat);
            _mirMat = new Material(androidMat);
            _mirMat.SetFloat("_Mirror", 1f);
            _body = Part("android", mesh, _mat, new Vector3(Height, Height, Height));
            // Reflection: mirrored below the floor at y=0.
            _mirror = Part("reflection", mesh, _mirMat, new Vector3(Height, -Height, Height));
            _headMesh = SculptureShow.IcoSphere(5);
            _headM = new Material(headMat);
            _headMirM = new Material(headMat);
            _headMirM.SetFloat("_Mirror", 1f);
            var big = new Bounds(Vector3.zero, Vector3.one * 500f);
            _headRp = new RenderParams(_headM) { worldBounds = big, shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off };
            _headMirRp = new RenderParams(_headMirM) { worldBounds = big, shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off };
            var q = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(q.GetComponent<Collider>());
            q.name = "backglow";
            q.transform.SetParent(transform, false);
            _glowM = new Material(glowMat);
            q.GetComponent<Renderer>().sharedMaterial = _glowM;
            _glow = q.transform;
            _ready = true;
        }

        Transform Part(string name, Mesh mesh, Material m, Vector3 scale)
        {
            var g = new GameObject(name);
            g.transform.SetParent(transform, false);
            g.AddComponent<MeshFilter>().sharedMesh = mesh;
            var r = g.AddComponent<MeshRenderer>();
            r.sharedMaterial = m;
            r.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            g.transform.localScale = scale;
            return g.transform;
        }

        // tools/android_mesh.py "TCRS": like the crowd format plus four bone
        // indices and weights per vertex. glTF space: mirror x (and winding).
        static Mesh LoadSkinned(TextAsset t)
        {
            var b = t.bytes;
            if (b.Length < 12 || b[0] != 'T' || b[1] != 'C' || b[2] != 'R' || b[3] != 'S') return null;
            int nv = System.BitConverter.ToInt32(b, 4), ni = System.BitConverter.ToInt32(b, 8);
            if (b.Length < 12 + nv * 32 + ni * 2 || ni % 3 != 0) return null;
            var pos = new Vector3[nv];
            var nrm = new Vector3[nv];
            var bi = new List<Vector4>(nv);
            var bw = new List<Vector4>(nv);
            int o = 12;
            for (int i = 0; i < nv; i++, o += 32)
            {
                pos[i] = new Vector3(-System.BitConverter.ToSingle(b, o), System.BitConverter.ToSingle(b, o + 4), System.BitConverter.ToSingle(b, o + 8));
                nrm[i] = new Vector3(-System.BitConverter.ToSingle(b, o + 12), System.BitConverter.ToSingle(b, o + 16), System.BitConverter.ToSingle(b, o + 20));
                bi.Add(new Vector4(b[o + 24], b[o + 25], b[o + 26], b[o + 27]));
                bw.Add(new Vector4(b[o + 28], b[o + 29], b[o + 30], b[o + 31]) / 255f);
            }
            var idx = new int[ni];
            for (int i = 0; i < ni; i += 3, o += 6)
            {
                idx[i] = System.BitConverter.ToUInt16(b, o);
                idx[i + 1] = System.BitConverter.ToUInt16(b, o + 4);
                idx[i + 2] = System.BitConverter.ToUInt16(b, o + 2);
            }
            var m = new Mesh { name = "android", indexFormat = UnityEngine.Rendering.IndexFormat.UInt32 };
            m.SetVertices(pos);
            m.SetNormals(nrm);
            m.SetUVs(1, bi);
            m.SetUVs(2, bw);
            m.SetTriangles(idx, 0);
            return m;
        }

        // android_rig.json: "joints": { "name": [x, y, z], ... }, "head_radii": [..]
        bool ParseRig(string json)
        {
            var num = @"(-?[0-9.eE+-]+)";
            foreach (Match mm in Regex.Matches(json, "\"(\\w+)\"\\s*:\\s*\\[\\s*" + num + "\\s*,\\s*" + num + "\\s*,\\s*" + num + "\\s*\\]"))
            {
                var v = new Vector3(
                    -float.Parse(mm.Groups[2].Value, CultureInfo.InvariantCulture),
                    float.Parse(mm.Groups[3].Value, CultureInfo.InvariantCulture),
                    float.Parse(mm.Groups[4].Value, CultureInfo.InvariantCulture));
                if (mm.Groups[1].Value == "head_radii") _headR = new Vector3(-v.x, v.y, v.z);
                else _j[mm.Groups[1].Value] = v;
            }
            foreach (var k in new[] { "pelvis", "waist", "neck", "head", "shoulder_l", "elbow_l", "shoulder_r", "elbow_r", "hip_l", "knee_l", "hip_r", "knee_r" })
                if (!_j.ContainsKey(k)) return false;
            return true;
        }

        static Matrix4x4 About(Vector3 pivot, Quaternion r) =>
            Matrix4x4.Translate(pivot) * Matrix4x4.Rotate(r) * Matrix4x4.Translate(-pivot);

        void Pose(float phrase, float pumpWave, float pumpAmp, float twist, float yaw, float pitch)
        {
            // Rest-pose object space; _l joints sit at +x after the mirror.
            _w[Pelvis] = Matrix4x4.Translate(new Vector3(0, -0.014f * _dip, 0))
                       * About(_j["pelvis"], Quaternion.Euler(0, 0, 2.5f * Mathf.Sin(phrase * 0.5f) * (1f - 0.5f * _armsDown)));
            _w[Chest] = _w[Pelvis] * About(_j["waist"], Quaternion.Euler(4f * pumpWave * pumpAmp, twist * Mathf.Rad2Deg, 0));
            _w[Neck] = _w[Chest] * About(_j["neck"], Quaternion.Euler(pitch * Mathf.Rad2Deg, yaw * Mathf.Rad2Deg, 0));
            foreach (var (upper, fore, side) in new[] { (UpperL, ForeL, "l"), (UpperR, ForeR, "r") })
            {
                float sg = Mathf.Sign(_j["shoulder_" + side].x);
                // Lower outward in the frontal plane (clockwise seen from the
                // front for the +x arm): breakdown brings the arms down, the
                // pump swings them a little and flexes the elbows.
                float lower = 2.25f * _armsDown + 0.35f * pumpWave * pumpAmp;
                _w[upper] = _w[Chest] * About(_j["shoulder_" + side], Quaternion.AngleAxis(-sg * lower * Mathf.Rad2Deg, Vector3.forward));
                float flex = 0.9f * pumpWave * pumpAmp + 0.35f * _armsDown;
                _w[fore] = _w[upper] * About(_j["elbow_" + side], Quaternion.AngleAxis(sg * flex * Mathf.Rad2Deg, Vector3.forward));
            }
            foreach (var (thigh, shin, side) in new[] { (ThighL, ShinL, "l"), (ThighR, ShinR, "r") })
            {
                _w[thigh] = _w[Pelvis] * About(_j["hip_" + side], Quaternion.AngleAxis(-22f * _dip, Vector3.right));
                _w[shin] = _w[thigh] * About(_j["knee_" + side], Quaternion.AngleAxis(42f * _dip, Vector3.right));
            }
        }

        void Update()
        {
            if (!_ready) return;
            var s = TrippinLink.State;
            float dt = Mathf.Min(Time.deltaTime, 1f / 30f);
            float lb = s.lvl4 != null && s.lvl4.Length > 0 ? s.lvl4[0] : 0f;
            float pv = s.pres4 != null && s.pres4.Length > 2 ? s.pres4[2] : 0f;
            _bass += (lb - _bass) * (1f - Mathf.Exp(-dt / 0.15f));
            _vocal += (pv - _vocal) * (1f - Mathf.Exp(-dt / 0.12f));
            _energy += (s.energy - _energy) * (1f - Mathf.Exp(-dt / 0.8f));
            _armsDown += (Mathf.SmoothStep(0, 1, s.calm) - _armsDown) * (1f - Mathf.Exp(-dt / 1.6f));
            float kt = Mathf.Max(s.kick, s.hits4 != null && s.hits4.Length > 0 ? s.hits4[0] : 0f) * (1f - s.calm);
            if (kt > 0.45f && kt > _kick + 0.25f) _wave = -0.1f;
            _kick = kt;
            _panel = kt > _panel ? Mathf.Lerp(_panel, kt, 1f - Mathf.Exp(-dt / 0.04f)) : _panel * Mathf.Exp(-dt / 0.35f);
            // Knee dip: a softer envelope of the same kicks.
            _dip = kt > _dip ? Mathf.Lerp(_dip, kt, 1f - Mathf.Exp(-dt / 0.07f)) : _dip * Mathf.Exp(-dt / 0.28f);
            _wave += dt * 2.2f;
            if (!s.drums) _calmLong += dt;
            if (s.drums && !_drumsWas && _calmLong > 4f) _burst = 1f;
            if (s.drums) _calmLong = 0f;
            _drumsWas = s.drums;
            float barSec = 240f / Mathf.Max(s.bpm, 60f);
            _burst = Mathf.Max(0f, _burst - dt / (2f * barSec));
            _flow += dt * (0.3f + 1.4f * _energy);
            _jaw += (Mathf.Clamp01(_vocal * 1.8f - 0.25f) - _jaw) * (1f - Mathf.Exp(-dt / 0.08f));
            _lookT += dt * (0.15f + 0.25f * _energy);

            float phrase = s.beat / 64f * Mathf.PI * 2f;
            // A smooth two-beat pump on the flow clock (never the bar phase).
            float pumpWave = 0.5f - 0.5f * Mathf.Cos(s.flow * Mathf.PI);
            float pumpAmp = (0.25f + 0.75f * _energy) * (1f - _armsDown);
            float twist = 0.22f * Mathf.Sin(phrase * 0.5f) * (1f - 0.5f * _armsDown);
            float yaw = 0.45f * (Mathf.PerlinNoise(_lookT, 1.3f) - 0.5f) * 2f + 0.2f * Mathf.Sin(phrase);
            float pitch = 0.12f + 0.08f * pumpWave * pumpAmp + 0.12f * (Mathf.PerlinNoise(_lookT * 0.7f, 5.1f) - 0.5f);
            Pose(phrase, pumpWave, pumpAmp, twist, yaw, pitch);

            foreach (var m in new[] { _mat, _mirMat })
            {
                m.SetMatrixArray("_Bones", _w);
                m.SetFloat("_Panel", _panel * 0.8f + 3f * Mathf.SmoothStep(0, 1, _burst));
                m.SetFloat("_PanelWave", _wave);
                m.SetFloat("_Flow", _flow);
                m.SetFloat("_Calm", s.calm);
                m.SetFloat("_FloorY", 0f);
            }

            // The head rides the neck bone, scaled to the head ellipsoid.
            var local = _w[Neck] * Matrix4x4.TRS(_j["head"], Quaternion.identity, _headR * 1.06f);
            var headM = _body.localToWorldMatrix * local;
            // Eyes follow the camera (its direction in head space).
            var toCam = headM.inverse.MultiplyPoint3x4(cam.transform.position).normalized;
            var look = new Vector4(Mathf.Clamp(toCam.x * 2f, -1, 1), Mathf.Clamp(toCam.y * 2f, -1, 1), 0, 0);
            foreach (var m in new[] { _headM, _headMirM })
            {
                m.SetFloat("_Jaw", _jaw * (1f - 0.5f * s.calm));
                m.SetFloat("_Eyes", _bass);
                m.SetFloat("_Pulse", _panel);
                m.SetVector("_Look", look);
                m.SetFloat("_FloorY", 0f);
            }
            Graphics.RenderMesh(_headRp, _headMesh, 0, headM);
            Graphics.RenderMesh(_headMirRp, _headMesh, 0, _mirror.localToWorldMatrix * local);

            // Camera: low, looking up; drifting round the front.
            _orbit += dt * (0.04f + 0.08f * _energy) * Mathf.Sin(phrase * 0.5f + 0.7f);
            float a = Mathf.Clamp(_orbit, -0.8f, 0.8f);
            if (Mathf.Abs(_orbit) > 0.8f) _orbit = Mathf.Sign(_orbit) * 0.8f;
            float dist = 21f + 6f * s.calm;
            var want = new Vector3(Mathf.Sin(a) * dist, 4f + 2f * Mathf.Sin(phrase * 0.25f), Mathf.Cos(a) * dist);
            cam.transform.position = Vector3.Lerp(cam.transform.position, want, 1f - Mathf.Exp(-dt * 0.8f));
            cam.transform.LookAt(new Vector3(0, Height * 0.6f, 0));

            // Back glow: a big soft disc behind the android, facing the camera.
            var back = -new Vector3(cam.transform.position.x, 0, cam.transform.position.z).normalized;
            _glow.position = new Vector3(0, Height * 0.62f, 0) + back * 30f;
            _glow.rotation = Quaternion.LookRotation(back);
            _glow.localScale = Vector3.one * 70f;
            _glowM.SetFloat("_Glow", 0.12f + 0.12f * s.intensity + 0.3f * Mathf.SmoothStep(0, 1, _burst));
            _glowM.SetFloat("_Hue", 0.4f + 0.1f * Mathf.Sin(phrase * 0.25f));
        }
    }
}
