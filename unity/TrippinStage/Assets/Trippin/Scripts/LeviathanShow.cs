// Screen-content show: a leviathan made of light, a manta-like creature
// with wide undulating wings, a long whip tail and ~2k trailing strands,
// swimming through a dark void of drifting motes. Its own thing on the
// screen: no stage, no crowd.
//  - Shape: each kick sends a swell-and-glow pulse from head to tail (a
//    delayed envelope per spine segment); the body breathes with the bass;
//    a drop flares the strands outward.
//  - Motion: swim and wing-beat speed integrate the smoothed energy; the
//    path curves and swings direction on phrase-length sines; strands
//    trail by simulation, swirled by the smoothed mids.
//  - No beat flashes (it's a flight scene): glow rides the pulse envelope.
// The camera glides alongside, its orbit swinging with the phrase.

using UnityEngine;

namespace TrippinStage
{
    public class LeviathanShow : MonoBehaviour
    {
        public Camera cam;
        public ComputeShader sim;
        public Material filamentMat, pointsMat;

        const int Spine = 96, RibPts = 33, Stringers = 16, StrandPts = 24, Strands = 1024;
        const float SegLen = 0.36f;
        const int Motes = 6000;
        const float MoteBox = 90f;

        ComputeBuffer _spine, _right, _up, _body, _strand, _strandPrev, _motes;
        readonly Vector4[] _spineD = new Vector4[Spine], _rightD = new Vector4[Spine], _upD = new Vector4[Spine];
        readonly Vector3[] _pts = new Vector3[Spine];
        Vector4[] _moteD;
        Material _ribM, _strM, _strandM, _moteM;
        RenderParams _ribRp, _strRp, _strandRp, _moteRp;
        int _kBody, _kStrands;

        Vector3 _head = Vector3.zero, _fwd = Vector3.forward;
        float _yaw, _pitch, _travel, _swim, _wing, _orbit;
        float _bass, _mid, _energy, _kick, _flare, _calmLong;
        bool _drumsWas = true;
        readonly float[] _kickHist = new float[128];
        int _kh;
        Vector3 _camLook;
        bool _primed, _camSet;

        void Awake()
        {
            _spine = new ComputeBuffer(Spine, 16);
            _right = new ComputeBuffer(Spine, 16);
            _up = new ComputeBuffer(Spine, 16);
            _body = new ComputeBuffer(Spine * RibPts + Stringers * Spine, 16);
            _strand = new ComputeBuffer(Strands * StrandPts, 16);
            _strandPrev = new ComputeBuffer(Strands * StrandPts, 16);
            _strand.SetData(new Vector4[Strands * StrandPts]);
            _strandPrev.SetData(new Vector4[Strands * StrandPts]);
            _kBody = sim.FindKernel("Body");
            _kStrands = sim.FindKernel("Strands");

            _ribM = Line(0, RibPts, 0.05f, 0.55f, 0.004f, 0.1f, 0f);
            _strM = Line(Spine * RibPts, Spine, 0.045f, 0.6f, 0.03f, 0.3f, 0.6f);
            _strandM = new Material(filamentMat);
            _strandM.SetBuffer("_P", _strand);
            _strandM.SetInt("_Base", 0);
            _strandM.SetInt("_LineLen", StrandPts);
            _strandM.SetFloat("_Width", 0.03f);
            _strandM.SetFloat("_Hue", 0.15f);
            _strandM.SetFloat("_HueLine", 0.00073f);
            _strandM.SetFloat("_HueAlong", 0.25f);
            _strandM.SetFloat("_Taper", 0.85f);
            var big = new Bounds(Vector3.zero, Vector3.one * 2000f);
            _ribRp = new RenderParams(_ribM) { worldBounds = big };
            _strRp = new RenderParams(_strM) { worldBounds = big };
            _strandRp = new RenderParams(_strandM) { worldBounds = big };

            // Motes: plankton drifting in the void, wrapped around the camera.
            _motes = new ComputeBuffer(Motes, 16);
            _moteD = new Vector4[Motes];
            var rnd = new System.Random(11);
            for (int i = 0; i < Motes; i++)
                _moteD[i] = new Vector4(((float)rnd.NextDouble() - 0.5f) * MoteBox, ((float)rnd.NextDouble() - 0.5f) * MoteBox, ((float)rnd.NextDouble() - 0.5f) * MoteBox, 0);
            _moteM = new Material(pointsMat);
            _moteM.SetBuffer("_Pos", _motes);
            _moteM.SetFloat("_Size", 0.06f);
            _moteRp = new RenderParams(_moteM) { worldBounds = big };
        }

