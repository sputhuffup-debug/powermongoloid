//! Governing your towns (GP-34, GP-17 part, SIM-62, SIM-63, M4a;
//! LORDSHIP_DESIGN section 8, ADR 0020). Run only.
//!
//! - [`step`] is progress slot 15 (hourly, run only): pending edicts are
//!   applied and their first wave starts, then building sites advance and
//!   Market towns post hire offers; at dawn, stewards and their habits,
//!   and the later waves.
//! - [`Steward`]: the person who runs a town the player owns, with traits
//!   from their own life that act as visible habits.
//! - The Dues, Table and Steward loyalty terms, summed by `tenure::step`:
//!   [`dues_term`], [`table_term`], [`steward_term`].
//! - The data blocks `edicts`, `stewards`, `market` and `buildings`
//!   (progression.ron).
//!
//! M4a step 1 lands the types, the slot and the data; steps 4, 8 and 9 fill
//! them in (lane B owns this file).

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use sim_core::{Dial, RunState, StateHash, StateHasher};
use sim_economy::Occupation;

/// A town's steward (GP-34, M4a step 9): on the villager who runs `town`.
/// `traits` index progression.ron `stewards.traits`. Hashed and saved.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Steward {
    /// The settlement he runs.
    pub town: Entity,
    /// His traits, as indices into `stewards.traits`, at most two.
    pub traits: Vec<u8>,
    /// Appointed by the player (a follower), not named from the residents.
    pub appointed: bool,
    /// The tick he took office.
    pub since: u64,
}

impl StateHash for Steward {
    fn hash_state(&self, h: &mut StateHasher) {
        h.entity(self.town);
        h.u32(self.traits.len() as u32);
        for &t in &self.traits {
            h.u8(t);
        }
        h.bool(self.appointed);
        h.u64(self.since);
    }
}

sim_core::save_fields!(Steward {
    town,
    traits,
    appointed,
    since
});

/// GP-34: progress slot 15, hourly, run only (span
/// `sim_military::govern::step`): pending edicts and their first wave,
/// building sites, Market hire offers; at dawn, stewards and later waves.
/// Empty until M4a step 4.
pub fn step(world: &mut World) {
    let _span = tracing::info_span!("sim_military::govern::step").entered();
    if world.resource::<RunState>().in_run() {
        govern(world);
    }
}

/// Edicts, sites, hire offers and, at dawn, stewards (M4a steps 4, 8, 9).
fn govern(_world: &mut World) {}

/// GP-34: the Dues loyalty term at town `s` (summed by `tenure::step`).
/// 0.0 until M4a step 4.
pub fn dues_term(_world: &World, _s: Entity) -> f32 {
    0.0
}

/// GP-34: the Table loyalty term at town `s` (summed by `tenure::step`).
/// 0.0 until M4a step 4.
pub fn table_term(_world: &World, _s: Entity) -> f32 {
    0.0
}

/// GP-34: the Steward loyalty term at town `s` (a native steward; summed
/// by `tenure::step`). 0.0 until M4a step 9.
pub fn steward_term(_world: &World, _s: Entity) -> f32 {
    0.0
}

fn non_negative(v: f32) -> bool {
    v.is_finite() && v >= 0.0
}

/// The Tithe (`edicts.tithe`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TitheData {
    /// Days of the town's need it keeps.
    pub keep_days: f32,
    /// Its Dues term (0 at a submitted town).
    pub term: f32,
}

impl Default for TitheData {
    fn default() -> Self {
        Self {
            keep_days: 0.5,
            term: -0.05,
        }
    }
}

/// The Levy (`edicts.levy`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LevyData {
    /// Share of the people mustered at each wave.
    pub per_day: f32,
    /// Share of the people mustered at most.
    pub cap: f32,
    /// Its Dues term (0 at an invited town).
    pub term: f32,
    /// Its Dues term at a conquered or sacked town.
    pub harsh_term: f32,
}

impl Default for LevyData {
    fn default() -> Self {
        Self {
            per_day: 0.02,
            cap: 0.1,
            term: -0.08,
            harsh_term: -0.12,
        }
    }
}

/// Rationing (`edicts.ration`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RationData {
    /// Hunger multiplier.
    pub hunger: f32,
    /// Work cycle multiplier.
    pub work: f32,
    /// Its Table term.
    pub term: f32,
}

