//! Orders (GP-02, GP-03). Commands are validated and applied in arrival
//! order; then each captain works on its first order, serially in SpawnId
//! order. Settlement dealings (recruiting, food, arms, trade) move goods
//! between two stockpiles directly: this is the army's serial logistics
//! step, the counterpart of the economy's transfer resolver (ADR 0010).
//!
//! Food dealings read and draw on the storehouse. In a run, every food
//! dealing but plunder sees the village's pantry instead: its storehouse
//! and homes, the food its Food security counts, drawn storehouse first,
//! then the best-stocked homes (`food_sources`, ADR 0018). Every food
//! dealing the player ordered ends with a line in the military log: what
//! moved, in days of the army's food, or why nothing did.
//!
//! In a run, the lands' dues end the step (`lands`, GP-34): a town of the
//! player's on Tithe or Levy hands food, arms and mustered men to the
//! nearest player captain within reach, in the hour its dial turned and
//! then daily.

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use rand::RngExt;
use sim_agents::{
    ActionKind, Activity, CombatStats, Deed, Employment, Enlisted, GoodwillLedger,
    SettlementLoyalty, Stage, Stance, Threat, Travel, TravelState, Villager, Villagers,
};
use sim_core::rng::rng_key;
use sim_core::{
    Allegiance, Calendar, Cause, CommandKind, FactionId, FactionModifiers, OrderSpec, Position,
    Posture, RunState, SimCommand, SimSeed, SimTick, SpawnId, Stat,
};
use sim_economy::{Economy, GoodId, Occupation, SettlementEconomy, Stockpile, Workplace};
use sim_world::{FlowCache, Settlement, WorldMap};

use crate::components::{
    ArmyStats, Captain, Equipment, Factions, Follower, MilitaryLog, Order, describe,
};
use crate::conquest::{militia_or_foes_near, owner_of, transfer};
use crate::data::Army;
use crate::index::FighterGrid;
use crate::progression::Progression;
use crate::wild::{Harvest, harvest};

/// Commands read this tick, waiting to be applied by `apply_commands`.
/// Always empty between ticks.
#[derive(Resource, Debug, Default)]
pub struct CommandInbox(pub Vec<SimCommand>);

pub(crate) fn has_commands(inbox: Res<CommandInbox>) -> bool {
    !inbox.0.is_empty()
}

pub(crate) fn collect_commands(
    mut reader: MessageReader<SimCommand>,
    mut inbox: ResMut<CommandInbox>,
) {
    let _span = tracing::info_span!("sim_military::collect_commands").entered();
    inbox.0.extend(reader.read().cloned());
}

fn find_captain(world: &mut World, id: u64) -> Option<Entity> {
    let mut q = world.query::<(Entity, &SpawnId, &Captain)>();
    q.iter(world).find(|(_, s, _)| s.0 == id).map(|(e, ..)| e)
}

/// A populated settlement by SpawnId.
pub fn find_settlement(world: &mut World, id: u64) -> Option<Entity> {
    let mut q = world.query_filtered::<(Entity, &SpawnId), With<SettlementEconomy>>();
    q.iter(world).find(|(_, s)| s.0 == id).map(|(e, _)| e)
}

pub(crate) fn settlement_name(world: &World, s: Entity) -> String {
    let names = world.resource::<sim_core::NameTable>();
    world
        .get::<Settlement>(s)
        .map_or_else(|| "?".into(), |st| names.get(st.name).to_string())
}

pub(crate) fn log(world: &mut World, text: String) {
    let tick = world.resource::<SimTick>().0;
    tracing::debug!(tick, "{text}");
    world.resource_mut::<MilitaryLog>().push(tick, text);
}

pub(crate) fn faction_name(world: &World, f: FactionId) -> String {
    world.resource::<Army>().faction_name(f).to_string()
}

/// Input set: applies this tick's commands in arrival order. Invalid
/// commands (someone else's captain, an unknown settlement) are rejected
/// and logged; they still count as processed.
pub(crate) fn apply_commands(world: &mut World) {
    let _span = tracing::info_span!("sim_military::apply_commands").entered();
    let commands = std::mem::take(&mut world.resource_mut::<CommandInbox>().0);
    for cmd in commands {
        if let Err(why) = apply(world, &cmd) {
            world
                .resource_mut::<ArmyStats>()
                .of(cmd.issuer)
                .rejected_commands += 1;
            let who = faction_name(world, cmd.issuer);
            log(world, format!("{who}: order refused ({why})"));
        }
    }
}

fn validate_spec(
    world: &mut World,
    issuer: FactionId,
    spec: &OrderSpec,
) -> Result<(), &'static str> {
    let size = world.resource::<WorldMap>().size() as i32;
    let in_map = |x: i32, y: i32| x >= 0 && y >= 0 && x < size && y < size;
    match *spec {
        OrderSpec::MoveTo { x, y } | OrderSpec::Scout { x, y } => {
            in_map(x, y).then_some(()).ok_or("off the map")
        }
        OrderSpec::Return => Ok(()),
        OrderSpec::Recruit { settlement, count } => {
            if count == 0 {
                return Err("nobody to recruit");
            }
            find_settlement(world, settlement)
                .map(|_| ())
                .ok_or("no such settlement")
        }
        OrderSpec::GatherFood { settlement }
        | OrderSpec::Equip { settlement }
        | OrderSpec::Trade { settlement } => find_settlement(world, settlement)
            .map(|_| ())
            .ok_or("no such settlement"),
        // GP-05: you can't attack what you hold, and garrison only that.
        OrderSpec::Attack { settlement } => {
            let s = find_settlement(world, settlement).ok_or("no such settlement")?;
            if owner_of(world, s) == issuer {
                return Err("your own settlement");
            }
            Ok(())
        }
        OrderSpec::Garrison { settlement } => {
            let s = find_settlement(world, settlement).ok_or("no such settlement")?;
            if owner_of(world, s) != issuer {
                return Err("not your settlement");
            }
            Ok(())
        }
        // SIM-38: within progression.ron `forage.max_radius`.
        OrderSpec::Forage { radius } => {
            let max = world.resource::<Progression>().data.forage.max_radius;
            if radius == 0 || radius > max {
                return Err("bad forage radius");
            }
            Ok(())
        }
        OrderSpec::Give { settlement } => find_settlement(world, settlement)
            .map(|_| ())
            .ok_or("no such settlement"),
        // GP-32 (M4a): the parley lands in step 5, the agent in step 7.
        OrderSpec::Demand { .. } | OrderSpec::Subvert { .. } => Err("not yet"),
    }
}

/// The captain `id` if it belongs to `issuer`. Heroes (M4) will never be
/// captains of any faction, so no command can reach them (SIM-52).
fn own_captain(world: &mut World, issuer: FactionId, id: u64) -> Result<Entity, &'static str> {
    let e = find_captain(world, id).ok_or("no such captain")?;
    let c = world.get::<Captain>(e).ok_or("no such captain")?;
    if c.faction != issuer {
        return Err("not your captain");
    }
    Ok(e)
}

fn apply(world: &mut World, cmd: &SimCommand) -> Result<(), &'static str> {
    match cmd.kind {
        CommandKind::Noop => Ok(()),
        CommandKind::IssueOrder {
            captain,
            order,
            queue,
        } => {
            let e = own_captain(world, cmd.issuer, captain)?;
            validate_spec(world, cmd.issuer, &order)?;
            let mut c = world.get_mut::<Captain>(e).expect("captain");
            if !queue {
                c.orders.clear();
                c.goal = None;
            }
            c.orders.push_back(Order::new(order));
            // AI orders repeat as targets move; only the player's are news.
            if cmd.origin == sim_core::CommandOrigin::Player {
                let who = faction_name(world, cmd.issuer);
                log(
                    world,
                    format!("{who}: captain {captain} ordered to {}", describe(&order)),
                );
            }
            Ok(())
        }
        CommandKind::SetPosture { captain, posture } => {
            let e = own_captain(world, cmd.issuer, captain)?;
            world.get_mut::<Captain>(e).expect("captain").posture = posture;
            let who = faction_name(world, cmd.issuer);
            log(
                world,
                format!("{who}: captain {captain} is now {}", posture.name()),
            );
            Ok(())
        }
        CommandKind::CancelOrders { captain } => {
            let e = own_captain(world, cmd.issuer, captain)?;
            let mut c = world.get_mut::<Captain>(e).expect("captain");
            c.orders.clear();
            c.goal = None;
            Ok(())
        }
        CommandKind::AppointCaptain { follower } => appoint(world, cmd.issuer, follower),
        // GP-27: boons, traits and invitations are picked by command, so
        // picks replay.
        CommandKind::ChooseOffer { offer, choice } => {
            crate::progression::choose(world, cmd.issuer, offer, choice)
        }
        // GP-21: the player gives up the run.
        CommandKind::AbandonRun => crate::progression::abandon(world, cmd.issuer),
        // GP-34 (M4a step 4): the dials, applied by the next hourly
        // `govern::step`; building orders (step 8) and stewards (step 9)
        // are still refused "not yet" (lane B writes this arm).
        CommandKind::Govern { settlement, act } => {
            crate::govern::command(world, cmd.issuer, settlement, act)
        }
    }
}

/// GP-01: promotes a follower to captain. The new captain takes every
/// second follower (by SpawnId) and half the baggage. Refused at army.ron
/// `max_captains` (the champion counts), and in a run (GP-25) when the
/// player's other captains already fill
/// [`crate::progression::captain_slots`] ("no captain slot").
fn appoint(world: &mut World, issuer: FactionId, follower: u64) -> Result<(), &'static str> {
    let mut q = world.query::<(Entity, &SpawnId, &Follower)>();
    let (f, old) = q
        .iter(world)
        .find(|(_, s, _)| s.0 == follower)
        .map(|(e, _, f)| (e, f.captain))
        .ok_or("no such follower")?;
    let old_captain = world.get::<Captain>(old).ok_or("no such captain")?.clone();
    if old_captain.faction != issuer {
        return Err("not your follower");
    }
    let army = world.resource::<Army>();
    let max = army.data.max_captains;
    let mut q = world.query::<&Captain>();
    let captains = q.iter(world).filter(|c| c.faction == issuer).count() as u32;
    if captains >= max {
        return Err("too many captains");
    }
    // GP-25: in a run, the player's captains also need slots from the land
    // held; the champion's own army takes none.
    if let Some(slots) = crate::progression::captain_slots(world, issuer)
        && captains.saturating_sub(1) >= slots
    {
        return Err("no captain slot");
    }
    split_army(world, old, f, None, old_captain.posture);
    let who = faction_name(world, issuer);
    log(world, format!("{who}: {follower} appointed captain"));
    Ok(())
}

/// GP-01, GP-28: splits a band off `old`'s army under `f`, one of its
/// followers, who becomes a captain of the same faction with `posture`.
/// The other followers, by SpawnId, are numbered from 0: `take: None`
/// moves every odd-numbered one (half the army, as `AppointCaptain`
/// does) and half of each baggage good (rounded down); `take: Some(n)`
/// moves the first `n` odd-numbered ones, then even-numbered ones (the
/// rival's raiding bands), with each baggage good shared by head count
/// (rounded down). A band split off in the middle of a battle is still in
/// it (SIM-36). Returns how many followers moved. Call from serial steps.
pub(crate) fn split_army(
    world: &mut World,
    old: Entity,
    f: Entity,
    take: Option<u32>,
    posture: Posture,
) -> u32 {
    let Some(old_captain) = world.get::<Captain>(old).cloned() else {
        return 0;
    };
    let mut q = world.query::<(Entity, &SpawnId, &Follower)>();
    let mut members: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(e, _, fl)| fl.captain == old && *e != f)
        .map(|(e, id, _)| (*id, e))
        .collect();
    members.sort_unstable();
    let odd = members.iter().skip(1).step_by(2);
    let chosen: Vec<Entity> = match take {
        None => odd.map(|m| m.1).collect(),
        Some(n) => odd
            .chain(members.iter().step_by(2))
            .take(n as usize)
            .map(|m| m.1)
            .collect(),
    };
    for &e in &chosen {
        world.get_mut::<Follower>(e).expect("follower").captain = f;
    }
    let moved = chosen.len() as u32;
    // The band's share of each good: half, or by head count.
    let (num, den) = match take {
        None => (1u64, 2u64),
        Some(_) => (moved as u64 + 1, members.len() as u64 + 2),
    };
    let capacity = world.resource::<Army>().baggage_capacity(moved + 1);
    let mut baggage = Stockpile::with_capacity(capacity);
    if let Some(mut old_bag) = world.get_mut::<Stockpile>(old) {
        let goods: Vec<(GoodId, u32)> = old_bag.goods.iter().map(|(g, q)| (*g, *q)).collect();
        for (g, q) in goods {
            let share = old_bag.take(g, (q as u64 * num / den) as u32);
            baggage.add(g, share);
        }
    }
    // SIM-36: a band split off in the middle of a battle is still in it,
    // so the men who fought count it when it ends.
    let mut captain = Captain::new(old_captain.faction, posture);
    captain.battle_since = old_captain.battle_since;
    captain.last_engaged = old_captain.last_engaged;
    captain.battle_foes = old_captain.battle_foes;
    world
        .entity_mut(f)
        .remove::<Follower>()
        .insert((captain, baggage));
    moved
}

