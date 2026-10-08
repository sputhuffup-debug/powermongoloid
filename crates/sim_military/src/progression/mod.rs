//! Runs (GP-21, GP-27, SIM-34): the progression data (`progression.ron`),
//! the run's start draws and setup, the run's progress, and offers.
//!
//! - [`Progression`]: the data, hashed like every data file.
//! - [`start`]: [`draw_start`], a pure function of the seed and the generated
//!   world: the rival's seat, the start spot, the band, the champion's trait,
//!   run modifiers and the three starting boons on offer.
//! - [`setup_run`]: before the first tick of a run, spawns the band and its
//!   champion, applies one-off run modifiers and posts the start offer.
//! - [`offers`]: answering offers ([`choose`]) and abandoning the run
//!   ([`abandon`]), both reached only through `SimCommand`s.
//! - [`RunProgress`]: what the run has drawn and picked so far.
//! - [`progress`]: the hourly serial driver of settlement-level steps, in
//!   every preset (`MilitarySet::Progress`).
//!
//! Developed worlds and the M2/M3 scenarios never call [`setup_run`]: their
//! `RunState.config` is `None`, and nothing here runs or draws for them.

pub mod offers;
pub mod start;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use serde::{Deserialize, Serialize};
use sim_agents::{CombatStats, Villagers};
use sim_core::{
    Allegiance, FactionId, OfferKind, RunState, SimEvent, SimEventKind, SimSeed, SpawnId, Stat,
    StateHash, StateHasher, post_offer,
};
use sim_economy::{Economy, SettlementEconomy, Stockpile};
use sim_world::RegionId;

use crate::components::{Equipment, Factions, Follower};
use crate::data::UnitStats;
use crate::spawn::{ArmySpec, soldier_stats, spawn_army};

pub use offers::{abandon, choose};
pub use start::{RunWorld, SiteInfo, StartDraws, StartTerrain, draw_start};

/// `assets/data/progression.ron`, embedded at compile time.
pub const DEFAULT_PROGRESSION_RON: &str = include_str!("../../../../assets/data/progression.ron");

/// Where a run starts (`start` in progression.ron, GP-21).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StartData {
    /// Monuments within this many tiles of the start (straight line) count as
    /// nearby.
    pub monument_radius: f32,
    /// The start has at least this many monuments nearby, when possible.
    pub min_monuments: u32,
    /// ... and at most this many.
    pub max_monuments: u32,
    /// Tiles from every settlement centre.
    pub min_settlement_distance: f32,
    /// Tiles from the start region's edge (coast or border).
    pub edge_distance: u32,
    /// Seeded spread (tiles) added to the distance from the rival's seat
    /// when ranking start tiles.
    pub jitter: f32,
    /// The kinds of start spot and their weights.
    pub terrain_weights: Vec<(StartTerrain, f32)>,
}

/// Followers of one kind in a band template.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BandGroup {
    /// How many.
    pub count: u32,
    /// Their fighting stats.
    pub unit: UnitStats,
    /// They start with weapons.
    pub armed: bool,
    /// They start with tools.
    pub tools: bool,
}

/// A band template (GP-27): who follows the champion at the start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BandData {
    /// Display name.
    pub name: String,
    /// Follower groups, spawned in order.
    pub groups: Vec<BandGroup>,
    /// Days of food in the baggage for the whole band.
    pub food_days: f32,
}

impl BandData {
    /// Followers in the band (the champion not counted).
    pub fn followers(&self) -> u32 {
        self.groups.iter().map(|g| g.count).sum()
    }
}

/// A trait (SIM-34).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraitData {
    /// Its index in `traits`.
    pub id: u16,
    /// Display name.
    pub name: String,
    /// Chronicle phrase ("Brand, who never gave ground").
    pub phrase: String,
    /// Stat changes while the holder lives.
    pub effects: Vec<(Stat, f32)>,
}

/// When a boon can be offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoonWhen {
    /// The starting boon, before the first tick (GP-27).
    Start,
    /// Holding 25% or 50% of a region (GP-24).
    Dominion,
}

