//! Accent layer: Auto sometimes lays a second, light scene over the main one
//! (after the blendable layers of Imaginando's VS). The renderer gives the
//! accent its own feedback pair and adds its lights into the scene target
//! under the dancer (`Renderer::set_accent`, shaders/accent.wgsl).
//!
//! Only cheap scenes of lights on black qualify (`POOL`, each <= 0.22 ms at
//! 1080p on the dev 5070 Ti): an accent over an environment, or one that
//! fills the frame (confetti, rings), reads as a muddle, and the pricier
//! ones (lissajous, laser_fan at ~0.6 ms) would push a busy base scene past
//! the M2 budget. Never over `@heavy`, Unity, video or flight scenes.

/// Scenes that make good accents: sparse lights and floating objects on
/// black, all cheap. Ids; missing ones are skipped.
pub const POOL: &[&str] = &["comets", "helix", "orbiters"];

/// Share of cuts (while the track drives) that get an accent.
pub const CHANCE: f32 = 0.4;

#[derive(Default)]
pub struct Accent {
    /// The accent the auto-pilot wants now.
    want: Option<usize>,
    /// The accent on screen (kept while it fades out).
    show: Option<usize>,
    amt: f32,
}

impl Accent {
    /// A scene cut. `pool` holds the accent candidates that are usable
    /// (ticked, compiled); `base_ok` is false over scenes that take no
    /// accent; `driving` is false in breakdowns. `roll` and `pick` are
    /// random 0..1.
    pub fn on_cut(&mut self, base: usize, pool: &[usize], base_ok: bool, driving: bool, roll: f32, pick: f32) {
        let choices: Vec<usize> = pool.iter().copied().filter(|&i| i != base).collect();
        self.want = if base_ok && driving && roll < CHANCE && !choices.is_empty() {
            let i = ((pick * choices.len() as f32) as usize).min(choices.len() - 1);
            Some(choices[i])
        } else {
            None
        };
        // The base scene changes hard on a cut, so the accent may too.
        if self.want.is_some() {
            self.show = self.want;
        }
    }

    /// Fade the accent out (a breakdown, Manual mode, a timeline show, the
    /// setting off).
    pub fn clear(&mut self) {
        self.want = None;
    }

    /// Ease the amount toward `amount` (or out) and return what to draw.
    pub fn tick(&mut self, dt: f32, amount: f32) -> Option<(usize, f32)> {
        let target = if self.want.is_some() { amount } else { 0.0 };
        let rate = if target > self.amt { 1.5 } else { 2.5 };
        self.amt += (target - self.amt) * (dt * rate).min(1.0);
        if self.want.is_none() && self.amt < 0.004 {
            self.amt = 0.0;
            self.show = None;
        }
        self.show.map(|i| (i, self.amt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_only_when_allowed_and_never_the_base() {
        let mut a = Accent::default();
        a.on_cut(3, &[3, 7], true, true, 0.1, 0.0);
        assert_eq!(a.tick(0.0, 0.6).map(|x| x.0), Some(7), "never the base scene");
        let mut b = Accent::default();
        b.on_cut(1, &[3, 7], false, true, 0.1, 0.5);
        assert_eq!(b.tick(0.0, 0.6), None, "not over a heavy/flight base");
        b.on_cut(1, &[3, 7], true, false, 0.1, 0.5);
        assert_eq!(b.tick(0.0, 0.6), None, "not in a breakdown");
        b.on_cut(1, &[3, 7], true, true, 0.9, 0.5);
        assert_eq!(b.tick(0.0, 0.6), None, "only CHANCE of cuts");
    }

    #[test]
    fn fades_in_and_out_without_jumps() {
        let mut a = Accent::default();
        a.on_cut(0, &[5], true, true, 0.0, 0.0);
        let mut last = 0.0;
        for _ in 0..240 {
            let (_, amt) = a.tick(1.0 / 60.0, 0.6).unwrap();
            assert!((amt - last).abs() < 0.03);
            last = amt;
        }
        assert!((last - 0.6).abs() < 0.01);
        a.clear();
        let mut frames = 0;
        while let Some((i, amt)) = a.tick(1.0 / 60.0, 0.6) {
            assert_eq!(i, 5, "keeps drawing the old accent while it fades");
            assert!(amt <= last + 1e-6);
            last = amt;
            frames += 1;
            assert!(frames < 600);
        }
    }
}