/// Daily food value an army eats.
pub(crate) fn daily_need(world: &World, people: u32) -> f32 {
    world.resource::<Villagers>().data.needs.hunger_per_day * people as f32
}

/// Moves up to `max_value` of food (most nourishing first) from one
/// stockpile to another. Returns the food value moved.
pub(crate) fn move_food(world: &mut World, from: Entity, to: Entity, max_value: f32) -> f32 {
    if from == to || max_value <= 0.0 {
        return 0.0;
    }
    let econ = world.resource::<Economy>().clone();
    with_two(world, from, to, |src, dst| {
        let mut moved = 0.0;
        for g in econ.foods_by_value() {
            let v = econ.food_value(g);
            if v <= 0.0 {
                continue;
            }
            let want = ((max_value - moved) / v).floor() as u32;
            let qty = want.min(src.get(g)).min(dst.room());
            if qty == 0 {
                continue;
            }
            src.take(g, qty);
            dst.add(g, qty);
            moved += qty as f32 * v;
        }
        moved
    })
    .unwrap_or(0.0)
}

/// Runs `f` on two different entities' stockpiles.
fn with_two<R>(
    world: &mut World,
    a: Entity,
    b: Entity,
    f: impl FnOnce(&mut Stockpile, &mut Stockpile) -> R,
) -> Option<R> {
    let [mut ea, mut eb] = world.get_entity_mut([a, b]).ok()?;
    let mut sa = ea.get_mut::<Stockpile>()?;
    let mut sb = eb.get_mut::<Stockpile>()?;
    Some(f(&mut sa, &mut sb))
}

pub(crate) fn move_good(world: &mut World, from: Entity, to: Entity, g: GoodId, qty: u32) -> u32 {
    if from == to {
        return 0;
    }
    with_two(world, from, to, |src, dst| {
        let qty = qty.min(src.get(g)).min(dst.room());
        src.take(g, qty);
        dst.add(g, qty);
        qty
    })
    .unwrap_or(0)
}

pub(crate) fn storehouse(world: &World, s: Entity) -> Option<Entity> {
    world.get::<SettlementEconomy>(s).map(|se| se.storehouse)
}

/// Where an army's dealings find food at settlement `s`, as `FoodSecurity`
/// counts it (ADR 0018): the storehouse, then, when `homes`, the homes,
/// best-stocked first (ties by SpawnId). Empty for a non-settlement.
pub(crate) fn food_sources(world: &World, s: Entity, homes: bool) -> Vec<Entity> {
    let Some(se) = world.get::<SettlementEconomy>(s) else {
        return Vec::new();
    };
    let mut sources = vec![se.storehouse];
    if homes {
        let econ = world.resource::<Economy>();
        let mut stocked: Vec<(f32, u64, Entity)> = se
            .homes
            .iter()
            .map(|&h| {
                let food = world
                    .get::<Stockpile>(h)
                    .map_or(0.0, |st| st.food_value(econ));
                let id = world.get::<SpawnId>(h).map_or(u64::MAX, |id| id.0);
                (food, id, h)
            })
            .collect();
        stocked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        sources.extend(stocked.into_iter().map(|(.., h)| h));
    }
    sources
}

/// The food value `sources` hold (0.0, not -0.0, when they hold none).
pub(crate) fn food_held(world: &World, sources: &[Entity]) -> f32 {
    let econ = world.resource::<Economy>();
    sources
        .iter()
        .filter_map(|&e| world.get::<Stockpile>(e))
        .fold(0.0, |held, st| held + st.food_value(econ))
}

/// Moves up to `max_value` of food from `sources`, in order, to `to`
/// ([`move_food`] on each until it has `max_value`). Returns the food
/// value moved.
pub(crate) fn take_food(world: &mut World, sources: &[Entity], to: Entity, max_value: f32) -> f32 {
    let mut moved = 0.0;
    for &src in sources {
        let left = max_value - moved;
        if left <= 0.0 {
            break;
        }
        moved += move_food(world, src, to, left);
    }
    moved
}

/// Whether a dealing at `posture` sees the village's homes as well as its
/// storehouse (ADR 0018): in a run, every dealing but plunder.
fn deals_from_homes(world: &World, posture: Posture) -> bool {
    in_run(world) && posture != Posture::Aggressive
}

/// Whether this world plays a run (GP-21).
fn in_run(world: &World) -> bool {
    world.get_resource::<RunState>().is_some_and(|r| r.in_run())
}

/// Days of its own need a village keeps from an army gathering at
/// `posture` (GP-03): army.ron `gather_keep_days`, but in a run a neutral
/// army's is progression.ron `gather.neutral_keep_days` (ADR 0018).
fn gather_keep(world: &World, posture: Posture) -> f32 {
    if posture == Posture::Neutral && in_run(world) {
        world
            .resource::<Progression>()
            .data
            .gather
            .neutral_keep_days
    } else {
        world.resource::<Army>().keep_days(posture)
    }
}

/// Food settlement `s` holds beyond `keep_days` for its people: in its
/// storehouse, plus its homes when `homes` (the pantry, ADR 0018).
pub(crate) fn spare_food(world: &World, s: Entity, keep_days: f32, homes: bool) -> f32 {
    let Some(se) = world.get::<SettlementEconomy>(s) else {
        return 0.0;
    };
    let held = food_held(world, &food_sources(world, s, homes));
    (held - keep_days * daily_need(world, se.population)).max(0.0)
}

/// `food` as days of captain `captain`'s army's need.
fn days_of(world: &mut World, captain: Entity, food: f32) -> f32 {
    let people = army_size(world, captain);
    food / daily_need(world, people).max(1e-3)
}

/// People in captain `captain`'s army, the captain included.
pub(crate) fn army_size(world: &mut World, captain: Entity) -> u32 {
    let mut q = world.query::<&Follower>();
    q.iter(world).filter(|f| f.captain == captain).count() as u32 + 1
}

fn baggage_days(world: &mut World, captain: Entity) -> f32 {
    let people = army_size(world, captain);
    let econ = world.resource::<Economy>();
    let food = world
        .get::<Stockpile>(captain)
        .map_or(0.0, |s| s.food_value(econ));
    food / daily_need(world, people).max(1e-3)
}

/// Marks a settlement as provoked by `faction`: its militia rises (SIM-13).
pub fn provoke(world: &mut World, s: Entity, faction: FactionId) {
    let tick = world.resource::<SimTick>().0;
    let half_day = world.resource::<Calendar>().ticks_per_day / 2;
    let Some(mut threat) = world.get_mut::<Threat>(s) else {
        return;
    };
    let fresh = threat.provoked_by != Some(faction);
    threat.provoked_by = Some(faction);
    threat.provoked_until = tick + half_day;
    threat.level = 1.0;
    if fresh {
        world.resource_mut::<ArmyStats>().of(faction).provocations += 1;
        goodwill_deed(world, s, faction, Deed::Provoke);
        let (who, name) = (faction_name(world, faction), settlement_name(world, s));
        log(world, format!("{name} takes up arms against {who}"));
    }
}

/// Ends `faction`'s provocation of `s` (GP-05): its militia stands down at
/// their next step (`act::perform`). A provocation by anyone else stays.
pub(crate) fn call_off(world: &mut World, s: Entity, faction: FactionId) {
    if let Some(mut threat) = world.get_mut::<Threat>(s)
        && threat.provoked_by == Some(faction)
    {
        threat.provoked_by = None;
        threat.provoked_until = 0;
    }
}

/// GP-22: posts `faction`'s `deed` at settlement `s` to the goodwill
/// ledger, with its delta from progression.ron `goodwill_deeds`. Nothing
/// for an entity without a SpawnId or a zero delta.
pub(crate) fn goodwill_deed(world: &mut World, s: Entity, faction: FactionId, deed: Deed) {
    let delta = world
        .resource::<Progression>()
        .data
        .goodwill_deeds
        .delta(deed);
    if let Some(id) = world.get::<SpawnId>(s).map(|id| id.0) {
        world
            .resource_mut::<GoodwillLedger>()
            .push(id, faction, delta, deed);
    }
}

/// The food value one unit of `g` fetches when `faction` trades it: army.ron
/// `trade_value` times the faction's `TradeValue` modifier (GP-23; exactly
/// `trade_value` without modifiers). Both what a barter offers and what it
/// pays use it.
fn trade_price(army: &Army, mods: &FactionModifiers, faction: FactionId, g: GoodId) -> f32 {
    army.trade_value(g) * mods.mult(faction, Stat::TradeValue)
}

/// The baggage's goods a barter may offer: tradeable (army.ron
/// `trade_value` above 0) and not food, in GoodId order.
fn trade_goods(world: &World, captain: Entity) -> Vec<(GoodId, u32)> {
    let army = world.resource::<Army>();
    let econ = world.resource::<Economy>();
    world
        .get::<Stockpile>(captain)
        .map(|b| {
            b.goods
                .iter()
                .filter(|(g, q)| **q > 0 && army.trade_value(**g) > 0.0 && !econ.is_food(**g))
                .map(|(g, q)| (*g, *q))
                .collect()
        })
        .unwrap_or_default()
}

/// The food value the baggage's [`trade_goods`] fetch at [`trade_price`]:
/// the most a barter can buy.
fn offer_value(world: &World, captain: Entity) -> f32 {
    let army = world.resource::<Army>();
    let mods = world.resource::<FactionModifiers>();
    let faction = world
        .get::<Captain>(captain)
        .map_or(FactionId::INDEPENDENT, |c| c.faction);
    trade_goods(world, captain)
        .iter()
        .map(|(g, q)| trade_price(army, mods, faction, *g) * *q as f32)
        .sum()
}

/// Barters the baggage's non-food goods for food (Trade, and passive
/// gathering), at [`trade_price`], for food beyond `keep_days` of the
/// village's need: from its storehouse, or its pantry when `homes` (ADR
/// 0018). Payment goes to the storehouse. Returns the food value bought.
fn barter(
    world: &mut World,
    captain: Entity,
    s: Entity,
    keep_days: f32,
    room: f32,
    homes: bool,
) -> f32 {
    let Some(store) = storehouse(world, s) else {
        return 0.0;
    };
    let army = world.resource::<Army>().clone();
    let mods = world.resource::<FactionModifiers>().clone();
    let faction = world
        .get::<Captain>(captain)
        .map_or(FactionId::INDEPENDENT, |c| c.faction);
    let price = |g: GoodId| trade_price(&army, &mods, faction, g);
    let goods = trade_goods(world, captain);
    let offer = offer_value(world, captain);
    let want = offer.min(spare_food(world, s, keep_days, homes)).min(room);
    let sources = food_sources(world, s, homes);
    let bought = take_food(world, &sources, captain, want);
    // Pay for it, good by good in GoodId order.
    let mut owed = bought;
    for (g, q) in goods {
        if owed <= 0.0 {
            break;
        }
        let v = price(g);
        let n = ((owed / v).ceil() as u32).min(q);
        let paid = move_good(world, captain, store, g, n);
        owed -= paid as f32 * v;
    }
    if bought > 0.0 && world.get::<Captain>(captain).is_some() {
        goodwill_deed(world, s, faction, Deed::Trade);
    }
    bought
}

/// Food value the baggage lacks to feed the army for `gather_target_days`.
pub(crate) fn baggage_room(world: &mut World, captain: Entity) -> f32 {
    let days = world.resource::<Army>().data.gather_target_days;
    let people = army_size(world, captain);
    let econ = world.resource::<Economy>();
    let held = world
        .get::<Stockpile>(captain)
        .map_or(0.0, |s| s.food_value(econ));
    (days * daily_need(world, people) - held).max(0.0)
}

/// A food dealing that moved nothing, for its line in the military log
/// (GP-02, ADR 0018).
#[derive(Debug, Clone, Copy)]
enum Dealing {
    /// Asking, the hungry take or an owner's supply: the keep that applied
    /// and whether the village's homes counted.
    Asked { keep: f32, homes: bool },
    /// A barter (passive gathering or Trade), likewise.
    Barter { keep: f32, homes: bool },
    /// Plunder (aggressive gathering).
    Plunder,
}

/// `d` days for a log line: one decimal, or two under 0.05 so a sliver
/// never reads "0.0".
fn days_text(d: f32) -> String {
    if d.abs() < 0.05 {
        format!("{d:.2}")
    } else {
        format!("{d:.1}")
    }
}