        Material Line(int baseIdx, int len, float width, float gain, float hueLine, float hueAlong, float taper)
        {
            var m = new Material(filamentMat);
            m.SetBuffer("_P", _body);
            m.SetInt("_Base", baseIdx);
            m.SetInt("_LineLen", len);
            m.SetFloat("_Width", width);
            m.SetFloat("_Gain", gain);
            m.SetFloat("_Hue", 0.55f);
            m.SetFloat("_HueLine", hueLine);
            m.SetFloat("_HueAlong", hueAlong);
            m.SetFloat("_Taper", taper);
            return m;
        }

        void OnDestroy()
        {
            _spine?.Release(); _right?.Release(); _up?.Release(); _body?.Release();
            _strand?.Release(); _strandPrev?.Release(); _motes?.Release();
        }

        void Update()
        {
            var s = TrippinLink.State;
            float dt = Mathf.Min(Time.deltaTime, 1f / 30f);
            float bps = Mathf.Max(s.bpm, 60f) / 60f;

            // --- smoothed audio (envelopes, never raw values into motion)
            float lb = s.lvl4 != null && s.lvl4.Length > 3 ? s.lvl4[0] : 0f;
            float lm = s.lvl4 != null && s.lvl4.Length > 3 ? 0.5f * (s.lvl4[1] + s.lvl4[2]) : 0f;
            _bass += (lb - _bass) * (1f - Mathf.Exp(-dt / 0.2f));
            _mid += (lm - _mid) * (1f - Mathf.Exp(-dt / 0.35f));
            _energy += (s.energy - _energy) * (1f - Mathf.Exp(-dt / 0.8f));
            float kt = Mathf.Max(s.kick, s.hits4 != null && s.hits4.Length > 0 ? s.hits4[0] : 0f) * (1f - s.calm);
            _kick = kt > _kick ? Mathf.Lerp(_kick, kt, 1f - Mathf.Exp(-dt / 0.05f)) : _kick * Mathf.Exp(-dt / 0.3f);
            _kh = (_kh + 1) % _kickHist.Length;
            _kickHist[_kh] = _kick;
            if (!s.drums) _calmLong += dt;
            if (s.drums && !_drumsWas && _calmLong > 4f) _flare = 1f;
            if (s.drums) _calmLong = 0f;
            _drumsWas = s.drums;
            _flare = Mathf.Max(0f, _flare - dt * 0.7f);

            // --- steering: phrase-length swings plus a pull back to the
            // middle of the void, so it wanders but stays on screen.
            float phrase = s.beat / 64f * Mathf.PI * 2f;
            float speed = (2.2f + 7f * _energy) * (1f - 0.45f * s.calm) + 6f * _flare;
            _travel += speed * dt;
            float yawRate = 0.32f * Mathf.Sin(phrase * 0.5f + 0.3f) + 0.18f * (Mathf.PerlinNoise(_travel * 0.02f, 3.1f) - 0.5f) * 2f;
            var toCentre = -_head;
            float away = toCentre.magnitude;
            if (away > 1f)
            {
                var flat = new Vector3(toCentre.x, 0, toCentre.z).normalized;
                float home = Mathf.Atan2(flat.x, flat.z) * Mathf.Rad2Deg;
                yawRate += Mathf.DeltaAngle(_yaw, home) * Mathf.Deg2Rad * Mathf.Clamp01((away - 25f) / 30f) * 0.6f;
            }
            _yaw += yawRate * Mathf.Rad2Deg * dt;
            float pitchWant = 14f * Mathf.Sin(phrase * 0.25f + 1.1f) - Mathf.Clamp(_head.y * 1.5f, -25f, 25f);
            _pitch = Mathf.Lerp(_pitch, pitchWant, 1f - Mathf.Exp(-dt * 0.6f));
            _fwd = Quaternion.Euler(-_pitch, _yaw, 0) * Vector3.forward;
            _head += _fwd * speed * dt;

            // --- spine: follow the leader, plus a side-to-side swim wave.
            if (!_primed)
            {
                for (int i = 0; i < Spine; i++) _pts[i] = _head - _fwd * SegLen * i;
                _camLook = _head;
                cam.transform.position = _head + new Vector3(-30, 6, -20);
                _primed = true;
            }
            _pts[0] = _head;
            for (int i = 1; i < Spine; i++)
            {
                var d = _pts[i] - _pts[i - 1];
                float l = d.magnitude;
                _pts[i] = _pts[i - 1] + (l > 1e-5f ? d / l : -_fwd) * SegLen;
            }
            _swim += dt * bps * (0.35f + 0.6f * _energy);
            _wing += dt * bps * Mathf.PI * (0.25f + 0.35f * _energy) * (1f - 0.4f * s.calm);
            var upRef = Vector3.up;
            for (int i = 0; i < Spine; i++)
            {
                var t = i == 0 ? (_pts[0] - _pts[1]) : (_pts[i - 1] - _pts[i]);
                t = t.sqrMagnitude > 1e-8f ? t.normalized : _fwd;
                var right = Vector3.Cross(upRef, t);
                right = right.sqrMagnitude > 1e-6f ? right.normalized : Vector3.right;
                var up = Vector3.Cross(t, right);
                float s01 = i / (Spine - 1f);
                // Tail whips wider than the head.
                float sway = Mathf.Sin(_swim * Mathf.PI * 2f - s01 * 7f) * (0.15f + 2.4f * s01 * s01);
                // The kick wave travels head -> tail (about a beat end to end).
                int delay = (int)(s01 * 45f);
                float pulse = _kickHist[(_kh - delay + _kickHist.Length) % _kickHist.Length];
                var p = _pts[i] + right * sway;
                _spineD[i] = new Vector4(p.x, p.y, p.z, pulse);
                _rightD[i] = right;
                _upD[i] = up;
            }
            _spine.SetData(_spineD);
            _right.SetData(_rightD);
            _up.SetData(_upD);

            // --- GPU: body lattice, then the strands.
            foreach (var k in new[] { _kBody, _kStrands })
            {
                sim.SetBuffer(k, "_Spine", _spine);
                sim.SetBuffer(k, "_Right", _right);
                sim.SetBuffer(k, "_Up", _up);
            }
            sim.SetBuffer(_kBody, "_Body", _body);
            sim.SetBuffer(_kStrands, "_Strand", _strand);
            sim.SetBuffer(_kStrands, "_StrandPrev", _strandPrev);
            sim.SetInt("_Strands", Strands);
            sim.SetFloat("_Dt", dt);
            sim.SetFloat("_T", s.flow * 0.12f);
            sim.SetFloat("_WingPhase", _wing);
            sim.SetFloat("_WingAmp", (0.5f + 0.7f * _energy) * (1f - 0.35f * s.calm));
            sim.SetFloat("_Breath", 1f + 0.12f * _bass * (1f - 0.5f * s.calm));
            sim.SetFloat("_Turb", 0.3f + 1.2f * _mid);
            sim.SetVector("_Fwd", _fwd);
            sim.SetFloat("_Stream", 10f + 3f * speed);
            sim.SetFloat("_Flare", _flare);
            sim.SetFloat("_Len", 16f + 7f * _mid);
            sim.Dispatch(_kBody, (Spine * RibPts + Stringers * Spine + 63) / 64, 1, 1);
            sim.Dispatch(_kStrands, (Strands + 63) / 64, 1, 1);

            float gain = 0.5f + 0.3f * s.intensity;
            _ribM.SetFloat("_Gain", gain * 0.8f);
            _strM.SetFloat("_Gain", gain);
            _strandM.SetFloat("_Gain", gain * (0.5f + 0.8f * _flare));
            Graphics.RenderPrimitives(_ribRp, MeshTopology.Triangles, Spine * (RibPts - 1) * 6);
            Graphics.RenderPrimitives(_strRp, MeshTopology.Triangles, Stringers * (Spine - 1) * 6);
            Graphics.RenderPrimitives(_strandRp, MeshTopology.Triangles, Strands * (StrandPts - 1) * 6);

            // --- camera: glide alongside, orbit swinging with the phrase.
            var focus = _pts[Spine / 4];
            _orbit += dt * (0.05f + 0.1f * _energy) * Mathf.Sin(phrase * 0.5f);
            float dist = 38f + 8f * s.calm;
            var side = Quaternion.Euler(0, _yaw + 70f + Mathf.Rad2Deg * _orbit, 0) * Vector3.forward;
            var want = focus + side * dist + Vector3.up * (16f + 7f * Mathf.Sin(phrase * 0.5f + 0.8f));
            // First frame: start in place rather than gliding in from afar.
            cam.transform.position = _camSet ? Vector3.Lerp(cam.transform.position, want, 1f - Mathf.Exp(-dt * 0.8f)) : want;
            _camSet = true;
            _camLook = Vector3.Lerp(_camLook, focus, 1f - Mathf.Exp(-dt * 1.5f));
            cam.transform.LookAt(_camLook);

            // --- motes, wrapped into a box around the camera.
            var cp = cam.transform.position;
            float drift = dt * (0.3f + 0.5f * _energy);
            for (int i = 0; i < Motes; i++)
            {
                var m = _moteD[i];
                float x = m.x + drift * Mathf.Sin(i * 0.37f), y = m.y + drift * 0.3f, z = m.z + drift * Mathf.Cos(i * 0.61f);
                x = cp.x + Wrap(x - cp.x); y = cp.y + Wrap(y - cp.y); z = cp.z + Wrap(z - cp.z);
                _moteD[i] = new Vector4(x, y, z, 0);
            }
            _motes.SetData(_moteD);
            _moteM.SetFloat("_Gain", 0.18f + 0.12f * s.intensity);
            Graphics.RenderPrimitives(_moteRp, MeshTopology.Triangles, Motes * 6);
        }

        static float Wrap(float d) => d - MoteBox * Mathf.Floor(d / MoteBox + 0.5f);
    }
}
