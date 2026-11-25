//! Echo: a whole-frame feedback post effect (after the "Render Output"
//! feedback layer of Imaginando's VS). The finished frame, dancer included,
//! is max-blended with its own last echo, zoomed out and turned a little, so
//! light leaves trails that follow the motion (shaders/echo.wgsl,
//! `Renderer::set_echo`). It runs in its own feedback pair, so scenes that
//! read `prev()` never see it.
//!
//! Auto turns it on through breakdowns (long, dreamy trails) and on some cuts
//! while the track drives. Never on flight scenes (a zooming echo on top of
//! forward travel reads as racing) or scenes with their own feedback trails
//! (trails of trails smear into mush).

use crate::config::Tristate;

/// Share of cuts (while the track drives) that get echo under Auto.
pub const CHANCE: f32 = 0.3;

#[derive(Default)]
pub struct Echo {
    /// Auto's pick for the current scene.
    picked: bool,
    amt: f32,
}

impl Echo {
    /// A scene cut: Auto rolls whether this scene gets echo while driving.
    pub fn on_cut(&mut self, roll: f32) {
        self.picked = roll < CHANCE;
    }

    /// Ease toward the wanted amount and return it (0 = off). `scene_ok` is
    /// false on flight scenes and scenes with their own feedback: Auto skips
    /// those, On does not (On means on: the first build kept it off there
    /// too, and it "didn't do anything"). `calm` is the breakdown state 0..1.
    pub fn tick(&mut self, dt: f32, mode: Tristate, amount: f32, scene_ok: bool, calm: f32) -> f32 {
        let on = match mode {
            Tristate::Off => false,
            Tristate::On => true,
            Tristate::Auto => scene_ok && (self.picked || calm > 0.6),
        };
        let target = if on { amount } else { 0.0 };
        self.amt += (target - self.amt) * (dt * 1.5).min(1.0);
        if !on && self.amt < 0.004 {
            self.amt = 0.0;
        }
        self.amt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_breakdowns_and_cuts() {
        let mut e = Echo::default();
        e.on_cut(0.9);
        for _ in 0..300 {
            e.tick(1.0 / 60.0, Tristate::Auto, 0.8, true, 0.0);
        }
        assert_eq!(e.tick(0.0, Tristate::Auto, 0.8, true, 0.0), 0.0, "not picked, driving");
        for _ in 0..300 {
            e.tick(1.0 / 60.0, Tristate::Auto, 0.8, true, 1.0);
        }
        assert!(e.tick(0.0, Tristate::Auto, 0.8, true, 1.0) > 0.79, "on in a breakdown");
        for _ in 0..600 {
            e.tick(1.0 / 60.0, Tristate::Auto, 0.8, false, 1.0);
        }
        assert_eq!(e.tick(0.0, Tristate::Auto, 0.8, false, 1.0), 0.0, "never on a flight scene");
        e.on_cut(0.1);
        let mut last = 0.0;
        for _ in 0..300 {
            let a = e.tick(1.0 / 60.0, Tristate::Auto, 0.8, true, 0.0);
            assert!(a - last < 0.03, "eases in");
            last = a;
        }
        assert!(last > 0.79, "picked on this cut");
        assert_eq!(
            {
                let mut x = Echo::default();
                x.tick(1.0, Tristate::Off, 0.8, true, 1.0)
            },
            0.0
        );
        let mut on = Echo::default();
        assert!(on.tick(1.0, Tristate::On, 0.8, false, 0.0) > 0.79, "On means on, even on a flight scene");
    }
}
