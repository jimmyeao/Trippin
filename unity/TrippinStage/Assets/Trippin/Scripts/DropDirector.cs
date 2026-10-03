// Shared build -> drop state for every show. Ticked once a frame by
// ShowManager (always active), so it stays right across show switches.
//  - Tension: a slow integrator, 0..1. It climbs while the drums are out
//    (a breakdown) and while Trippin's `build` trend is positive, and drains
//    when the drums are back and the energy stops rising. It is an
//    integrator on purpose: shows can feed it straight into motion and
//    spacing without jerking (never feed them raw audio).
//  - Impact: 1 at the moment the drums come back after a real breakdown,
//    then decays over ~6 beats. Shows read it for one-shot bursts.
//  - Dropped: true for the single frame the drop lands.
// `build` is only a fast-vs-slow energy trend, not a riser detector, so the
// drop itself is still "drums return after >4 s out" (as StageDirector had
// it); Tension supplies the anticipation before it.
// Flight/tunnel shows must use these for shape and motion only: no strobes.

using UnityEngine;

namespace TrippinStage
{
    public static class DropDirector
    {
        public static float Tension { get; private set; }
        public static float Impact { get; private set; }
        public static bool Dropped { get; private set; }

        static float _calmLong;
        static bool _drumsWas = true;
        static int _frame = -1;

        public static void Tick(ShowState s, float dt)
        {
            if (_frame == Time.frameCount) return;
            _frame = Time.frameCount;
            Dropped = false;
            dt = Mathf.Min(dt, 0.1f);

            float build = Mathf.Max(0f, s.build);
            float rate;
            if (!s.drums) rate = 0.10f + 0.5f * build;          // ~6 s to full in a plain breakdown
            else rate = build > 0.2f ? 0.25f * build : -0.35f;   // rising energy keeps it up, else it drains
            Tension = Mathf.Clamp01(Tension + rate * dt);

            if (!s.drums) _calmLong += dt;
            if (s.drums && !_drumsWas && _calmLong > 4f)
            {
                Dropped = true;
                Impact = 1f;
            }
            if (s.drums) _calmLong = 0f;
            _drumsWas = s.drums;

            if (!Dropped && Impact > 0f) Tension = Mathf.Max(0f, Tension - 1.5f * dt);

            float beatsPerSec = Mathf.Max(0.5f, s.bpm / 60f);
            Impact = Mathf.Max(0f, Impact - dt * beatsPerSec / 6f);
        }
    }
}
