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
//! - [`dominion`]: the hourly dominion tally, its boon offers (GP-24) and
//!   captain slots from the population held (GP-25).
//! - [`progress`]: the hourly serial driver of settlement-level steps, in
//!   every preset (`MilitarySet::Progress`).
//!
//! Developed worlds and the M2/M3 scenarios never call [`setup_run`]: their
//! `RunState.config` is `None`, and nothing here runs or draws for them.

pub mod dominion;
pub mod offers;
pub mod start;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use serde::{Deserialize, Serialize};
use sim_agents::{CombatStats, Deed, Villagers};
use sim_core::{
    Allegiance, FactionId, FactionModifiers, ModifierSource, OfferKind, RunState, SimEvent,
    SimEventKind, SimSeed, SpawnId, Stat, StateHash, StateHasher, post_offer,
};
use sim_economy::{Economy, SettlementEconomy, Stockpile};
use sim_world::{Monument, MonumentKind, RegionId};

use crate::components::{Equipment, Factions, Follower};
use crate::data::UnitStats;
use crate::spawn::{ArmySpec, soldier_stats, spawn_army};

pub use dominion::{DominionData, Holding, Holdings, RegionTally, captain_slots};
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
    /// Goodwill in the populated settlement nearest the champion (GP-22),
    /// posted to the goodwill ledger, so word of it spreads.
    Goodwill(f32),
    /// This fraction of the food in every storehouse is lost at the start.
    Stores(f32),
    /// Raider bands come this many times as often (the director, M4 step 7).
    RaiderPace(f32),
    /// The rival's levies and defence scale by this (M4 step 7).
    RivalStrength(f32),
    /// A band of this many sellswords for hire near the start (later step).
    Sellswords(u32),
    /// This many more captain slots for the picker (GP-25), from the pick
    /// on (see [`captain_slots`]).
    CaptainSlots(u8),
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
    /// GP-24: what the player holds raises the chance of a Dominion boon:
    /// its weight is multiplied by `1 + sum(w x amount)` over these
    /// `(holding, w)` pairs (see [`dominion::boon_weight`]). Optional;
    /// Start boons leave it out.
    #[serde(default)]
    pub favour: Vec<(Holding, f32)>,
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

/// Goodwill deltas per deed (GP-22): `goodwill_deeds` in progression.ron.
/// Each is the change (-1..1) at the settlement where the deed happened;
/// villagers.ron `goodwill` spreads a fraction of it to its neighbours.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GoodwillDeeds {
    /// Food bought fairly, with goods (Trade, passive gathering).
    pub trade: f32,
    /// Arms bought with food (passive Equip).
    pub equip: f32,
    /// Food taken from the reserve by a hungry neutral army.
    pub seize: f32,
    /// Food plundered by an aggressive army.
    pub plunder: f32,
    /// Provoking the settlement into arms.
    pub provoke: f32,
    /// Each of its militia killed (toward the faction they fought).
    pub killing: f32,
    /// A request answered (M4 step 3b).
    pub request: f32,
}

impl GoodwillDeeds {
    /// The delta for `deed`. [`Deed::Boon`] carries its own amount (the
    /// boon's `Goodwill` effect), so it is 0 here.
    pub fn delta(&self, deed: Deed) -> f32 {
        match deed {
            Deed::Trade => self.trade,
            Deed::Equip => self.equip,
            Deed::Seize => self.seize,
            Deed::Plunder => self.plunder,
            Deed::Provoke => self.provoke,
            Deed::Killing => self.killing,
            Deed::Request => self.request,
            Deed::Boon => 0.0,
        }
    }
}

/// Wild foraging (SIM-38): `forage` in progression.ron. See
/// [`crate::wild`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ForageData {
    /// Food value a full patch holds per grass or forest tile.
    pub per_tile: f32,
    /// Fraction (0..1) of a patch's capacity that grows back each day.
    pub regrow_per_day: f32,
    /// A foraging band harvests once every this many ticks.
    pub every_ticks: u64,
    /// Food value each member (captain included) gathers per harvest,
    /// before the faction's `ForageYield` modifier.
    pub per_member: f32,
    /// The largest radius (tiles) a `Forage` order may have.
    pub max_radius: u16,
    /// The radius the game's Forage button uses.
    pub default_radius: u16,
}

