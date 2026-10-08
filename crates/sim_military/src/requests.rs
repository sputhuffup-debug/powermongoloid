//! Village requests (GP-22): settlements ask the player for help, and the
//! sim, not the player, decides when the help arrived.
//!
//! Once a game hour, in a run, the progress driver calls [`step`]. It looks
//! at every populated settlement in SpawnId order:
//!
//! - **Posting.** A settlement that is independent or the player's (never
//!   another warlord's, never the rival's seat), still has residents, and
//!   has no open request and no cooldown posts at most one:
//!   - **DriveOff** when an army at war with the player (raiders, the
//!     rival, beasts) and not its owner's stands within army.ron
//!     `threat.radius` of its centre;
//!   - otherwise **BringFood** when its food security
//!     (`sim_agents::FoodSecurity::days`: the storehouse and homes, the
//!     same measure loyalty reads) has been below `requests.food_days` at
//!     `requests.low_samples` hourly samples in a row. The hysteresis keeps
//!     the work day's dip from flapping requests on and off.
//!   - **ClearCamp** (GP-23) runs beside those, on its own track: each
//!     monument whose guard camp still has a captain
//!     ([`crate::monuments::Guard`]), in SpawnId order, is named by at most
//!     one open request, posted by the nearest settlement within
//!     `requests.camp_radius` that may ask (the rules above) and has no
//!     open ClearCamp and no ClearCamp cooldown of its own. A settlement
//!     can thus ask for food and to clear a camp at the same time.
//!
//!   Its [`Reward`] is drawn then (stream `sim_military::requests ^
//!   settlement` at the tick) from what the settlement really holds.
//! - **Completion**, detected here: BringFood once food delivered by the
//!   player's `Give` orders ([`delivered`]) reaches the need; DriveOff once
//!   no hostile army has been within the threat radius for
//!   `requests.clear_hours` while a player captain stands within
//!   `requests.reward_radius` (if none does, the danger passed without the
//!   player, and the request fails); ClearCamp once the camp has no
//!   captain left, with a player captain within `requests.reward_radius`
//!   of the monument (otherwise someone else cleared it, and the request
//!   fails). The reward goes from the real
//!   storehouse (`move_food`, `move_good`) and residents
//!   ([`crate::orders::enlist_into`]) to the nearest player captain within
//!   `requests.reward_radius`, as far as stocks, baggage room and willing
//!   villagers allow, and goodwill rises by `goodwill_deeds.request`
//!   ([`Deed::Request`]).
//! - **Expiry** at the deadline. Either way the settlement then waits
//!   `requests.cooldown_hours` before asking again.
//!
//! Events: `RequestPosted`, `RequestDone` (cause `Cause::Request`) and
//! `RequestFailed`, each with `subject` = the request id, `other` = the
//! settlement, `at` = its centre and `amount` = the kind's tag.

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use rand::RngExt;
use sim_agents::{Deed, FoodSecurity, Owner};
use sim_core::rng::rng_key;
use sim_core::{
    Calendar, Cause, FactionId, Position, RunState, SimEvent, SimEventKind, SimSeed, SimTick,
    SpawnId, StateHash, StateHasher,
};
use sim_economy::{Economy, GoodId, SettlementEconomy, Stockpile};
use sim_world::Settlement;

use crate::components::Captain;
use crate::data::Army;
use crate::index::ArmyIndex;
use crate::orders::{
    daily_need, enlist_into, faction_name, goodwill_deed, log, move_food, move_good,
    recruit_candidates, settlement_name, storehouse,
};
use crate::progression::{Progression, RequestData, RunProgress};

/// What a settlement asks for. Append only (saves store the tag).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RequestKind {
    /// Drive off the hostile army near it. `since`: the last hourly sample
    /// that saw one within the threat radius.
    DriveOff {
        /// Tick of the last sample with a hostile army near.
        since: u64,
    },
    /// Bring `need` food value; `delivered` so far (by `Give`).
    BringFood {
        /// Food value asked for.
        need: f32,
        /// Food value delivered so far.
        delivered: f32,
    },
    /// Clear the guard camp at `monument` (a SpawnId, GP-23).
    ClearCamp {
        /// The guarded monument.
        monument: u64,
    },
}