/// What a boon or run modifier does. Append only.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    /// A stat modifier: the picker's faction for boons, everyone (the world
    /// row) for run modifiers. Folded into `FactionModifiers` (M4 step 4).
    Stat(Stat, f32),
    /// Days of food added to the champion's baggage.
    FoodDays(f32),
    /// A weapon for the champion and every follower.
    ArmBand,
    /// Goodwill in the settlement nearest the champion (GP-22, applied once
    /// goodwill exists, M4 step 3).
    Goodwill(f32),
    /// This fraction of the food in every storehouse is lost at the start.
    Stores(f32),
    /// Raider bands come this many times as often (the director, M4 step 7).
    RaiderPace(f32),
    /// The rival's levies and defence scale by this (M4 step 7).
    RivalStrength(f32),
    /// A band of this many sellswords for hire near the start (later step).
    Sellswords(u32),
}

/// A boon card (GP-24, GP-27).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoonData {
    /// Its index in `boons`.
    pub id: u16,
    /// Display name.
    pub name: String,
    /// What the card says.
    pub text: String,
    /// When it can be offered.
    pub when: BoonWhen,
    /// Relative chance to be offered.
    pub weight: f32,
    /// What it does.
    pub effects: Vec<Effect>,
}

/// A run modifier card (GP-27).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModifierData {
    /// Its index in `modifiers`.
    pub id: u16,
    /// Display name.
    pub name: String,
    /// What the card says.
    pub text: String,
    /// Cards sharing a group never come in the same run.
    pub group: Option<String>,
    /// Relative chance to be drawn.
    pub weight: f32,
    /// What it does.
    pub effects: Vec<Effect>,
}

/// The whole of progression.ron, as written. See docs/dev/DATA.md.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgressionData {
    /// The start spot.
    pub start: StartData,
    /// Band templates.
    pub bands: Vec<BandData>,
    /// Trait ids the champion's starting trait is drawn from.
    pub champion_traits: Vec<u16>,
    /// Every trait, by id.
    pub traits: Vec<TraitData>,
    /// Every boon, by id.
    pub boons: Vec<BoonData>,
    /// How many run modifiers are drawn: (min, max).
    pub modifiers_per_run: (u32, u32),
    /// Every run modifier, by id.
    pub modifiers: Vec<ModifierData>,
}

/// Progression data (GP-21, GP-27, SIM-34). Hashed as its RON text.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Progression {
    /// The data as written.
    pub data: ProgressionData,
}

impl Progression {
    /// Parses and validates progression.ron text.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        let data: ProgressionData = ron::from_str(text).map_err(|e| e.to_string())?;
        let p = Self { data };
        p.validate()?;
        Ok(p)
    }

    /// The shipped progression data.
    pub fn default_data() -> Self {
        Self::from_ron(DEFAULT_PROGRESSION_RON).expect("assets/data/progression.ron is valid")
    }

    /// Checks the data hangs together.
    pub fn validate(&self) -> Result<(), String> {
        let d = &self.data;
        let weight_ok = |w: f32| w.is_finite() && w > 0.0;
        let non_negative = |v: f32| v.is_finite() && v >= 0.0;
        let s = &d.start;
        if !weight_ok(s.monument_radius) || s.min_monuments > s.max_monuments {
            return Err("progression: start needs monument_radius > 0 and min <= max".into());
        }
        if !non_negative(s.min_settlement_distance) || !non_negative(s.jitter) {
            return Err("progression: start distances must be >= 0".into());
        }
        if s.terrain_weights.is_empty() || !s.terrain_weights.iter().all(|(_, w)| weight_ok(*w)) {
            return Err("progression: start needs terrain weights > 0".into());
        }
        if d.bands.is_empty() {
            return Err("progression: need at least one band".into());
        }
        for b in &d.bands {
            if b.groups.is_empty() || b.groups.iter().any(|g| g.count == 0) {
                return Err(format!("progression: band {:?} needs groups of 1+", b.name));
            }
            if !non_negative(b.food_days) {
                return Err(format!("progression: band {:?} food_days < 0", b.name));
            }
        }
        for (i, t) in d.traits.iter().enumerate() {
            if t.id as usize != i {
                return Err(format!(
                    "progression: trait {:?} has id {}, not {i}",
                    t.name, t.id
                ));
            }
        }
        if d.champion_traits.is_empty()
            || d.champion_traits
                .iter()
                .any(|&t| t as usize >= d.traits.len())
        {
            return Err("progression: champion_traits must name traits".into());
        }
        for (i, b) in d.boons.iter().enumerate() {
            if b.id as usize != i || !weight_ok(b.weight) {
                return Err(format!(
                    "progression: boon {:?}: id must be {i}, weight > 0",
                    b.name
                ));
            }
        }
        if d.boons.iter().filter(|b| b.when == BoonWhen::Start).count() < 3 {
            return Err("progression: need at least three Start boons".into());
        }
        for (i, m) in d.modifiers.iter().enumerate() {
            if m.id as usize != i || !weight_ok(m.weight) {
                return Err(format!(
                    "progression: modifier {:?}: id must be {i}, weight > 0",
                    m.name
                ));
            }
        }
        let (lo, hi) = d.modifiers_per_run;
        if lo > hi || hi as usize > d.modifiers.len() {
            return Err("progression: modifiers_per_run must be (min <= max <= cards)".into());
        }
        Ok(())
    }

    /// Boon card `card`.
    pub fn boon(&self, card: u16) -> Option<&BoonData> {
        self.data.boons.get(card as usize)
    }

    /// Trait `id`.
    pub fn trait_data(&self, id: u16) -> Option<&TraitData> {
        self.data.traits.get(id as usize)
    }

    /// Run modifier card `card`.
    pub fn modifier(&self, card: u16) -> Option<&ModifierData> {
        self.data.modifiers.get(card as usize)
    }
}