/// Logs why captain `captain`'s food dealing at `s` moved nothing (ADR
/// 0018), the first that applies: the baggage has no room for a unit of
/// the most nourishing food (`room`, food value); a barter with no goods
/// to trade, or goods worth less than a unit of the cheapest food; a
/// plunder of an empty storehouse; the village holds no more than it keeps
/// (in days of its own need, over the pool the dealing reads); else what
/// it could spare came to less than a unit of the food it holds.
fn say_why(world: &mut World, captain: Entity, s: Entity, room: f32, dealing: Dealing) {
    let faction = world.get::<Captain>(captain).expect("captain").faction;
    let (who, name) = (faction_name(world, faction), settlement_name(world, s));
    let (dearest, cheapest) = {
        let econ = world.resource::<Economy>();
        let foods = econ.foods_by_value();
        let value = |g: Option<&GoodId>| g.map_or(0.0, |&g| econ.food_value(g));
        (value(foods.first()), value(foods.last()))
    };
    let text = if room < dearest.max(1e-3) {
        let have = baggage_days(world, captain);
        let target = world.resource::<Army>().data.gather_target_days;
        format!(
            "{who}'s baggage is full ({have:.1} of {target:.0} days): nothing gathered at {name}"
        )
    } else {
        match dealing {
            Dealing::Barter { .. } if trade_goods(world, captain).is_empty() => {
                format!("{who} has no goods to trade for food at {name}")
            }
            Dealing::Barter { .. } if offer_value(world, captain) < cheapest => {
                let offer = offer_value(world, captain);
                format!(
                    "{who} has too little to trade for food at {name} (its goods fetch {offer:.2} food, less than a unit)"
                )
            }
            Dealing::Plunder => {
                format!("{who} found nothing to plunder at {name} (its storehouse is empty)")
            }
            Dealing::Asked { keep, homes } | Dealing::Barter { keep, homes } => {
                let pop = world
                    .get::<SettlementEconomy>(s)
                    .map_or(0, |se| se.population);
                let held = food_held(world, &food_sources(world, s, homes))
                    / daily_need(world, pop).max(1e-3);
                // Two decimals where one would show the same as the keep.
                let held_text = if (held - keep).abs() < 0.05 {
                    format!("{held:.2}")
                } else {
                    format!("{held:.1}")
                };
                if held <= keep {
                    format!(
                        "{name} had no food to spare for {who} (it holds {held_text} days of food and keeps {keep:.1})"
                    )
                } else {
                    format!(
                        "{name} had only crumbs to spare for {who} (it holds {held_text} days of food and keeps {keep:.1}: less than a unit to share)"
                    )
                }
            }
        }
    };
    log(world, text);
}

/// GatherFood (GP-02) by posture: passive buys; neutral is given some and,
/// when hungry, takes a day's food; aggressive takes freely (up to
/// `gather_target_days` for the army) and provokes the settlement. A
/// settlement the army's faction owns supplies it instead ([`supply`]).
///
/// Outside a run every dealing reads the storehouse, and a neutral army is
/// given half of the smaller of the village's spare (beyond army.ron
/// `gather_keep_days`) and the baggage's room. In a run (ADR 0018) every
/// dealing but plunder reads the pantry ([`food_sources`]), and a neutral
/// army is given half of what the village holds beyond progression.ron
/// `gather.neutral_keep_days`, up to the baggage's room. Plunder stays
/// storehouse-only in every preset.
///
/// Whatever moved is logged with its worth in days of the army's food;
/// when `told` (the player's or the AI's GatherFood, not `forage`) a
/// dealing that moved nothing says why ([`say_why`]).
fn gather_food(world: &mut World, captain: Entity, s: Entity, hungry: bool, told: bool) {
    let c = world.get::<Captain>(captain).expect("captain").clone();
    // A settlement feeds its owner's armies, whatever their posture: they
    // never pay, plunder or provoke it (GP-05).
    if owner_of(world, s) == c.faction {
        supply(world, captain, s, told);
        return;
    }
    let Some(store) = storehouse(world, s) else {
        return;
    };
    let room = baggage_room(world, captain);
    let homes = deals_from_homes(world, c.posture);
    let keep = gather_keep(world, c.posture);
    let name = settlement_name(world, s);
    let who = faction_name(world, c.faction);
    match c.posture {
        Posture::Passive => {
            let bought = barter(world, captain, s, keep, room, homes);
            world.resource_mut::<ArmyStats>().of(c.faction).bought += bought;
            if bought > 0.0 {
                let d = days_text(days_of(world, captain, bought));
                log(world, format!("{who} bought food from {name} (+{d} days)"));
            } else if told {
                say_why(world, captain, s, room, Dealing::Barter { keep, homes });
            }
        }
        Posture::Neutral => {
            // Asks for a share of what the village can spare: half of the
            // smaller of the spare and the room; in a run, half the spare
            // up to the room (ADR 0018)...
            let spare = spare_food(world, s, keep, homes);
            let share = if homes {
                (0.5 * spare).min(room)
            } else {
                spare.min(room) * 0.5
            };
            let given = take_food(world, &food_sources(world, s, homes), captain, share);
            // ...and when hungry takes a day's food for the army, even from
            // the village's reserve.
            let seized = if hungry {
                let people = army_size(world, captain);
                let need = (daily_need(world, people) - given).max(0.0);
                let reserve = spare_food(world, s, 0.0, homes);
                let sources = food_sources(world, s, homes);
                take_food(world, &sources, captain, need.min(reserve))
            } else {
                0.0
            };
            let mut stats = world.resource_mut::<ArmyStats>();
            stats.of(c.faction).given += given;
            stats.of(c.faction).seized += seized;
            if seized > 0.0 {
                goodwill_deed(world, s, c.faction, Deed::Seize);
                let d = days_text(days_of(world, captain, given + seized));
                log(world, format!("{who} took food from {name} (+{d} days)"));
            } else if given > 0.0 {
                let d = days_text(days_of(world, captain, given));
                log(world, format!("{name} shared food with {who} (+{d} days)"));
            } else if told {
                let keep = if hungry { 0.0 } else { keep };
                say_why(world, captain, s, room, Dealing::Asked { keep, homes });
            }
        }
        Posture::Aggressive => {
            // Plunder takes from the storehouse only, in every preset.
            let spare = spare_food(world, s, keep, false).min(room);
            let seized = move_food(world, store, captain, spare);
            world.resource_mut::<ArmyStats>().of(c.faction).seized += seized;
            if seized > 0.0 {
                goodwill_deed(world, s, c.faction, Deed::Plunder);
                let d = days_text(days_of(world, captain, seized));
                log(world, format!("{who} plundered {name} (+{d} days)"));
                provoke(world, s, c.faction);
            } else if told {
                say_why(world, captain, s, room, Dealing::Plunder);
            }
        }
    }
}

/// Owned-settlement supply (GP-05): the owner's army takes the food the
/// settlement holds beyond `garrison.keep_days` of its own need, up to its
/// baggage target, unpaid and without provoking anyone: from the
/// storehouse, and in a run from the pantry, storehouse first (ADR 0018).
/// Counted in `FactionStats::supplied`. Logs what came, or, when `told`,
/// why nothing did ([`say_why`]). Returns the food value taken.
fn supply(world: &mut World, captain: Entity, s: Entity, told: bool) -> f32 {
    let faction = world.get::<Captain>(captain).expect("captain").faction;
    if storehouse(world, s).is_none() {
        return 0.0;
    }
    let homes = in_run(world);
    let keep = world.resource::<Army>().data.garrison.keep_days;
    let room = baggage_room(world, captain);
    let spare = spare_food(world, s, keep, homes).min(room);
    let sources = food_sources(world, s, homes);
    let got = take_food(world, &sources, captain, spare);
    if got > 0.0 {
        world.resource_mut::<ArmyStats>().of(faction).supplied += got;
        let (who, name) = (faction_name(world, faction), settlement_name(world, s));
        let d = days_text(days_of(world, captain, got));
        log(world, format!("{name} supplied {who} (+{d} days)"));
    } else if told {
        say_why(world, captain, s, room, Dealing::Asked { keep, homes });
    }
    got
}

/// Trade (GP-02): barters the baggage's goods for the food the settlement
/// holds beyond army.ron `trade_keep_days`, from its storehouse, or in a
/// run its pantry (ADR 0018), and logs what it bought or why it bought
/// nothing.
fn trade(world: &mut World, captain: Entity, s: Entity) {
    let faction = world.get::<Captain>(captain).expect("captain").faction;
    let room = baggage_room(world, captain);
    let keep = world.resource::<Army>().data.trade_keep_days;
    let homes = in_run(world);
    let bought = barter(world, captain, s, keep, room, homes);
    world.resource_mut::<ArmyStats>().of(faction).bought += bought;
    if bought > 0.0 {
        let (who, name) = (faction_name(world, faction), settlement_name(world, s));
        let d = days_text(days_of(world, captain, bought));
        log(world, format!("{who} bought food from {name} (+{d} days)"));
    } else {
        say_why(world, captain, s, room, Dealing::Barter { keep, homes });
    }
}

/// Give (GP-22): the army hands the food its baggage holds beyond
/// `give_keep_days` of its daily need to the settlement's storehouse (most
/// nourishing first, as far as the storehouse has room). Returns the food
/// value delivered, which is what `move_food` actually moved, and counts it
/// in `FactionStats::donated`. The orders step credits this value to the
/// settlement's open BringFood request ([`crate::requests::delivered`]).
/// A gift also pauses the army's automatic gathering from villages
/// (`forage`) for a quarter day, as any dealing does, so a neutral army
/// doesn't seize the food straight back.
pub(crate) fn give(world: &mut World, captain: Entity, s: Entity) -> f32 {
    let faction = world.get::<Captain>(captain).expect("captain").faction;
    let Some(store) = storehouse(world, s) else {
        return 0.0;
    };
    let keep = world.resource::<Army>().data.give_keep_days;
    let people = army_size(world, captain);
    let held = {
        let econ = world.resource::<Economy>();
        world
            .get::<Stockpile>(captain)
            .map_or(0.0, |b| b.food_value(econ))
    };
    let spare = (held - keep * daily_need(world, people)).max(0.0);
    let delivered = move_food(world, captain, store, spare);
    let (who, name) = (faction_name(world, faction), settlement_name(world, s));
    if delivered > 0.0 {
        // The baggage now holds `give_keep_days`, at the edge of what makes
        // a neutral army help itself to a nearby village (`forage`): without
        // a pause it would take the gift straight back. A gift counts as
        // the army's dealing with villages for the next quarter day.
        let tick = world.resource::<SimTick>().0;
        world
            .get_mut::<Captain>(captain)
            .expect("captain")
            .last_forage = tick.max(1);
        world.resource_mut::<ArmyStats>().of(faction).donated += delivered;
        log(world, format!("{who} gave food to {name}"));
    } else {
        log(world, format!("{who} had no food to spare for {name}"));
    }
    delivered
}

fn good_named(econ: &Economy, name: &str) -> Option<GoodId> {
    econ.data
        .goods
        .iter()
        .position(|g| g.name == name)
        .map(|i| GoodId(i as u8))
}

/// Equip (GP-02, SIM-32): weapons and tools from the storehouse, by
/// posture, then handed to whoever lacks them (SpawnId order).
fn equip(world: &mut World, captain: Entity, s: Entity) {
    let c = world.get::<Captain>(captain).expect("captain").clone();
    let army = world.resource::<Army>().clone();
    let econ = world.resource::<Economy>().clone();
    let Some(store) = storehouse(world, s) else {
        return;
    };
    let name = settlement_name(world, s);
    let who = faction_name(world, c.faction);
    let mut took = 0;
    for good in ["weapons", "tools"] {
        let Some(g) = good_named(&econ, good) else {
            continue;
        };
        let have = world.get::<Stockpile>(store).map_or(0, |st| st.get(g));
        let n = match c.posture {
            Posture::Passive => {
                // Pay in food, at most half the baggage.
                let price = army.trade_value(g).max(0.1);
                let food = world
                    .get::<Stockpile>(captain)
                    .map_or(0.0, |b| b.food_value(&econ));
                let n = have.min((food * 0.5 / price) as u32);
                let paid = move_food(world, captain, store, n as f32 * price);
                world.resource_mut::<ArmyStats>().of(c.faction).bought += paid;
                if paid > 0.0 {
                    goodwill_deed(world, s, c.faction, Deed::Equip);
                }
                n
            }
            Posture::Neutral => have / 2,
            Posture::Aggressive => have,
        };
        took += move_good(world, store, captain, g, n);
    }
    if took > 0 {
        log(world, format!("{who} took {took} arms from {name}"));
        if c.posture == Posture::Aggressive && owner_of(world, s) != c.faction {
            provoke(world, s, c.faction);
        }
    }
    arm_followers(world, captain);
}

/// Hands weapons and tools from the baggage to followers lacking them.
pub fn arm_followers(world: &mut World, captain: Entity) {
    let econ = world.resource::<Economy>().clone();
    let (Some(weapons), Some(tools)) = (good_named(&econ, "weapons"), good_named(&econ, "tools"))
    else {
        return;
    };
    let mut q = world.query::<(Entity, &SpawnId, &Follower, &Equipment)>();
    let mut members: Vec<(SpawnId, Entity, Equipment)> = q
        .iter(world)
        .filter(|(_, _, f, _)| f.captain == captain)
        .map(|(e, id, _, eq)| (*id, e, *eq))
        .collect();
    members.sort_unstable_by_key(|m| m.0);
    for (_, e, eq) in members {
        let mut new = eq;
        let Some(mut bag) = world.get_mut::<Stockpile>(captain) else {
            return;
        };
        if !eq.weapon && bag.take(weapons, 1) == 1 {
            new.weapon = true;
        }
        if !eq.tools && bag.take(tools, 1) == 1 {
            new.tools = true;
        }
        if new != eq {
            *world.get_mut::<Equipment>(e).expect("equipment") = new;
        }
    }
}

