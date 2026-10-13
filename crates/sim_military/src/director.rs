//! The pacing director (GP-16): the run's conductor.
//!
//! Once a game hour ([`step`], slot 12 of the progress driver, runs only) it
//! measures the player's **tension** from hashed state: threats near the
//! player's armies and settlements, hunger in the player's baggage, the
//! player's armies in battle, and how long it has been quiet (the time
//! since `Chronicle::last_notable`). Quiet lowers tension; the rest raise
//! it. Then it may bring a band of raiders ([`Raider`], `RAIDERS`,
//! aggressive) to a seeded tile around a village near the player
//! (stream `sim_military::director` at the tick), aimed at that village
//! (`Raider::target`): on arrival the village asks the player to drive
//! them off (a `DriveOff` request), so a band also nudges a request into
//! being. Once it has raided its target with nothing else to do, the band
//! lingers by the village `band_linger_hours`, then withdraws out of
//! every settlement's reach (`raiders::think`, ADR 0018). A band comes:
//! - **when it is about to be too quiet**: the next hourly check would
//!   pass `quiet_limit_hours` without a notable event, or
//! - **when the run's first event is due**: nothing has happened yet
//!   (no request, raid or fight) by `first_event_within_hours`, or
//! - **when tension drops below zero**, at most every `spacing_hours`
//!   (divided by the restless-roads `RaiderPace`) and while fewer than
//!   `max_bands` of the director's bands roam.
//!
//! Each band reports `RaidersArrived`. Personalities (steady, chaotic,
//! harsh) live in `assets/data/director.ron`, one per [`Difficulty`]; the
//! run's `RunConfig::difficulty` picks one. `auto_pause` lists the event
//! kinds the game pauses on (UI, step 10).

use std::f32::consts::TAU;

use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sim_agents::{Owner, Threat, Villagers};
use sim_core::rng::rng_key;
use sim_core::{
    Calendar, Chronicle, Difficulty, FactionId, Posture, RunState, SimEvent, SimEventKind, SimSeed,
    SimTick, SpawnId, StateHash, StateHasher,
};
use sim_economy::SettlementEconomy;
use sim_world::{NO_REGION, Settlement, WorldMap};

use crate::components::{Captain, Raider};
use crate::data::Army;
use crate::index::ArmyIndex;
use crate::orders::log;
use crate::progression::{Effect, Progression, RunProgress, champion_of};
use crate::requests::RequestBoard;
use crate::spawn::{ArmySpec, nearest_passable, spawn_army};

/// `assets/data/director.ron`, embedded at compile time.
pub const DEFAULT_DIRECTOR_RON: &str = include_str!("../../../assets/data/director.ron");

/// How much each input weighs in the director's tension (GP-16).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TensionWeights {
    /// Hostile armies near the player's armies, or threat at the player's
    /// settlements (0..1).
    pub threat: f32,
    /// Hunger in the player's baggage: 1 with no food, 0 at two days or
    /// more.
    pub hunger: f32,
    /// One of the player's armies is in battle (0 or 1).
    pub fights: f32,
    /// Time since the last notable event, as a share of the quiet limit
    /// (0..1). Subtracted: quiet lowers tension.
    pub quiet: f32,
}

/// The size of the director's raider bands.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RaiderBandData {
    /// People in a band (its captain included) on day 0.
    pub min: u32,
    /// ... at most.
    pub max: u32,
    /// More people per day of the run (rounded down).
    pub per_day: f32,
    /// Every raider carries a weapon.
    pub armed: bool,
}

impl RaiderBandData {
    /// People in a band on `day` (days since the start).
    pub fn size_on(&self, day: f32) -> u32 {
        let grown = self.min as f32 + (self.per_day * day.max(0.0)).floor();
        (grown as u32).clamp(self.min, self.max)
    }
}

/// A director personality, one per [`Difficulty`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Personality {
    /// The difficulty that picks it.
    pub difficulty: Difficulty,
    /// Tension weights.
    pub weights: TensionWeights,
    /// Never more than this many game hours (100 ticks each) without a
    /// notable event: a band comes before.
    pub quiet_limit_hours: f32,
    /// The run's first event (a request, a raid or a fight) comes within
    /// this many hours.
    pub first_event_within_hours: f32,
    /// At least this many hours between bands brought because tension is
    /// low (divided by the restless roads' `RaiderPace`).
    pub spacing_hours: f32,
    /// Band size.
    pub raider_band: RaiderBandData,
    /// Bands brought for low tension only while fewer than this many of
    /// the director's bands roam (bands against quiet always come).
    pub max_bands: u32,
    /// The game pauses when an event of one of these kinds arrives (UI).
    pub auto_pause: Vec<SimEventKind>,
}