impl StateHash for Progression {
    fn hash_state(&self, h: &mut StateHasher) {
        h.bytes(
            ron::to_string(&self.data)
                .expect("progression data serialises")
                .as_bytes(),
        );
    }
}

/// The champion (SIM-35): the player's own figure. A marker on the person,
/// never in `Captain`, so it stays with the person if the army is handed on.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Champion;

impl StateHash for Champion {
    fn hash_state(&self, _h: &mut StateHasher) {}
}

/// A person's traits (SIM-34), as trait ids in the order they were gained.
#[derive(Component, Debug, Clone, PartialEq, Eq, Default)]
pub struct Traits(pub Vec<u16>);

impl StateHash for Traits {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.0.len() as u32);
        for &t in &self.0 {
            h.u16(t);
        }
    }
}

/// What a run has drawn and picked so far (GP-21, GP-27). Always present;
/// all defaults outside a run. Hashed.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct RunProgress {
    /// The start spot.
    pub start: IVec2,
    /// The start spot's kind.
    pub terrain: StartTerrain,
    /// The region the band starts in.
    pub start_region: RegionId,
    /// The rival's seat (a settlement's SpawnId); `None` outside a run.
    pub rival_seat: Option<u64>,
    /// The seat's centre.
    pub seat_center: IVec2,
    /// The rival's home region (the seat's).
    pub rival_region: RegionId,
    /// The band template drawn (index into `bands`).
    pub band: u16,
    /// Run modifier cards drawn, in draw order.
    pub modifiers: Vec<u16>,
    /// Boon cards picked, in pick order. Kept for the run (GP-24).
    pub boons: Vec<u16>,
    /// Goodwill gains waiting for the goodwill ledger (settlement SpawnId,
    /// amount), from the "Known" start boon. Applied in M4 step 3.
    pub pending_goodwill: Vec<(u64, f32)>,
    /// Whether the one succession of the run has happened (SIM-35, step 6).
    pub succession_used: bool,
}

impl StateHash for RunProgress {
    fn hash_state(&self, h: &mut StateHasher) {
        h.ivec2(self.start);
        h.u8(self.terrain as u8);
        h.u8(self.start_region);
        match self.rival_seat {
            Some(s) => {
                h.u8(1);
                h.u64(s);
            }
            None => h.u8(0),
        }
        h.ivec2(self.seat_center);
        h.u8(self.rival_region);
        h.u16(self.band);
        h.u32(self.modifiers.len() as u32);
        for &m in &self.modifiers {
            h.u16(m);
        }
        h.u32(self.boons.len() as u32);
        for &b in &self.boons {
            h.u16(b);
        }
        h.u32(self.pending_goodwill.len() as u32);
        for &(s, g) in &self.pending_goodwill {
            h.u64(s);
            h.f32(g);
        }
        h.bool(self.succession_used);
    }
}

/// What [`setup_run`] made.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunStart {
    /// The champion: the band's captain.
    pub champion: Entity,
    /// Where the band starts.
    pub start: IVec2,
    /// The start boon offer (GP-27), answered with `ChooseOffer`.
    pub offer: u64,
}