impl Default for RationData {
    fn default() -> Self {
        Self {
            hunger: 0.75,
            work: 1.1,
            term: -0.1,
        }
    }
}

/// A feast (`edicts.feast`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeastData {
    /// Days of food the town needs to hold a feast.
    pub min_food_days: f32,
    /// Days of its need the feast eats.
    pub food_days: f32,
    /// Game hours the town gathers on the green.
    pub gather_hours: f32,
    /// Work scores times this while it gathers (the Festival deed's
    /// goodwill is `goodwill_deeds.festival`).
    pub work: f32,
    /// Its Table term afterwards ...
    pub term: f32,
    /// ... for this many days.
    pub term_days: f32,
    /// Days before the next feast.
    pub cooldown_days: f32,
}

impl Default for FeastData {
    fn default() -> Self {
        Self {
            min_food_days: 1.0,
            food_days: 0.5,
            gather_hours: 24.0,
            work: 0.3,
            term: 0.08,
            term_days: 5.0,
            cooldown_days: 6.0,
        }
    }
}

/// Edicts (GP-34, M4a step 4): `edicts` in progression.ron. See
/// docs/dev/DATA.md and LORDSHIP_DESIGN section 8.1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EdictsData {
    /// A dial locks for this many days from the hour it starts.
    pub lock_days: f32,
    /// Tithes and levies reach owner captains within this many tiles.
    pub reach: f32,
    /// Share of the workers a wave moves ...
    pub moves_share: f32,
    /// ... at least this many.
    pub min_moves: u32,
    /// Fields keeps carters above this share of the workers.
    pub carter_floor: f32,
    /// Fields moves workers to food up to this share.
    pub fields_cap: f32,
    /// Forge moves workers to the forge up to this share.
    pub forge_cap: f32,
    /// Walls adds up to this many carters.
    pub walls_carters: u32,
    /// Walls builds sites this many times as fast.
    pub walls_build: f32,
    /// Walls adds this to the militia's defence.
    pub walls_defence: f32,
    /// Walls multiplies the threat radius by this.
    pub walls_threat: f32,
    /// The Tithe.
    pub tithe: TitheData,
    /// The Levy.
    pub levy: LevyData,
    /// Rationing.
    pub ration: RationData,
    /// A feast.
    pub feast: FeastData,
}

impl Default for EdictsData {
    /// The shipped values.
    fn default() -> Self {
        Self {
            lock_days: 2.0,
            reach: 60.0,
            moves_share: 0.1,
            min_moves: 2,
            carter_floor: 0.08,
            fields_cap: 0.85,
            forge_cap: 0.2,
            walls_carters: 6,
            walls_build: 1.5,
            walls_defence: 0.25,
            walls_threat: 1.25,
            tithe: TitheData::default(),
            levy: LevyData::default(),
            ration: RationData::default(),
            feast: FeastData::default(),
        }
    }
}

impl EdictsData {
    /// Checks the block.
    pub fn validate(&self) -> Result<(), String> {
        let unit = |v: f32| (0.0..=1.0).contains(&v);
        let term = |v: f32| (-1.0..=1.0).contains(&v);
        let f = &self.feast;
        let ok = [
            self.lock_days,
            self.reach,
            self.walls_build,
            self.walls_defence,
            self.walls_threat,
            self.tithe.keep_days,
            self.ration.hunger,
            self.ration.work,
            f.min_food_days,
            f.food_days,
            f.gather_hours,
            f.work,
            f.term_days,
            f.cooldown_days,
        ]
        .into_iter()
        .all(non_negative)
            && [
                self.moves_share,
                self.carter_floor,
                self.fields_cap,
                self.forge_cap,
                self.levy.per_day,
                self.levy.cap,
            ]
            .into_iter()
            .all(unit)
            && [
                self.tithe.term,
                self.levy.term,
                self.levy.harsh_term,
                self.ration.term,
                f.term,
            ]
            .into_iter()
            .all(term);
        if !ok {
            return Err(
                "progression: edicts values must be >= 0, shares in 0..1 and terms in -1..1".into(),
            );
        }
        Ok(())
    }
}