/// The pacing director's data (`assets/data/director.ron`, GP-16). Hashed
/// as its RON text.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirectorData {
    /// One personality per difficulty.
    pub personalities: Vec<Personality>,
    /// A band appears this many tiles (min, max) from the village it
    /// raids (or from the champion).
    pub spawn_distance: (f32, f32),
    /// Days of food in a band's baggage.
    pub band_food_days: f32,
    /// Game hours a band stays by the village it was sent to raid once it
    /// has nothing else to do (no prey, no village of the player's); then
    /// it withdraws beyond army.ron `threat.radius` of every settlement
    /// (`raiders::think`, ADR 0018). Optional: director text saved before
    /// it existed loads with the shipped value.
    #[serde(default = "default_linger_hours")]
    pub band_linger_hours: f32,
}

/// The shipped `band_linger_hours`, for director text saved before it
/// existed.
fn default_linger_hours() -> f32 {
    12.0
}

impl DirectorData {
    /// Parses and validates director.ron text.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        let d: DirectorData = ron::from_str(text).map_err(|e| e.to_string())?;
        d.validate()?;
        Ok(d)
    }

    /// The shipped director data.
    pub fn default_data() -> Self {
        Self::from_ron(DEFAULT_DIRECTOR_RON).expect("assets/data/director.ron is valid")
    }

    /// Checks the data hangs together.
    pub fn validate(&self) -> Result<(), String> {
        let ok = |v: f32| v.is_finite() && v >= 0.0;
        for difficulty in Difficulty::ALL {
            let n = self
                .personalities
                .iter()
                .filter(|p| p.difficulty == difficulty)
                .count();
            if n != 1 {
                return Err(format!(
                    "director: need one personality for {}, found {n}",
                    difficulty.name()
                ));
            }
        }
        for p in &self.personalities {
            let w = p.weights;
            let b = p.raider_band;
            if ![w.threat, w.hunger, w.fights, w.quiet, b.per_day]
                .into_iter()
                .all(ok)
                || !(p.quiet_limit_hours.is_finite() && p.quiet_limit_hours >= 2.0)
                || !(p.first_event_within_hours.is_finite() && p.first_event_within_hours >= 1.0)
                || !(p.spacing_hours.is_finite() && p.spacing_hours > 0.0)
                || b.min == 0
                || b.min > b.max
            {
                return Err(format!(
                    "director: {} needs weights >= 0, quiet_limit_hours >= 2, \
                     first_event_within_hours >= 1, spacing_hours > 0 and a band of 1 <= min <= max",
                    p.difficulty.name()
                ));
            }
        }
        let (lo, hi) = self.spawn_distance;
        if !(ok(lo) && hi.is_finite() && lo <= hi && hi > 0.0)
            || !ok(self.band_food_days)
            || !ok(self.band_linger_hours)
        {
            return Err(
                "director: spawn_distance needs 0 <= min <= max, max > 0, and band_food_days and band_linger_hours >= 0"
                    .into(),
            );
        }
        Ok(())
    }

    /// The personality for `difficulty`.
    pub fn personality(&self, difficulty: Difficulty) -> Option<&Personality> {
        self.personalities
            .iter()
            .find(|p| p.difficulty == difficulty)
    }
}

impl StateHash for DirectorData {
    fn hash_state(&self, h: &mut StateHasher) {
        h.bytes(
            ron::to_string(self)
                .expect("director data serialises")
                .as_bytes(),
        );
    }
}

/// The director's state (GP-16). Always present; all defaults outside a
/// run. Hashed.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct Director {
    /// The last hour's tension (see the module docs): above 0, the player
    /// has enough on its hands; below, it has been quiet.
    pub tension: f32,
    /// When the director last brought a band.
    pub last_spawn: Option<u64>,
    /// No band for low tension before this tick (spacing).
    pub next_check: u64,
    /// The tick of the run's first event (a request, raiders, a raid or a
    /// fight: [`eventful`]), once the director has seen it.
    pub first_event: Option<u64>,
    /// Bands brought so far.
    pub bands: u32,
}