/// GP-21, GP-27: sets up a run before its first tick. Draws the start
/// ([`draw_start`]), records it in [`RunProgress`], seats the player at the
/// start spot, spawns the band with its [`Champion`] (carrying one trait),
/// applies one-off run modifiers, posts the start boon offer and reports
/// [`SimEventKind::RunStarted`].
///
/// Returns `None` (and changes nothing) outside a run, or if the world has
/// no settlements to draw from. Edits state directly and writes no player
/// commands, so a replay that re-runs it applies everything once.
pub fn setup_run(world: &mut World) -> Option<RunStart> {
    let _span = tracing::info_span!("sim_military::setup_run").entered();
    world.resource::<RunState>().config?;
    let data = world.resource::<Progression>().clone();
    let seed = *world.resource::<SimSeed>();
    let costs = world.resource::<Villagers>().travel_costs.clone();
    let draws = {
        let w = RunWorld::from_world(world, &costs)?;
        draw_start(seed, &data, &w)?
    };
    let band = data.data.bands[draws.band as usize].clone();
    world.insert_resource(RunProgress {
        start: draws.start,
        terrain: draws.terrain,
        start_region: draws.start_region,
        rival_seat: Some(draws.seat),
        seat_center: draws.seat_center,
        rival_region: draws.rival_region,
        band: draws.band,
        modifiers: draws.modifiers.clone(),
        boons: Vec::new(),
        pending_goodwill: Vec::new(),
        succession_used: false,
    });
    // Seat the player first, so spawn_army doesn't seat it at a snapped tile.
    world
        .resource_mut::<Factions>()
        .set_seat(FactionId::PLAYER, draws.start);
    let first = band.groups[0];
    let champion = spawn_army(
        world,
        &ArmySpec {
            food_days: band.food_days,
            armed: first.armed,
            tools: first.tools,
            soldier: Some(first.unit),
            ..ArmySpec::new(FactionId::PLAYER, draws.start, band.followers())
        },
    );
    outfit_groups(world, champion, &band);
    world
        .entity_mut(champion)
        .insert((Champion, Traits(vec![draws.champion_trait])));
    apply_start_modifiers(world, &data, &draws.modifiers);
    let offer = post_offer(
        world,
        FactionId::PLAYER,
        OfferKind::StartBoon,
        draws.boon_choices.to_vec(),
        None,
    );
    // draw_start drew the choices from `boons ^ 0`: the run's first offer.
    debug_assert_eq!(offer, 0, "the start boon is the run's first offer");
    let champion_id = world.get::<SpawnId>(champion).expect("spawned").0;
    world.write_message(
        SimEvent::new(SimEventKind::RunStarted, FactionId::PLAYER)
            .subject(champion_id)
            .other(draws.seat)
            .at(draws.start)
            .amount(draws.band as i32),
    );
    Some(RunStart {
        champion,
        start: draws.start,
        offer,
    })
}

/// Gives the band's followers their group's stats and gear, group by group
/// in SpawnId order (spawn_army spawned them all as the first group).
fn outfit_groups(world: &mut World, captain: Entity, band: &BandData) {
    let mut q = world.query::<(Entity, &SpawnId, &Follower)>();
    let mut members: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, f)| f.captain == captain)
        .map(|(e, id, _)| (*id, e))
        .collect();
    members.sort_unstable();
    let mut members = members.into_iter();
    for g in &band.groups {
        for (_, e) in members.by_ref().take(g.count as usize) {
            *world.get_mut::<CombatStats>(e).expect("soldier") = soldier_stats(g.unit);
            *world.get_mut::<Equipment>(e).expect("soldier") = Equipment {
                weapon: g.armed,
                tools: g.tools,
            };
        }
    }
}

/// GP-27: applies the one-off start effects of run modifier `cards`, in
/// draw order. Only `Stores` acts now; the other effects are read later
/// (`FactionModifiers` in M4 step 4, the director in step 7).
fn apply_start_modifiers(world: &mut World, data: &Progression, cards: &[u16]) {
    for &card in cards {
        let Some(m) = data.modifier(card) else {
            continue;
        };
        for effect in &m.effects {
            if let Effect::Stores(fraction) = *effect {
                spoil_stores(world, fraction);
            }
        }
    }
}

