//! Accent layer: Auto sometimes lays a light overlay over the main scene
//! (after the blendable layers of Imaginando's VS). The overlays are their
//! own shaders in `shaders/accents/` (embers, streaks, swarm,
//! bokeh, shafts, ripples, notes, sparkles, bubbles, fireflies, petals,
//! comets, fog, spectrum, wave, halo), built to sit
//! on top of a scene: borrowing regular scenes (comets, helix, orbiters)
//! read as odd. The renderer gives the accent its own feedback pair and
//! screen-adds its lights into the scene target under the dancer
//! (`Renderer::set_accent`, shaders/accent.wgsl). Keep each overlay cheap
//! (<= 0.2 ms at 1080p on the dev 5070 Ti). Never over `@heavy`, Unity,
//! video or flight scenes.

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
    /// A scene cut. `count` is how many overlays there are; `base_ok` is
    /// false over scenes that take no accent; `driving` is false in
    /// breakdowns. `roll` and `pick` are random 0..1.
    pub fn on_cut(&mut self, count: usize, base_ok: bool, driving: bool, roll: f32, pick: f32) {
        self.want = if base_ok && driving && roll < CHANCE && count > 0 {
            Some(((pick * count as f32) as usize).min(count - 1))
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
    fn picks_only_when_allowed() {
        let mut a = Accent::default();
        a.on_cut(3, true, true, 0.1, 0.99);
        assert_eq!(a.tick(0.0, 0.6).map(|x| x.0), Some(2), "index stays in range");
        let mut b = Accent::default();
        b.on_cut(3, false, true, 0.1, 0.5);
        assert_eq!(b.tick(0.0, 0.6), None, "not over a heavy/flight base");
        b.on_cut(3, true, false, 0.1, 0.5);
        assert_eq!(b.tick(0.0, 0.6), None, "not in a breakdown");
        b.on_cut(3, true, true, 0.9, 0.5);
        assert_eq!(b.tick(0.0, 0.6), None, "only CHANCE of cuts");
        b.on_cut(0, true, true, 0.1, 0.5);
        assert_eq!(b.tick(0.0, 0.6), None, "no overlays loaded");
    }

    /// Every overlay in shaders/accents parses and validates.
    #[test]
    fn accent_overlays_validate() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders");
        let common = std::fs::read_to_string(dir.join("common.wgsl")).unwrap();
        let files = crate::render::accent_files(&dir);
        assert!(files.len() >= 3, "embers, streaks, swarm");
        for f in files {
            let src = format!("{common}\n{}", std::fs::read_to_string(&f).unwrap());
            let m = wgpu::naga::front::wgsl::parse_str(&src).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
            wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::all(),
            )
            .validate(&m)
            .unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        }
    }

    #[test]
    fn fades_in_and_out_without_jumps() {
        let mut a = Accent::default();
        a.on_cut(1, true, true, 0.0, 0.0);
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
            assert_eq!(i, 0, "keeps drawing the old accent while it fades");
            assert!(amt <= last + 1e-6);
            last = amt;
            frames += 1;
            assert!(frames < 600);
        }
    }
}