impl StateHash for Director {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.tension);
        let opt = |h: &mut StateHasher, v: Option<u64>| match v {
            Some(t) => {
                h.u8(1);
                h.u64(t);
            }
            None => h.u8(0),
        };
        opt(h, self.last_spawn);
        h.u64(self.next_check);
        opt(h, self.first_event);
        h.u32(self.bands);
    }
}

/// Whether an event of this kind counts as the run's first event: notable,
/// and not the run's own start, an offer picked or a trait gained by it.
pub fn eventful(kind: SimEventKind) -> bool {
    kind.notable()
        && !matches!(
            kind,
            SimEventKind::RunStarted
                | SimEventKind::OfferChosen
                | SimEventKind::TraitGained
                | SimEventKind::RunEnded
        )
}

/// The product of the run modifiers' `RaiderPace` effects (1.0 with none):
/// restless roads (GP-27).
pub fn raider_pace(world: &World) -> f32 {
    let data = world.resource::<Progression>();
    let pace: f32 = world
        .resource::<RunProgress>()
        .modifiers
        .iter()
        .filter_map(|&card| data.modifier(card))
        .flat_map(|m| m.effects.iter())
        .filter_map(|e| match *e {
            Effect::RaiderPace(f) => Some(f),
            _ => None,
        })
        .product();
    pace.max(0.01)
}

/// The personality for this run's difficulty.
pub fn personality(world: &World) -> Option<Personality> {
    let difficulty = world.resource::<RunState>().config?.difficulty;
    world
        .resource::<DirectorData>()
        .personality(difficulty)
        .cloned()
}

/// GP-16: before the first tick of a run, sets the first low-tension band
/// no earlier than `spacing_hours` (paced) into the run. Called by
/// `progression::setup_run`.
pub fn setup(world: &mut World) {
    let Some(p) = personality(world) else {
        return;
    };
    let pace = raider_pace(world);
    let next = world.resource::<Calendar>().hours(p.spacing_hours / pace);
    world.resource_mut::<Director>().next_check = next;
}

/// The tension inputs, each 0..1: (threat, hunger, fights).
fn inputs(world: &mut World) -> (f32, f32, f32) {
    let army = world.resource::<Army>().clone();
    let radius = army.data.threat.radius;
    let hunger_per_day = world.resource::<Villagers>().data.needs.hunger_per_day;
    let index = world.resource::<ArmyIndex>();
    let mine: Vec<(Vec2, f32, u32)> = index
        .armies
        .iter()
        .filter(|a| a.faction == FactionId::PLAYER)
        .map(|a| (a.pos, a.food, a.followers + 1))
        .collect();
    let near = index.armies.iter().any(|a| {
        army.at_war(a.faction, FactionId::PLAYER)
            && mine.iter().any(|m| m.0.distance(a.pos) <= radius)
    });
    let mut q = world.query::<(&Owner, &Threat)>();
    let settled = q
        .iter(world)
        .filter(|(o, _)| o.faction == FactionId::PLAYER)
        .map(|(_, t)| t.level)
        .fold(0.0f32, f32::max);
    let threat = if near { 1.0 } else { settled.clamp(0.0, 1.0) };
    let (food, people) = mine
        .iter()
        .fold((0.0f32, 0u32), |(f, n), m| (f + m.1, n + m.2));
    let hunger = if people == 0 {
        0.0
    } else {
        let days = food / (hunger_per_day * people as f32).max(1e-3);
        (1.0 - days / 2.0).clamp(0.0, 1.0)
    };
    let mut q = world.query::<&Captain>();
    let fights = q
        .iter(world)
        .any(|c| c.faction == FactionId::PLAYER && c.battle_since.is_some());
    (threat, hunger, if fights { 1.0 } else { 0.0 })
}

