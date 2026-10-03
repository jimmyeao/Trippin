// Small helpers for reading Trippin's fast audio vocabulary safely.
// The owner judges a show on real music, where pres4 (slow presence) and the
// energy clocks move slowly, so a show that reads only those looks inert.
// Give each show at least one band level (lvl4) or kick (hits4) mapped onto
// SHAPE through Follow(): a fast attack and slower release, so the shape
// responds within ~70 ms but never steps or flickers. Never feed these into
// a motion integrator, and keep them off brightness (shape, not flashing).

using UnityEngine;

namespace TrippinStage
{
    public static class Eased
    {
        /// One-pole follower with separate attack and release rates (per second).
        public static float Follow(float cur, float target, float up, float down, float dt)
        {
            float k = 1f - Mathf.Exp(-(target > cur ? up : down) * dt);
            return cur + (target - cur) * k;
        }

        public static float Lvl(ShowState s, int band) =>
            s.lvl4 != null && s.lvl4.Length > band ? Mathf.Clamp01(s.lvl4[band]) : 0f;

        public static float Hit(ShowState s, int band) =>
            s.hits4 != null && s.hits4.Length > band ? Mathf.Clamp01(s.hits4[band]) : 0f;
    }
}