/// Village requests (GP-22): `requests` in progression.ron. See
/// [`crate::requests`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RequestData {
    /// A settlement asks for food when its `FoodSecurity.days` (storehouse
    /// and homes) is below this ...
    pub food_days: f32,
    /// ... at this many hourly samples in a row.
    pub low_samples: u8,
    /// A BringFood request asks for this many days of the settlement's
    /// daily need (food value) ...
    pub need_days: f32,
    /// ... but never more than this much food value.
    pub max_need: f32,
    /// Game hours (100 ticks each) a request stays open.
    pub deadline_hours: f32,
    /// Game hours after a request ends (done or failed) before the same
    /// settlement asks again.
    pub cooldown_hours: f32,
    /// DriveOff: game hours with no hostile army within the threat radius
    /// before the danger counts as gone.
    pub clear_hours: f32,
    /// The reward goes to the nearest player captain within this many
    /// tiles of the settlement's centre.
    pub reward_radius: f32,
    /// DriveOff: the settlement promises this many days of its own daily
    /// need in food (times a seeded 0.5..1) ...
    pub reward_food_days: f32,
    /// ... but at most this much food value.
    pub reward_max_food: f32,
    /// At most this many units of one non-food good the armies value
    /// (army.ron `trade_value`), drawn from what the storehouse holds.
    pub reward_goods: u32,
    /// Volunteers promised: a seeded count in (min, max).
    pub volunteers: (u32, u32),
    /// ClearCamp (M4 step 4): a guarded monument within this many tiles of
    /// a settlement's centre may be the subject of its request.
    pub camp_radius: f32,
}

/// Invitations (GP-22): `invitations` in progression.ron. See
/// [`crate::invitations`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InvitationData {
    /// A settlement whose goodwill toward the player is at least this ...
    pub goodwill: f32,
    /// ... with a player captain within this many tiles of its centre ...
    pub radius: f32,
    /// ... and whose owner is weak: its residents' mean loyalty below this
    /// (and its garrison smaller than the player's soldiers nearby), or it
    /// has none, offers lordship.
    pub weak_loyalty: f32,
    /// Game hours the offer stands.
    pub expiry_hours: f32,
    /// Days after an invitation is answered or lapses before the same
    /// settlement offers again.
    pub cooldown_days: f32,
}

/// Drifters (GP-22): `drifters` in progression.ron. See
/// [`crate::drifters`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DriftersData {
    /// Settlements whose centre is within this many tiles of a player
    /// captain ...
    pub radius: f32,
    /// ... and whose goodwill toward the player is at least this send
    /// volunteers, once a day.
    pub goodwill: f32,
    /// A seeded count in (min, max) per settlement ...
    pub per_settlement: (u32, u32),
    /// ... and at most this many per captain per day.
    pub max_per_day: u32,
}

/// One monument kind's bonus (GP-23): an entry of `monuments.kinds` in
/// progression.ron. See [`rebuild_modifiers`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MonumentBonus {
    /// The kind.
    pub kind: MonumentKind,
    /// The stat it raises.
    pub stat: Stat,
    /// Added to `stat` per monument of this kind held ...
    pub bonus: f32,
    /// ... up to this much in all, per faction.
    pub cap: f32,
    /// Added (beyond the cap) for each region in which the faction holds
    /// every monument of this kind (at least `monuments.set_min` of them).
    pub set_bonus: f32,
}

/// Who guards a monument (GP-23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardKind {
    /// A raider camp: raiders (`FactionId::RAIDERS`, without the
    /// `Raider` AI) who stay put.
    RaiderCamp,
    /// A beast lair (`FactionId::BEASTS`).
    Beasts,
}

impl GuardKind {
    /// The faction the guards fight for.
    pub fn faction(self) -> FactionId {
        match self {
            GuardKind::RaiderCamp => FactionId::RAIDERS,
            GuardKind::Beasts => FactionId::BEASTS,
        }
    }
}

/// A kind of monument guard: an entry of `monuments.guards`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GuardData {
    /// Raiders or beasts.
    pub kind: GuardKind,
    /// The chance (0..1) that a monument gets this guard. The chances of
    /// all entries add up to at most 1; the rest stay unguarded.
    pub chance: f32,
    /// Followers (besides the captain) at the start spot ...
    pub base: u32,
    /// ... plus this many per tile between the start and the monument ...
    pub per_tile: f32,
    /// ... but never more than this.
    pub max: u32,
    /// The guards carry weapons.
    pub armed: bool,
}