/// A dial's setting in a steward's habit row (GP-34, M4a step 9): one
/// value of `WorkFocus`, `Dues` or `Table`, named alone in the data.
/// Append only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Setting {
    /// `WorkFocus::Balanced`.
    Balanced,
    /// `WorkFocus::Fields`.
    Fields,
    /// `WorkFocus::Forge`.
    Forge,
    /// `WorkFocus::Walls`.
    Walls,
    /// `Dues::Light`.
    Light,
    /// `Dues::Tithe`.
    Tithe,
    /// `Dues::Levy`.
    Levy,
    /// `Table::Normal`.
    Normal,
    /// `Table::Ration`.
    Ration,
    /// `Table::Feast`.
    Feast,
}

impl Setting {
    /// The dial this setting belongs to.
    pub fn dial(self) -> Dial {
        match self {
            Setting::Balanced | Setting::Fields | Setting::Forge | Setting::Walls => Dial::Work,
            Setting::Light | Setting::Tithe | Setting::Levy => Dial::Dues,
            Setting::Normal | Setting::Ration | Setting::Feast => Dial::Table,
        }
    }
}

/// When a steward's habit applies (GP-34, M4a step 9), read at dawn from
/// the town's state.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// Always.
    Always,
    /// The town has at least this many people.
    PeopleAtLeast(u32),
    /// The town has under this many days of food.
    FoodDaysUnder(f32),
    /// The town has at least this many days of food.
    FoodDaysAtLeast(f32),
    /// A building site is open there, or a vow names it.
    SiteOpenOrVow,
}

/// Where a steward's trait comes from (GP-34, M4a step 9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraitSource {
    /// A Veteran or Elite soldier.
    Veteran,
    /// Was one of these trades (`Veteran.trade` for a follower).
    Trades(Vec<Occupation>),
    /// Lives in or comes from the town.
    Resident,
}

/// One steward trait (GP-34, M4a step 9): `stewards.traits[]`. Its habits
/// are `(dial, setting, condition)` rows applied in table order at dawn to
/// the dials the player hasn't set; `never` lists settings he won't choose
/// (Native's veto beats any habit).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StewardTrait {
    /// Display name.
    pub name: String,
    /// Who has it.
    pub from: TraitSource,
    /// Its habits, in order.
    #[serde(default)]
    pub habits: Vec<(Dial, Setting, Condition)>,
    /// Settings he never chooses on his own.
    #[serde(default)]
    pub never: Vec<(Dial, Setting)>,
    /// His dials lock for this many days instead of `edicts.lock_days`.
    #[serde(default)]
    pub lock_days: Option<f32>,
    /// The Steward loyalty term he brings.
    #[serde(default)]
    pub term: f32,
}

/// Stewards (GP-34, M4a step 9): `stewards` in progression.ron. See
/// docs/dev/DATA.md and LORDSHIP_DESIGN section 8.2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StewardsData {
    /// `false`: stewards set no dial (scenario tests opt out).
    pub habits: bool,
    /// The traits, in table order (the first named wins a conflict).
    pub traits: Vec<StewardTrait>,
}

impl Default for StewardsData {
    /// The shipped values.
    fn default() -> Self {
        use Condition::*;
        use Setting::*;
        let row = |name: &str, from, habits, never, lock_days, term| StewardTrait {
            name: name.into(),
            from,
            habits,
            never,
            lock_days,
            term,
        };
        Self {
            habits: true,
            traits: vec![
                row(
                    "Stern",
                    TraitSource::Veteran,
                    vec![
                        (Dial::Dues, Levy, PeopleAtLeast(100)),
                        (Dial::Table, Ration, FoodDaysUnder(2.0)),
                    ],
                    vec![(Dial::Table, Feast)],
                    None,
                    0.0,
                ),
                row(
                    "Forge-wise",
                    TraitSource::Trades(vec![Occupation::Smith, Occupation::Miner]),
                    vec![(Dial::Work, Forge, Always)],
                    vec![],
                    None,
                    0.0,
                ),
                row(
                    "Husband",
                    TraitSource::Trades(vec![
                        Occupation::Farmer,
                        Occupation::Fisher,
                        Occupation::Shepherd,
                        Occupation::Baker,
                    ]),
                    vec![
                        (Dial::Work, Fields, FoodDaysUnder(3.0)),
                        (Dial::Table, Feast, FoodDaysAtLeast(2.0)),
                    ],
                    vec![(Dial::Work, Forge)],
                    None,
                    0.0,
                ),
                row(
                    "Reeve",
                    TraitSource::Trades(vec![Occupation::Carter]),
                    vec![(Dial::Dues, Tithe, FoodDaysAtLeast(1.0))],
                    vec![],
                    Some(1.0),
                    0.0,
                ),
                row(
                    "Wright",
                    TraitSource::Trades(vec![Occupation::Woodcutter]),
                    vec![(Dial::Work, Walls, SiteOpenOrVow)],
                    vec![],
                    None,
                    0.0,
                ),
                row(
                    "Native",
                    TraitSource::Resident,
                    vec![],
                    vec![(Dial::Dues, Levy)],
                    None,
                    0.05,
                ),
            ],
        }
    }
}