/// Recruit (GP-02): adults of the settlement join, idle ones first, each
/// with a chance set by posture, the village's hunger and the army's food.
fn recruit(world: &mut World, captain: Entity, s: Entity, count: u16) {
    let tick = world.resource::<SimTick>().0;
    let c = world.get::<Captain>(captain).expect("captain").clone();
    // GP-06: another warlord's village sends nobody; your own sends more
    // the more loyal it is.
    let owner = owner_of(world, s);
    if owner != FactionId::INDEPENDENT && owner != c.faction {
        let (who, name) = (faction_name(world, c.faction), settlement_name(world, s));
        let holder = faction_name(world, owner);
        log(
            world,
            format!("{name} answers to {holder}: nobody joins {who}"),
        );
        return;
    }
    let cid = world.get::<SpawnId>(captain).expect("id").0;
    let army = world.resource::<Army>().clone();
    let food_days = world
        .get::<sim_agents::FoodSecurity>(s)
        .map_or(10.0, |f| f.days);
    let mut p = army.willing(c.posture);
    if food_days < 2.0 {
        p += army.data.recruit.hungry_village_bonus;
    }
    if baggage_days(world, captain) > 2.0 {
        p += army.data.recruit.fed_army_bonus;
    }
    if owner == c.faction {
        let loyalty = world.get::<SettlementLoyalty>(s).map_or(0.0, |l| l.mean);
        p += army.data.recruit.loyalty_bonus * loyalty;
    }
    // SIM-34: the faction's RecruitChance modifier (a Beloved champion);
    // exactly 1.0 without one.
    p *= world
        .resource::<FactionModifiers>()
        .mult(c.faction, Stat::RecruitChance);
    let candidates = recruit_candidates(world, s);
    let mut rng = world
        .resource::<SimSeed>()
        .stream(rng_key("sim_military::recruit") ^ cid, tick);
    let mut joined = Vec::new();
    for e in candidates {
        if joined.len() >= count as usize {
            break;
        }
        if rng.random::<f32>() < p {
            joined.push(e);
        }
    }
    for &e in &joined {
        enlist_into(world, e, captain);
    }
    let n = joined.len() as u32;
    let (who, name) = (faction_name(world, c.faction), settlement_name(world, s));
    log(world, format!("{n} of {name} joined {who}"));
}

/// Who of settlement `s` could join an army (GP-02): working-age, alive,
/// civilian residents who aren't enlisted, idle ones first, then by
/// SpawnId. Recruiting, drifters and request volunteers all draw from
/// this list.
pub(crate) fn recruit_candidates(world: &mut World, s: Entity) -> Vec<Entity> {
    let days_per_year = world.resource::<Calendar>().days_per_year();
    let working = world.resource::<Villagers>().data.households.working_age;
    let mut q = world.query_filtered::<(Entity, &SpawnId, &Villager, &Occupation, &CombatStats), Without<Enlisted>>();
    let mut candidates: Vec<(bool, SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, v, _, cs)| {
            let years = v.age_days as u32 / days_per_year;
            v.settlement == s
                && cs.alive()
                && cs.stance == Stance::Civilian
                && years >= working.min
                && years <= working.max
        })
        .map(|(e, id, _, occ, _)| (*occ != Occupation::Idle, *id, e))
        .collect();
    candidates.sort_unstable_by_key(|(busy, id, _)| (*busy, *id));
    candidates.into_iter().map(|(_, _, e)| e).collect()
}

/// GP-02, GP-22: villager `villager` joins `captain`'s army, with the
/// bookkeeping every way of joining shares: [`enlist`] (allegiance, gear,
/// follower), the baggage grows by army.ron `baggage_per_head`, the home
/// settlement's population drops by one, and the faction's
/// `FactionStats::recruited` counts it. Used by `Recruit`, drifters and
/// request volunteers. Returns false, changing nothing, if `villager` isn't
/// a villager, is already enlisted, or `captain` isn't a captain.
pub fn enlist_into(world: &mut World, villager: Entity, captain: Entity) -> bool {
    let Some(faction) = world.get::<Captain>(captain).map(|c| c.faction) else {
        return false;
    };
    let Some(s) = world.get::<Villager>(villager).map(|v| v.settlement) else {
        return false;
    };
    if world.get::<Enlisted>(villager).is_some() {
        return false;
    }
    let per_head = world.resource_scope(|world, army: Mut<Army>| {
        enlist(world, villager, captain, faction, &army);
        army.data.baggage_per_head
    });
    if let Some(mut bag) = world.get_mut::<Stockpile>(captain) {
        bag.capacity += per_head;
    }
    if let Some(mut se) = world.get_mut::<SettlementEconomy>(s) {
        se.population = se.population.saturating_sub(1);
    }
    world.resource_mut::<ArmyStats>().of(faction).recruited += 1;
    true
}

/// Turns a villager into a follower. It keeps its identity and home, so its
/// village notices the loss; it drops its job. Its [`Veteran`] remembers
/// the trade it had (M4a, `Veteran::trade`, in every preset; read only for
/// stewards).
///
/// [`Veteran`]: crate::veterans::Veteran
pub fn enlist(world: &mut World, e: Entity, captain: Entity, faction: FactionId, army: &Army) {
    let trade = world.get::<Occupation>(e).copied();
    if let Some(mut job) = world.get_mut::<Employment>(e)
        && let Some(wp) = job.workplace.take()
        && let Some(mut w) = world.get_mut::<Workplace>(wp)
    {
        w.workers.retain(|x| *x != e);
    }
    if let Some(mut occ) = world.get_mut::<Occupation>(e) {
        *occ = Occupation::Idle;
    }
    let s = army.data.soldier;
    if let Some(mut cs) = world.get_mut::<CombatStats>(e) {
        let share = (cs.health / cs.max_health.max(1.0)).clamp(0.0, 1.0);
        *cs = CombatStats {
            health: s.health * share,
            max_health: s.health,
            attack: s.attack,
            defense: s.defense,
            morale: s.morale,
            stance: Stance::Soldier,
        };
    }
    if let Some(mut act) = world.get_mut::<Activity>(e) {
        *act = Activity {
            kind: ActionKind::Follow,
            stage: Stage::Perform,
            ..Default::default()
        };
    }
    if let Some(mut t) = world.get_mut::<Travel>(e) {
        t.state = TravelState::Idle;
    }
    world.entity_mut(e).insert((
        Allegiance(faction),
        Enlisted,
        Follower {
            captain,
            slot: 0,
            deserting: false,
        },
        Equipment::default(),
        // SIM-36: a recruit's service starts now; M4a: with its trade.
        crate::veterans::Veteran {
            trade,
            ..Default::default()
        },
    ));
}

pub(crate) fn center(world: &World, s: Entity) -> Option<IVec2> {
    world.get::<Settlement>(s).map(|st| st.center)
}

/// Whether militia within `radius` of `at` still stand against `faction`.
fn militia_standing(world: &World, at: IVec2, faction: FactionId, radius: f32) -> bool {
    let grid = world.resource::<FighterGrid>();
    let mut any = false;
    grid.for_each_within(at.as_vec2(), radius, |e, _| {
        if e.side.against == Some(faction) && !e.routed {
            any = true;
        }
    });
    any
}

/// Input set, after commands: each captain works on its first order and
/// forages when hungry. Serial, in SpawnId order.
pub(crate) fn run_orders(world: &mut World) {
    let _span = tracing::info_span!("sim_military::run_orders").entered();
    let tick = world.resource::<SimTick>().0;
    let mut q = world.query::<(Entity, &SpawnId, &Captain)>();
    let mut captains: Vec<(SpawnId, Entity)> = q.iter(world).map(|(e, id, _)| (*id, e)).collect();
    captains.sort_unstable();
    for (_, e) in captains {
        if world.get::<Captain>(e).is_none() {
            continue;
        }
        step_order(world, e, tick);
        forage(world, e, tick);
        ensure_field(world, e);
        ensure_rally(world, e, tick);
    }
    lands(world, tick);
}

/// The weapons and tools in settlement `s`'s storehouse (GP-34).
pub(crate) fn store_arms(world: &World, s: Entity) -> u32 {
    let econ = world.resource::<Economy>();
    let Some(st) = storehouse(world, s).and_then(|e| world.get::<Stockpile>(e)) else {
        return 0;
    };
    ["weapons", "tools"]
        .into_iter()
        .filter_map(|g| good_named(econ, g))
        .map(|g| st.get(g))
        .sum()
}

/// The nearest captain of `faction` within `reach` tiles of settlement
/// `s`'s centre (GP-34), by distance, then SpawnId; with `room`, only one
/// whose baggage has room. Tithes and levies go to it.
pub(crate) fn collector(
    world: &mut World,
    s: Entity,
    faction: FactionId,
    reach: f32,
    room: bool,
) -> Option<Entity> {
    let at = center(world, s)?;
    let mut q = world.query::<(Entity, &SpawnId, &Captain, &Position, Option<&Stockpile>)>();
    q.iter(world)
        .filter(|(_, _, c, _, bag)| {
            c.faction == faction && (!room || bag.is_some_and(|b| b.room() > 0))
        })
        .map(|(e, id, _, p, _)| ((p.0 - at).as_vec2().length(), *id, e))
        .filter(|(d, ..)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(.., e)| e)
}

/// GP-34 (M4a step 4): the lands' dues, a pass of the logistics step
/// (`run_orders`), run only, on the tick after each hourly
/// `govern::step`. Each settlement the player owns with Dues at Tithe or
/// Levy, in SpawnId order, delivers when its dial turned in that hourly
/// step or it was the day tick (so in the hour you set it, then once a
/// day):
///
/// - **Tithe:** the nearest owner captain within `edicts.reach` with room
///   gets the food beyond `edicts.tithe.keep_days` (the pantry, storehouse
///   first, as `supply` reads it; at most its baggage room) and the
///   storehouse's weapons and tools, then arms its followers.
/// - **Levy:** every mustered resident (SpawnId order) is enlisted
///   (`enlist_into`) into the nearest owner captain within reach, a
///   garrison standing at the town included, and walks to it.
///
/// No collector within reach moves nothing that day. One `Tribute` event
/// (feed only) and log line per receiving captain.
pub(crate) fn lands(world: &mut World, tick: u64) {
    let _span = tracing::info_span!("sim_military::orders::lands").entered();
    let cal = world.resource::<Calendar>().clone();
    if tick == 0 || !cal.on_hour(tick - 1) || !in_run(world) {
        return;
    }
    let hour = tick - 1;
    let daily = cal.on_day(hour);
    let data = world.resource::<Progression>().data.edicts;
    let mut q = world.query::<(Entity, &SpawnId, &sim_agents::Edicts)>();
    let mut towns: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, e)| {
            e.dues != sim_core::Dues::Light && (daily || e.dial(sim_core::Dial::Dues).since == hour)
        })
        .map(|(e, id, _)| (*id, e))
        .collect();
    if towns.is_empty() {
        return;
    }
    towns.sort_unstable();
    let owner = FactionId::PLAYER;
    // Per receiving captain: (food value, arms, men, towns).
    let mut sent: Vec<(SpawnId, Entity, f32, u32, u32, Vec<String>)> = Vec::new();
    for (_, s) in towns {
        if owner_of(world, s) != owner {
            continue;
        }
        let dues = world.get::<sim_agents::Edicts>(s).expect("edicts").dues;
        let (mut food, mut arms, mut men) = (0.0, 0u32, 0u32);
        let to = match dues {
            sim_core::Dues::Tithe => {
                let Some(c) = collector(world, s, owner, data.reach, true) else {
                    continue;
                };
                let spare = spare_food(world, s, data.tithe.keep_days, true);
                let room = baggage_room(world, c);
                food = take_food(world, &food_sources(world, s, true), c, spare.min(room));
                let econ = world.resource::<Economy>().clone();
                if let Some(store) = storehouse(world, s) {
                    for g in ["weapons", "tools"]
                        .into_iter()
                        .filter_map(|g| good_named(&econ, g))
                    {
                        let have = world.get::<Stockpile>(store).map_or(0, |st| st.get(g));
                        arms += move_good(world, store, c, g, have);
                    }
                }
                arm_followers(world, c);
                c
            }
            sim_core::Dues::Levy => {
                let Some(c) = collector(world, s, owner, data.reach, false) else {
                    continue;
                };
                for v in crate::govern::mustered(world, s) {
                    world
                        .entity_mut(v)
                        .remove::<(sim_agents::Mustered, sim_agents::Rationed)>();
                    if enlist_into(world, v, c) {
                        men += 1;
                    }
                }
                c
            }
            sim_core::Dues::Light => continue,
        };
        if food <= 0.0 && arms == 0 && men == 0 {
            continue;
        }
        let name = settlement_name(world, s);
        let id = *world.get::<SpawnId>(to).expect("captain id");
        match sent.iter_mut().find(|r| r.1 == to) {
            Some(r) => {
                r.2 += food;
                r.3 += arms;
                r.4 += men;
                r.5.push(name);
            }
            None => sent.push((id, to, food, arms, men, vec![name])),
        }
    }
    sent.sort_unstable_by_key(|r| r.0);
    for (id, c, food, arms, men, names) in sent {
        let at = world.get::<Position>(c).map_or(IVec2::ZERO, |p| p.0);
        world.write_message(
            sim_core::SimEvent::new(sim_core::SimEventKind::Tribute, owner)
                .subject(id.0)
                .other(((arms as u64) << 32) | food.floor().max(0.0) as u64)
                .at(at)
                .amount(men as i32)
                .cause(Cause::Edict),
        );
        let days = days_of(world, c, food);
        let who = faction_name(world, owner);
        log(
            world,
            format!(
                "{who}'s lands sent captain {} {} days of food, {arms} arms and {men} men ({})",
                id.0,
                days_text(days),
                names.join(", ")
            ),
        );
    }
}