impl RequestKind {
    /// Stable tag of the variant (events, hashing, saves).
    pub fn tag(self) -> u8 {
        match self {
            RequestKind::DriveOff { .. } => 0,
            RequestKind::BringFood { .. } => 1,
            RequestKind::ClearCamp { .. } => 2,
        }
    }

    /// What the request asks, for logs.
    pub fn name(self) -> &'static str {
        match self {
            RequestKind::DriveOff { .. } => "drive off raiders",
            RequestKind::BringFood { .. } => "bring food",
            RequestKind::ClearCamp { .. } => "clear the camp",
        }
    }
}

/// What a settlement promises for its request, drawn when it is posted
/// from what it holds. Paid as far as its stocks allow at the time.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Reward {
    /// Food value from the storehouse.
    pub food: f32,
    /// Non-food goods from the storehouse, in GoodId order.
    pub goods: Vec<(GoodId, u32)>,
    /// Residents who join the rewarded captain's army.
    pub volunteers: u32,
    /// Goodwill toward the player on completion (`goodwill_deeds.request`).
    pub goodwill: f32,
}

/// One open request (GP-22).
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Unique per run, increasing.
    pub id: u64,
    /// The asking settlement's SpawnId.
    pub settlement: u64,
    /// What it asks.
    pub kind: RequestKind,
    /// Tick posted.
    pub posted: u64,
    /// The request fails at this tick.
    pub deadline: u64,
    /// What it promises.
    pub reward: Reward,
}

/// Per-settlement bookkeeping between hourly samples.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Watch {
    /// The settlement's SpawnId.
    pub settlement: u64,
    /// Hourly samples in a row with food security below
    /// `requests.food_days`.
    pub low_food: u8,
    /// No new request before this tick (the cooldown after one ends).
    pub quiet_until: u64,
    /// No new ClearCamp request before this tick (its own cooldown).
    pub camp_quiet_until: u64,
}

/// The run's requests (GP-22). Always present; empty outside a run.
/// Hashed.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct RequestBoard {
    /// The id the next request gets.
    pub next_id: u64,
    /// Open requests, oldest first: per settlement at most one DriveOff or
    /// BringFood, and at most one ClearCamp.
    pub open: Vec<Request>,
    /// Per settlement sampled so far, sorted by SpawnId.
    pub watch: Vec<Watch>,
    /// Requests posted this run.
    pub posted: u32,
    /// Requests completed this run.
    pub done: u32,
    /// Requests that failed (expired, or the danger passed without help).
    pub failed: u32,
}

impl RequestBoard {
    /// The oldest open request of settlement `s` (a SpawnId).
    pub fn of(&self, s: u64) -> Option<&Request> {
        self.open.iter().find(|r| r.settlement == s)
    }

    /// The open ClearCamp request naming `monument` (a SpawnId), if any.
    pub fn camp(&self, monument: u64) -> Option<&Request> {
        self.open
            .iter()
            .find(|r| r.kind == RequestKind::ClearCamp { monument })
    }

    fn watch_mut(&mut self, s: u64) -> &mut Watch {
        let i = match self.watch.binary_search_by_key(&s, |w| w.settlement) {
            Ok(i) => i,
            Err(i) => {
                self.watch.insert(
                    i,
                    Watch {
                        settlement: s,
                        ..Watch::default()
                    },
                );
                i
            }
        };
        &mut self.watch[i]
    }
}

impl StateHash for RequestBoard {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.next_id);
        h.u32(self.open.len() as u32);
        for r in &self.open {
            h.u64(r.id);
            h.u64(r.settlement);
            h.u8(r.kind.tag());
            match r.kind {
                RequestKind::DriveOff { since } => h.u64(since),
                RequestKind::BringFood { need, delivered } => {
                    h.f32(need);
                    h.f32(delivered);
                }
                RequestKind::ClearCamp { monument } => h.u64(monument),
            }
            h.u64(r.posted);
            h.u64(r.deadline);
            h.f32(r.reward.food);
            h.u32(r.reward.goods.len() as u32);
            for &(g, q) in &r.reward.goods {
                h.u8(g.0);
                h.u32(q);
            }
            h.u32(r.reward.volunteers);
            h.f32(r.reward.goodwill);
        }
        h.u32(self.watch.len() as u32);
        for w in &self.watch {
            h.u64(w.settlement);
            h.u8(w.low_food);
            h.u64(w.quiet_until);
            h.u64(w.camp_quiet_until);
        }
        h.u32(self.posted);
        h.u32(self.done);
        h.u32(self.failed);
    }
}

