//! World generation settings, loaded from `assets/data/worldgen.ron`.
//!
//! The file holds one full settings block per [`WorldPreset`] (GP-21):
//! `developed`, the 2048-tile world of M1-M3, and `run`, the 1024-tile run
//! island. [`WorldGenSettings::default_data`] is always `developed`.

use std::collections::BTreeSet;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use sim_core::{StateHash, StateHasher};

use crate::SizeTier;
use crate::monuments::MonumentKind;

/// The data file, embedded at compile time so every build generates the same
/// world without depending on the working directory.
pub const DEFAULT_WORLDGEN_RON: &str = include_str!("../../../assets/data/worldgen.ron");

/// Which shipped world to generate (GP-21, ADR 0013).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum WorldPreset {
    /// The 2048-tile world of M1-M3 with one populated region: stress
    /// tests, benchmarks and the M2/M3 tests.
    #[default]
    Developed,
    /// The 1024-tile run island: four regions, every settlement populated at
    /// a drawn size, and monuments.
    Run,
}

impl WorldPreset {
    /// Every preset, in file order.
    pub const ALL: [WorldPreset; 2] = [WorldPreset::Developed, WorldPreset::Run];

    /// The preset's key in worldgen.ron ("developed", "run").
    pub fn name(self) -> &'static str {
        match self {
            WorldPreset::Developed => "developed",
            WorldPreset::Run => "run",
        }
    }

    /// The preset named `name`, if any.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name() == name)
    }
}

/// The whole of worldgen.ron: one complete settings block per preset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldGenPresets {
    /// The M1-M3 world ([`WorldPreset::Developed`]).
    pub developed: WorldGenSettings,
    /// The run island ([`WorldPreset::Run`]).
    pub run: WorldGenSettings,
}

impl WorldGenPresets {
    /// Parses worldgen.ron text and validates every preset.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        let p: Self = ron::from_str(text).map_err(|e| e.to_string())?;
        for preset in WorldPreset::ALL {
            p.get(preset)
                .validate()
                .map_err(|e| format!("{}: {e}", preset.name()))?;
        }
        Ok(p)
    }

    /// The presets shipped in `assets/data/worldgen.ron`.
    pub fn default_data() -> Self {
        Self::from_ron(DEFAULT_WORLDGEN_RON).expect("assets/data/worldgen.ron is valid")
    }

    /// The settings of one preset.
    pub fn get(&self, preset: WorldPreset) -> &WorldGenSettings {
        match preset {
            WorldPreset::Developed => &self.developed,
            WorldPreset::Run => &self.run,
        }
    }
}

/// One preset of worldgen.ron. See docs/dev/DATA.md.
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
    /// Monument placement (GP-23); none in `developed`.
    pub monuments: MonumentSettings,
    /// Settlement size draws (GP-21); none in `developed`.
    pub sizes: SizeSettings,
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

/// Monument placement (GP-23): how many per region, how far from
/// settlements and each other, and which kinds stand where.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonumentSettings {
    /// Monuments placed in each region; 0 places none.
    pub per_region: u32,
    /// Minimum distance in tiles from every settlement centre. Keep it above
    /// army.ron `threat.radius`, or a guard camp keeps a village frightened.
    pub min_settlement_distance: f32,
    /// Minimum distance in tiles between two monuments.
    pub min_spacing: f32,
    /// The kinds that may be drawn, with their weights and sites.
    pub kinds: Vec<MonumentKindRule>,
}

/// One monument kind's draw weight and where it may stand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonumentKindRule {
    /// The kind this rule is for (each kind at most once).
    pub kind: MonumentKind,
    /// Relative chance of being drawn for a slot; must be positive.
    pub weight: f32,
    /// Where it may stand.
    pub site: SiteRule,
}

/// Where a monument may stand. Every rule also excludes Water, Road,
/// Mountain and Farmland tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SiteRule {
    /// Open ground within `min..=max` tiles (4-neighbour steps) of water.
    NearWater {
        /// Fewest steps from water.
        min: u8,
        /// Most steps from water.
        max: u8,
    },
    /// On hills.
    Hills,
    /// On hills within `within` tiles of a mountain.
    HillsNearMountain {
        /// Most steps from a mountain tile.
        within: u8,
    },
    /// Open ground within `min..=max` tiles of a road.
    NearRoad {
        /// Fewest steps from a road.
        min: u8,
        /// Most steps from a road.
        max: u8,
    },
    /// At a forest's edge (forest next to open ground), or beside a road.
    ForestEdgeOrRoad,
    /// On grass or hills.
    GrassOrHills,
}

/// Settlement sizes (GP-21): the tiers and the seat. With no tiers, no
/// sizes are drawn and population is apportioned as in M2/M3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SizeSettings {
    /// Size tiers; each tier's weight is its share of the settlements.
    pub tiers: Vec<SizeTierRule>,
    /// The largest settlement (the rival's seat) has at least this many people.
    pub seat_min: u32,
}