fn step_order(world: &mut World, e: Entity, tick: u64) {
    let army = world.resource::<Army>().clone();
    let cal = world.resource::<Calendar>().clone();
    let tile = world.get::<Position>(e).expect("position").0;
    let mut c = world.get::<Captain>(e).expect("captain").clone();
    let Some(mut order) = c.orders.front().copied() else {
        if c.goal.is_some() {
            c.goal = None;
            *world.get_mut::<Captain>(e).expect("captain") = c;
        }
        return;
    };
    let arrive = army.data.formation.arrive_radius;
    let near = |goal: IVec2, r: f32| (goal - tile).as_vec2().length() <= r;
    let mut done = false;
    let settlement_of = |world: &mut World, id: u64| find_settlement(world, id);
    match order.spec {
        OrderSpec::MoveTo { x, y } => {
            let g = IVec2::new(x, y);
            c.goal = Some(g);
            done = near(g, arrive);
        }
        OrderSpec::Return => match world.resource::<Factions>().seat(c.faction) {
            Some(seat) => {
                c.goal = Some(seat);
                done = near(seat, arrive);
            }
            None => done = true,
        },
        OrderSpec::Scout { x, y } => {
            if order.step == 0 {
                order.step = 1;
                order.origin = tile;
                order.since = tick;
            }
            let g = if order.step == 1 {
                IVec2::new(x, y)
            } else {
                order.origin
            };
            c.goal = Some(g);
            if near(g, arrive) {
                if order.step == 1 {
                    order.step = 2;
                } else {
                    done = true;
                }
            }
        }
        OrderSpec::Forage { radius } => {
            // SIM-38: hold here and live off the land, a harvest every
            // `forage.every_ticks`, until the baggage is full or the
            // patches within the radius are empty.
            c.goal = None;
            if order.step == 0 {
                order.step = 1;
                order.since = tick;
                order.origin = tile;
            }
            let every = world
                .resource::<Progression>()
                .data
                .forage
                .every_ticks
                .max(1);
            let held = tick - order.since;
            if held > 0 && held.is_multiple_of(every) {
                *world.get_mut::<Captain>(e).expect("captain") = c.clone();
                let who = faction_name(world, c.faction);
                match harvest(world, e, radius, tick) {
                    Harvest::Gathered(_) => {}
                    Harvest::Full => {
                        log(world, format!("{who}: the baggage is full of wild food"));
                        done = true;
                    }
                    Harvest::Exhausted => {
                        log(world, format!("{who}: nothing left to forage here"));
                        done = true;
                    }
                }
                c = world.get::<Captain>(e).expect("captain").clone();
            }
        }
        OrderSpec::Garrison { settlement } => {
            // GP-05: stand at the centre for as long as the settlement is
            // ours; its spare food feeds the army (`forage`).
            match settlement_of(world, settlement) {
                Some(s) if owner_of(world, s) == c.faction => {
                    c.goal = Some(center(world, s).unwrap_or(tile));
                }
                Some(s) => {
                    done = true;
                    let (who, name) = (faction_name(world, c.faction), settlement_name(world, s));
                    log(world, format!("{who} no longer holds {name}"));
                }
                None => done = true,
            }
        }
        // GP-32 (M4a): refused by `validate_spec` until steps 5 and 7, so
        // never queued; dropped if one ever is.
        OrderSpec::Demand { .. } | OrderSpec::Subvert { .. } => done = true,
        OrderSpec::Recruit { settlement, .. }
        | OrderSpec::GatherFood { settlement }
        | OrderSpec::Equip { settlement }
        | OrderSpec::Trade { settlement }
        | OrderSpec::Give { settlement }
        | OrderSpec::Attack { settlement } => {
            let Some(s) = settlement_of(world, settlement) else {
                c.orders.pop_front();
                *world.get_mut::<Captain>(e).expect("captain") = c;
                return;
            };
            let at = center(world, s).unwrap_or(tile);
            c.goal = Some(at);
            let reach = army.data.recruit.radius;
            if let OrderSpec::Attack { .. } = order.spec {
                *world.get_mut::<Captain>(e).expect("captain") = c.clone();
                done = attack(world, e, s, at, &mut order, tick, &army, &cal);
            } else if near(at, reach) {
                // Write back first: the dealings below read the captain.
                *world.get_mut::<Captain>(e).expect("captain") = c.clone();
                match order.spec {
                    OrderSpec::Recruit { count, .. } => recruit(world, e, s, count),
                    OrderSpec::GatherFood { .. } => {
                        let hungry = baggage_days(world, e) < 1.0;
                        gather_food(world, e, s, hungry, true);
                    }
                    OrderSpec::Equip { .. } => equip(world, e, s),
                    OrderSpec::Trade { .. } => trade(world, e, s),
                    OrderSpec::Give { .. } => {
                        // GP-22: what arrived counts toward the
                        // settlement's open BringFood request.
                        let delivered = give(world, e, s);
                        crate::requests::delivered(world, s, c.faction, delivered);
                    }
                    _ => {}
                }
                c = world.get::<Captain>(e).expect("captain").clone();
                done = true;
            }
        }
    }
    if done {
        c.orders.pop_front();
        if c.orders.is_empty() {
            c.goal = None;
        }
    } else if let Some(front) = c.orders.front_mut() {
        *front = order;
    }
    *world.get_mut::<Captain>(e).expect("captain") = c;
}

/// Attack (GP-05), one tick. Returns whether the order is over.
///
/// 0. March on the settlement.
/// 1. Within half the threat radius: provoke it every tick and fight its
///    militia. Once at least `conquest.break_ticks` have passed and no
///    militia stands within `conquest.militia_radius` of the centre, the
///    defenders are broken; after `conquest.give_up_days` the assault
///    fails.
/// 2. Hold: the break calls off the provocation, so the militia stands
///    down, and `forage` leaves the held settlement alone (an aggressive
///    army would otherwise plunder and provoke it again). The captain within `conquest.hold_radius` of the centre with
///    nobody contesting it ([`militia_or_foes_near`]) for
///    `conquest.hold_hours` takes the settlement ([`transfer`],
///    [`Cause::Conquest`]). Any contest restarts the hold; after
///    `conquest.timeout_hours` the attack fails.
///
/// A settlement that became the attacker's meanwhile ends the order.
#[allow(clippy::too_many_arguments)]
fn attack(
    world: &mut World,
    e: Entity,
    s: Entity,
    at: IVec2,
    order: &mut Order,
    tick: u64,
    army: &Army,
    cal: &Calendar,
) -> bool {
    let faction = world.get::<Captain>(e).expect("captain").faction;
    if owner_of(world, s) == faction {
        return true;
    }
    let tile = world.get::<Position>(e).expect("position").0;
    let near = |r: f32| (at - tile).as_vec2().length() <= r;
    let rules = &army.data.conquest;
    let names = |world: &World| (faction_name(world, faction), settlement_name(world, s));
    if order.step == 0 && near(army.data.threat.radius * 0.5) {
        order.step = 1;
        order.since = tick;
    }
    if order.step == 1 {
        provoke(world, s, faction);
        let fought = tick.saturating_sub(order.since);
        if fought > rules.break_ticks && !militia_standing(world, at, faction, rules.militia_radius)
        {
            let (who, name) = names(world);
            log(world, format!("{who} broke the defenders of {name}"));
            // The village submits: call off the provocation, or its
            // half-day timer would keep raising militia through the hold.
            call_off(world, s, faction);
            order.step = 2;
            order.since = tick;
            order.held_since = None;
        } else if fought > cal.days(rules.give_up_days) {
            let (who, name) = names(world);
            log(world, format!("{who} gave up the attack on {name}"));
            return true;
        }
        return false;
    }
    if order.step == 2 {
        let held = near(rules.hold_radius)
            && !militia_or_foes_near(world, at, faction, rules.militia_radius);
        if !held {
            order.held_since = None;
        } else {
            let start = *order.held_since.get_or_insert(tick);
            if tick - start >= cal.hours(rules.hold_hours) {
                let captain = world.get::<SpawnId>(e).map_or(0, |id| id.0);
                transfer(world, s, faction, Cause::Conquest { captain });
                return true;
            }
        }
        if tick.saturating_sub(order.since) > cal.hours(rules.timeout_hours) {
            let (who, name) = names(world);
            log(world, format!("{who} failed to take {name}"));
            return true;
        }
    }
    false
}

/// Armies feed themselves from settlements they pass, by posture (GP-03):
/// passive buys when hungry, neutral asks (and takes when hungry),
/// aggressive takes whenever the baggage is short ([`gather_food`], from
/// the pantry in a run but for plunder, ADR 0018). A settlement the army's
/// faction owns supplies it instead (GP-05, [`supply`]): whenever the
/// baggage holds less than `gather_target_days`, at most once a day, never
/// provoking. Foraging is never told: a draw that moved nothing logs no
/// line (it retries every 30 ticks).
/// A settlement the army is holding after breaking its defenders (`Attack`
/// step 2) is left alone. Soldiers who never eat
/// ([`crate::monuments::NoUpkeep`]: guard camps, a band for hire) never
/// gather.
fn forage(world: &mut World, e: Entity, tick: u64) {
    // GP-23, GP-27: a monument's guard camp or a band for hire never eats,
    // so never gathers (a band of the independents would otherwise draw on
    // an independent village as its owner's supplies).
    if world.get::<crate::monuments::NoUpkeep>(e).is_some() {
        return;
    }
    let tpd = world.resource::<Calendar>().ticks_per_day;
    let c = world.get::<Captain>(e).expect("captain").clone();
    if tick < c.last_forage + tpd / 4 && c.last_forage != 0 {
        return;
    }
    // Looked at twice a minute of game time, not every tick.
    let id = world.get::<SpawnId>(e).map_or(0, |s| s.0);
    if !(tick + id).is_multiple_of(30) {
        return;
    }
    let days = baggage_days(world, e);
    let tile = world.get::<Position>(e).expect("position").0;
    let data = &world.resource::<Army>().data;
    let (radius, target_days) = (data.upkeep.forage_radius, data.gather_target_days);
    let mut q = world.query_filtered::<(Entity, &SpawnId, &Settlement), With<SettlementEconomy>>();
    let nearest = q
        .iter(world)
        .map(|(s, id, st)| ((st.center - tile).as_vec2().length(), *id, s))
        .filter(|(d, ..)| *d <= radius)
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let Some((_, sid, s)) = nearest else {
        return;
    };
    // GP-05: a village that submitted to this army's Attack (step 2, the
    // hold) is neither plundered nor provoked while it is held, or its
    // militia would rise again and contest the hold. The army lives on its
    // baggage until the hold ends; the slot isn't used up.
    if holding(&c, sid) {
        return;
    }
    if owner_of(world, s) == c.faction {
        // Supply: a draw that found nothing spare doesn't use up the day,
        // so the army tries again (every 30 ticks) once the farms deliver.
        let due = c.last_forage == 0 || tick >= c.last_forage + tpd;
        if due && days < target_days && supply(world, e, s, false) > 0.0 {
            world.get_mut::<Captain>(e).expect("captain").last_forage = tick.max(1);
        }
        return;
    }
    if days >= forages_below(c.posture) {
        return;
    }
    world.get_mut::<Captain>(e).expect("captain").last_forage = tick.max(1);
    gather_food(world, e, s, days < 1.0, false);
}

/// Days of food under which an army helps itself, without an order, at a
/// village it passes that its faction doesn't own (GP-03, `forage`):
/// passive (buys) and neutral (asks, and takes a day's food) under a day,
/// aggressive (plunders) under three. The captain panel says so (ADR 0018).
pub fn forages_below(posture: Posture) -> f32 {
    match posture {
        Posture::Passive | Posture::Neutral => 1.0,
        Posture::Aggressive => 3.0,
    }
}

/// Whether the captain's current order is an `Attack` on settlement `sid`
/// that has broken its defenders and is holding the centre (step 2).
fn holding(c: &Captain, sid: SpawnId) -> bool {
    matches!(c.orders.front(), Some(o) if o.step >= 2
        && o.spec == OrderSpec::Attack { settlement: sid.0 })
}

/// Builds (or reuses) the rally field followers use to rejoin the
/// captain: toward the captain's tile, over the army's extent. Rebuilt
/// when the captain has moved a few tiles, and now and then to take in
/// strays.
fn ensure_rally(world: &mut World, e: Entity, tick: u64) {
    const MOVE: i32 = 6;
    const MARGIN: i32 = 24;
    const MAX_HALF: i32 = 160;
    let tile = world.get::<Position>(e).expect("position").0;
    let id = world.get::<SpawnId>(e).map_or(0, |s| s.0);
    let mut c = world.get::<Captain>(e).expect("captain").clone();
    let stale = match c.rally {
        None => true,
        Some(r) => (r - tile).abs().max_element() >= MOVE || (tick + id).is_multiple_of(120),
    };
    if stale {
        let mut lo = tile;
        let mut hi = tile;
        let mut q = world.query::<(&Follower, &Position)>();
        for (f, p) in q.iter(world) {
            if f.captain == e {
                lo = lo.min(p.0);
                hi = hi.max(p.0);
            }
        }
        let size = world.resource::<WorldMap>().size() as i32;
        let lo = (lo - IVec2::splat(MARGIN))
            .max(tile - IVec2::splat(MAX_HALF))
            .max(IVec2::ZERO);
        let hi = (hi + IVec2::splat(MARGIN))
            .min(tile + IVec2::splat(MAX_HALF))
            .min(IVec2::splat(size - 1));
        c.rally = Some(tile);
        c.rally_window = (lo, hi);
        *world.get_mut::<Captain>(e).expect("captain") = c.clone();
    }
    let Some(rally) = c.rally else { return };
    let costs = world.resource::<Villagers>().travel_costs.clone();
    world.resource_scope(|world, mut flows: Mut<FlowCache>| {
        let map = world.resource::<WorldMap>();
        flows.get_or_build(map, &costs, (rally, c.rally_window.0, c.rally_window.1));
    });
}