/// "Rats in the stores": every storehouse loses `fraction` of each food,
/// rounded down, in settlement SpawnId order.
fn spoil_stores(world: &mut World, fraction: f32) {
    let econ = world.resource::<Economy>().clone();
    let mut q = world.query::<(&SpawnId, &SettlementEconomy)>();
    let mut stores: Vec<(SpawnId, Entity)> =
        q.iter(world).map(|(id, se)| (*id, se.storehouse)).collect();
    stores.sort_unstable();
    let fraction = fraction.clamp(0.0, 1.0);
    for (_, store) in stores {
        let Some(mut stock) = world.get_mut::<Stockpile>(store) else {
            continue;
        };
        let foods: Vec<_> = stock
            .goods
            .iter()
            .filter(|(g, _)| econ.is_food(**g))
            .map(|(g, q)| (*g, *q))
            .collect();
        for (g, q) in foods {
            stock.take(g, (q as f32 * fraction).floor() as u32);
        }
    }
}

/// `MilitarySet::Progress` (SimSet::Needs, after `MilitarySet::Upkeep` and
/// `AgentSet::Standing`), once a game hour: the one exclusive driver of
/// settlement- and run-level steps (guide D7). It calls its sub-steps in a
/// fixed order, each in its own tracing span; daily sub-steps check the day
/// themselves and run-only ones check `RunState::in_run`. It runs in every
/// preset: conquest and revolts are general rules.
///
/// The order, with the M4 steps that fill it in:
/// 1. `veterans::end_battles` (step 6)
/// 2. `monuments::hold` (step 4)
/// 3. [`crate::conquest::revolts`] (daily)
/// 4. `wild::regrow` (daily, run; step 3)
/// 5. `drifters::draw` (daily, run; step 3)
/// 6. `requests::step` (run; step 3)
/// 7. invitations (run; step 3)
/// 8. `dominion::tally` (run; step 5)
/// 9. `champion::rank_ups` (run; step 6)
/// 10. `rebuild_modifiers` (step 4)
/// 11. `rival::clock` (run; step 7)
/// 12. `director::step` (run; step 7)
pub fn progress(world: &mut World) {
    let _span = tracing::info_span!("sim_military::progress").entered();
    crate::conquest::revolts(world);
}