/// One size tier: its share of settlements and its range of people.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SizeTierRule {
    /// The tier this rule is for (each tier at most once).
    pub tier: SizeTier,
    /// Share of the settlements in this tier (relative to the other weights).
    pub weight: f32,
    /// Fewest people.
    pub min: u32,
    /// Most people.
    pub max: u32,
}

impl WorldGenSettings {
    /// Parses and validates one preset's settings (a single settings block,
    /// not the two-preset file; tests and experiments).
    pub fn from_ron(text: &str) -> Result<Self, String> {
        let s: Self = ron::from_str(text).map_err(|e| e.to_string())?;
        s.validate()?;
        Ok(s)
    }

    /// The `developed` preset shipped in `assets/data/worldgen.ron`: the
    /// world every M1-M3 test, benchmark and the stress mode use.
    pub fn default_data() -> Self {
        Self::preset(WorldPreset::Developed)
    }

    /// A preset shipped in `assets/data/worldgen.ron`.
    pub fn preset(preset: WorldPreset) -> Self {
        let all = WorldGenPresets::default_data();
        match preset {
            WorldPreset::Developed => all.developed,
            WorldPreset::Run => all.run,
        }
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
        self.validate_monuments()?;
        self.validate_sizes()
    }

    fn validate_monuments(&self) -> Result<(), String> {
        let m = &self.monuments;
        if m.per_region > 0 && m.kinds.is_empty() {
            return Err("monuments: per_region > 0 needs at least one kind".into());
        }
        if !(m.min_settlement_distance >= 0.0 && m.min_spacing >= 0.0) {
            return Err("monuments: distances must be >= 0".into());
        }
        let mut seen = BTreeSet::new();
        for rule in &m.kinds {
            if !seen.insert(rule.kind) {
                return Err(format!("monuments: {} listed twice", rule.kind.name()));
            }
            if !(rule.weight.is_finite() && rule.weight > 0.0) {
                return Err(format!(
                    "monuments: {} weight must be > 0",
                    rule.kind.name()
                ));
            }
            if let SiteRule::NearWater { min, max } | SiteRule::NearRoad { min, max } = rule.site
                && min > max
            {
                return Err(format!(
                    "monuments: {} site needs min <= max",
                    rule.kind.name()
                ));
            }
        }
        // Monument names come from the same syllables as settlement names,
        // never reusing one.
        let names = self.names.prefixes.len() * self.names.suffixes.len();
        let regions = (self.regions as usize).min(crate::NO_REGION as usize);
        if names < self.settlements.max as usize + m.per_region as usize * regions {
            return Err(
                "names: not enough syllable combinations for settlements and monuments".into(),
            );
        }
        Ok(())
    }

    fn validate_sizes(&self) -> Result<(), String> {
        let z = &self.sizes;
        if z.tiers.is_empty() && z.seat_min > 0 {
            return Err("sizes: seat_min needs at least one tier".into());
        }
        let mut seen = BTreeSet::new();
        for t in &z.tiers {
            if !seen.insert(t.tier) {
                return Err(format!("sizes: {} listed twice", t.tier.name()));
            }
            if !(t.weight.is_finite() && t.weight > 0.0) {
                return Err(format!("sizes: {} weight must be > 0", t.tier.name()));
            }
            if !(0 < t.min && t.min <= t.max) {
                return Err(format!("sizes: {} needs 0 < min <= max", t.tier.name()));
            }
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
        assert_eq!(s.regions, 10);
        assert_eq!((s.settlements.min, s.settlements.max), (40, 80));
        // D6: the developed world has no monuments and no drawn sizes.
        assert_eq!(s.monuments.per_region, 0);
        assert!(s.sizes.tiers.is_empty());
        assert_eq!(s, WorldGenSettings::preset(WorldPreset::Developed));
    }

    #[test]
    fn run_preset_is_valid() {
        let s = WorldGenSettings::preset(WorldPreset::Run);
        assert_eq!(s.size, 1024);
        assert_eq!(s.regions, 4);
        assert!(s.monuments.per_region > 0);
        assert!(!s.sizes.tiers.is_empty());
        // Monuments must sit outside a village's threat radius (army.ron 45).
        assert!(s.monuments.min_settlement_distance > 45.0);
    }

    #[test]
    fn preset_names_round_trip() {
        for p in WorldPreset::ALL {
            assert_eq!(WorldPreset::parse(p.name()), Some(p));
        }
        assert_eq!(WorldPreset::parse("nope"), None);
    }

    #[test]
    fn rejects_bad_monument_and_size_rules() {
        let mut s = WorldGenSettings::preset(WorldPreset::Run);
        s.monuments.kinds[0].weight = 0.0;
        assert!(s.validate().is_err());

        let mut s = WorldGenSettings::preset(WorldPreset::Run);
        s.sizes.tiers[0].min = s.sizes.tiers[0].max + 1;
        assert!(s.validate().is_err());

        let mut s = WorldGenSettings::preset(WorldPreset::Run);
        s.monuments.kinds.clear();
        assert!(s.validate().is_err());
    }

    #[test]
    fn rejects_size_not_multiple_of_chunk() {
        let s = WorldGenSettings::default_data().with_size(1000);
        assert!(s.validate().is_err());
    }
}