/// Builds (or reuses) the flow field toward the captain's goal.
fn ensure_field(world: &mut World, e: Entity) {
    let Some(goal) = world.get::<Captain>(e).and_then(|c| c.goal) else {
        return;
    };
    let tile = world.get::<Position>(e).expect("position").0;
    let mut c = world.get::<Captain>(e).expect("captain").clone();
    let (lo, hi) = c.window;
    let inside = |p: IVec2| p.x >= lo.x && p.y >= lo.y && p.x <= hi.x && p.y <= hi.y;
    let fresh = lo == IVec2::ZERO && hi == IVec2::ZERO;
    // Keep the window while the army and goal are inside it.
    let key_ok = !fresh && inside(tile) && inside(goal);
    if !key_ok {
        let margin = IVec2::splat(48);
        let size = world.resource::<WorldMap>().size() as i32;
        c.window = (
            (tile.min(goal) - margin).max(IVec2::ZERO),
            (tile.max(goal) + margin).min(IVec2::splat(size - 1)),
        );
        *world.get_mut::<Captain>(e).expect("captain") = c.clone();
    }
    let costs = world.resource::<Villagers>().travel_costs.clone();
    world.resource_scope(|world, mut flows: Mut<FlowCache>| {
        let map = world.resource::<WorldMap>();
        flows.get_or_build(map, &costs, (goal, c.window.0, c.window.1));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conquest::tests::settlements;
    use crate::progression::tests::run_app;
    use crate::spawn::{ArmySpec, spawn_army};
    use crate::upkeep::{Deserters, apply_desertions};
    use bevy_app::App;
    use sim_agents::{Loyalty, Owner};
    use sim_core::SimStepper;

    /// Puts `value` worth of the best food into `s`'s storehouse, making
    /// room for it.
    fn stock_food(world: &mut World, s: Entity, value: f32) {
        let econ = world.resource::<Economy>().clone();
        let g = econ.foods_by_value()[0];
        let qty = (value / econ.food_value(g)).ceil() as u32;
        let store = storehouse(world, s).expect("storehouse");
        let mut st = world.get_mut::<Stockpile>(store).unwrap();
        st.capacity += qty;
        st.add(g, qty);
    }

    fn store_food(world: &World, s: Entity) -> f32 {
        let store = storehouse(world, s).expect("storehouse");
        let econ = world.resource::<Economy>();
        world.get::<Stockpile>(store).unwrap().food_value(econ)
    }

    fn empty_store(world: &mut World, s: Entity) {
        let store = storehouse(world, s).expect("storehouse");
        world.get_mut::<Stockpile>(store).unwrap().goods.clear();
    }

    /// The homes of `s`, by SpawnId.
    fn homes(world: &World, s: Entity) -> Vec<Entity> {
        let mut homes = world.get::<SettlementEconomy>(s).unwrap().homes.clone();
        homes.sort_by_key(|&h| world.get::<SpawnId>(h).map(|id| id.0));
        homes
    }

    /// Puts `value` worth of the best food into `home`, making room for it.
    fn stock_home(world: &mut World, home: Entity, value: f32) {
        let econ = world.resource::<Economy>().clone();
        let g = econ.foods_by_value()[0];
        let qty = (value / econ.food_value(g)).ceil() as u32;
        let mut st = world.get_mut::<Stockpile>(home).unwrap();
        st.capacity += qty;
        st.add(g, qty);
    }

    /// Spreads `value` worth of the best food over `s`'s homes.
    fn stock_homes(world: &mut World, s: Entity, value: f32) {
        let homes = homes(world, s);
        let each = value / homes.len() as f32;
        for h in homes {
            stock_home(world, h, each);
        }
    }

    /// The food value `s`'s homes hold.
    fn homes_food(world: &World, s: Entity) -> f32 {
        food_held(world, &homes(world, s))
    }

    fn empty_homes(world: &mut World, s: Entity) {
        let econ = world.resource::<Economy>().clone();
        for h in homes(world, s) {
            let mut st = world.get_mut::<Stockpile>(h).unwrap();
            st.goods.retain(|g, _| !econ.is_food(*g));
        }
    }

    /// A village of at least `min_pop` people (the first by SpawnId) and
    /// its daily need.
    fn village(world: &mut World, min_pop: u32) -> (Entity, IVec2, f32) {
        let (_, s, at) = settlements(world)
            .into_iter()
            .find(|&(_, s, _)| world.get::<SettlementEconomy>(s).unwrap().population >= min_pop)
            .expect("a village that big");
        let pop = world.get::<SettlementEconomy>(s).unwrap().population;
        (s, at, daily_need(world, pop))
    }

    /// A neutral player band of 13 at `at` with `days` of food.
    fn band(world: &mut World, at: IVec2, posture: Posture, days: f32) -> Entity {
        let spec = ArmySpec {
            posture,
            food_days: days,
            ..ArmySpec::new(FactionId::PLAYER, at, 12)
        };
        spawn_army(world, &spec)
    }

    fn baggage_food(world: &World, captain: Entity) -> f32 {
        let econ = world.resource::<Economy>();
        world.get::<Stockpile>(captain).unwrap().food_value(econ)
    }

    fn empty_baggage(world: &mut World, captain: Entity) {
        let econ = world.resource::<Economy>().clone();
        let mut bag = world.get_mut::<Stockpile>(captain).unwrap();
        bag.goods.retain(|g, _| !econ.is_food(*g));
    }

    fn army(
        world: &mut World,
        faction: FactionId,
        at: IVec2,
        posture: Posture,
        days: f32,
    ) -> Entity {
        let spec = ArmySpec {
            posture,
            food_days: days,
            armed: true,
            ..ArmySpec::new(faction, at, 5)
        };
        spawn_army(world, &spec)
    }

    fn lines(world: &World) -> Vec<String> {
        let log = &world.resource::<MilitaryLog>().entries;
        log.iter().map(|(_, l)| l.clone()).collect()
    }

    /// GP-05: an aggressive army holding a village it has broken (Attack
    /// step 2) neither plunders nor provokes it, even when its baggage is
    /// short, so the militia stays down and the hold takes the village.
    #[test]
    fn a_held_village_is_neither_plundered_nor_provoked() {
        let (mut app, _) = run_app(42);
        let (aid, a, ac) = settlements(app.world_mut())[0];
        let world = app.world_mut();
        let pop = world.get::<SettlementEconomy>(a).unwrap().population;
        let need = daily_need(world, pop);
        stock_food(world, a, 3.0 * need);
        // Under the aggressive foraging threshold (3 days), so `forage`
        // would plunder any other village.
        let cap = army(world, FactionId::PLAYER, ac, Posture::Aggressive, 2.0);
        let tick = world.resource::<SimTick>().0;
        let mut order = Order::new(OrderSpec::Attack { settlement: aid });
        order.step = 2;
        order.since = tick;
        world
            .get_mut::<Captain>(cap)
            .unwrap()
            .orders
            .push_back(order);
        let before = world.resource::<ArmyStats>().get(FactionId::PLAYER);

        let rules = world.resource::<Army>().data.conquest.clone();
        let cal = world.resource::<Calendar>().clone();
        let span = cal.hours(rules.hold_hours) + 100;
        for _ in 0..span {
            app.step(1);
            let world = app.world();
            assert_ne!(
                world.get::<Threat>(a).unwrap().provoked_by,
                Some(FactionId::PLAYER),
                "provoked during the hold at tick {}",
                world.resource::<SimTick>().0
            );
            if owner_of(world, a) == FactionId::PLAYER {
                break;
            }
        }
        let world = app.world_mut();
        let after = world.resource::<ArmyStats>().get(FactionId::PLAYER);
        assert_eq!(after.provocations, before.provocations);
        assert_eq!(after.seized, before.seized, "plundered the held village");
        assert_eq!(owner_of(world, a), FactionId::PLAYER, "{:?}", lines(world));
        assert_eq!(after.conquests, before.conquests + 1);
    }

    /// GP-05: an owned village feeds its owner's army from what it holds
    /// beyond `garrison.keep_days`, unpaid and unprovoked, whatever the
    /// army's posture: through GatherFood and through foraging (at most
    /// daily, and a draw that finds nothing spare doesn't use up the day).
    #[test]
    fn an_owned_village_supplies_its_owners_army_and_is_never_plundered() {
        let (mut app, _) = run_app(42);
        let (_, a, ac) = settlements(app.world_mut())[0];
        let world = app.world_mut();
        transfer(world, a, FactionId::PLAYER, Cause::Conquest { captain: 0 }).unwrap();
        let keep = world.resource::<Army>().data.garrison.keep_days;
        let pop = world.get::<SettlementEconomy>(a).unwrap().population;
        let need = daily_need(world, pop);
        empty_store(world, a);
        stock_food(world, a, (keep + 5.0) * need);
        let cap = army(world, FactionId::PLAYER, ac, Posture::Aggressive, 0.0);
        empty_baggage(world, cap);
        let before = world.resource::<ArmyStats>().get(FactionId::PLAYER);

        gather_food(world, cap, a, true, true);
        let after = world.resource::<ArmyStats>().get(FactionId::PLAYER);
        assert!(after.supplied > before.supplied, "nothing supplied");
        assert_eq!(after.seized, before.seized, "plundered its own village");
        assert_eq!(after.provocations, before.provocations);
        assert_eq!(world.get::<Threat>(a).unwrap().provoked_by, None);
        // move_food rounds down per good, so the store keeps at least
        // keep_days of food.
        assert!(store_food(world, a) >= keep * need - 1e-3);

        // Foraging: the owned branch draws when the baggage is short.
        empty_baggage(world, cap);
        let id = world.get::<SpawnId>(cap).unwrap().0;
        let t = (world.resource::<SimTick>().0 / 30 + 10) * 30 + (30 - id % 30) % 30;
        assert!((t + id).is_multiple_of(30));
        world.get_mut::<Captain>(cap).unwrap().last_forage = 0;
        let supplied = after.supplied;
        forage(world, cap, t);
        let now = world.resource::<ArmyStats>().get(FactionId::PLAYER);
        assert!(now.supplied > supplied, "forage drew nothing");
        assert_eq!(world.get::<Captain>(cap).unwrap().last_forage, t);
        assert_eq!(now.seized, before.seized);

        // A store with nothing spare: no draw, and the slot stays open. In
        // a run the homes count too (ADR 0018), so they are emptied.
        empty_baggage(world, cap);
        empty_store(world, a);
        empty_homes(world, a);
        stock_food(world, a, keep * need * 0.5);
        world.get_mut::<Captain>(cap).unwrap().last_forage = 0;
        forage(world, cap, t);
        let last = world.resource::<ArmyStats>().get(FactionId::PLAYER);
        assert_eq!(last.supplied, now.supplied);
        assert_eq!(world.get::<Captain>(cap).unwrap().last_forage, 0);
    }

    /// GP-02, GP-03 (ADR 0018): in a run a neutral band asking a village
    /// for food is given half of what its pantry (storehouse and homes)
    /// holds beyond `gather.neutral_keep_days`, up to the baggage's room,
    /// drawn from the best-stocked home first once the storehouse is
    /// empty, and the line says how many days it brought.
    #[test]
    fn in_a_run_a_neutral_gather_draws_on_stores_and_homes() {
        let (mut app, _) = run_app(42);
        let world = app.world_mut();
        let (a, ac, need) = village(world, 40);
        let keep = world
            .resource::<Progression>()
            .data
            .gather
            .neutral_keep_days;
        empty_store(world, a);
        empty_homes(world, a);
        // Two days in the homes: one in the last home (by SpawnId, so the
        // tie-break doesn't pick it), one spread over the rest.
        let hs = homes(world, a);
        let (richest, rest) = hs.split_last().unwrap();
        stock_home(world, *richest, need);
        for &h in rest {
            stock_home(world, h, need / rest.len() as f32);
        }
        let econ = world.resource::<Economy>().clone();
        let food_of = |w: &World, h: Entity| w.get::<Stockpile>(h).unwrap().food_value(&econ);
        let rest0: Vec<f32> = rest.iter().map(|&h| food_of(world, h)).collect();
        let richest0 = food_of(world, *richest);
        let cap = band(world, ac, Posture::Neutral, 1.5);
        let room = baggage_room(world, cap);
        let band_need = daily_need(world, 13);
        let bag0 = baggage_food(world, cap);

        gather_food(world, cap, a, false, true);
        let gained = baggage_food(world, cap) - bag0;
        let pantry = store_food(world, a) + homes_food(world, a);
        println!(
            "village need {need:.1}, room {room:.1}: gained {gained:.1} ({:.2} days), pantry left {pantry:.1}",
            gained / band_need
        );
        assert!(gained >= band_need, "less than a day: {gained}");
        assert!(gained <= room + 1e-3, "beyond the room");
        // move_food rounds down, so the village keeps at least its keep.
        assert!(pantry >= keep * need - 1e-3, "took the village's keep");
        assert!(
            (richest0 - food_of(world, *richest) - gained).abs() < 1e-3,
            "the richest home gives first"
        );
        let rest1: Vec<f32> = rest.iter().map(|&h| food_of(world, h)).collect();
        assert_eq!(rest0, rest1, "the other homes keep theirs");
        let log = lines(world);
        assert!(
            log.iter()
                .any(|l| l.contains("shared food with Your warband (+")),
            "{log:?}"
        );
    }

    /// GP-02 (ADR 0018): a GatherFood that moves nothing says why, the
    /// first reason that applies, and only when told (not when foraging).
    #[test]
    fn a_gather_that_moves_nothing_says_why() {
        let (mut app, _) = run_app(42);
        let world = app.world_mut();
        let (a, ac, need) = village(world, 40);
        let last = |w: &World| lines(w).last().cloned().unwrap_or_default();
        let count = |w: &World| w.resource::<MilitaryLog>().seq;

        // A pantry under the neutral keep (0.5 days): nothing to spare.
        empty_store(world, a);
        empty_homes(world, a);
        let first = homes(world, a)[0];
        stock_home(world, first, 0.4 * need);
        let cap = band(world, ac, Posture::Neutral, 1.5);
        let bag0 = baggage_food(world, cap);
        gather_food(world, cap, a, false, true);
        let line = last(world);
        assert!(
            line.contains("had no food to spare for Your warband (it holds ")
                && line.contains("days of food and keeps 0.5)"),
            "{line}"
        );
        assert_eq!(baggage_food(world, cap), bag0, "nothing moved");

        // Not told (foraging): no line at all.
        let n = count(world);
        gather_food(world, cap, a, false, false);
        assert_eq!(count(world), n, "{:?}", lines(world));

        // A full baggage.
        stock_homes(world, a, 5.0 * need);
        let target = world.resource::<Army>().data.gather_target_days;
        let econ = world.resource::<Economy>().clone();
        let grain = *econ.foods_by_value().last().unwrap();
        {
            let units = (target * daily_need(world, 13) / econ.food_value(grain)).ceil() as u32;
            let mut bag = world.get_mut::<Stockpile>(cap).unwrap();
            bag.capacity += units;
            bag.add(grain, units);
        }
        gather_food(world, cap, a, false, true);
        let line = last(world);
        assert!(
            line.starts_with("Your warband's baggage is full (")
                && line.contains("nothing gathered at"),
            "{line}"
        );

        // Passive with no goods to trade.
        let buyer = band(world, ac, Posture::Passive, 1.5);
        world
            .get_mut::<Stockpile>(buyer)
            .unwrap()
            .goods
            .retain(|g, _| econ.is_food(*g));
        gather_food(world, buyer, a, false, true);
        let line = last(world);
        assert!(
            line.contains("Your warband has no goods to trade for food at"),
            "{line}"
        );

        // Aggressive at an empty storehouse with full homes: plunder never
        // reaches the homes.
        empty_store(world, a);
        let raider = band(world, ac, Posture::Aggressive, 1.5);
        let seized = world.resource::<ArmyStats>().get(FactionId::PLAYER).seized;
        gather_food(world, raider, a, true, true);
        let line = last(world);
        assert!(
            line.contains("Your warband found nothing to plunder at")
                && line.contains("(its storehouse is empty)"),
            "{line}"
        );
        assert_eq!(
            world.resource::<ArmyStats>().get(FactionId::PLAYER).seized,
            seized
        );
    }

    /// GP-02 (ADR 0018): Trade and an owner's supply say what they got or
    /// why not too, and the reason names the real cause: goods worth less
    /// than a unit of food, a village with nothing beyond its keep, or a
    /// share smaller than a unit; a sliver of a day reads in two decimals.
    #[test]
    fn trade_and_supply_say_why_too() {
        let (mut app, _) = run_app(42);
        let world = app.world_mut();
        let (a, ac, need) = village(world, 40);
        let last = |w: &World| lines(w).last().cloned().unwrap_or_default();
        let count = |w: &World| w.resource::<MilitaryLog>().seq;
        let econ = world.resource::<Economy>().clone();
        let wood = GoodId(
            econ.data
                .goods
                .iter()
                .position(|g| g.name == "wood")
                .expect("wood") as u8,
        );
        let trade_keep = world.resource::<Army>().data.trade_keep_days;
        empty_store(world, a);
        empty_homes(world, a);
        stock_food(world, a, (trade_keep + 2.0) * need);
        let cap = band(world, ac, Posture::Neutral, 1.0);

        // One wood (0.2 food) buys less than a unit of grain (0.3).
        world.get_mut::<Stockpile>(cap).unwrap().add(wood, 1);
        trade(world, cap, a);
        let line = last(world);
        assert!(
            line.contains("Your warband has too little to trade for food at")
                && line.contains("(its goods fetch 0.20 food, less than a unit)"),
            "{line}"
        );
        // Fifty wood buy food, and the line says how much.
        world.get_mut::<Stockpile>(cap).unwrap().add(wood, 49);
        let bag0 = baggage_food(world, cap);
        trade(world, cap, a);
        assert!(baggage_food(world, cap) > bag0);
        let line = last(world);
        assert!(
            line.contains("Your warband bought food from") && line.contains(" days)"),
            "{line}"
        );
        // Goods to trade, but nothing beyond the village's keep.
        world.get_mut::<Stockpile>(cap).unwrap().add(wood, 20);
        empty_store(world, a);
        stock_food(world, a, 0.5 * need);
        trade(world, cap, a);
        let line = last(world);
        assert!(
            line.contains("had no food to spare for Your warband (it holds 0.5")
                && line.contains("keeps 1.0)"),
            "{line}"
        );

        // A neutral ask where the village holds a crumb beyond its keep:
        // half of it is less than a unit of grain, so nothing moves and the
        // line says so (not "no food to spare").
        let keep = world
            .resource::<Progression>()
            .data
            .gather
            .neutral_keep_days;
        let grain = *econ.foods_by_value().last().unwrap();
        let unit = econ.food_value(grain);
        let n = (keep * need / unit).floor() as u32 + 1;
        empty_store(world, a);
        {
            let store = storehouse(world, a).unwrap();
            let mut st = world.get_mut::<Stockpile>(store).unwrap();
            st.capacity += n;
            st.add(grain, n);
        }
        let bag0 = baggage_food(world, cap);
        gather_food(world, cap, a, false, true);
        assert_eq!(baggage_food(world, cap), bag0, "{:?}", lines(world));
        let line = last(world);
        assert!(
            line.contains("had only crumbs to spare for Your warband (it holds 0.5")
                && line.contains("keeps 0.5: less than a unit to share)"),
            "{line}"
        );

        // An owned village with less than a day for itself supplies
        // nothing; told, it says why, and foraging (not told) says nothing.
        transfer(world, a, FactionId::PLAYER, Cause::Conquest { captain: 0 }).unwrap();
        empty_store(world, a);
        stock_food(world, a, 0.5 * need);
        let n = count(world);
        assert_eq!(supply(world, cap, a, false), 0.0);
        assert_eq!(count(world), n, "{:?}", lines(world));
        assert_eq!(supply(world, cap, a, true), 0.0);
        let line = last(world);
        assert!(
            line.contains("had no food to spare for Your warband (it holds 0.5")
                && line.contains("keeps 1.0)"),
            "{line}"
        );

        // A sliver of a day reads in two decimals, never "+0.0".
        assert_eq!(days_text(0.02), "0.02");
        assert_eq!(days_text(1.04), "1.0");
        assert_eq!(days_text(0.0), "0.00");
    }

    /// GP-02, GP-03 (ADR 0018): plunder takes from the storehouse only,
    /// in a run too; the homes keep their food.
    #[test]
    fn plunder_takes_only_the_storehouse() {
        let (mut app, _) = run_app(42);
        let world = app.world_mut();
        let (a, ac, need) = village(world, 40);
        empty_store(world, a);
        empty_homes(world, a);
        stock_food(world, a, 0.5 * need);
        stock_homes(world, a, 3.0 * need);
        let (store0, homes0) = (store_food(world, a), homes_food(world, a));
        let cap = band(world, ac, Posture::Aggressive, 0.0);
        empty_baggage(world, cap);
        gather_food(world, cap, a, true, true);
        let seized = world.resource::<ArmyStats>().get(FactionId::PLAYER).seized;
        assert!(seized > 0.0, "{:?}", lines(world));
        assert!(seized <= store0 + 1e-3);
        assert!((store0 - store_food(world, a) - seized).abs() < 1e-3);
        assert_eq!(homes_food(world, a), homes0, "plundered the homes");
        assert!(
            lines(world)
                .iter()
                .any(|l| l.contains("Your warband plundered") && l.contains("days)")),
            "{:?}",
            lines(world)
        );
    }

    /// GP-02, GP-03 (ADR 0018): outside a run gathering reads only the
    /// storehouse and keeps army.ron's neutral keep: full homes behind an
    /// empty storehouse give nothing, and the line says so. (The run
    /// island without a run, which is all `in_run` looks at.)
    #[test]
    fn outside_a_run_gathering_reads_only_the_storehouse() {
        use sim_core::{SimCorePlugin, SimStepper};
        use sim_world::{WorldGenSettings, WorldPreset};
        let mut app = bevy_app::App::new();
        app.add_plugins(SimCorePlugin {
            seed: 42,
            run: None,
        })
        .add_plugins(sim_world::WorldPlugin {
            settings: WorldGenSettings::preset(WorldPreset::Run),
            ..Default::default()
        })
        .add_plugins(sim_economy::EconomyPlugin)
        .add_plugins(sim_agents::AgentsPlugin { population: None })
        .add_plugins(crate::MilitaryPlugin);
        app.prepare_sim();
        let world = app.world_mut();
        assert!(!world.resource::<RunState>().in_run());
        let mut q = world.query::<(Entity, &SpawnId, &Settlement, &SettlementEconomy)>();
        let (_, a, ac, pop) = q
            .iter(world)
            .filter(|(.., se)| se.population >= 40)
            .map(|(e, id, st, se)| (id.0, e, st.center, se.population))
            .min_by_key(|v| v.0)
            .expect("a village");
        let need = daily_need(world, pop);
        empty_store(world, a);
        stock_homes(world, a, 5.0 * need);
        let cap = band(world, ac, Posture::Neutral, 1.5);
        let bag0 = baggage_food(world, cap);
        gather_food(world, cap, a, false, true);
        assert_eq!(baggage_food(world, cap), bag0, "{:?}", lines(world));
        assert_eq!(
            world.resource::<ArmyStats>().get(FactionId::PLAYER).given,
            0.0
        );
        let line = lines(world).last().cloned().unwrap_or_default();
        assert!(
            line.contains("had no food to spare for Your warband (it holds 0.0 days")
                && line.contains("keeps 1.0)"),
            "{line}"
        );
    }

    /// GP-06, GP-22 (ADR 0018): a neutral gift in a run never takes a
    /// village below the food that costs it loyalty or makes it ask for
    /// food.
    #[test]
    fn the_run_neutral_keep_never_costs_loyalty() {
        let p = Progression::default_data();
        let v = Villagers::default_data(&Calendar::default_data());
        let keep = p.data.gather.neutral_keep_days;
        assert!(keep >= v.data.loyalty.food_days, "loyalty.food_days");
        assert!(keep >= p.data.requests.food_days, "requests.food_days");
    }

    /// GP-06: another warlord's village sends no recruits, and your own
    /// sends more the more loyal it is (`recruit.loyalty_bonus`).
    #[test]
    fn recruiting_answers_to_the_owner_and_its_loyalty() {
        let recruits = |owner: FactionId, mean: f32| {
            let (mut app, _) = run_app(42);
            let (_, a, ac) = settlements(app.world_mut())[0];
            let world = app.world_mut();
            transfer(world, a, owner, Cause::None).unwrap();
            world.get_mut::<SettlementLoyalty>(a).unwrap().mean = mean;
            let cap = army(world, FactionId::PLAYER, ac, Posture::Neutral, 3.0);
            recruit(world, cap, a, 1000);
            let n = world
                .resource::<ArmyStats>()
                .get(FactionId::PLAYER)
                .recruited;
            (n, lines(world))
        };
        let (rival, log) = recruits(FactionId::RIVAL, 1.0);
        assert_eq!(rival, 0, "a rival's village sent recruits");
        assert!(log.iter().any(|l| l.contains("answers to")), "{log:?}");
        let (cold, _) = recruits(FactionId::PLAYER, 0.0);
        let (loyal, _) = recruits(FactionId::PLAYER, 1.0);
        assert!(cold > 0, "nobody joined at all");
        assert!(loyal > cold, "loyal {loyal}, disloyal {cold}");
    }

    /// GP-05: an aggressive army taking arms from its own village doesn't
    /// provoke it; from an independent one it does.
    #[test]
    fn equipping_from_your_own_village_provokes_nobody() {
        let (mut app, _) = run_app(42);
        let list = settlements(app.world_mut());
        let world = app.world_mut();
        let econ = world.resource::<Economy>().clone();
        let weapons = good_named(&econ, "weapons").unwrap();
        for (i, &(_, s, at)) in list[..2].iter().enumerate() {
            if i == 0 {
                transfer(world, s, FactionId::PLAYER, Cause::None).unwrap();
            }
            let store = storehouse(world, s).unwrap();
            let mut st = world.get_mut::<Stockpile>(store).unwrap();
            st.capacity += 20;
            st.add(weapons, 20);
            let cap = army(world, FactionId::PLAYER, at, Posture::Aggressive, 3.0);
            equip(world, cap, s);
        }
        let threat = |w: &World, s: Entity| w.get::<Threat>(s).unwrap().provoked_by;
        assert_eq!(threat(world, list[0].1), None, "provoked its own village");
        assert_eq!(threat(world, list[1].1), Some(FactionId::PLAYER));
    }

    /// GP-05, GP-06: a deserter goes home sworn to whoever owns its
    /// settlement now, with the residents' current loyalty, not the
    /// loyalty it had (toward another owner) when it enlisted.
    #[test]
    fn a_deserter_comes_home_to_the_current_owner() {
        let (mut app, _) = run_app(42);
        let (_, a, ac) = settlements(app.world_mut())[0];
        let world = app.world_mut();
        crate::conquest::transfer_seeded(world, a, FactionId::PLAYER, Cause::None, Some(0.9));
        let cap = army(world, FactionId::PLAYER, ac, Posture::Neutral, 3.0);
        let mut q = world.query_filtered::<(Entity, &SpawnId, &Villager), Without<Enlisted>>();
        let (e, id) = q
            .iter(world)
            .filter(|(_, _, v)| v.settlement == a)
            .map(|(e, id, _)| (e, *id))
            .min_by_key(|(_, id)| *id)
            .unwrap();
        let army_data = world.resource::<Army>().clone();
        enlist(world, e, cap, FactionId::PLAYER, &army_data);
        world.get_mut::<Loyalty>(e).unwrap().0 = 0.9;
        crate::conquest::transfer_seeded(world, a, FactionId::RIVAL, Cause::None, Some(0.2));
        assert_eq!(world.get::<Owner>(a).unwrap().faction, FactionId::RIVAL);
        assert_eq!(world.get::<Allegiance>(e).unwrap().0, FactionId::PLAYER);
        world
            .resource_mut::<Deserters>()
            .0
            .push((id, e, FactionId::PLAYER));
        apply_desertions(world);
        assert!(world.get::<Enlisted>(e).is_none());
        assert_eq!(world.get::<Allegiance>(e).unwrap().0, FactionId::RIVAL);
        assert_eq!(world.get::<Loyalty>(e).unwrap().0, 0.2);
    }

    /// Settlement `s`'s goodwill toward `f`.
    fn goodwill(world: &World, s: Entity, f: FactionId) -> f32 {
        world.get::<sim_agents::Goodwill>(s).unwrap().get(f)
    }

    /// GP-22: deeds post goodwill changes with progression.ron's deltas:
    /// a fair trade +, a plunder and its provocation -. Applied in
    /// `AgentSet::Standing`, a fraction reaches the settlements within
    /// `goodwill.spread_radius`, and the ledger is emptied.
    #[test]
    fn deeds_move_goodwill_and_word_spreads() {
        let (mut app, _) = run_app(42);
        let list = settlements(app.world_mut());
        let world = app.world_mut();
        let deeds = world.resource::<Progression>().data.goodwill_deeds;
        let g = world.resource::<Villagers>().data.goodwill.clone();
        // The first village with a neighbour in range, and that neighbour.
        let (a, b) = list
            .iter()
            .find_map(|&(_, a, ac)| {
                list.iter()
                    .find(|&&(_, b, bc)| b != a && (bc - ac).as_vec2().length() <= g.spread_radius)
                    .map(|&(_, b, _)| (a, b))
            })
            .expect("two villages within the spread radius");
        let ac = center(world, a).unwrap();
        let pop = world.get::<SettlementEconomy>(a).unwrap().population;
        let need = daily_need(world, pop);
        stock_food(world, a, 5.0 * need);

        // Passive: buys food with wood, a fair trade.
        let econ = world.resource::<Economy>().clone();
        let wood = good_named(&econ, "wood").unwrap();
        let buyer = army(world, FactionId::PLAYER, ac, Posture::Passive, 0.0);
        empty_baggage(world, buyer);
        world.get_mut::<Stockpile>(buyer).unwrap().add(wood, 50);
        gather_food(world, buyer, a, true, true);
        // Aggressive raiders plunder (and so provoke).
        let raider = army(world, FactionId::RAIDERS, ac, Posture::Aggressive, 0.0);
        empty_baggage(world, raider);
        gather_food(world, raider, a, true, true);
        // Keep `forage` from gathering again during the step below.
        for cap in [buyer, raider] {
            world.get_mut::<Captain>(cap).unwrap().last_forage = 1;
        }
        let sa = world.get::<SpawnId>(a).unwrap().0;
        let mut posted: Vec<(u64, FactionId, Deed)> = world
            .resource::<GoodwillLedger>()
            .0
            .iter()
            .map(|c| (c.settlement, c.faction, c.cause))
            .collect();
        posted.sort();
        assert_eq!(
            posted,
            vec![
                (sa, FactionId::PLAYER, Deed::Trade),
                (sa, FactionId::RAIDERS, Deed::Plunder),
                (sa, FactionId::RAIDERS, Deed::Provoke),
            ]
        );
        let (b0, a0) = (
            goodwill(world, b, FactionId::RAIDERS),
            goodwill(world, a, FactionId::RAIDERS),
        );
        app.step(1);
        let world = app.world_mut();
        assert!(world.resource::<GoodwillLedger>().is_empty());
        let near = |x: f32, y: f32| (x - y).abs() < 1e-4;
        let hit = deeds.plunder + deeds.provoke;
        // Killings in this tick could add more; there are none at tick 1.
        assert!(near(goodwill(world, a, FactionId::RAIDERS) - a0, hit));
        assert!(near(
            goodwill(world, b, FactionId::RAIDERS) - b0,
            hit * g.spread_fraction
        ));
        assert!(near(goodwill(world, a, FactionId::PLAYER), deeds.trade));
        assert!(near(
            goodwill(world, b, FactionId::PLAYER),
            deeds.trade * g.spread_fraction
        ));
    }

    /// GP-22: Give hands the baggage's food beyond `give_keep_days` of the
    /// army's need to the storehouse, counts what was delivered in
    /// `donated`, and ends the order.
    #[test]
    fn giving_hands_over_food_beyond_the_keep() {
        let (mut app, _) = run_app(42);
        let (aid, a, ac) = settlements(app.world_mut())[0];
        let world = app.world_mut();
        let cap = army(world, FactionId::PLAYER, ac, Posture::Neutral, 5.0);
        let keep = world.resource::<Army>().data.give_keep_days;
        let people = army_size(world, cap);
        let need = daily_need(world, people);
        let econ = world.resource::<Economy>().clone();
        let bag = |w: &World| w.get::<Stockpile>(cap).unwrap().food_value(&econ);
        let (bag0, store0) = (bag(world), store_food(world, a));
        world
            .get_mut::<Captain>(cap)
            .unwrap()
            .orders
            .push_back(Order::new(OrderSpec::Give { settlement: aid }));
        app.step(1);
        let world = app.world_mut();
        let donated = world.resource::<ArmyStats>().get(FactionId::PLAYER).donated;
        println!(
            "bag {bag0:.1} -> {:.1}, keep {:.1}, donated {donated:.1}",
            bag(world),
            keep * need
        );
        assert!(donated > 0.0, "{:?}", lines(world));
        assert!(world.get::<Captain>(cap).unwrap().orders.is_empty(), "done");
        // The army ate a little this tick, and move_food rounds down per
        // good: the rest stays in the baggage.
        assert!(bag(world) >= keep * need - 1.0, "gave away its own keep");
        assert!(donated <= bag0 - keep * need + 1e-3);
        assert!(store_food(world, a) > store0, "the storehouse got it");
        assert!(lines(world).iter().any(|l| l.contains("gave food")));
        // Left with its keep beside the village, the neutral army doesn't
        // take food straight back (`forage` pauses for a quarter day after
        // a gift), though its baggage soon dips below a day.
        let taken = |w: &World| {
            let s = w.resource::<ArmyStats>().get(FactionId::PLAYER);
            s.given + s.seized
        };
        // (A well-stocked storehouse, so there would be food to take.)
        let pop = world.get::<SettlementEconomy>(a).unwrap().population;
        let village_need = daily_need(world, pop);
        stock_food(world, a, 10.0 * village_need);
        let before = taken(world);
        app.step(550);
        let world = app.world_mut();
        let days = baggage_days(world, cap);
        assert!(days < 1.0, "still {days} days in the baggage");
        assert_eq!(taken(world), before, "took food back: {:?}", lines(world));

        // An army with nothing to spare gives nothing.
        let poor = army(world, FactionId::PLAYER, ac, Posture::Neutral, 0.5);
        assert_eq!(give(world, poor, a), 0.0);
    }

    /// SIM-38: a foraging band holds where it stands and fills its
    /// baggage from the wild, emptying the patches around it; the order
    /// ends when the baggage is full or the land is bare. A radius of 0 or
    /// beyond `forage.max_radius` is refused.
    #[test]
    fn a_foraging_band_lives_off_the_land() {
        use crate::progression::tests::command;
        use crate::wild::WildFood;
        let (mut app, start) = run_app(42);
        let c = start.champion;
        let world = app.world_mut();
        let id = world.get::<SpawnId>(c).unwrap().0;
        let f = world.resource::<Progression>().data.forage;
        let at = world.get::<Position>(c).unwrap().0;
        let r = f.default_radius;
        empty_baggage(world, c);
        let wild0 = world.resource::<WildFood>().available(at, r as f32);
        assert!(wild0 > 0.0, "the start has wild food");
        let rejected = |app: &App| {
            app.world()
                .resource::<ArmyStats>()
                .get(FactionId::PLAYER)
                .rejected_commands
        };
        for radius in [0, f.max_radius + 1] {
            let before = rejected(&app);
            command(
                &mut app,
                FactionId::PLAYER,
                CommandKind::IssueOrder {
                    captain: id,
                    order: OrderSpec::Forage { radius },
                    queue: false,
                },
            );
            assert_eq!(rejected(&app), before + 1, "radius {radius}");
        }
        command(
            &mut app,
            FactionId::PLAYER,
            CommandKind::IssueOrder {
                captain: id,
                order: OrderSpec::Forage { radius: r },
                queue: false,
            },
        );
        app.step(f.every_ticks * 6);
        let world = app.world_mut();
        let stats = world.resource::<ArmyStats>().get(FactionId::PLAYER);
        let cap = world.get::<Captain>(c).unwrap().clone();
        let left = world.resource::<WildFood>().available(at, r as f32);
        println!("foraged {:.2}, wild {wild0:.1} -> {left:.1}", stats.foraged);
        assert_eq!(cap.goal, None, "holds");
        assert_eq!(
            cap.orders.front().map(|o| o.spec),
            Some(OrderSpec::Forage { radius: r })
        );
        assert!(
            world
                .get::<Position>(c)
                .unwrap()
                .0
                .as_vec2()
                .distance(at.as_vec2())
                < 3.0
        );
        assert!(stats.foraged > 0.0, "{:?}", lines(world));
        assert!(left < wild0, "the patches were not touched");
        // Six harvests of per_member each, give or take one unit a harvest.
        let people = army_size(world, c) as f32;
        let econ = world.resource::<Economy>().clone();
        let grain = econ.food_value(*econ.foods_by_value().last().unwrap());
        let expect = 6.0 * f.per_member * people;
        assert!(
            (stats.foraged - expect).abs() <= 6.0 * grain,
            "{} vs {expect}",
            stats.foraged
        );
        assert!(
            (wild0 - left - expect).abs() < 1e-2,
            "depleted by the harvest"
        );

        // Bare land ends the order at the next harvest.
        let patches = world.resource::<WildFood>().patches_within(at, r as f32);
        for p in patches {
            world.resource_mut::<WildFood>().level[p] = 0.0;
        }
        app.step(f.every_ticks);
        let world = app.world_mut();
        assert!(world.get::<Captain>(c).unwrap().orders.is_empty());
        assert!(
            lines(world)
                .iter()
                .any(|l| l.contains("nothing left to forage"))
        );

        // A full baggage ends it too.
        let target = world.resource::<Army>().data.gather_target_days;
        let people = army_size(world, c);
        let need = daily_need(world, people);
        let grain_id = *econ.foods_by_value().last().unwrap();
        {
            let mut bag = world.get_mut::<Stockpile>(c).unwrap();
            let units = (target * need / grain).ceil() as u32 + 10;
            bag.capacity += units;
            bag.add(grain_id, units);
        }
        command(
            &mut app,
            FactionId::PLAYER,
            CommandKind::IssueOrder {
                captain: id,
                order: OrderSpec::Forage { radius: r },
                queue: false,
            },
        );
        app.step(f.every_ticks);
        let world = app.world_mut();
        assert!(world.get::<Captain>(c).unwrap().orders.is_empty());
        assert!(lines(world).iter().any(|l| l.contains("baggage is full")));
    }
}
