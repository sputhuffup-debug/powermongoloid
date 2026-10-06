//! Detail bands (RND-03, ADR 0009): what a thing looks like depends on how
//! far it is from the camera. Thresholds come from `assets/data/render.ron`.

use bevy::prelude::*;
use serde::Deserialize;

pub const DEFAULT_RENDER_RON: &str = include_str!("../../../assets/data/render.ron");

#[derive(Resource, Debug, Clone, PartialEq, Deserialize)]
pub struct RenderSettings {
    pub detail_distance: f32,
    pub billboard_distance: f32,
    pub dot_distance: f32,
    pub hysteresis: f32,
    pub terrain_texture_tiles: f32,
}

impl Default for RenderSettings {
    fn default() -> Self {
        ron::from_str(DEFAULT_RENDER_RON).expect("assets/data/render.ron is valid")
    }
}

/// Nearest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Band {
    /// Stylized textures.
    #[default]
    Detail,
    /// Flat-shaded low-poly.
    LowPoly,
    /// Camera-facing silhouettes.
    Billboard,
    /// Density dots.
    Dot,
}

impl Band {
    pub const ALL: [Band; 4] = [Band::Detail, Band::LowPoly, Band::Billboard, Band::Dot];

    pub fn index(self) -> usize {
        self as usize
    }
}

impl RenderSettings {
    /// The band for a distance, ignoring hysteresis.
    pub fn classify(&self, distance: f32) -> Band {
        if distance < self.detail_distance {
            Band::Detail
        } else if distance < self.billboard_distance {
            Band::LowPoly
        } else if distance < self.dot_distance {
            Band::Billboard
        } else {
            Band::Dot
        }
    }

    /// The band for a distance, given the current band: moving outward
    /// needs (1 + h) x the threshold, moving inward (1 - h) x.
    pub fn select(&self, current: Option<Band>, distance: f32) -> Band {
        let plain = self.classify(distance);
        match current {
            None => plain,
            Some(cur) if plain > cur => self.classify(distance / (1.0 + self.hysteresis)).max(cur),
            Some(cur) if plain < cur => self.classify(distance / (1.0 - self.hysteresis)).min(cur),
            Some(cur) => cur,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_settings_are_ordered() {
        let s = RenderSettings::default();
        assert!(s.detail_distance < s.billboard_distance && s.billboard_distance < s.dot_distance);
        assert!(s.hysteresis > 0.0 && s.hysteresis < 0.5);
    }

    #[test]
    fn classify_by_distance() {
        let s = RenderSettings::default();
        assert_eq!(s.classify(10.0), Band::Detail);
        assert_eq!(s.classify(200.0), Band::LowPoly);
        assert_eq!(s.classify(800.0), Band::Billboard);
        assert_eq!(s.classify(5000.0), Band::Dot);
    }

    #[test]
    fn hysteresis_prevents_flicker_at_the_boundary() {
        let s = RenderSettings::default();
        let t = s.detail_distance;
        // Just past the threshold: stays textured.
        assert_eq!(s.select(Some(Band::Detail), t * 1.05), Band::Detail);
        // Well past: switches to low-poly.
        assert_eq!(s.select(Some(Band::Detail), t * 1.15), Band::LowPoly);
        // Coming back just inside: stays low-poly...
        assert_eq!(s.select(Some(Band::LowPoly), t * 0.95), Band::LowPoly);
        // ...until well inside.
        assert_eq!(s.select(Some(Band::LowPoly), t * 0.85), Band::Detail);
        // First sighting uses the plain thresholds.
        assert_eq!(s.select(None, t * 1.05), Band::LowPoly);
        // Big jumps cross several bands at once.
        assert_eq!(
            s.select(Some(Band::Detail), s.dot_distance * 2.0),
            Band::Dot
        );
    }
}