/// GP-22: `faction`'s `Give` delivered `value` food to settlement `s`.
/// Player deliveries count toward `s`'s open BringFood request; anything
/// else changes nothing. Called by the orders step.
pub fn delivered(world: &mut World, s: Entity, faction: FactionId, value: f32) {
    if faction != FactionId::PLAYER || value <= 0.0 {
        return;
    }
    let Some(id) = world.get::<SpawnId>(s).map(|id| id.0) else {
        return;
    };
    let mut board = world.resource_mut::<RequestBoard>();
    if let Some(r) = board.open.iter_mut().find(|r| r.settlement == id)
        && let RequestKind::BringFood { delivered, .. } = &mut r.kind
    {
        *delivered += value;
    }
}

/// A settlement as the hourly step sees it.
struct Place {
    id: u64,
    entity: Entity,
    center: IVec2,
    owner: FactionId,
    food_days: f32,
    population: u32,
}

/// The progress driver's request step (GP-22), hourly, in a run only. See
/// the module docs. Span `sim_military::requests`.
pub fn step(world: &mut World) {
    if !world.resource::<RunState>().in_run() {
        return;
    }
    let _span = tracing::info_span!("sim_military::requests").entered();
    let tick = world.resource::<SimTick>().0;
    let cal = world.resource::<Calendar>().clone();
    let data = world.resource::<Progression>().data.requests;
    let radius = world.resource::<Army>().data.threat.radius;
    let seat = world.resource::<RunProgress>().rival_seat;
    let mut q = world.query::<(
        Entity,
        &SpawnId,
        &Settlement,
        &SettlementEconomy,
        &FoodSecurity,
        Option<&Owner>,
    )>();
    let mut places: Vec<Place> = q
        .iter(world)
        .map(|(e, id, st, se, fs, o)| Place {
            id: id.0,
            entity: e,
            center: st.center,
            owner: o.map_or(FactionId::INDEPENDENT, |o| o.faction),
            food_days: fs.days,
            population: se.population,
        })
        .collect();
    places.sort_unstable_by_key(|p| p.id);
    let hostiles = hostile_armies(world);
    let mut board = std::mem::take(&mut *world.resource_mut::<RequestBoard>());
    for p in &places {
        let threatened = hostiles.iter().any(|(f, pos)| {
            *f != p.owner && pos.distance(p.center.as_vec2() + Vec2::splat(0.5)) < radius
        });
        let low = {
            let w = board.watch_mut(p.id);
            w.low_food = if p.food_days < data.food_days {
                w.low_food.saturating_add(1)
            } else {
                0
            };
            w.low_food
        };
        // ClearCamp requests run on their own track (`camp_requests`).
        if let Some(i) = board
            .open
            .iter()
            .position(|r| r.settlement == p.id && !is_camp(r))
        {
            let outcome =
                progress_request(world, &mut board.open[i], p, threatened, tick, &cal, &data);
            if outcome != Outcome::Open {
                let r = board.open.remove(i);
                board.watch_mut(p.id).quiet_until = tick + cal.hours(data.cooldown_hours);
                match outcome {
                    Outcome::Done(captain) => {
                        board.done += 1;
                        complete(world, &r, p, captain);
                    }
                    _ => {
                        board.failed += 1;
                        fail(world, &r, p);
                    }
                }
            }
            continue;
        }
        let asks = may_ask(p, seat) && tick >= board.watch_mut(p.id).quiet_until;
        if !asks {
            continue;
        }
        let kind = if threatened {
            RequestKind::DriveOff { since: tick }
        } else if low >= data.low_samples {
            let need = (data.need_days * daily_need(world, p.population)).min(data.max_need);
            RequestKind::BringFood {
                need,
                delivered: 0.0,
            }
        } else {
            continue;
        };
        let reward = draw_reward(world, p, kind, tick, &data);
        let r = Request {
            id: board.next_id,
            settlement: p.id,
            kind,
            posted: tick,
            deadline: tick + cal.hours(data.deadline_hours),
            reward,
        };
        board.next_id += 1;
        board.posted += 1;
        post(world, &r, p);
        board.open.push(r);
    }
    camp_requests(world, &mut board, &places, tick, &cal, &data);
    *world.resource_mut::<RequestBoard>() = board;
}

