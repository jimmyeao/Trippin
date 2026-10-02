// Screen-content show: a towering android, fists raised, standing on a
// glossy floor in a void with a soft glow behind it, seen from low down.
// The body is a generated person (Alice + Hunyuan3D, tools/android_mesh.py)
// re-skinned as white ceramic and dark chrome armour with glowing seams.
//  - Shape: kicks push the armour panels out in a wave that climbs from
//    the feet to the head; a drop bursts them open, easing back over two
//    bars; the visor glows with the bass.
//  - Motion: the head turns on a phrase-length sine; light flows up the
//    seams on the energy clock; the camera drifts around the front,
//    swinging direction with the phrase. No beat flashes.

using UnityEngine;

namespace TrippinStage
{
    public class ColossusShow : MonoBehaviour
    {
        public Camera cam;
        public Material androidMat, glowMat;

        const float Height = 24f;

        Transform _body, _mirror, _glow;
        Material _mat, _mirMat, _glowM;
        Vector3 _head = new Vector3(0, 0.8f, 0);
        float _bass, _energy, _kick, _panel, _burst, _wave = -1f, _flow, _orbit, _calmLong;
        bool _drumsWas = true, _ready;

        void Awake()
        {
            var t = Resources.Load<TextAsset>("Android/android");
            var mesh = t != null ? StageDirector.LoadCrowdMesh(t) : null;
            if (mesh == null)
            {
                Debug.LogError("[Colossus] Resources/Android/android.bytes missing (tools/android_mesh.py)");
                return;
            }
            mesh.bounds = new Bounds(new Vector3(0, 0.5f, 0), new Vector3(1.4f, 1.4f, 1.4f));
            _head = FindHead(mesh.vertices);
            _mat = new Material(androidMat);
            _mirMat = new Material(androidMat);
            _mirMat.SetFloat("_Mirror", 1f);
            _body = Part("android", mesh, _mat, Vector3.zero, new Vector3(Height, Height, Height));
            // Reflection: mirrored below the floor at y=0.
            _mirror = Part("reflection", mesh, _mirMat, Vector3.zero, new Vector3(Height, -Height, Height));
            var q = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(q.GetComponent<Collider>());
            q.name = "backglow";
            q.transform.SetParent(transform, false);
            _glowM = new Material(glowMat);
            q.GetComponent<Renderer>().sharedMaterial = _glowM;
            _glow = q.transform;
            _ready = true;
        }

        Transform Part(string name, Mesh mesh, Material m, Vector3 pos, Vector3 scale)
        {
            var g = new GameObject(name);
            g.transform.SetParent(transform, false);
            g.AddComponent<MeshFilter>().sharedMesh = mesh;
            var r = g.AddComponent<MeshRenderer>();
            r.sharedMaterial = m;
            r.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            g.transform.localPosition = pos;
            g.transform.localScale = scale;
            return g.transform;
        }

        // The helmet centre, the same way android_mesh.py finds it: walk up
        // the centre line from the shoulders until the head ends. (Mesh x
        // is mirrored for Unity, which doesn't change the centre line.)
        static Vector3 FindHead(Vector3[] v)
        {
            float top = 0.75f;
            bool Any(float y0)
            {
                foreach (var p in v)
                    if (Mathf.Abs(p.x) < 0.035f && p.y > y0 && p.y < y0 + 0.012f) return true;
                return false;
            }
            while (top < 1f && Any(top)) top += 0.004f;
            float z = 0f; int n = 0;
            foreach (var p in v)
                if (Mathf.Abs(p.x) < 0.035f && p.y > top - 0.02f && p.y < top + 0.012f) { z += p.z; n++; }
            return new Vector3(0, top - 0.068f, n > 0 ? z / n - 0.01f : 0f);
        }

        void Update()
        {
            if (!_ready) return;
            var s = TrippinLink.State;
            float dt = Mathf.Min(Time.deltaTime, 1f / 30f);
            float lb = s.lvl4 != null && s.lvl4.Length > 0 ? s.lvl4[0] : 0f;
            _bass += (lb - _bass) * (1f - Mathf.Exp(-dt / 0.15f));
            _energy += (s.energy - _energy) * (1f - Mathf.Exp(-dt / 0.8f));
            float kt = Mathf.Max(s.kick, s.hits4 != null && s.hits4.Length > 0 ? s.hits4[0] : 0f) * (1f - s.calm);
            // Kick: the panels pop (fast attack, slow settle) and a wave climbs the body.
            if (kt > 0.45f && kt > _kick + 0.25f) _wave = -0.1f;
            _kick = kt;
            _panel = kt > _panel ? Mathf.Lerp(_panel, kt, 1f - Mathf.Exp(-dt / 0.04f)) : _panel * Mathf.Exp(-dt / 0.35f);
            _wave += dt * 2.2f;
            if (!s.drums) _calmLong += dt;
            if (s.drums && !_drumsWas && _calmLong > 4f) _burst = 1f;
            if (s.drums) _calmLong = 0f;
            _drumsWas = s.drums;
            float barSec = 240f / Mathf.Max(s.bpm, 60f);
            _burst = Mathf.Max(0f, _burst - dt / (2f * barSec));
            _flow += dt * (0.3f + 1.4f * _energy);

            float phrase = s.beat / 64f * Mathf.PI * 2f;
            float yaw = 0.45f * Mathf.Sin(phrase * 0.5f) * (1f - 0.4f * s.calm);
            foreach (var m in new[] { _mat, _mirMat })
            {
                m.SetVector("_Head", new Vector4(_head.x, _head.y, _head.z, yaw));
                m.SetFloat("_Panel", _panel * 0.8f + 3f * Mathf.SmoothStep(0, 1, _burst));
                m.SetFloat("_PanelWave", _wave);
                m.SetFloat("_Flow", _flow);
                m.SetFloat("_Visor", _bass);
                m.SetFloat("_Calm", s.calm);
                m.SetFloat("_FloorY", 0f);
            }

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
