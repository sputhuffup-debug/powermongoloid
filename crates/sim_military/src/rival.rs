//! The rival warlord (GP-28): scripted in M4, rules-based in M5.
//!
//! At the start of a run ([`setup`], from `progression::setup_run`) the
//! rival owns every settlement of the region around its seat (the island's
//! largest settlement), with residents' loyalty at `start_loyalty`, and a
//! garrison stands at the seat with a standing `Garrison` order.
//!
//! Its clock (progression.ron `rival`, one row per [`Difficulty`], scaled
//! by the stronger or weaker rival run modifiers) runs once a game hour in
//! [`clock`], slot 11 of the progress driver:
//! - **Levy**: every `levy_every_days` it enlists real villagers of its
//!   lands into the seat's garrison (`enlist_into`), round-robin over its
//!   settlements in SpawnId order.
//! - **Defence**: the seat's defence bonus rises in steps
//!   (`defence_steps`); `combat::resolve_strikes` divides blows on the
//!   rival's people within `seat_radius` of the seat by `Defence + bonus`.
//!   It stands in for walls until M5's sieges.
//! - **Raids**: from `raid_from_day`, every `raid_every_days`, a band is
//!   split off the garrison ([`crate::orders::split_army`]), so raids
//!   weaken the seat. Each band carries a [`RivalBand`] and is steered by
//!   [`think`] (in `SimInputSet::Ai`) with AI-origin commands: it attacks
//!   the player's nearest settlement or contests the player's nearest
//!   monument, else hunts the player's armies.
//!
//! The player wins by taking the seat (`conquest::transfer_seeded` ends the
//! run); the rest of the rival's lands then become independent
//! ([`free_lands`]).

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sim_agents::{Owner, Villagers};
use sim_core::rng::rng_key;
use sim_core::{
    Calendar, CommandKind, Difficulty, FactionId, OrderSpec, Position, Posture, RunState,
    SimCommand, SimEvent, SimEventKind, SimSeed, SimTick, SpawnId, StateHash, StateHasher,
};
use sim_economy::SettlementEconomy;
use sim_world::{Monument, Settlement};

use crate::components::{Captain, Factions, Follower, Order};
use crate::index::ArmyIndex;
use crate::monuments::Holder;
use crate::orders::{enlist_into, log, recruit_candidates, split_army};
use crate::progression::{Effect, Progression, RunProgress};
use crate::spawn::{ArmySpec, spawn_army};

/// How often a raiding band reconsiders its target (ticks).
const THINK_EVERY: u64 = 300;

/// The rival's clock for one difficulty: a row of progression.ron `rival`
/// (GP-28). Days count from the run's first tick.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RivalData {
    /// The difficulty this row is for (one row each).
    pub difficulty: Difficulty,
    /// Loyalty (0..1) of the rival's people at the start.
    pub start_loyalty: f32,
    /// Followers of the seat's garrison at the start.
    pub garrison: u32,
    /// Days of food in the garrison's baggage at the start.
    pub garrison_food_days: f32,
    /// The defence bonus covers the rival's people within this many tiles
    /// of the seat's centre.
    pub seat_radius: f32,
    /// Days between levies.
    pub levy_every_days: f32,
    /// Villagers enlisted at each levy.
    pub levy_size: u32,
    /// `(day, bonus)`: from that day on, blows on the rival's people near
    /// the seat are divided by `Defence + bonus`. Ascending days.
    pub defence_steps: Vec<(f32, f32)>,
    /// The first raid leaves on this day.
    pub raid_from_day: f32,
    /// Days between raids.
    pub raid_every_days: f32,
    /// People in a raiding band (its captain included), at most half the
    /// garrison.
    pub raid_size: u32,
}