/// Whether `p` may ask the player for anything: independent or the
/// player's, not the rival's seat, and not emptied (everyone enlisted or
/// dead: nobody to ask, and a zero need would be "met" at once).
fn may_ask(p: &Place, seat: Option<u64>) -> bool {
    matches!(p.owner, FactionId::INDEPENDENT | FactionId::PLAYER)
        && p.population > 0
        && Some(p.id) != seat
}

fn is_camp(r: &Request) -> bool {
    matches!(r.kind, RequestKind::ClearCamp { .. })
}

/// GP-23: the ClearCamp track of [`step`]. First the open ClearCamp
/// requests, settlements in SpawnId order: done once the camp has no
/// captain left and a player captain stands within `reward_radius` of the
/// monument (it gets the reward), failed if the camp went without one or
/// at the deadline. Then each guarded monument in SpawnId order that no
/// open request names is offered to the nearest settlement within
/// `camp_radius` (ties: lowest SpawnId) that may ask and has no open
/// ClearCamp and no ClearCamp cooldown.
fn camp_requests(
    world: &mut World,
    board: &mut RequestBoard,
    places: &[Place],
    tick: u64,
    cal: &Calendar,
    data: &RequestData,
) {
    let camps = crate::monuments::guarded(world);
    for p in places {
        let Some(i) = board
            .open
            .iter()
            .position(|r| r.settlement == p.id && is_camp(r))
        else {
            continue;
        };
        let RequestKind::ClearCamp { monument } = board.open[i].kind else {
            continue;
        };
        let outcome = if camps.iter().any(|c| c.0 == monument) {
            if tick >= board.open[i].deadline {
                Outcome::Failed
            } else {
                Outcome::Open
            }
        } else {
            let at = crate::monuments::centre_of(world, monument).unwrap_or(p.center);
            match nearest_player_captain(world, at, data.reward_radius) {
                Some(c) => Outcome::Done(Some(c)),
                // Someone else cleared it.
                None => Outcome::Failed,
            }
        };
        if outcome == Outcome::Open {
            continue;
        }
        let r = board.open.remove(i);
        board.watch_mut(p.id).camp_quiet_until = tick + cal.hours(data.cooldown_hours);
        match outcome {
            Outcome::Done(captain) => {
                board.done += 1;
                complete(world, &r, p, captain);
            }
            _ => {
                board.failed += 1;
                fail(world, &r, p);
            }
        }
    }
    let seat = world.resource::<RunProgress>().rival_seat;
    let r2 = data.camp_radius * data.camp_radius;
    for (monument, at) in camps {
        if board.camp(monument).is_some() {
            continue;
        }
        let mut best: Option<(i64, u64, usize)> = None;
        for (i, p) in places.iter().enumerate() {
            let d2 = (p.center - at).as_i64vec2().length_squared();
            if d2 as f32 > r2
                || !may_ask(p, seat)
                || board
                    .open
                    .iter()
                    .any(|r| r.settlement == p.id && is_camp(r))
                || tick < board.watch_mut(p.id).camp_quiet_until
            {
                continue;
            }
            if best.is_none_or(|b| (d2, p.id) < (b.0, b.1)) {
                best = Some((d2, p.id, i));
            }
        }
        let Some((.., i)) = best else {
            continue;
        };
        let p = &places[i];
        let kind = RequestKind::ClearCamp { monument };
        let reward = draw_reward(world, p, kind, tick, data);
        let r = Request {
            id: board.next_id,
            settlement: p.id,
            kind,
            posted: tick,
            deadline: tick + cal.hours(data.deadline_hours),
            reward,
        };
        board.next_id += 1;
        board.posted += 1;
        post(world, &r, p);
        board.open.push(r);
    }
}

/// Where an open request stands after this hour's sample.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Outcome {
    Open,
    /// Done; the reward goes to this captain, if any is near.
    Done(Option<Entity>),
    Failed,
}

