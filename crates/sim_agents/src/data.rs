//! Villager data from `assets/data/villagers.ron`.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use sim_core::{Calendar, StateHash, StateHasher};
use sim_economy::Occupation;
use sim_world::{Terrain, TravelCosts};

/// `assets/data/villagers.ron`, embedded at compile time.
pub const DEFAULT_VILLAGERS_RON: &str = include_str!("../../../assets/data/villagers.ron");

/// Which region to populate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RegionChoice {
    /// The region with the most settlements (ties: lowest region id).
    Auto,
    Index(u8),
}

/// How many villagers to create, and where (`population` in villagers.ron).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PopulationData {
    pub total: u32,
    pub region: RegionChoice,
    pub min_per_settlement: u32,
}

/// An inclusive range of ages in years.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct YearRange {
    pub min: u32,
    pub max: u32,
}

/// Household composition: sizes and age ranges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HouseholdData {
    pub size_weights: Vec<u32>,
    pub adult_age: YearRange,
    pub child_age: YearRange,
    pub elder_chance: f32,
    pub elder_age: YearRange,
    pub working_age: YearRange,
}

/// The share of working-age villagers in one occupation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OccupationShare {
    pub occupation: Occupation,
    pub share: f32,
}

/// Need rates and thresholds (`needs` in villagers.ron).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeedsData {
    pub hunger_per_day: f32,
    pub rest_lost_per_day_awake: f32,
    pub rest_gained_per_day_asleep: f32,
    pub hungry: f32,
    pub starving: f32,
    pub sated: f32,
    pub meal_ticks: u32,
}

/// A response curve mapping an input in 0..1 to a consideration in 0..1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Curve {
    Logistic { mid: f32, steepness: f32 },
    Linear,
}

impl Curve {
    /// Evaluates the curve at `x` (clamped to 0..1).
    pub fn eval(self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        match self {
            Curve::Logistic { mid, steepness } => 1.0 / (1.0 + (-steepness * (x - mid)).exp()),
            Curve::Linear => x,
        }
    }
}

/// Scoring for the Eat action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EatAi {
    pub weight: f32,
    pub hunger: Curve,
}

/// Scoring for fetching food home.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FetchAi {
    pub weight: f32,
    pub home_stock_low: f32,
}

/// Scoring for sleep.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SleepAi {
    pub weight: f32,
    pub tiredness: Curve,
    /// At night the urge to sleep is at least this, however rested.
    pub night_drive: f32,
    pub night: f32,
    pub evening: f32,
    pub day: f32,
}

/// Scoring for work, by phase of the day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkAi {
    pub weight: f32,
    pub morning: f32,
    pub work: f32,
    pub evening: f32,
    pub night: f32,
}

/// Scoring for going home, by phase of the day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoHomeAi {
    pub weight: f32,
    pub evening: f32,
    pub night: f32,
    pub day: f32,
}

/// The idle baseline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdleAi {
    pub weight: f32,
}

/// Reactions to armies (SIM-13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleeAi {
    pub weight: f32,
    pub fear: Curve,
    /// Flee only from armies this close (tiles).
    pub radius: f32,
    /// How far to run (tiles).
    pub distance: i32,
}

/// Scoring for hiding at home (SIM-13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HideAi {
    pub weight: f32,
    pub fear: Curve,
}

/// Scoring for joining the militia (SIM-13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FightAi {
    pub weight: f32,
    pub courage: Curve,
    /// Per-person spread of courage, +/- this much.
    pub spread: f32,
    /// Below this morale a defender gives up and flees.
    pub break_morale: f32,
}

/// All utility-AI weights and curves (`ai` in villagers.ron; ADR 0006).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiData {
    pub decision_interval: u64,
    pub inertia: f32,
    pub eat: EatAi,
    pub fetch: FetchAi,
    pub sleep: SleepAi,
    pub work: WorkAi,
    pub go_home: GoHomeAi,
    pub idle: IdleAi,
    pub flee: FleeAi,
    pub hide: HideAi,
    pub fight: FightAi,
    pub distance_falloff: f32,
}

/// Walking speed per terrain, in tiles per tick (0 = impassable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Speeds {
    pub water: f32,
    pub beach: f32,
    pub grass: f32,
    pub farmland: f32,
    pub forest: f32,
    pub hills: f32,
    pub mountain: f32,
    pub road: f32,
}

impl Speeds {
    /// Speed on terrain `t`.
    pub fn of(&self, t: Terrain) -> f32 {
        match t {
            Terrain::Water => self.water,
            Terrain::Beach => self.beach,
            Terrain::Grass => self.grass,
            Terrain::Farmland => self.farmland,
            Terrain::Forest => self.forest,
            Terrain::Hills => self.hills,
            Terrain::Mountain => self.mountain,
            Terrain::Road => self.road,
        }
    }
}

/// Walking speeds and the slope penalty for paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MovementData {
    pub speed: Speeds,
    pub slope_cost: f32,
}