impl RivalData {
    /// Checks the row hangs together.
    pub fn validate(&self) -> Result<(), String> {
        let positive = |v: f32| v.is_finite() && v > 0.0;
        let non_negative = |v: f32| v.is_finite() && v >= 0.0;
        let name = self.difficulty.name();
        if !(0.0..=1.0).contains(&self.start_loyalty)
            || !non_negative(self.garrison_food_days)
            || !non_negative(self.seat_radius)
            || !positive(self.levy_every_days)
            || !non_negative(self.raid_from_day)
            || !positive(self.raid_every_days)
            || self.raid_size < 2
        {
            return Err(format!(
                "progression: rival {name} needs start_loyalty in 0..1, levy_every_days and \
                 raid_every_days > 0, other days and radii >= 0, raid_size >= 2"
            ));
        }
        let mut last = f32::NEG_INFINITY;
        for &(day, bonus) in &self.defence_steps {
            if !non_negative(day) || !non_negative(bonus) || day < last {
                return Err(format!(
                    "progression: rival {name} defence_steps need ascending days >= 0 and bonuses >= 0"
                ));
            }
            last = day;
        }
        Ok(())
    }

    /// The defence bonus on `day` (days since the start), before scaling.
    pub fn defence_on(&self, day: f32) -> f32 {
        self.defence_steps
            .iter()
            .take_while(|(d, _)| *d <= day)
            .last()
            .map_or(0.0, |s| s.1)
    }
}

/// The rival's state (GP-28). Always present; `seat: None` outside a run.
/// Hashed.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct RivalState {
    /// The seat (a settlement's SpawnId).
    pub seat: Option<u64>,
    /// The seat's centre.
    pub seat_center: IVec2,
    /// The seat's current defence bonus (added to the `Defence`
    /// multiplier of the rival's people within `seat_radius`).
    pub defence_bonus: f32,
    /// The bonus covers this many tiles around the seat's centre (the
    /// run's `rival.seat_radius`).
    pub seat_radius: f32,
    /// The tick of the next levy.
    pub next_levy: u64,
    /// The tick of the next raid.
    pub next_raid: u64,
    /// Raiding bands sent so far.
    pub raids: u32,
    /// Villagers levied so far.
    pub levied: u32,
}

impl StateHash for RivalState {
    fn hash_state(&self, h: &mut StateHasher) {
        match self.seat {
            Some(s) => {
                h.u8(1);
                h.u64(s);
            }
            None => h.u8(0),
        }
        h.ivec2(self.seat_center);
        h.f32(self.defence_bonus);
        h.f32(self.seat_radius);
        h.u64(self.next_levy);
        h.u64(self.next_raid);
        h.u32(self.raids);
        h.u32(self.levied);
    }
}

/// A captain leading one of the rival's raiding bands (GP-28), steered by
/// [`think`]. Moves to the heir when the captain falls.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RivalBand {
    /// What the band is after: a settlement or monument SpawnId, or `None`
    /// while it hunts the player's armies or heads home.
    pub target: Option<u64>,
    /// The tick of its next decision.
    pub next_think: u64,
}

impl StateHash for RivalBand {
    fn hash_state(&self, h: &mut StateHasher) {
        match self.target {
            Some(t) => {
                h.u8(1);
                h.u64(t);
            }
            None => h.u8(0),
        }
        h.u64(self.next_think);
    }
}

/// The product of the run modifiers' `RivalStrength` effects (1.0 with
/// none): stronger or weaker rival (GP-27).
pub fn strength(world: &World) -> f32 {
    let data = world.resource::<Progression>();
    world
        .resource::<RunProgress>()
        .modifiers
        .iter()
        .filter_map(|&card| data.modifier(card))
        .flat_map(|m| m.effects.iter())
        .filter_map(|e| match *e {
            Effect::RivalStrength(f) => Some(f),
            _ => None,
        })
        .product()
}

/// The clock row for this run's difficulty.
pub fn clock_data(world: &World) -> Option<RivalData> {
    let difficulty = world.resource::<RunState>().config?.difficulty;
    world.resource::<Progression>().rival(difficulty).cloned()
}