fn progress_request(
    world: &mut World,
    r: &mut Request,
    p: &Place,
    threatened: bool,
    tick: u64,
    cal: &Calendar,
    data: &RequestData,
) -> Outcome {
    match &mut r.kind {
        RequestKind::DriveOff { since } => {
            if threatened {
                *since = tick;
            } else if tick.saturating_sub(*since) >= cal.hours(data.clear_hours) {
                return match nearest_player_captain(world, p.center, data.reward_radius) {
                    Some(c) => Outcome::Done(Some(c)),
                    // The danger passed without the player.
                    None => Outcome::Failed,
                };
            }
        }
        RequestKind::BringFood { need, delivered } => {
            if *delivered >= *need {
                return Outcome::Done(nearest_player_captain(world, p.center, data.reward_radius));
            }
        }
        // Detected on their own track (`camp_requests`), never here.
        RequestKind::ClearCamp { .. } => {}
    }
    if tick >= r.deadline {
        Outcome::Failed
    } else {
        Outcome::Open
    }
}

/// Armies at war with the player this tick: (faction, captain position).
fn hostile_armies(world: &World) -> Vec<(FactionId, Vec2)> {
    let army = world.resource::<Army>();
    world
        .resource::<ArmyIndex>()
        .armies
        .iter()
        .filter(|a| army.at_war(a.faction, FactionId::PLAYER))
        .map(|a| (a.faction, a.pos))
        .collect()
}

/// The player's captain nearest `at` within `radius` tiles (ties: lowest
/// SpawnId).
pub(crate) fn nearest_player_captain(world: &mut World, at: IVec2, radius: f32) -> Option<Entity> {
    let mut q = world.query::<(Entity, &SpawnId, &Captain, &Position)>();
    q.iter(world)
        .filter(|(_, _, c, _)| c.faction == FactionId::PLAYER)
        .map(|(e, id, _, p)| ((p.0 - at).length_squared(), *id, e))
        .filter(|(d2, ..)| (*d2 as f32) <= radius * radius)
        .min_by_key(|(d2, id, _)| (*d2, *id))
        .map(|(.., e)| e)
}

/// Draws a request's reward from the settlement's real stocks (stream
/// `sim_military::requests ^ settlement` at the tick): DriveOff and
/// ClearCamp promise food, `reward_food_days` of its need times U(0.5, 1), at most
/// `reward_max_food` and what the storehouse holds; every kind may promise up to `reward_goods` units of
/// one non-food good the armies value, and a seeded number of volunteers.
fn draw_reward(
    world: &World,
    p: &Place,
    kind: RequestKind,
    tick: u64,
    data: &RequestData,
) -> Reward {
    let mut rng = world
        .resource::<SimSeed>()
        .stream(rng_key("sim_military::requests") ^ p.id, tick);
    let econ = world.resource::<Economy>();
    let army = world.resource::<Army>();
    let stock = storehouse(world, p.entity)
        .and_then(|e| world.get::<Stockpile>(e))
        .cloned()
        .unwrap_or_default();
    let mut reward = Reward {
        goodwill: world.resource::<Progression>().data.goodwill_deeds.request,
        ..Reward::default()
    };
    let share = rng.random_range(0.5f32..=1.0);
    if let RequestKind::DriveOff { .. } | RequestKind::ClearCamp { .. } = kind {
        let want = (data.reward_food_days * daily_need(world, p.population) * share)
            .min(data.reward_max_food);
        reward.food = want.min(stock.food_value(econ));
    }
    let goods: Vec<(GoodId, u32)> = stock
        .goods
        .iter()
        .filter(|(g, q)| **q > 0 && !econ.is_food(**g) && army.trade_value(**g) > 0.0)
        .map(|(g, q)| (*g, *q))
        .collect();
    if !goods.is_empty() && data.reward_goods > 0 {
        let (g, have) = goods[rng.random_range(0..goods.len())];
        let qty = rng.random_range(1..=data.reward_goods.min(have));
        reward.goods.push((g, qty));
    }
    let (lo, hi) = data.volunteers;
    reward.volunteers = rng.random_range(lo..=hi);
    reward
}

fn event(kind: SimEventKind, r: &Request, p: &Place) -> SimEvent {
    SimEvent::new(kind, FactionId::PLAYER)
        .subject(r.id)
        .other(p.id)
        .at(p.center)
        .amount(r.kind.tag() as i32)
}

fn post(world: &mut World, r: &Request, p: &Place) {
    world.write_message(event(SimEventKind::RequestPosted, r, p));
    let name = settlement_name(world, p.entity);
    log(world, format!("{name} asks for help: {}", r.kind.name()));
}

fn fail(world: &mut World, r: &Request, p: &Place) {
    world.write_message(event(SimEventKind::RequestFailed, r, p));
    let name = settlement_name(world, p.entity);
    log(world, format!("{name}'s request lapsed: {}", r.kind.name()));
}