impl StewardsData {
    /// Checks the block: every habit's and veto's setting belongs to its
    /// dial, names are distinct, numbers are in range.
    pub fn validate(&self) -> Result<(), String> {
        for (i, t) in self.traits.iter().enumerate() {
            if t.name.trim().is_empty() || self.traits[..i].iter().any(|o| o.name == t.name) {
                return Err(format!(
                    "progression: steward trait {:?} needs a distinct name",
                    t.name
                ));
            }
            let fits = t.habits.iter().all(|(d, s, _)| s.dial() == *d)
                && t.never.iter().all(|(d, s)| s.dial() == *d);
            if !fits {
                return Err(format!(
                    "progression: steward trait {:?}: a setting on the wrong dial",
                    t.name
                ));
            }
            let conditions_ok = t.habits.iter().all(|(_, _, c)| match *c {
                Condition::FoodDaysUnder(d) | Condition::FoodDaysAtLeast(d) => non_negative(d),
                _ => true,
            });
            if !conditions_ok
                || t.lock_days.is_some_and(|d| !non_negative(d))
                || !(-1.0..=1.0).contains(&t.term)
            {
                return Err(format!(
                    "progression: steward trait {:?}: days must be >= 0 and term in -1..1",
                    t.name
                ));
            }
        }
        if self.traits.len() > u8::MAX as usize {
            return Err("progression: too many steward traits".into());
        }
        Ok(())
    }
}

/// Market towns' hired men (SIM-62, M4a step 9): `market` in
/// progression.ron.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MarketData {
    /// A Market town you own posts a Hire offer every this many days.
    pub hire_days: f32,
    /// Men in the band at least ...
    pub min_men: u32,
    /// ... and at most ...
    pub max_men: u32,
    /// ... one per this many people.
    pub people_per_man: u32,
}

impl Default for MarketData {
    /// The shipped values.
    fn default() -> Self {
        Self {
            hire_days: 5.0,
            min_men: 3,
            max_men: 6,
            people_per_man: 80,
        }
    }
}

impl MarketData {
    /// Checks the block.
    pub fn validate(&self) -> Result<(), String> {
        if !(self.hire_days.is_finite() && self.hire_days > 0.0)
            || self.min_men > self.max_men
            || self.people_per_man == 0
        {
            return Err(
                "progression: market needs hire_days > 0, min_men <= max_men, people_per_man > 0"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Ordered buildings' limits and effects (SIM-63, M4a step 8): `buildings`
/// in progression.ron.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildingsData {
    /// Buildings a town may be ordered to build per run.
    pub max_per_town: u32,
    /// A granary keeps this many days of the town's need from plunder,
    /// seizure and the tithe.
    pub granary_keep_days: f32,
    /// A palisade adds this to the militia's defence.
    pub palisade_defence: f32,
    /// A palisade multiplies an Attack's hold by this.
    pub palisade_hold: f32,
    /// A palisade multiplies a Demand's defence by this.
    pub palisade_demand: f32,
    /// A watchtower multiplies the threat radius by this.
    pub watchtower_threat: f32,
}

impl Default for BuildingsData {
    /// The shipped values.
    fn default() -> Self {
        Self {
            max_per_town: 2,
            granary_keep_days: 1.5,
            palisade_defence: 0.5,
            palisade_hold: 1.5,
            palisade_demand: 1.5,
            watchtower_threat: 1.5,
        }
    }
}

impl BuildingsData {
    /// Checks the block.
    pub fn validate(&self) -> Result<(), String> {
        if ![
            self.granary_keep_days,
            self.palisade_defence,
            self.palisade_hold,
            self.palisade_demand,
            self.watchtower_threat,
        ]
        .into_iter()
        .all(non_negative)
        {
            return Err("progression: buildings values must be >= 0".into());
        }
        Ok(())
    }
}