/// Monuments (GP-23): `monuments` in progression.ron. See
/// [`crate::monuments`] and [`rebuild_modifiers`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonumentData {
    /// Soldiers within this many tiles of a monument stand at it.
    pub hold_radius: f32,
    /// One faction's soldiers alone at a monument for this many game hours
    /// take it.
    pub hold_hours: f32,
    /// A held monument drifts back to nobody after this many days without
    /// its holder's soldiers at it ...
    pub drift_days: f32,
    /// ... unless it is within this many tiles of a settlement its holder
    /// owns.
    pub drift_distance: f32,
    /// A set bonus needs at least this many monuments of the kind in the
    /// region.
    pub set_min: u32,
    /// The bonus of each kind.
    pub kinds: Vec<MonumentBonus>,
    /// Who may guard a monument.
    pub guards: Vec<GuardData>,
}

impl MonumentData {
    /// The bonus of `kind`, if listed.
    pub fn bonus(&self, kind: MonumentKind) -> Option<&MonumentBonus> {
        self.kinds.iter().find(|b| b.kind == kind)
    }
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
    /// Goodwill deltas per deed (GP-22).
    pub goodwill_deeds: GoodwillDeeds,
    /// Wild foraging (SIM-38).
    pub forage: ForageData,
    /// Village requests (GP-22).
    pub requests: RequestData,
    /// Invitations (GP-22).
    pub invitations: InvitationData,
    /// Drifters (GP-22).
    pub drifters: DriftersData,
    /// Monuments: holding, guards and bonuses (GP-23).
    pub monuments: MonumentData,
    /// Dominion boons and captain slots (GP-24, GP-25).
    pub dominion: DominionData,
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
        for b in &d.boons {
            if b.favour.iter().any(|(_, w)| !non_negative(*w)) {
                return Err(format!(
                    "progression: boon {:?}: favour must be >= 0",
                    b.name
                ));
            }
        }
        d.dominion.validate()?;
        if !d.dominion.thresholds.is_empty()
            && d.boons
                .iter()
                .filter(|b| b.when == BoonWhen::Dominion)
                .count()
                < 3
        {
            return Err(
                "progression: dominion thresholds need at least three Dominion boons".into(),
            );
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
        let g = &d.goodwill_deeds;
        if [
            g.trade, g.equip, g.seize, g.plunder, g.provoke, g.killing, g.request,
        ]
        .iter()
        .any(|v| !(-1.0..=1.0).contains(v))
        {
            return Err("progression: goodwill_deeds must be in -1..1".into());
        }
        let f = &d.forage;
        if !non_negative(f.per_tile) || !non_negative(f.per_member) {
            return Err("progression: forage per_tile and per_member must be >= 0".into());
        }
        if !(0.0..=1.0).contains(&f.regrow_per_day) {
            return Err("progression: forage.regrow_per_day must be in 0..1".into());
        }
        if f.every_ticks == 0 || f.default_radius == 0 || f.default_radius > f.max_radius {
            return Err(
                "progression: forage needs every_ticks > 0 and 0 < default_radius <= max_radius"
                    .into(),
            );
        }
        let r = &d.requests;
        if [
            r.food_days,
            r.need_days,
            r.max_need,
            r.deadline_hours,
            r.cooldown_hours,
            r.clear_hours,
            r.reward_radius,
            r.reward_food_days,
            r.reward_max_food,
            r.camp_radius,
        ]
        .iter()
        .any(|v| !non_negative(*v))
        {
            return Err("progression: requests values must be >= 0".into());
        }
        if r.low_samples == 0 || r.deadline_hours <= 0.0 || r.volunteers.0 > r.volunteers.1 {
            return Err(
                "progression: requests need low_samples > 0, deadline_hours > 0 and volunteers (min <= max)"
                    .into(),
            );
        }
        let i = &d.invitations;
        if !(-1.0..=1.0).contains(&i.goodwill)
            || !non_negative(i.radius)
            || !(0.0..=1.0).contains(&i.weak_loyalty)
            || !weight_ok(i.expiry_hours)
            || !non_negative(i.cooldown_days)
        {
            return Err(
                "progression: invitations need goodwill and weak_loyalty in range, radius and cooldown_days >= 0, expiry_hours > 0"
                    .into(),
            );
        }
        let dr = &d.drifters;
        if !non_negative(dr.radius)
            || !(-1.0..=1.0).contains(&dr.goodwill)
            || dr.per_settlement.0 > dr.per_settlement.1
        {
            return Err(
                "progression: drifters need radius >= 0, goodwill in -1..1 and per_settlement (min <= max)"
                    .into(),
            );
        }
        let m = &d.monuments;
        if !weight_ok(m.hold_radius)
            || !non_negative(m.hold_hours)
            || !non_negative(m.drift_days)
            || !non_negative(m.drift_distance)
            || m.set_min == 0
        {
            return Err(
                "progression: monuments need hold_radius > 0, hold_hours, drift_days and drift_distance >= 0, set_min > 0"
                    .into(),
            );
        }
        for (i, b) in m.kinds.iter().enumerate() {
            if m.kinds[..i].iter().any(|o| o.kind == b.kind) {
                return Err(format!(
                    "progression: monument kind {:?} listed twice",
                    b.kind
                ));
            }
            if !non_negative(b.bonus) || !non_negative(b.cap) || !non_negative(b.set_bonus) {
                return Err(format!(
                    "progression: monument kind {:?} needs bonus, cap and set_bonus >= 0",
                    b.kind
                ));
            }
        }
        let mut chance = 0.0;
        for g in &m.guards {
            if !(0.0..=1.0).contains(&g.chance) || !non_negative(g.per_tile) || g.base > g.max {
                return Err(format!(
                    "progression: guard {:?} needs chance in 0..1, per_tile >= 0 and base <= max",
                    g.kind
                ));
            }
            chance += g.chance;
        }
        if chance > 1.0 + 1e-6 {
            return Err("progression: monument guard chances add up to more than 1".into());
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
    /// Whether the one succession of the run has happened (SIM-35, step 6).
    pub succession_used: bool,
    /// GP-22: `(settlement SpawnId, tick)` pairs, sorted by settlement: no
    /// new invitation from that settlement before the tick (set when an
    /// invitation is accepted, declined or lapses).
    pub invite_cooldowns: Vec<(u64, u64)>,
    /// Drifters who have joined the player's bands this run (GP-22).
    pub drifters: u32,
    /// GP-24: the dominion tally, indexed by region (`WorldMap::region_count`
    /// entries), refreshed hourly by [`dominion::tally`]. Empty before the
    /// first tally.
    pub dominion: Vec<RegionTally>,
    /// GP-25: the player's captain slots ([`captain_slots`]), stored hourly
    /// (and after a boon pick) for the UI. `AppointCaptain` checks the live
    /// value.
    pub captain_slots: u32,
}

impl RunProgress {
    /// The tick before which settlement `s` (a SpawnId) offers no new
    /// invitation; 0 if none was ever set.
    pub fn invite_cooldown(&self, s: u64) -> u64 {
        self.invite_cooldowns
            .binary_search_by_key(&s, |c| c.0)
            .map_or(0, |i| self.invite_cooldowns[i].1)
    }

    /// Sets settlement `s`'s invitation cooldown to `until`, keeping the
    /// list sorted.
    pub fn set_invite_cooldown(&mut self, s: u64, until: u64) {
        match self.invite_cooldowns.binary_search_by_key(&s, |c| c.0) {
            Ok(i) => self.invite_cooldowns[i].1 = until,
            Err(i) => self.invite_cooldowns.insert(i, (s, until)),
        }
    }
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
        h.bool(self.succession_used);
        h.u32(self.invite_cooldowns.len() as u32);
        for &(s, until) in &self.invite_cooldowns {
            h.u64(s);
            h.u64(until);
        }
        h.u32(self.drifters);
        h.u32(self.dominion.len() as u32);
        for t in &self.dominion {
            h.u32(t.total);
            h.u32(t.held_by_player);
            h.u8(t.flags);
        }
        h.u32(self.captain_slots);
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
        succession_used: false,
        invite_cooldowns: Vec::new(),
        drifters: 0,
        dominion: Vec::new(),
        captain_slots: 0,
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
    // GP-23: every monument gets a holder; some get a guard camp.
    crate::monuments::setup(world, draws.start);
    // SIM-38: the island's wild food, full at the start.
    crate::wild::ensure_built(world);
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
/// 2. [`crate::monuments::hold`]
/// 3. [`crate::conquest::revolts`] (daily)
/// 4. [`crate::wild::regrow`] (daily, run)
/// 5. [`crate::drifters::draw`] (daily, run)
/// 6. [`crate::requests::step`] (run)
/// 7. [`crate::invitations::step`] (run)
/// 8. [`dominion::tally`] (run)
/// 9. `champion::rank_ups` (run; step 6)
/// 10. [`rebuild_modifiers`], then [`dominion::refresh_slots`] (run), which
///     reads the rebuilt modifiers
/// 11. `rival::clock` (run; step 7)
/// 12. `director::step` (run; step 7)
pub fn progress(world: &mut World) {
    let _span = tracing::info_span!("sim_military::progress").entered();
    crate::monuments::hold(world);
    crate::conquest::revolts(world);
    crate::wild::regrow(world);
    crate::drifters::draw(world);
    crate::requests::step(world);
    crate::invitations::step(world);
    dominion::tally(world);
    rebuild_modifiers(world);
    dominion::refresh_slots(world);
}

/// GP-23, GP-24, SIM-34: rebuilds [`FactionModifiers`] from scratch, once a
/// game hour (slot 10 of [`progress`]; tick 0 counts, so the start boon
/// picked on the first tick is folded in at once). Span
/// `sim_military::rebuild_modifiers`. Contributions, in this order:
///
/// 1. Held monuments, per faction (never faction 0, nobody) and kind:
///    `min(count x bonus, cap)` from progression.ron `monuments.kinds`,
///    then a set bonus for each region in which the faction holds every
///    monument of the kind (at least `monuments.set_min` of them) - see
///    [`monument_modifiers`].
/// 2. The player's boons (`RunProgress::boons`, pick order): their `Stat`
///    effects.
/// 3. Every champion's traits (SpawnId order), for the champion's faction.
/// 4. Run modifiers (`RunProgress::modifiers`, draw order): their `Stat`
///    effects go to the world row, which applies to every faction.
///
/// Nothing else writes the resource; consumers read `mult` or `value` at
/// the point of use. Outside a run every list is empty and the modifiers
/// stay exactly zero (every multiplier 1.0).
pub fn rebuild_modifiers(world: &mut World) {
    let _span = tracing::info_span!("sim_military::rebuild_modifiers").entered();
    let data = world.resource::<Progression>().clone();
    let progress = world.resource::<RunProgress>().clone();
    let mut q = world.query::<(&SpawnId, &Monument, &crate::monuments::Holder)>();
    let mut held: Vec<(SpawnId, MonumentKind, RegionId, FactionId)> = q
        .iter(world)
        .map(|(id, m, h)| (*id, m.kind, m.region, h.faction))
        .collect();
    held.sort_unstable_by_key(|h| h.0);
    let held: Vec<(MonumentKind, RegionId, FactionId)> =
        held.into_iter().map(|(_, k, r, f)| (k, r, f)).collect();
    let mut mods = FactionModifiers::default();
    monument_modifiers(&mut mods, &held, &data.data.monuments);
    for &card in &progress.boons {
        let Some(b) = data.boon(card) else { continue };
        for e in &b.effects {
            if let Effect::Stat(stat, v) = *e {
                mods.add(FactionId::PLAYER, stat, ModifierSource::Boon { card }, v);
            }
        }
    }
    let mut q = world.query_filtered::<(&SpawnId, &Allegiance, &Traits), With<Champion>>();
    let mut champions: Vec<(SpawnId, FactionId, Vec<u16>)> = q
        .iter(world)
        .map(|(id, a, t)| (*id, a.0, t.0.clone()))
        .collect();
    champions.sort_unstable_by_key(|c| c.0);
    for (_, faction, traits) in champions {
        for id in traits {
            let Some(t) = data.trait_data(id) else {
                continue;
            };
            for &(stat, v) in &t.effects {
                mods.add(faction, stat, ModifierSource::Trait { id }, v);
            }
        }
    }
    for &card in &progress.modifiers {
        let Some(m) = data.modifier(card) else {
            continue;
        };
        for e in &m.effects {
            if let Effect::Stat(stat, v) = *e {
                mods.add_world(stat, ModifierSource::RunModifier { card }, v);
            }
        }
    }
    if *world.resource::<FactionModifiers>() != mods {
        *world.resource_mut::<FactionModifiers>() = mods;
    }
}

/// GP-23: adds the bonuses of the monuments in `held` (kind, region,
/// holder; in SpawnId order) to `mods`. For each holding faction (never
/// faction 0) in id order, then each kind in id order: `min(count x bonus,
/// cap)` as [`ModifierSource::Monuments`]. Then for each kind and region
/// in id order with at least `set_min` monuments of the kind, all held by
/// one faction: `set_bonus` for it as [`ModifierSource::SetBonus`]. Kinds
/// without a `monuments.kinds` entry give nothing.
pub fn monument_modifiers(
    mods: &mut FactionModifiers,
    held: &[(MonumentKind, RegionId, FactionId)],
    data: &MonumentData,
) {
    let mut factions: Vec<FactionId> = held
        .iter()
        .map(|h| h.2)
        .filter(|f| *f != FactionId::INDEPENDENT)
        .collect();
    factions.sort_unstable();
    factions.dedup();
    for &f in &factions {
        for kind in MonumentKind::ALL {
            let Some(b) = data.bonus(kind) else { continue };
            let count = held.iter().filter(|h| h.0 == kind && h.2 == f).count();
            if count == 0 {
                continue;
            }
            let v = (count as f32 * b.bonus).min(b.cap);
            mods.add(f, b.stat, ModifierSource::Monuments { kind: kind as u8 }, v);
        }
    }
    let mut regions: Vec<RegionId> = held.iter().map(|h| h.1).collect();
    regions.sort_unstable();
    regions.dedup();
    for kind in MonumentKind::ALL {
        let Some(b) = data.bonus(kind) else { continue };
        for &region in &regions {
            let mut set = held.iter().filter(|h| h.0 == kind && h.1 == region);
            let Some(first) = set.clone().next() else {
                continue;
            };
            let holder = first.2;
            if holder == FactionId::INDEPENDENT
                || b.set_bonus == 0.0
                || (set.clone().count() as u32) < data.set_min
                || !set.all(|h| h.2 == holder)
            {
                continue;
            }
            mods.add(
                holder,
                b.stat,
                ModifierSource::SetBonus {
                    kind: kind as u8,
                    region,
                },
                b.set_bonus,
            );
        }
    }
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
pub(crate) mod tests {
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
        // GP-25: the slot cap leaves army.ron's hard limit (which counts
        // the champion) room for the champion.
        let army = crate::Army::default_data(
            &sim_core::Calendar::default_data(),
            &Economy::default_data(),
        );
        assert!(p.data.dominion.cap < army.data.max_captains);
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
        assert!(check(&|d| d.goodwill_deeds.plunder = -1.5).is_err());
        assert!(check(&|d| d.forage.every_ticks = 0).is_err());
        assert!(check(&|d| d.forage.default_radius = d.forage.max_radius + 1).is_err());
        assert!(check(&|d| d.forage.regrow_per_day = 2.0).is_err());
        assert!(check(&|d| d.requests.low_samples = 0).is_err());
        assert!(check(&|d| d.requests.volunteers = (3, 1)).is_err());
        assert!(check(&|d| d.requests.max_need = -1.0).is_err());
        assert!(check(&|d| d.invitations.goodwill = 1.5).is_err());
        assert!(check(&|d| d.invitations.expiry_hours = 0.0).is_err());
        assert!(check(&|d| d.drifters.per_settlement = (2, 1)).is_err());
        assert!(check(&|d| d.requests.camp_radius = -1.0).is_err());
        assert!(check(&|d| d.monuments.hold_radius = 0.0).is_err());
        assert!(check(&|d| d.monuments.set_min = 0).is_err());
        assert!(check(&|d| d.monuments.kinds[0].cap = -0.1).is_err());
        assert!(
            check(&|d| {
                let k = d.monuments.kinds[0];
                d.monuments.kinds.push(k);
            })
            .is_err()
        );
        assert!(check(&|d| d.monuments.guards[0].chance = 0.9).is_err());
        assert!(check(&|d| d.monuments.guards[0].base = d.monuments.guards[0].max + 1).is_err());
        assert!(check(&|d| d.dominion.thresholds = vec![0.5, 0.25]).is_err());
        assert!(check(&|d| d.dominion.per_captain = 0).is_err());
        assert!(check(&|d| d.dominion.first_village_slots = d.dominion.cap + 1).is_err());
        assert!(check(&|d| d.boons[6].favour = vec![(Holding::Farms, -1.0)]).is_err());
        assert!(
            check(&|d| d.boons.retain(|b| b.when == BoonWhen::Start)).is_err(),
            "thresholds need Dominion boons"
        );
        assert!(
            check(&|d| {
                d.boons.retain(|b| b.when == BoonWhen::Start);
                d.dominion.thresholds.clear();
            })
            .is_ok(),
            "no thresholds, no Dominion boons needed"
        );
    }

    /// GP-23: the shipped monument data lists every kind once, and a
    /// guarded monument never frightens a village: the threat radius plus
    /// the largest watchtower bonus (the cap, and a set in every region of
    /// the run island) stays inside the distance worldgen keeps between
    /// monuments and settlement centres.
    #[test]
    fn shipped_monument_data_fits_the_island() {
        use sim_world::{WorldGenSettings, WorldPreset};
        let p = Progression::default_data();
        let m = &p.data.monuments;
        for kind in MonumentKind::ALL {
            assert!(m.bonus(kind).is_some(), "{} has a bonus", kind.name());
        }
        let island = WorldGenSettings::preset(WorldPreset::Run);
        let army = crate::Army::default_data(
            &sim_core::Calendar::default_data(),
            &Economy::default_data(),
        );
        let widest: f32 = m
            .kinds
            .iter()
            .filter(|b| b.stat == Stat::ThreatRadius)
            .map(|b| b.cap + b.set_bonus * island.regions as f32)
            .sum();
        assert!(
            army.data.threat.radius + widest < island.monuments.min_settlement_distance,
            "threat radius {} + watchtowers {widest} must stay below {}",
            army.data.threat.radius,
            island.monuments.min_settlement_distance
        );
    }

    /// GP-23, SIM-34: a modifier reaches its consumers. The champion's
    /// trait, folded into `FactionModifiers` by the hourly rebuild, speeds
    /// the whole band's march: the captain walks farther in the same time
    /// and the followers keep up (both movement systems read it). Other
    /// factions get nothing.
    #[test]
    fn a_march_speed_trait_speeds_the_whole_band() {
        use sim_core::{CommandKind, OrderSpec, Position, SimStepper};
        let march = |bonus: Option<f32>| {
            let (mut app, start) = run_app(42);
            let world = app.world_mut();
            if let Some(v) = bonus {
                let t = world.get::<Traits>(start.champion).unwrap().0[0];
                world.resource_mut::<Progression>().data.traits[t as usize].effects =
                    vec![(Stat::MarchSpeed, v)];
            }
            let to = world.resource::<RunProgress>().seat_center;
            let id = world.get::<SpawnId>(start.champion).unwrap().0;
            command(
                &mut app,
                FactionId::PLAYER,
                CommandKind::IssueOrder {
                    captain: id,
                    order: OrderSpec::MoveTo { x: to.x, y: to.y },
                    queue: false,
                },
            );
            app.step(399);
            let world = app.world_mut();
            let mods = world.resource::<FactionModifiers>().clone();
            let at = world.get::<Position>(start.champion).unwrap().0;
            let mut q = world.query::<(&Follower, &Position)>();
            let spread: Vec<f32> = q
                .iter(world)
                .filter(|(f, _)| f.captain == start.champion)
                .map(|(_, p)| (p.0 - at).as_vec2().length())
                .collect();
            let mean = spread.iter().sum::<f32>() / spread.len().max(1) as f32;
            ((at - start.start).as_vec2().length(), mean, mods)
        };
        let (base, base_spread, base_mods) = march(None);
        let (fast, fast_spread, mods) = march(Some(0.5));
        assert_eq!(base_mods.value(FactionId::PLAYER, Stat::MarchSpeed), 0.0);
        assert_eq!(mods.value(FactionId::PLAYER, Stat::MarchSpeed), 0.5);
        // Guard camps' monuments may give the raiders a road marker's
        // bonus; the trait adds nothing to theirs.
        assert_eq!(
            mods.value(FactionId::RAIDERS, Stat::MarchSpeed),
            base_mods.value(FactionId::RAIDERS, Stat::MarchSpeed)
        );
        assert!(
            fast > base * 1.3,
            "+50% march speed: {fast:.1} tiles against {base:.1}"
        );
        assert!(
            fast_spread <= base_spread + 2.0,
            "the followers keep up: {fast_spread:.1} tiles from the captain against {base_spread:.1}"
        );
    }

    fn bonus(kind: MonumentKind, stat: Stat) -> MonumentBonus {
        MonumentBonus {
            kind,
            stat,
            bonus: 0.01,
            cap: 0.03,
            set_bonus: 0.02,
        }
    }

    /// GP-23: each faction's monuments of a kind add `bonus` each up to
    /// `cap`; holding every monument of a kind in a region (at least
    /// `set_min`) adds `set_bonus` beyond the cap; nobody (faction 0) gets
    /// nothing, a split region no set, and a lone monument no set.
    #[test]
    fn caps_and_set_bonus() {
        use MonumentKind::*;
        let data = MonumentData {
            hold_radius: 10.0,
            hold_hours: 2.0,
            drift_days: 3.0,
            drift_distance: 100.0,
            set_min: 2,
            kinds: vec![
                bonus(StandingStones, Stat::Morale),
                bonus(RuinedFort, Stat::Defence),
                MonumentBonus {
                    cap: 2.0,
                    bonus: 1.0,
                    set_bonus: 1.0,
                    ..bonus(Watchtower, Stat::ThreatRadius)
                },
            ],
            guards: Vec::new(),
        };
        let (p, r) = (FactionId::PLAYER, FactionId::RIVAL);
        let nobody = FactionId::INDEPENDENT;
        let held = [
            // Stones: the player holds 5 (capped at 3%), every one in
            // regions 0 and 1 (sets), and region 2's lone stones (no set).
            (StandingStones, 0, p),
            (StandingStones, 0, p),
            (StandingStones, 1, p),
            (StandingStones, 1, p),
            (StandingStones, 2, p),
            // Forts: 2 of region 0's 3 (no set), the rival 1.
            (RuinedFort, 0, p),
            (RuinedFort, 0, p),
            (RuinedFort, 0, r),
            // Forts of region 3: both nobody's (no bonus, no set).
            (RuinedFort, 3, nobody),
            (RuinedFort, 3, nobody),
            // A kind with no bonus listed gives nothing.
            (HolyWell, 0, p),
            (HolyWell, 0, p),
            // Watchtowers: flat tiles, 3 held, capped at 2, one set.
            (Watchtower, 1, r),
            (Watchtower, 1, r),
            (Watchtower, 2, r),
        ];
        let mut mods = FactionModifiers::default();
        monument_modifiers(&mut mods, &held, &data);
        let eq = |a: f32, b: f32| (a - b).abs() < 1e-6;
        assert!(eq(mods.value(p, Stat::Morale), 0.03 + 2.0 * 0.02));
        assert!(eq(mods.value(p, Stat::Defence), 0.02));
        assert!(eq(mods.value(r, Stat::Defence), 0.01));
        assert!(eq(mods.value(r, Stat::ThreatRadius), 2.0 + 1.0));
        assert_eq!(mods.value(p, Stat::FoodYield), 0.0);
        assert_eq!(mods.value(nobody, Stat::Defence), 0.0);
        assert_eq!(mods.value(r, Stat::Morale), 0.0);
        let sets: Vec<_> = mods
            .breakdown
            .iter()
            .filter(|e| matches!(e.source, ModifierSource::SetBonus { .. }))
            .map(|e| (e.faction, e.source))
            .collect();
        assert_eq!(
            sets,
            vec![
                (Some(p), ModifierSource::SetBonus { kind: 0, region: 0 }),
                (Some(p), ModifierSource::SetBonus { kind: 0, region: 1 }),
                (Some(r), ModifierSource::SetBonus { kind: 7, region: 1 }),
            ]
        );
        // Order-independent input: the same holdings shuffled give the
        // same totals.
        let mut shuffled = held.to_vec();
        shuffled.reverse();
        let mut again = FactionModifiers::default();
        monument_modifiers(&mut again, &shuffled, &data);
        assert_eq!(again.by_faction, mods.by_faction);
    }

    /// A run app on the run island, set up (the plugins `sim_headless`
    /// adds, minus the empty M5 ones).
    pub(crate) fn run_app(seed: u64) -> (bevy_app::App, RunStart) {
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

    /// Sends `kind` as a player-origin command from `issuer` and steps a tick.
    pub(crate) fn command(app: &mut bevy_app::App, issuer: FactionId, kind: sim_core::CommandKind) {
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