/// GP-16: the director's hourly step (slot 12 of `progression::progress`,
/// run only). Records the first event once seen, measures tension, and
/// brings a raider band when it is about to be too quiet, when the first
/// event is due, or when tension is below zero and spacing allows (see
/// the module docs). Span `sim_military::director`.
pub fn step(world: &mut World) {
    let _span = tracing::info_span!("sim_military::director").entered();
    if !world.resource::<RunState>().in_run() {
        return;
    }
    let Some(p) = personality(world) else {
        return;
    };
    let tick = world.resource::<SimTick>().0;
    let cal = world.resource::<Calendar>().clone();
    let hour = cal.ticks_per_hour();
    if world.resource::<Director>().first_event.is_none() {
        // A request is posted in this same hourly driver, before this
        // step; anything else is in the chronicle with its tick.
        let posted = (world.resource::<RequestBoard>().posted > 0).then_some(tick);
        let chronicled = world
            .resource::<Chronicle>()
            .entries()
            .iter()
            .find(|e| eventful(e.event.kind))
            .map(|e| e.tick);
        let first = match (posted, chronicled) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        if first.is_some() {
            world.resource_mut::<Director>().first_event = first;
        }
    }
    let quiet_for = tick.saturating_sub(world.resource::<Chronicle>().last_notable());
    let limit = cal.hours(p.quiet_limit_hours);
    let quiet = (quiet_for as f32 / limit.max(1) as f32).min(1.0);
    let (threat, hunger, fights) = inputs(world);
    let w = p.weights;
    let tension = w.threat * threat + w.hunger * hunger + w.fights * fights - w.quiet * quiet;
    // GP-33 (M4a): bandits are not the director's bands; they don't count
    // toward `max_bands` (they do count for tension when near).
    let mut q =
        world.query_filtered::<&Captain, (With<Raider>, Without<crate::components::Bandits>)>();
    let roaming = q.iter(world).count() as u32;
    let director = world.resource::<Director>().clone();
    let too_quiet = quiet_for + hour > limit;
    let first_due =
        director.first_event.is_none() && tick + hour > cal.hours(p.first_event_within_hours);
    let calm = tension < 0.0 && tick >= director.next_check && roaming < p.max_bands;
    if director.tension != tension {
        world.resource_mut::<Director>().tension = tension;
    }
    if !(too_quiet || first_due || calm) {
        return;
    }
    let pace = raider_pace(world);
    let spawned = bring_band(world, &p, tick, &cal);
    let mut d = world.resource_mut::<Director>();
    if spawned {
        d.first_event.get_or_insert(tick);
        d.last_spawn = Some(tick);
        d.bands += 1;
        d.next_check = tick + cal.hours(p.spacing_hours / pace);
    } else {
        // Nowhere to put a band: try again in an hour.
        d.next_check = tick + hour;
    }
}