/// GP-28: before the first tick of a run, hands the rival region's
/// populated settlements to the rival (quietly, with loyalty at
/// `start_loyalty`: no conquest is reported), seats the rival at `seat`,
/// spawns the seat's garrison (`garrison` followers, armed, scaled by the
/// rival's strength) with a standing `Garrison` order, and sets the clock.
/// Called by `progression::setup_run` after the band and the guard camps
/// are spawned.
pub fn setup(world: &mut World, seat: u64, seat_center: IVec2, region: sim_world::RegionId) {
    let _span = tracing::info_span!("sim_military::rival::setup").entered();
    let Some(data) = clock_data(world) else {
        return;
    };
    let strength = strength(world);
    world
        .resource_mut::<Factions>()
        .set_seat(FactionId::RIVAL, seat_center);
    let mut q = world.query_filtered::<(Entity, &SpawnId, &Settlement), With<Owner>>();
    let mut lands: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, s)| s.region == region)
        .map(|(e, id, _)| (*id, e))
        .collect();
    lands.sort_unstable();
    for (_, s) in lands {
        sim_agents::set_owner(world, s, FactionId::RIVAL, Some(data.start_loyalty));
    }
    let followers = (data.garrison as f32 * strength).round() as u32;
    spawn_garrison(world, seat, seat_center, followers, data.garrison_food_days);
    let cal = world.resource::<Calendar>().clone();
    *world.resource_mut::<RivalState>() = RivalState {
        seat: Some(seat),
        seat_center,
        defence_bonus: data.defence_on(0.0) * strength,
        seat_radius: data.seat_radius,
        next_levy: cal.days(data.levy_every_days),
        next_raid: cal.days(data.raid_from_day),
        raids: 0,
        levied: 0,
    };
}

/// Spawns an armed rival army at the seat holding a `Garrison` order on
/// it (a setup edit, not a command). Returns its captain.
fn spawn_garrison(
    world: &mut World,
    seat: u64,
    at: IVec2,
    followers: u32,
    food_days: f32,
) -> Entity {
    let captain = spawn_army(
        world,
        &ArmySpec {
            food_days,
            armed: true,
            ..ArmySpec::new(FactionId::RIVAL, at, followers)
        },
    );
    if let Some(mut c) = world.get_mut::<Captain>(captain) {
        c.orders
            .push_back(Order::new(OrderSpec::Garrison { settlement: seat }));
    }
    captain
}

/// The seat's garrison: the rival's captain (not a raiding band) holding a
/// `Garrison` order on the seat, else the one nearest the seat (ties:
/// lowest SpawnId).
pub fn garrison(world: &mut World) -> Option<Entity> {
    let state = world.resource::<RivalState>().clone();
    let seat = state.seat?;
    let mut q = world.query_filtered::<(Entity, &SpawnId, &Captain, &Position), Without<RivalBand>>();
    q.iter(world)
        .filter(|(_, _, c, _)| c.faction == FactionId::RIVAL)
        .map(|(e, id, c, p)| {
            let holds = matches!(
                c.orders.front().map(|o| o.spec),
                Some(OrderSpec::Garrison { settlement }) if settlement == seat
            );
            (!holds, (p.0 - state.seat_center).length_squared(), *id, e)
        })
        .min_by_key(|k| (k.0, k.1, k.2))
        .map(|k| k.3)
}

/// GP-28: the rival's clock, once a game hour (slot 11 of
/// `progression::progress`, run only). While the rival holds its seat, it
/// sets the seat's defence bonus for the day, levies when due and sends a
/// raid when due. Span `sim_military::rival::clock`.
pub fn clock(world: &mut World) {
    let _span = tracing::info_span!("sim_military::rival::clock").entered();
    if !world.resource::<RunState>().in_run() {
        return;
    }
    let Some(seat) = world.resource::<RivalState>().seat else {
        return;
    };
    let Some(data) = clock_data(world) else {
        return;
    };
    let held = crate::orders::find_settlement(world, seat)
        .is_some_and(|s| crate::conquest::owner_of(world, s) == FactionId::RIVAL);
    if !held {
        return;
    }
    let tick = world.resource::<SimTick>().0;
    let cal = world.resource::<Calendar>().clone();
    let strength = strength(world);
    let day = tick as f32 / cal.ticks_per_day as f32;
    let bonus = data.defence_on(day) * strength;
    if world.resource::<RivalState>().defence_bonus != bonus {
        world.resource_mut::<RivalState>().defence_bonus = bonus;
    }
    if tick >= world.resource::<RivalState>().next_levy {
        let size = (data.levy_size as f32 * strength).round() as u32;
        levy(world, seat, size, &data);
        world.resource_mut::<RivalState>().next_levy += cal.days(data.levy_every_days).max(1);
    }
    if tick >= world.resource::<RivalState>().next_raid {
        raid(world, data.raid_size);
        world.resource_mut::<RivalState>().next_raid += cal.days(data.raid_every_days).max(1);
    }
}

