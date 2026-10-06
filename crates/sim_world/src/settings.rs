//! World generation settings, loaded from `assets/data/worldgen.ron`.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use sim_core::{StateHash, StateHasher};

/// The data file, embedded at compile time so every build generates the same
/// world without depending on the working directory.
pub const DEFAULT_WORLDGEN_RON: &str = include_str!("../../../assets/data/worldgen.ron");

/// The whole of worldgen.ron. See docs/dev/DATA.md.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldGenSettings {
    pub size: u32,
    pub chunk_size: u32,
    pub height: HeightSettings,
    pub moisture: FbmSettings,
    pub terrain: TerrainSettings,
    pub settlements: SettlementSettings,
    pub roads: RoadSettings,
    pub regions: u32,
    pub names: NameSettings,
}

/// Continent shape, coastline and mountains.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeightSettings {
    pub octaves: u32,
    pub frequency: f32,
    pub lacunarity: f32,
    pub gain: f32,
    pub warp_frequency: f32,
    pub warp_strength: f32,
    pub ridge_octaves: u32,
    pub ridge_frequency: f32,
    pub ridge_weight: f32,
    pub island_inner: f32,
    pub island_outer: f32,
    pub coast_wobble: f32,
    pub coast_frequency: f32,
}

/// Fractal noise parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FbmSettings {
    pub octaves: u32,
    pub frequency: f32,
    pub lacunarity: f32,
    pub gain: f32,
}

/// Terrain proportions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainSettings {
    pub water_fraction: f32,
    pub beach_width: u32,
    pub beach_max_land_quantile: f32,
    pub mountain_land_fraction: f32,
    pub hills_land_fraction: f32,
    pub forest_lowland_fraction: f32,
}

/// Settlement placement and farmland.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlementSettings {
    pub target: u32,
    pub min: u32,
    pub max: u32,
    pub min_spacing_tiles_at_2048: f32,
    pub core_radius: i32,
    pub farmland_radius: i32,
    pub farmland_min_fertility: u8,
    pub farmland_falloff: u8,
    pub spread_weight: f32,
}

/// Road network costs and links.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoadSettings {
    pub nearest_neighbours: u32,
    pub max_link_tiles_at_2048: f32,
    pub cost_grass: u32,
    pub cost_farmland: u32,
    pub cost_beach: u32,
    pub cost_forest: u32,
    pub cost_hills: u32,
    pub cost_mountain: u32,
    pub cost_road: u32,
    pub slope_cost: f32,
    pub search_margin_tiles: i32,
}

/// Syllables for settlement names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NameSettings {
    pub prefixes: Vec<String>,
    pub suffixes: Vec<String>,
}

impl WorldGenSettings {
    /// Parses and validates worldgen.ron text.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        let s: Self = ron::from_str(text).map_err(|e| e.to_string())?;
        s.validate()?;
        Ok(s)
    }

    /// The settings shipped in `assets/data/worldgen.ron`.
    pub fn default_data() -> Self {
        Self::from_ron(DEFAULT_WORLDGEN_RON).expect("assets/data/worldgen.ron is valid")
    }

    /// The same settings at another map size (tests and experiments).
    pub fn with_size(mut self, size: u32) -> Self {
        self.size = size;
        self
    }

    /// Converts a length given in tiles at a 2048 map to this map's size.
    pub fn scaled(&self, tiles_at_2048: f32) -> f32 {
        tiles_at_2048 * self.size as f32 / 2048.0
    }

    /// Checks sizes and fractions are consistent.
    pub fn validate(&self) -> Result<(), String> {
        if self.chunk_size == 0 || self.size == 0 || !self.size.is_multiple_of(self.chunk_size) {
            return Err(format!(
                "size {} must be a positive multiple of chunk_size {}",
                self.size, self.chunk_size
            ));
        }
        let s = &self.settlements;
        if !(s.min <= s.target && s.target <= s.max && s.min > 0) {
            return Err("settlements: need 0 < min <= target <= max".into());
        }
        if self.names.prefixes.is_empty() || self.names.suffixes.is_empty() {
            return Err("names: prefixes and suffixes must not be empty".into());
        }
        if (self.names.prefixes.len() * self.names.suffixes.len()) < s.max as usize {
            return Err("names: not enough syllable combinations for max settlements".into());
        }
        Ok(())
    }
}

impl StateHash for WorldGenSettings {
    fn hash_state(&self, h: &mut StateHasher) {
        // RON serialisation is deterministic for these plain types.
        let text = ron::to_string(self).expect("settings serialise");
        h.bytes(text.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_data_is_valid() {
        let s = WorldGenSettings::default_data();
        assert_eq!(s.size, 2048);
        assert_eq!(s.chunk_size, 64);
    }

    #[test]
    fn rejects_size_not_multiple_of_chunk() {
        let s = WorldGenSettings::default_data().with_size(1000);
        assert!(s.validate().is_err());
    }
}