/// Brings one band of raiders: aimed at the populated settlement nearest
/// the player's champion that the player or nobody owns (the player's
/// first), appearing `spawn_distance` tiles from it (or from the champion)
/// on passable land outside the rival's region. Reports `RaidersArrived`
/// (`subject` the captain, `other` the target settlement or 0, `at` the
/// tile, `amount` the band's size). Returns false if no tile was found.
fn bring_band(world: &mut World, p: &Personality, tick: u64, cal: &Calendar) -> bool {
    let Some(champion) = champion_of(world, FactionId::PLAYER) else {
        return false;
    };
    let Some(anchor_from) = world.get::<sim_core::Position>(champion).map(|p| p.0) else {
        return false;
    };
    let progress = world.resource::<RunProgress>().clone();
    let mut q = world.query::<(&SpawnId, &Settlement, &Owner, &SettlementEconomy)>();
    let target = q
        .iter(world)
        .filter(|(id, _, o, se)| {
            se.population > 0
                && (o.faction == FactionId::PLAYER || o.faction == FactionId::INDEPENDENT)
                && Some(id.0) != progress.rival_seat
        })
        .map(|(id, s, o, _)| {
            (
                o.faction != FactionId::PLAYER,
                (s.center - anchor_from).length_squared(),
                id.0,
                s.center,
            )
        })
        .min_by_key(|k| (k.0, k.1, k.2))
        .map(|k| (k.2, k.3));
    let anchor = target.map_or(anchor_from, |t| t.1);
    let data = world.resource::<DirectorData>().clone();
    let seed = *world.resource::<SimSeed>();
    let mut rng = seed.stream(rng_key("sim_military::director"), tick);
    let costs = world.resource::<Villagers>().travel_costs.clone();
    let tile = {
        let map = world.resource::<WorldMap>();
        let (lo, hi) = data.spawn_distance;
        let mut found = None;
        for _ in 0..24 {
            let d = rng.random_range(lo..=hi);
            let a = rng.random_range(0.0..TAU);
            let at = (anchor.as_vec2() + Vec2::from_angle(a) * d)
                .round()
                .as_ivec2();
            let Some(t) = nearest_passable(map, &costs, at) else {
                continue;
            };
            let region = map.region(t);
            if region != NO_REGION && region != progress.rival_region {
                found = Some(t);
                break;
            }
        }
        found
    };
    let Some(tile) = tile else {
        log(world, "Raiders found no road to the player".into());
        return false;
    };
    let day = tick as f32 / cal.ticks_per_day as f32;
    let size = p.raider_band.size_on(day);
    let captain = spawn_army(
        world,
        &ArmySpec {
            posture: Posture::Aggressive,
            food_days: data.band_food_days,
            armed: p.raider_band.armed,
            raider: true,
            ..ArmySpec::new(FactionId::RAIDERS, tile, size.saturating_sub(1))
        },
    );
    if let Some(mut r) = world.get_mut::<Raider>(captain) {
        r.target = target.map(|t| t.0);
    }
    let id = world.get::<SpawnId>(captain).map_or(0, |s| s.0);
    world.write_message(
        SimEvent::new(SimEventKind::RaidersArrived, FactionId::RAIDERS)
            .subject(id)
            .other(target.map_or(0, |t| t.0))
            .at(tile)
            .amount(size as i32)
            .cause(sim_core::Cause::Raid { captain: id }),
    );
    log(
        world,
        format!("{size} raiders appeared at ({}, {})", tile.x, tile.y),
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_data_is_valid_and_bands_grow_by_day() {
        let d = DirectorData::default_data();
        for difficulty in Difficulty::ALL {
            assert!(d.personality(difficulty).is_some());
        }
        let b = RaiderBandData {
            min: 4,
            max: 10,
            per_day: 0.5,
            armed: false,
        };
        assert_eq!(b.size_on(0.0), 4);
        assert_eq!(b.size_on(1.9), 4);
        assert_eq!(b.size_on(2.0), 5);
        assert_eq!(b.size_on(100.0), 10);
        assert!(eventful(SimEventKind::RequestPosted));
        assert!(eventful(SimEventKind::RaidersArrived));
        assert!(!eventful(SimEventKind::RunStarted));
        assert!(!eventful(SimEventKind::OfferChosen));
        assert!(!eventful(SimEventKind::OfferPosted));
    }

    #[test]
    fn rejects_bad_data() {
        let ok = DirectorData::default_data();
        let check = |edit: &dyn Fn(&mut DirectorData)| {
            let mut d = ok.clone();
            edit(&mut d);
            d.validate()
        };
        assert!(check(&|_| {}).is_ok());
        assert!(check(&|d| d.personalities.truncate(2)).is_err());
        assert!(
            check(&|d| {
                let p = d.personalities[0].clone();
                d.personalities.push(p);
            })
            .is_err()
        );
        assert!(check(&|d| d.personalities[0].quiet_limit_hours = 1.0).is_err());
        assert!(check(&|d| d.personalities[0].first_event_within_hours = 0.5).is_err());
        assert!(check(&|d| d.personalities[0].spacing_hours = 0.0).is_err());
        assert!(check(&|d| d.personalities[0].weights.quiet = -1.0).is_err());
        assert!(check(&|d| d.personalities[0].raider_band.min = 0).is_err());
        assert!(check(&|d| d.personalities[0].raider_band.min = 99).is_err());
        assert!(check(&|d| d.spawn_distance = (50.0, 10.0)).is_err());
        assert!(check(&|d| d.band_food_days = -1.0).is_err());
        assert!(check(&|d| d.band_linger_hours = -1.0).is_err());
        assert!(check(&|d| d.band_linger_hours = f32::INFINITY).is_err());
    }

    /// GP-16 (ADR 0018): the default linger is the shipped one, so a save
    /// whose director text predates it lingers the same; and the field may
    /// be left out of the text.
    #[test]
    fn the_default_linger_matches_the_shipped_data() {
        let d = DirectorData::default_data();
        assert_eq!(default_linger_hours(), d.band_linger_hours);
        let without: String = DEFAULT_DIRECTOR_RON
            .lines()
            .filter(|l| !l.trim_start().starts_with("band_linger_hours"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!without.contains("band_linger_hours:"), "{without}");
        let back = DirectorData::from_ron(&without).expect("parses without the linger");
        assert_eq!(back, d);
    }
}