/// How idle strolls behave.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdleData {
    pub min_ticks: u32,
    pub max_ticks: u32,
    pub wander_radius: i32,
}

/// A civilian's fighting strength (SIM-13, SIM-32).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CivilianCombat {
    pub health: f32,
    pub attack: f32,
    pub defense: f32,
    pub morale: f32,
}

/// Original given and family names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NameData {
    pub given: Vec<String>,
    pub family: Vec<String>,
}

/// The whole of villagers.ron, as written. See docs/dev/DATA.md.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VillagerData {
    pub population: PopulationData,
    pub households: HouseholdData,
    pub occupations: Vec<OccupationShare>,
    pub needs: NeedsData,
    pub ai: AiData,
    pub movement: MovementData,
    pub idle: IdleData,
    pub combat: CivilianCombat,
    pub names: NameData,
}

/// Villager data plus values derived from it and the calendar.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Villagers {
    pub data: VillagerData,
    pub hunger_per_tick: f32,
    pub rest_loss_per_tick: f32,
    pub rest_gain_per_tick: f32,
    pub travel_costs: TravelCosts,
}

impl Villagers {
    /// Parses and validates villagers.ron text.
    pub fn from_ron(text: &str, calendar: &Calendar) -> Result<Self, String> {
        let data: VillagerData = ron::from_str(text).map_err(|e| e.to_string())?;
        Self::resolve(data, calendar)
    }

    /// The shipped data (panics if the embedded file is invalid; a unit test guards it).
    pub fn default_data(calendar: &Calendar) -> Self {
        Self::from_ron(DEFAULT_VILLAGERS_RON, calendar).expect("assets/data/villagers.ron is valid")
    }

    /// The same data with a different total population (tests, CLI).
    pub fn with_population(mut self, total: u32) -> Self {
        self.data.population.total = total;
        self
    }

    /// Validates the data and derives per-tick rates and travel costs.
    pub fn resolve(data: VillagerData, calendar: &Calendar) -> Result<Self, String> {
        let shares: f32 = data.occupations.iter().map(|o| o.share).sum();
        if (shares - 1.0).abs() > 1e-3 {
            return Err(format!(
                "villagers: occupation shares sum to {shares}, not 1"
            ));
        }
        for occ in Occupation::ALL {
            if !data.occupations.iter().any(|o| o.occupation == occ) {
                return Err(format!("villagers: no share for {}", occ.name()));
            }
        }
        if data.households.size_weights.is_empty()
            || data.households.size_weights.iter().all(|w| *w == 0)
        {
            return Err("villagers: household size weights must not all be zero".into());
        }
        if data.names.given.is_empty() || data.names.family.is_empty() {
            return Err("villagers: name lists must not be empty".into());
        }
        if data.ai.decision_interval == 0 {
            return Err("villagers: ai.decision_interval must be > 0".into());
        }
        let day = calendar.ticks_per_day as f32;
        let mut per_terrain = [None; 8];
        for t in Terrain::ALL {
            let speed = data.movement.speed.of(t);
            if speed > 0.0 {
                per_terrain[t as usize] = Some(((10.0 / speed).round() as u32).max(1));
            }
        }
        Ok(Self {
            hunger_per_tick: data.needs.hunger_per_day / day,
            rest_loss_per_tick: data.needs.rest_lost_per_day_awake / day,
            rest_gain_per_tick: data.needs.rest_gained_per_day_asleep / day,
            travel_costs: TravelCosts {
                per_terrain,
                slope_cost: data.movement.slope_cost,
            },
            data,
        })
    }

    /// The configured share of working villagers in `occ`.
    pub fn share(&self, occ: Occupation) -> f32 {
        self.data
            .occupations
            .iter()
            .find(|o| o.occupation == occ)
            .map_or(0.0, |o| o.share)
    }
}

impl StateHash for Villagers {
    fn hash_state(&self, h: &mut StateHasher) {
        h.bytes(
            ron::to_string(&self.data)
                .expect("villager data serialises")
                .as_bytes(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_data_is_valid() {
        let v = Villagers::default_data(&Calendar::default_data());
        assert_eq!(v.data.population.total, 5000);
        assert!(v.travel_costs.per_terrain[Terrain::Water as usize].is_none());
        let road = v.travel_costs.per_terrain[Terrain::Road as usize].unwrap();
        let grass = v.travel_costs.per_terrain[Terrain::Grass as usize].unwrap();
        assert!(road < grass, "roads are cheaper to travel");
    }

    #[test]
    fn curves() {
        let c = Curve::Logistic {
            mid: 0.5,
            steepness: 10.0,
        };
        assert!((c.eval(0.5) - 0.5).abs() < 1e-6);
        assert!(c.eval(0.9) > 0.95 && c.eval(0.1) < 0.05);
        assert_eq!(Curve::Linear.eval(2.0), 1.0);
    }
}