/// Enlists up to `size` villagers of the rival's lands into the seat's
/// garrison: one at a time from each settlement it owns in SpawnId order
/// (each settlement's idle people first), round and round. A garrison
/// that was wiped out is raised again at the seat. Reports `RivalLevy`.
fn levy(world: &mut World, seat: u64, size: u32, data: &RivalData) {
    if size == 0 {
        return;
    }
    let mut q = world.query::<(Entity, &SpawnId, &Owner, &SettlementEconomy)>();
    let mut lands: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, o, se)| o.faction == FactionId::RIVAL && se.population > 0)
        .map(|(e, id, ..)| (*id, e))
        .collect();
    lands.sort_unstable();
    let pools: Vec<Vec<Entity>> = lands
        .iter()
        .map(|(_, s)| recruit_candidates(world, *s))
        .collect();
    if pools.iter().all(|p| p.is_empty()) {
        return;
    }
    let state = world.resource::<RivalState>().clone();
    let captain = match garrison(world) {
        Some(c) => c,
        None => spawn_garrison(
            world,
            seat,
            state.seat_center,
            0,
            data.garrison_food_days,
        ),
    };
    let mut joined = 0;
    let deepest = pools.iter().map(Vec::len).max().unwrap_or(0);
    'levy: for k in 0..deepest {
        for pool in &pools {
            if joined >= size {
                break 'levy;
            }
            if let Some(&v) = pool.get(k)
                && enlist_into(world, v, captain)
            {
                joined += 1;
            }
        }
    }
    if joined == 0 {
        return;
    }
    world.resource_mut::<RivalState>().levied += joined;
    let id = world.get::<SpawnId>(captain).map_or(0, |s| s.0);
    world.write_message(
        SimEvent::new(SimEventKind::RivalLevy, FactionId::RIVAL)
            .subject(id)
            .at(state.seat_center)
            .amount(joined as i32),
    );
    log(world, format!("The rival levied {joined} men at its seat"));
}

/// Splits a raiding band of up to `size` people (at most half the
/// garrison) off the seat's garrison, under a follower drawn from stream
/// `sim_military::rival` at the tick, aggressive, with a [`RivalBand`].
/// Reports `RivalRaid`. A garrison of fewer than two followers sends
/// nobody.
fn raid(world: &mut World, size: u32) {
    let Some(g) = garrison(world) else {
        return;
    };
    let mut q = world.query::<(Entity, &SpawnId, &Follower)>();
    let mut members: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, f)| f.captain == g)
        .map(|(e, id, _)| (*id, e))
        .collect();
    members.sort_unstable();
    let size = size.min(members.len() as u32 / 2);
    if size < 2 {
        log(world, "The rival had no men to spare for a raid".into());
        return;
    }
    let tick = world.resource::<SimTick>().0;
    let seed = *world.resource::<SimSeed>();
    let mut rng = seed.stream(rng_key("sim_military::rival"), tick);
    let (lead_id, lead) = members[rng.random_range(0..members.len())];
    split_army(world, g, lead, Some(size - 1), Posture::Aggressive);
    world.entity_mut(lead).insert(RivalBand {
        target: None,
        next_think: tick,
    });
    world.resource_mut::<RivalState>().raids += 1;
    let at = world.resource::<RivalState>().seat_center;
    world.write_message(
        SimEvent::new(SimEventKind::RivalRaid, FactionId::RIVAL)
            .subject(lead_id.0)
            .at(at)
            .amount(size as i32),
    );
    log(world, format!("The rival sent {size} men raiding"));
}

/// GP-28: when the player takes the seat, every other settlement the rival
/// holds becomes independent, in SpawnId order (quietly: the run has ended).
pub fn free_lands(world: &mut World) {
    let mut q = world.query::<(Entity, &SpawnId, &Owner)>();
    let mut lands: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, o)| o.faction == FactionId::RIVAL)
        .map(|(e, id, _)| (*id, e))
        .collect();
    lands.sort_unstable();
    for (_, s) in lands {
        sim_agents::set_owner(world, s, FactionId::INDEPENDENT, None);
    }
}