/// Pays the reward to `captain` (if any), raises goodwill, and reports.
fn complete(world: &mut World, r: &Request, p: &Place, captain: Option<Entity>) {
    if let Some(c) = captain {
        pay(world, &r.reward, p.entity, c);
    }
    goodwill_deed(world, p.entity, FactionId::PLAYER, Deed::Request);
    world.write_message(event(SimEventKind::RequestDone, r, p).cause(Cause::Request { id: r.id }));
    let (who, name) = (
        faction_name(world, FactionId::PLAYER),
        settlement_name(world, p.entity),
    );
    log(world, format!("{who} answered {name}: {}", r.kind.name()));
}

/// The reward, as far as the storehouse, the baggage and the residents
/// allow: food, then goods, from the storehouse into the baggage; then the
/// first willing residents ([`recruit_candidates`]) join.
fn pay(world: &mut World, reward: &Reward, s: Entity, captain: Entity) {
    let Some(store) = storehouse(world, s) else {
        return;
    };
    if reward.food > 0.0 {
        move_food(world, store, captain, reward.food);
    }
    for &(g, q) in &reward.goods {
        move_good(world, store, captain, g, q);
    }
    if reward.volunteers > 0 {
        let candidates = recruit_candidates(world, s);
        for e in candidates.into_iter().take(reward.volunteers as usize) {
            enlist_into(world, e, captain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conquest::tests::settlements;
    use crate::progression::tests::{command, run_app};
    use crate::spawn::{ArmySpec, spawn_army};
    use bevy_app::App;
    use sim_agents::Goodwill;
    use sim_core::{Chronicle, CommandKind, EventFeed, OrderSpec, SimStepper};

    use crate::components::{ArmyStats, Follower};

    /// Clears the food (not other goods) from `s`'s storehouse and homes.
    fn starve(world: &mut World, s: Entity) {
        let econ = world.resource::<Economy>().clone();
        let se = world.get::<SettlementEconomy>(s).unwrap().clone();
        for e in std::iter::once(se.storehouse).chain(se.homes.iter().copied()) {
            if let Some(mut st) = world.get_mut::<Stockpile>(e) {
                st.goods.retain(|g, _| !econ.is_food(*g));
            }
        }
    }

    /// Puts `value` worth of the best food into `s`'s storehouse.
    fn stock_food(world: &mut World, s: Entity, value: f32) {
        let econ = world.resource::<Economy>().clone();
        let g = econ.foods_by_value()[0];
        let qty = (value / econ.food_value(g)).ceil() as u32;
        let store = storehouse(world, s).unwrap();
        let mut st = world.get_mut::<Stockpile>(store).unwrap();
        st.capacity += qty;
        st.add(g, qty);
    }

    fn feed_kinds(app: &App) -> Vec<(SimEventKind, u64, u64)> {
        app.world()
            .resource::<EventFeed>()
            .iter()
            .map(|e| (e.event.kind, e.event.subject, e.event.other))
            .collect()
    }

    fn goodwill(app: &App, s: Entity) -> f32 {
        app.world()
            .get::<Goodwill>(s)
            .unwrap()
            .get(FactionId::PLAYER)
    }

    fn followers(world: &mut World, captain: Entity) -> usize {
        let mut q = world.query::<&Follower>();
        q.iter(world).filter(|f| f.captain == captain).count()
    }

    fn army_at(world: &mut World, faction: FactionId, at: IVec2, days: f32) -> Entity {
        spawn_army(
            world,
            &ArmySpec {
                food_days: days,
                ..ArmySpec::new(faction, at, 5)
            },
        )
    }

    /// GP-22: a settlement short of food for two hourly samples asks for
    /// food; the player's Give counts toward it; once the need is met the
    /// request is done at the next hour: the reward goes to the nearest
    /// player captain, goodwill rises by `goodwill_deeds.request`, and the
    /// settlement waits its cooldown before asking again.
    #[test]
    fn a_hungry_village_asks_for_food_and_rewards_the_giver() {
        let (mut app, _) = run_app(42);
        let (sid, s, center) = settlements(app.world_mut())[0];
        starve(app.world_mut(), s);
        let data = app.world().resource::<Progression>().data.requests;
        let g0 = goodwill(&app, s);
        // Ticks 0 and 100 sample low food; the request is posted at 100.
        app.step(101);
        let board = app.world().resource::<RequestBoard>().clone();
        let r = board.of(sid).cloned().expect("a request");
        assert_eq!(r.posted, 100);
        let RequestKind::BringFood { need, delivered } = r.kind else {
            panic!("bring food, not {:?}", r.kind);
        };
        assert!(need > 0.0 && need <= data.max_need && delivered == 0.0);
        assert!(feed_kinds(&app).contains(&(SimEventKind::RequestPosted, r.id, sid)));

        // A well-fed player band gives food beside it.
        let world = app.world_mut();
        let cap = army_at(world, FactionId::PLAYER, center + IVec2::new(3, 0), 10.0);
        let cid = world.get::<SpawnId>(cap).unwrap().0;
        let men = followers(world, cap);
        let bag0 = world.get::<Stockpile>(cap).unwrap().clone();
        command(
            &mut app,
            FactionId::PLAYER,
            CommandKind::IssueOrder {
                captain: cid,
                order: OrderSpec::Give { settlement: sid },
                queue: false,
            },
        );
        let board = app.world().resource::<RequestBoard>();
        let RequestKind::BringFood { delivered, .. } = board.of(sid).unwrap().kind else {
            unreachable!()
        };
        assert!(delivered >= need, "Give delivered {delivered} of {need}");
        let donated = app
            .world()
            .resource::<ArmyStats>()
            .get(FactionId::PLAYER)
            .donated;
        assert!(
            (delivered - donated).abs() < 1e-3,
            "the request counts what arrived"
        );

        // Done at the next hour (tick 200).
        let tick = app.world().resource::<SimTick>().0;
        app.step(200 - tick + 1);
        let board = app.world().resource::<RequestBoard>().clone();
        assert!(board.of(sid).is_none(), "{:?}", board.open);
        assert!(board.done >= 1);
        let w = board.watch.iter().find(|w| w.settlement == sid).unwrap();
        assert_eq!(
            w.quiet_until,
            200 + app
                .world()
                .resource::<Calendar>()
                .hours(data.cooldown_hours)
        );
        let done = app
            .world()
            .resource::<Chronicle>()
            .entries()
            .iter()
            .find(|e| e.event.kind == SimEventKind::RequestDone)
            .map(|e| (e.tick, e.event.subject, e.event.other, e.event.cause))
            .expect("a RequestDone entry");
        assert_eq!(done, (200, r.id, sid, Cause::Request { id: r.id }));
        let deed = app
            .world()
            .resource::<Progression>()
            .data
            .goodwill_deeds
            .request;
        // The goodwill is applied with the next tick's ledger.
        app.step(1);
        assert!(
            (goodwill(&app, s) - g0 - deed).abs() < 1e-4,
            "goodwill {g0} -> {}",
            goodwill(&app, s)
        );
        // The reward (as far as the village still had it) reached the band.
        let world = app.world_mut();
        let bag = world.get::<Stockpile>(cap).unwrap().clone();
        let goods_gained: u32 = r
            .reward
            .goods
            .iter()
            .map(|&(g, _)| bag.get(g).saturating_sub(bag0.get(g)))
            .sum();
        let joined = followers(world, cap) - men;
        println!(
            "reward {:?}: goods +{goods_gained}, volunteers +{joined}",
            r.reward
        );
        assert!(joined as u32 <= r.reward.volunteers);
        if !r.reward.goods.is_empty() || r.reward.volunteers > 0 {
            assert!(goods_gained + joined as u32 > 0, "something was paid");
        }
    }

    /// GP-22: an army at war with the player near a village posts a
    /// DriveOff; once it is gone for `clear_hours` with a player captain
    /// there, the request is done and the village pays food. Without a
    /// player captain near, the danger passing is a failed request.
    #[test]
    fn raiders_near_a_village_ask_to_be_driven_off() {
        for helped in [true, false] {
            let (mut app, _) = run_app(42);
            let (sid, s, center) = settlements(app.world_mut())[0];
            let world = app.world_mut();
            let raiders = army_at(world, FactionId::RAIDERS, center + IVec2::new(10, 0), 3.0);
            // Fed, so it doesn't help itself to the village's food.
            let helper =
                helped.then(|| army_at(world, FactionId::PLAYER, center + IVec2::new(-6, 0), 1.5));
            let data = world.resource::<Progression>().data.requests;
            let g0 = goodwill(&app, s);
            app.step(1);
            let r = app
                .world()
                .resource::<RequestBoard>()
                .of(sid)
                .cloned()
                .expect("a DriveOff request");
            assert_eq!(r.kind, RequestKind::DriveOff { since: 0 });
            // The raiders leave (a test edit between ticks).
            let world = app.world_mut();
            let mut q = world.query::<(Entity, &Follower)>();
            let band: Vec<Entity> = q
                .iter(world)
                .filter(|(_, f)| f.captain == raiders)
                .map(|(e, _)| e)
                .collect();
            for e in band.into_iter().chain([raiders]) {
                world.despawn(e);
            }
            // Food in the storehouse at pay time (villagers fetch it home
            // through the day).
            stock_food(world, s, 2.0 * data.reward_max_food);
            let food0 = helper.map(|h| {
                let econ = world.resource::<Economy>().clone();
                world.get::<Stockpile>(h).unwrap().food_value(&econ)
            });
            app.step(100);
            let board = app.world().resource::<RequestBoard>().clone();
            assert!(board.of(sid).is_none(), "helped {helped}: {:?}", board.open);
            let kinds = feed_kinds(&app);
            if let (Some(h), Some(food0)) = (helper, food0) {
                assert!(kinds.contains(&(SimEventKind::RequestDone, r.id, sid)));
                assert!(!kinds.contains(&(SimEventKind::RequestFailed, r.id, sid)));
                let econ = app.world().resource::<Economy>().clone();
                let food = app.world().get::<Stockpile>(h).unwrap().food_value(&econ);
                assert!(r.reward.food > 0.0 && r.reward.food <= data.reward_max_food);
                assert!(food > food0, "the village paid food: {food0} -> {food}");
                // The goodwill is applied with the next tick's ledger.
                app.step(1);
                assert!(goodwill(&app, s) > g0);
            } else {
                assert!(kinds.contains(&(SimEventKind::RequestFailed, r.id, sid)));
                assert!(!kinds.contains(&(SimEventKind::RequestDone, r.id, sid)));
                app.step(1);
                assert_eq!(goodwill(&app, s), g0, "no help, no thanks");
            }
        }
    }

    /// GP-22: a request nobody answers fails at its deadline, and the
    /// settlement asks nothing more until its cooldown is over, even while
    /// still hungry; then it asks again. Another warlord's village never
    /// asks.
    #[test]
    fn an_unanswered_request_lapses_and_the_village_waits() {
        let (mut app, _) = run_app(42);
        let list = settlements(app.world_mut());
        let (sid, s, _) = list[0];
        let (other, o, _) = list[1];
        let world = app.world_mut();
        {
            let mut p = world.resource_mut::<Progression>();
            p.data.requests.deadline_hours = 2.0;
            p.data.requests.cooldown_hours = 3.0;
        }
        starve(world, s);
        starve(world, o);
        sim_agents::set_owner(world, o, FactionId::RIVAL, None);
        // Posted at 100, lapses at 300, quiet until 600, asks again at 600
        // (its low-food count kept running).
        let of = |app: &App, kind: SimEventKind| {
            feed_kinds(app)
                .iter()
                .filter(|k| k.0 == kind && k.2 == sid)
                .count()
        };
        app.step(301);
        let board = app.world().resource::<RequestBoard>().clone();
        assert!(board.of(sid).is_none(), "{:?}", board.open);
        assert_eq!(of(&app, SimEventKind::RequestPosted), 1);
        assert_eq!(of(&app, SimEventKind::RequestFailed), 1);
        // Keep it hungry through the cooldown (samples at 400 and 500).
        for _ in 0..2 {
            starve(app.world_mut(), s);
            app.step(100);
        }
        starve(app.world_mut(), s);
        app.step(99);
        let board = app.world().resource::<RequestBoard>().clone();
        assert!(board.of(sid).is_none(), "quiet until 600");
        assert_eq!(of(&app, SimEventKind::RequestPosted), 1);
        app.step(1);
        let board = app.world().resource::<RequestBoard>().clone();
        let again = board.of(sid).expect("asks again after the cooldown");
        assert_eq!(again.posted, 600);
        assert!(board.of(other).is_none(), "the rival's village asks nobody");
    }
}