/// The faction's champion (lowest SpawnId if, oddly, several).
pub fn champion_of(world: &mut World, faction: FactionId) -> Option<Entity> {
    let mut q = world.query_filtered::<(Entity, &SpawnId, &Allegiance), With<Champion>>();
    q.iter(world)
        .filter(|(_, _, a)| a.0 == faction)
        .min_by_key(|(_, id, _)| **id)
        .map(|(e, ..)| e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_data_is_valid() {
        let p = Progression::default_data();
        assert!(p.data.bands.len() >= 3, "band make-up varies");
        assert!(p.data.modifiers.len() >= 3, "modifiers vary");
        assert!(
            p.data
                .boons
                .iter()
                .filter(|b| b.when == BoonWhen::Start)
                .count()
                >= 3
        );
        for b in &p.data.bands {
            assert!(
                (8..=12).contains(&b.followers()),
                "{}: a band of about ten",
                b.name
            );
        }
    }

    #[test]
    fn rejects_bad_data() {
        let ok = Progression::default_data();
        let check = |edit: &dyn Fn(&mut ProgressionData)| {
            let mut d = ok.data.clone();
            edit(&mut d);
            Progression { data: d }.validate()
        };
        assert!(check(&|_| {}).is_ok());
        assert!(check(&|d| d.traits[1].id = 5).is_err(), "ids are indices");
        assert!(check(&|d| d.boons.retain(|b| b.when == BoonWhen::Dominion)).is_err());
        assert!(check(&|d| d.champion_traits.push(99)).is_err());
        assert!(check(&|d| d.modifiers_per_run = (2, 1)).is_err());
        assert!(check(&|d| d.bands[0].groups.clear()).is_err());
        assert!(check(&|d| d.start.min_monuments = 9).is_err());
    }

    /// A run app on the run island, set up (the plugins `sim_headless`
    /// adds, minus the empty M5 ones).
    fn run_app(seed: u64) -> (bevy_app::App, RunStart) {
        use sim_core::{RunConfig, SimCorePlugin, SimStepper};
        use sim_world::{WorldGenSettings, WorldPreset};
        let mut app = bevy_app::App::new();
        app.add_plugins(SimCorePlugin {
            seed,
            run: Some(RunConfig::default()),
        })
        .add_plugins(sim_world::WorldPlugin {
            settings: WorldGenSettings::preset(WorldPreset::Run),
            threads: sim_world::worldgen::default_threads(),
        })
        .add_plugins(sim_economy::EconomyPlugin)
        .add_plugins(sim_agents::AgentsPlugin { population: None })
        .add_plugins(crate::MilitaryPlugin);
        app.prepare_sim();
        let start = setup_run(app.world_mut()).expect("a run start");
        (app, start)
    }

    fn command(app: &mut bevy_app::App, issuer: FactionId, kind: sim_core::CommandKind) {
        use sim_core::SimStepper;
        app.world_mut()
            .write_message(sim_core::SimCommand::player(issuer, kind));
        app.step(1);
    }

    /// GP-21, GP-27: setup spawns the drawn band under a champion with one
    /// trait and posts three start boons; the pick applies once, and a
    /// second pick is refused. Abandoning is the player's alone and stops
    /// the clock.
    #[test]
    fn a_run_is_set_up_its_boon_picked_once_and_abandoned() {
        use sim_core::{
            CommandKind, Offers, RunOutcome, SimEventKind, SimTick, compute_state_hash,
        };
        let (mut app, start) = run_app(42);
        let world = app.world_mut();
        let progress = world.resource::<RunProgress>().clone();
        let data = world.resource::<Progression>().clone();
        let band = &data.data.bands[progress.band as usize];
        assert!(world.resource::<RunState>().config.is_some());
        assert_eq!(champion_of(world, FactionId::PLAYER), Some(start.champion));
        assert_eq!(
            world.get::<Traits>(start.champion).map(|t| t.0.len()),
            Some(1)
        );
        let mut q = world.query::<&Follower>();
        let followers = q
            .iter(world)
            .filter(|f| f.captain == start.champion)
            .count();
        assert_eq!(followers as u32, band.followers());
        assert_eq!(
            world.resource::<Factions>().seat(FactionId::PLAYER),
            Some(start.start)
        );

        // GP-27: the band's make-up. Followers in SpawnId order take their
        // group's stats and gear; the champion keeps army.ron's captain
        // stats and the first group's gear. Checked before any tick, while
        // hunger and upkeep have changed nothing.
        assert!(
            band.groups.len() > 1,
            "seed 42 draws a mixed band, not {:?}",
            band.name
        );
        let mut q = world.query::<(&SpawnId, &Follower, &CombatStats, &Equipment)>();
        let mut members: Vec<(SpawnId, CombatStats, Equipment)> = q
            .iter(world)
            .filter(|(_, f, ..)| f.captain == start.champion)
            .map(|(id, _, s, e)| (*id, *s, *e))
            .collect();
        members.sort_unstable_by_key(|m| m.0);
        let want = band
            .groups
            .iter()
            .flat_map(|g| std::iter::repeat_n(g, g.count as usize));
        assert_eq!(members.len(), band.followers() as usize);
        for ((id, stats, gear), g) in members.iter().zip(want) {
            assert_eq!(*stats, soldier_stats(g.unit), "{id:?} of {}", band.name);
            assert_eq!(
                *gear,
                Equipment {
                    weapon: g.armed,
                    tools: g.tools
                },
                "{id:?} of {}",
                band.name
            );
        }
        let captain = world.resource::<crate::Army>().data.captain;
        assert_eq!(
            world.get::<CombatStats>(start.champion).copied(),
            Some(soldier_stats(captain))
        );
        assert_eq!(
            world.get::<Equipment>(start.champion).copied(),
            Some(Equipment {
                weapon: band.groups[0].armed,
                tools: band.groups[0].tools
            })
        );

        // GP-27 "Rats in the stores": the card's one-off effect takes the
        // fraction (rounded down) of each food from every storehouse, and
        // leaves other goods and homes alone.
        let (rats, fraction) = data
            .data
            .modifiers
            .iter()
            .find_map(|m| {
                m.effects.iter().find_map(|e| match *e {
                    Effect::Stores(f) => Some((m.id, f)),
                    _ => None,
                })
            })
            .expect("a Stores run modifier");
        let econ = world.resource::<Economy>().clone();
        let stores = |world: &mut World| {
            let mut q = world.query::<(&SpawnId, &SettlementEconomy)>();
            let mut v: Vec<(SpawnId, Entity, Option<Entity>)> = q
                .iter(world)
                .map(|(id, se)| (*id, se.storehouse, se.homes.first().copied()))
                .collect();
            v.sort_unstable();
            v.into_iter()
                .map(|(id, store, home)| {
                    let get = |e: Option<Entity>| {
                        e.and_then(|e| world.get::<Stockpile>(e).cloned())
                            .unwrap_or_default()
                    };
                    (id, get(Some(store)), get(home))
                })
                .collect::<Vec<_>>()
        };
        let before = stores(world);
        apply_start_modifiers(world, &data, &[rats]);
        let after = stores(world);
        assert_eq!(before.len(), after.len());
        let mut lost = 0;
        for ((id, s0, h0), (_, s1, h1)) in before.iter().zip(&after) {
            assert_eq!(h1, h0, "settlement {id:?}: homes keep their food");
            for (&g, &q) in &s0.goods {
                let want = if econ.is_food(g) {
                    q - (q as f32 * fraction).floor() as u32
                } else {
                    q
                };
                assert_eq!(s1.get(g), want, "settlement {id:?}, good {g:?}: {q} before");
                lost += q - s1.get(g);
            }
        }
        assert!(lost > 0, "some storehouse lost food");
        let offer = world
            .resource::<Offers>()
            .get(start.offer)
            .cloned()
            .unwrap();
        assert_eq!((offer.kind, offer.choices.len()), (OfferKind::StartBoon, 3));

        // Pick the "Well fed" boon if offered, else the first.
        let choice = offer
            .choices
            .iter()
            .position(|&c| data.boon(c).unwrap().name == "Well fed")
            .unwrap_or(0) as u8;
        let food = |app: &bevy_app::App| {
            app.world()
                .get::<Stockpile>(start.champion)
                .map_or(0, |s| s.total())
        };
        let before = food(&app);
        let pick = CommandKind::ChooseOffer {
            offer: start.offer,
            choice,
        };
        command(&mut app, FactionId::PLAYER, pick.clone());
        let card = offer.choices[choice as usize];
        assert_eq!(app.world().resource::<RunProgress>().boons, vec![card]);
        assert!(app.world().resource::<Offers>().pending.is_empty());
        if let [Effect::FoodDays(days)] = data.boon(card).unwrap().effects[..] {
            // Exactly the grain for `days` days of the whole band, once
            // (soldiers may already eat a little of it this tick).
            let world = app.world();
            let econ = world.resource::<Economy>();
            let grain = *econ.foods_by_value().last().expect("a food");
            let need = world.resource::<Villagers>().data.needs.hunger_per_day
                * (band.followers() + 1) as f32
                * days;
            let qty = (need / econ.food_value(grain)).ceil() as u32;
            let gained = food(&app) as i64 - before as i64;
            assert!(
                gained > 0 && gained <= qty as i64,
                "{days} more days of food: {gained} units, {qty} expected"
            );
        }
        let events = app
            .world()
            .resource::<sim_core::Chronicle>()
            .entries()
            .to_vec();
        assert_eq!(events[0].event.kind, SimEventKind::RunStarted);
        assert!(
            events
                .iter()
                .any(|e| e.event.kind == SimEventKind::OfferChosen)
        );

        let rejected = |app: &bevy_app::App| {
            app.world()
                .resource::<crate::ArmyStats>()
                .get(FactionId::PLAYER)
                .rejected_commands
        };
        command(&mut app, FactionId::PLAYER, pick);
        assert_eq!(rejected(&app), 1, "a second pick is refused");
        assert_eq!(app.world().resource::<RunProgress>().boons, vec![card]);

        command(&mut app, FactionId::RAIDERS, CommandKind::AbandonRun);
        assert_eq!(app.world().resource::<RunState>().outcome, None);
        command(&mut app, FactionId::PLAYER, CommandKind::AbandonRun);
        let state = *app.world().resource::<RunState>();
        assert_eq!(state.outcome, Some(RunOutcome::Abandoned));
        let tick = app.world().resource::<SimTick>().0;
        let hash = compute_state_hash(app.world_mut());
        sim_core::SimStepper::step(&mut app, 20);
        assert_eq!(
            app.world().resource::<SimTick>().0,
            tick,
            "the clock stopped"
        );
        assert_eq!(compute_state_hash(app.world_mut()), hash);
    }
}