/// `SimInputSet::Ai`, after the raiders: each raiding band ([`RivalBand`],
/// SpawnId order) reconsiders every 300 ticks. It goes for the nearest of
/// the player's settlements (`Attack`, after `GatherFood` when its baggage
/// holds less than a day) and the player's monuments (`MoveTo`, to hold
/// it), ties to the lowest SpawnId; with neither, it hunts the player's
/// nearest captain, and with no one left to fight it goes home to
/// garrison the seat. Orders are re-issued only when the target changes or
/// the band has none, so an attack in progress isn't reset. Commands are
/// AI-origin and aren't recorded.
#[allow(clippy::type_complexity, clippy::too_many_arguments)] // Bevy system parameters.
pub fn think(
    tick: Res<SimTick>,
    run: Res<RunState>,
    state: Res<RivalState>,
    index: Res<ArmyIndex>,
    villagers: Res<Villagers>,
    mut bands: Query<(Entity, &SpawnId, &Captain, &Position, &mut RivalBand)>,
    captains: Query<(&SpawnId, &Captain, &Position), Without<RivalBand>>,
    settlements: Query<(&SpawnId, &Settlement, &Owner)>,
    monuments: Query<(&SpawnId, &Position, &Holder), With<Monument>>,
    mut out: MessageWriter<SimCommand>,
) {
    let _span = tracing::info_span!("sim_military::rival::think").entered();
    if !run.in_run() {
        return;
    }
    let mut ordered: Vec<_> = bands.iter_mut().collect();
    ordered.sort_unstable_by_key(|(_, id, ..)| **id);
    for (e, id, captain, pos, mut band) in ordered {
        if tick.0 < band.next_think {
            continue;
        }
        band.next_think = tick.0 + THINK_EVERY;
        let here = pos.0.as_vec2();
        let nearest = |a: &(f32, u64, IVec2, bool), b: &(f32, u64, IVec2, bool)| {
            a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))
        };
        let target = settlements
            .iter()
            .filter(|(_, _, o)| o.faction == FactionId::PLAYER)
            .map(|(sid, s, _)| (s.center.as_vec2().distance(here), sid.0, s.center, true))
            .chain(
                monuments
                    .iter()
                    .filter(|(_, _, h)| h.faction == FactionId::PLAYER)
                    .map(|(mid, p, _)| (p.0.as_vec2().distance(here), mid.0, p.0, false)),
            )
            .min_by(nearest);
        let idle = captain.orders.is_empty();
        let mut orders: Vec<(OrderSpec, bool)> = Vec::new();
        match target {
            Some((_, tid, at, settlement)) => {
                if band.target == Some(tid) && !idle {
                    continue;
                }
                band.target = Some(tid);
                if settlement {
                    let days = index.get(e).map_or(0.0, |a| {
                        a.food
                            / (villagers.data.needs.hunger_per_day * (a.followers + 1) as f32)
                                .max(1e-3)
                    });
                    if days < 1.0 {
                        orders.push((OrderSpec::GatherFood { settlement: tid }, false));
                    }
                    orders.push((OrderSpec::Attack { settlement: tid }, !orders.is_empty()));
                } else {
                    orders.push((OrderSpec::MoveTo { x: at.x, y: at.y }, false));
                }
            }
            None => {
                band.target = None;
                let prey = captains
                    .iter()
                    .filter(|(_, c, _)| c.faction == FactionId::PLAYER)
                    .map(|(cid, _, p)| (p.0.as_vec2().distance(here), cid.0, p.0, false))
                    .min_by(nearest);
                if let Some((_, _, at, _)) = prey {
                    orders.push((OrderSpec::MoveTo { x: at.x, y: at.y }, false));
                } else if let Some(seat) = state.seat {
                    let home = matches!(
                        captain.orders.front().map(|o| o.spec),
                        Some(OrderSpec::Garrison { .. })
                    );
                    if home {
                        continue;
                    }
                    orders.push((OrderSpec::Garrison { settlement: seat }, false));
                }
            }
        }
        for (order, queue) in orders {
            out.write(SimCommand::ai(
                captain.faction,
                CommandKind::IssueOrder {
                    captain: id.0,
                    order,
                    queue,
                },
            ));
        }
    }
}
