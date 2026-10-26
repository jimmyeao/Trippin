// The HQ kit: what the high-quality shows (the Unity versions of Trippin's
// 2D scenes) add to Kit.cs.
//  - ParticleField: GPU sparks, a ring of a few hundred thousand, emitted
//    in bursts and integrated in HqParticles.compute, drawn as streaks
//    (HqSpark.shader), with an optional reflection in water.
//  - HqPost: depth of field for the show on air. Off unless a show asks for
//    it in this frame, so every other show renders exactly as before.
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace TrippinStage
{
    public sealed class ParticleField
    {
        public readonly int Count;
        readonly ComputeShader _cs;
        readonly ComputeBuffer _buf;
        readonly int _kEmit, _kStep;
        readonly Material _mat, _mirMat;
        RenderParams _rp, _mirRp;
        int _head, _n, _total;
        readonly Vector4[] _b0 = new Vector4[64], _b1 = new Vector4[64], _b2 = new Vector4[64], _b3 = new Vector4[64], _b4 = new Vector4[64], _b5 = new Vector4[64];
        public Vector3 Gravity = new Vector3(0f, -9.8f, 0f);
        public Material Mat => _mat;
        public Material MirrorMat => _mirMat;

        /// Box: scattered through a volume whose half-extents are `axis`.
        public enum Shape { Sphere = 0, Ring = 1, Cone = 2, Shell = 3, Box = 4 }

        public ParticleField(ComputeShader cs, Material spark, int count, bool mirror, float waterY = 0f)
        {
            Count = count;
            _cs = cs;
            _buf = new ComputeBuffer(count, 16 * 4);
            _buf.SetData(new float[count * 16]);    // all dead
            _kEmit = cs.FindKernel("Emit");
            _kStep = cs.FindKernel("Step");
            _mat = new Material(spark);
            _mat.SetBuffer("_P", _buf);
            _rp = new RenderParams(_mat) { worldBounds = new Bounds(Vector3.zero, Vector3.one * 4000f) };
            if (mirror)
            {
                _mirMat = new Material(spark);
                _mirMat.SetBuffer("_P", _buf);
                _mirMat.SetFloat("_Mirror", 1f);
                _mirMat.SetFloat("_WaterY", waterY);
                _mirRp = new RenderParams(_mirMat) { worldBounds = _rp.worldBounds };
            }
        }

        public void Release() { _buf?.Release(); }

        /// Queue a burst for this frame (at most 64 a frame; extras drop).
        public void Burst(Vector3 centre, int count, Shape shape, float speed, float size, float life, Color col,
                          float drag = 1f, float grav = 1f, Vector3 inherit = default, Vector3 axis = default, float spread = 0.15f,
                          int ghosts = 0, float trail = 0.25f, float cone = 0f)
        {
            if (_n >= 64 || count <= 0) return;
            ghosts = Mathf.Clamp(ghosts, 0, 31);
            count = Mathf.Min(count * (ghosts + 1), Count / 4);
            _b0[_n] = new Vector4(centre.x, centre.y, centre.z, _total);
            _b1[_n] = new Vector4(speed, (float)shape, size, life);
            _b2[_n] = new Vector4(col.r, col.g, col.b, drag);
            _b3[_n] = new Vector4(inherit.x, inherit.y, inherit.z, grav);
            var ax = axis == default ? Vector3.up : axis;
            _b4[_n] = new Vector4(ax.x, ax.y, ax.z, spread);
            _b5[_n] = new Vector4(ghosts, trail, cone, 0f);
            _total += count;
            _n++;
        }

        /// Emit the frame's bursts, integrate, and draw.
        public void Tick(float dt, float gain, float streak = 0.045f, float crackle = 0f)
        {
            if (_n > 0)
            {
                _cs.SetVectorArray("_B0", _b0); _cs.SetVectorArray("_B1", _b1); _cs.SetVectorArray("_B2", _b2);
                _cs.SetVectorArray("_B3", _b3); _cs.SetVectorArray("_B4", _b4); _cs.SetVectorArray("_B5", _b5);
                _cs.SetInt("_BCount", _n);
                _cs.SetInt("_Total", _total);
                _cs.SetInt("_Head", _head);
                _cs.SetInt("_Count", Count);
                _cs.SetFloat("_Seed", Random.value * 997f);
                _cs.SetBuffer(_kEmit, "_P", _buf);
                _cs.Dispatch(_kEmit, (_total + 63) / 64, 1, 1);
                _head = (_head + _total) % Count;
                _n = 0; _total = 0;
            }
            _cs.SetInt("_Count", Count);
            _cs.SetFloat("_Dt", dt);
            _cs.SetVector("_Gravity", Gravity);
            _cs.SetBuffer(_kStep, "_P", _buf);
            _cs.Dispatch(_kStep, (Count + 255) / 256, 1, 1);
            foreach (var m in _mirMat != null ? new[] { _mat, _mirMat } : new[] { _mat })
            {
                m.SetFloat("_Gain", gain);
                m.SetFloat("_Streak", streak);
                m.SetFloat("_Crackle", crackle);
            }
            Graphics.RenderPrimitives(_rp, MeshTopology.Triangles, Count * 6);
            if (_mirMat != null) Graphics.RenderPrimitives(_mirRp, MeshTopology.Triangles, Count * 6);
        }
    }

    public static class HqBackdrops
    {
        /// A painted backdrop on the horizon (HqBackdrop.shader): a quad
        /// `width` x `height` standing on `foot` (its bottom centre), facing
        /// -z, and, with `mirrorMat`, its reflection below the waterline.
        public static void Make(Transform parent, Material mat, Material mirrorMat, Vector3 foot, float width, float height)
        {
            Part(parent, "backdrop", mat, foot, width, height, false);
            if (mirrorMat != null) Part(parent, "backdrop reflection", mirrorMat, foot, width, height, true);
        }

        static void Part(Transform parent, string name, Material mat, Vector3 foot, float w, float h, bool mirror)
        {
            float y1 = mirror ? -h : h;
            var mesh = new Mesh { name = name };
            mesh.vertices = new[] { new Vector3(-w / 2, 0, 0), new Vector3(w / 2, 0, 0), new Vector3(-w / 2, y1, 0), new Vector3(w / 2, y1, 0) };
            mesh.uv = new[] { new Vector2(0, 0), new Vector2(1, 0), new Vector2(0, 1), new Vector2(1, 1) };
            mesh.triangles = new[] { 0, 2, 1, 1, 2, 3 };
            mesh.bounds = new Bounds(Vector3.zero, Vector3.one * (w + h) * 2f);
            var g = new GameObject(name);
            g.transform.SetParent(parent, false);
            g.transform.localPosition = foot;
            g.AddComponent<MeshFilter>().sharedMesh = mesh;
            var r = g.AddComponent<MeshRenderer>();
            r.sharedMaterial = mat;
            r.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
        }
    }

    /// Depth of field for the show on air (the Post volume's DepthOfField,
    /// added off by StageBuilder). A show calls Focus each frame it wants it;
    /// HqPostDriver turns it off again in any frame nobody asked.
    public static class HqPost
    {
        static DepthOfField _dof;
        static int _setFrame = -1;

        static DepthOfField Dof()
        {
            if (_dof != null) return _dof;
            foreach (var v in Object.FindObjectsByType<Volume>(FindObjectsSortMode.None))
                if (v.isGlobal && v.sharedProfile != null && v.sharedProfile.TryGet(out _dof)) break;
            return _dof;
        }

        /// Bokeh depth of field this frame: sharp at `distance` metres, the
        /// blur growing as the f-number drops (`aperture`, 1-32). URP's
        /// bokeh discs are small at normal focal lengths: use a long lens
        /// (135-220 mm) wide open for a visible effect.
        public static void Focus(float distance, float aperture, float focalLength = 50f)
        {
            var d = Dof();
            if (d == null) return;
            d.active = true;
            d.mode.Override(DepthOfFieldMode.Bokeh);
            d.focusDistance.Override(Mathf.Max(0.1f, distance));
            d.aperture.Override(Mathf.Clamp(aperture, 1f, 32f));
            d.focalLength.Override(Mathf.Clamp(focalLength, 1f, 300f));
            _setFrame = Time.frameCount;
        }

        internal static void EndFrame()
        {
            if (_setFrame == Time.frameCount) return;
            var d = Dof();
            if (d != null && d.mode.value != DepthOfFieldMode.Off) d.mode.Override(DepthOfFieldMode.Off);
        }
    }

    /// Runs after the shows' Update and before the camera renders (the show
    /// manager renders in LateUpdate): depth of field goes off in any frame
    /// no show asked for it.
    [DefaultExecutionOrder(-1000)]
    public sealed class HqPostDriver : MonoBehaviour
    {
        void LateUpdate() => HqPost.EndFrame();
    }
}
