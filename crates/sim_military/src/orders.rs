//! Orders (GP-02, GP-03). Commands are validated and applied in arrival
//! order; then each captain works on its first order, serially in SpawnId
//! order. Settlement dealings (recruiting, food, arms, trade) move goods
//! between two stockpiles directly: this is the army's serial logistics
//! step, the counterpart of the economy's transfer resolver (ADR 0010).

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
    Posture, SimCommand, SimSeed, SimTick, SpawnId, Stat,
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
    // Split the army.
    let mut q = world.query::<(Entity, &SpawnId, &Follower)>();
    let mut members: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(e, _, fl)| fl.captain == old && *e != f)
        .map(|(e, id, _)| (*id, e))
        .collect();
    members.sort_unstable();
    let mut moved = 0;
    for (_, e) in members.iter().skip(1).step_by(2) {
        world.get_mut::<Follower>(*e).expect("follower").captain = f;
        moved += 1;
    }
    let capacity = world.resource::<Army>().baggage_capacity(moved + 1);
    let mut baggage = Stockpile::with_capacity(capacity);
    if let Some(mut old_bag) = world.get_mut::<Stockpile>(old) {
        let goods: Vec<(GoodId, u32)> = old_bag.goods.iter().map(|(g, q)| (*g, *q)).collect();
        for (g, q) in goods {
            let half = old_bag.take(g, q / 2);
            baggage.add(g, half);
        }
    }
    world
        .entity_mut(f)
        .remove::<Follower>()
        .insert((Captain::new(issuer, old_captain.posture), baggage));
    let who = faction_name(world, issuer);
    log(world, format!("{who}: {follower} appointed captain"));
    Ok(())
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

/// Food the settlement's storehouse holds beyond `keep_days` for its people.
pub(crate) fn spare_food(world: &World, s: Entity, keep_days: f32) -> f32 {
    let Some(se) = world.get::<SettlementEconomy>(s) else {
        return 0.0;
    };
    let econ = world.resource::<Economy>();
    let held = world
        .get::<Stockpile>(se.storehouse)
        .map_or(0.0, |st| st.food_value(econ));
    (held - keep_days * daily_need(world, se.population)).max(0.0)
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

/// Barters the baggage's non-food goods for food (Trade, and passive
/// gathering), at [`trade_price`]. Returns the food value bought.
fn barter(world: &mut World, captain: Entity, s: Entity, keep_days: f32, room: f32) -> f32 {
    let Some(store) = storehouse(world, s) else {
        return 0.0;
    };
    let army = world.resource::<Army>().clone();
    let econ = world.resource::<Economy>().clone();
    let mods = world.resource::<FactionModifiers>().clone();
    let faction = world
        .get::<Captain>(captain)
        .map_or(FactionId::INDEPENDENT, |c| c.faction);
    let price = |g: GoodId| trade_price(&army, &mods, faction, g);
    let goods: Vec<(GoodId, u32)> = world
        .get::<Stockpile>(captain)
        .map(|b| {
            b.goods
                .iter()
                .filter(|(g, _)| army.trade_value(**g) > 0.0 && !econ.is_food(**g))
                .map(|(g, q)| (*g, *q))
                .collect()
        })
        .unwrap_or_default();
    let offer: f32 = goods.iter().map(|(g, q)| price(*g) * *q as f32).sum();
    let want = offer.min(spare_food(world, s, keep_days)).min(room);
    let bought = move_food(world, store, captain, want);
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

/// GatherFood (GP-02) by posture: passive buys; neutral is given some and,
/// when hungry, takes a day's food; aggressive takes freely (up to
/// `gather_target_days` for the army) and provokes the settlement.
fn gather_food(world: &mut World, captain: Entity, s: Entity, hungry: bool) {
    let c = world.get::<Captain>(captain).expect("captain").clone();
    // A settlement feeds its owner's armies, whatever their posture: they
    // never pay, plunder or provoke it (GP-05).
    if owner_of(world, s) == c.faction {
        supply(world, captain, s);
        return;
    }
    let army = world.resource::<Army>().clone();
    let Some(store) = storehouse(world, s) else {
        return;
    };
    let room = baggage_room(world, captain);
    let keep = army.keep_days(c.posture);
    let name = settlement_name(world, s);
    let who = faction_name(world, c.faction);
    match c.posture {
        Posture::Passive => {
            let bought = barter(world, captain, s, keep, room);
            world.resource_mut::<ArmyStats>().of(c.faction).bought += bought;
            if bought > 0.0 {
                log(world, format!("{who} bought food from {name}"));
            }
        }
        Posture::Neutral => {
            // Asks for a share of what the village can spare...
            let spare = spare_food(world, s, keep).min(room);
            let given = move_food(world, store, captain, spare * 0.5);
            // ...and when hungry takes a day's food for the army, even from
            // the village's reserve.
            let seized = if hungry {
                let people = army_size(world, captain);
                let need = (daily_need(world, people) - given).max(0.0);
                let reserve = spare_food(world, s, 0.0);
                move_food(world, store, captain, need.min(reserve))
            } else {
                0.0
            };
            let mut stats = world.resource_mut::<ArmyStats>();
            stats.of(c.faction).given += given;
            stats.of(c.faction).seized += seized;
            if seized > 0.0 {
                goodwill_deed(world, s, c.faction, Deed::Seize);
                log(world, format!("{who} took food from {name}"));
            } else if given > 0.0 {
                log(world, format!("{name} shared food with {who}"));
            }
        }
        Posture::Aggressive => {
            let spare = spare_food(world, s, keep).min(room);
            let seized = move_food(world, store, captain, spare);
            world.resource_mut::<ArmyStats>().of(c.faction).seized += seized;
            if seized > 0.0 {
                goodwill_deed(world, s, c.faction, Deed::Plunder);
                log(world, format!("{who} plundered {name}"));
                provoke(world, s, c.faction);
            }
        }
    }
}

/// Owned-settlement supply (GP-05): the owner's army takes the food the
/// storehouse holds beyond `garrison.keep_days`, up to its baggage target,
/// unpaid and without provoking anyone. Counted in `FactionStats::supplied`.
/// Returns the food value taken.
fn supply(world: &mut World, captain: Entity, s: Entity) -> f32 {
    let faction = world.get::<Captain>(captain).expect("captain").faction;
    let Some(store) = storehouse(world, s) else {
        return 0.0;
    };
    let keep = world.resource::<Army>().data.garrison.keep_days;
    let room = baggage_room(world, captain);
    let spare = spare_food(world, s, keep).min(room);
    let got = move_food(world, store, captain, spare);
    if got > 0.0 {
        world.resource_mut::<ArmyStats>().of(faction).supplied += got;
        let (who, name) = (faction_name(world, faction), settlement_name(world, s));
        log(world, format!("{name} supplied {who}"));
    }
    got
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
/// village notices the loss; it drops its job.
pub fn enlist(world: &mut World, e: Entity, captain: Entity, faction: FactionId, army: &Army) {
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
                        gather_food(world, e, s, hungry);
                    }
                    OrderSpec::Equip { .. } => equip(world, e, s),
                    OrderSpec::Trade { .. } => {
                        let room = baggage_room(world, e);
                        let keep = world.resource::<Army>().data.trade_keep_days;
                        let bought = barter(world, e, s, keep, room);
                        world.resource_mut::<ArmyStats>().of(c.faction).bought += bought;
                    }
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
/// aggressive takes whenever the baggage is short. A settlement the army's
/// faction owns supplies it instead (GP-05): whenever the baggage holds
/// less than `gather_target_days`, at most once a day, never provoking.
/// A settlement the army is holding after breaking its defenders (`Attack`
/// step 2) is left alone. Monument guards ([`crate::monuments::Guard`])
/// never gather.
fn forage(world: &mut World, e: Entity, tick: u64) {
    // GP-23: a monument's guard camp never eats, so never gathers.
    if world.get::<crate::monuments::Guard>(e).is_some() {
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
        if due && days < target_days && supply(world, e, s) > 0.0 {
            world.get_mut::<Captain>(e).expect("captain").last_forage = tick.max(1);
        }
        return;
    }
    let wants = match c.posture {
        Posture::Passive | Posture::Neutral => days < 1.0,
        Posture::Aggressive => days < 3.0,
    };
    if !wants {
        return;
    }
    world.get_mut::<Captain>(e).expect("captain").last_forage = tick.max(1);
    gather_food(world, e, s, days < 1.0);
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

        gather_food(world, cap, a, true);
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

        // A store with nothing spare: no draw, and the slot stays open.
        empty_baggage(world, cap);
        empty_store(world, a);
        stock_food(world, a, keep * need * 0.5);
        world.get_mut::<Captain>(cap).unwrap().last_forage = 0;
        forage(world, cap, t);
        let last = world.resource::<ArmyStats>().get(FactionId::PLAYER);
        assert_eq!(last.supplied, now.supplied);
        assert_eq!(world.get::<Captain>(cap).unwrap().last_forage, 0);
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
        gather_food(world, buyer, a, true);
        // Aggressive raiders plunder (and so provoke).
        let raider = army(world, FactionId::RAIDERS, ac, Posture::Aggressive, 0.0);
        empty_baggage(world, raider);
        gather_food(world, raider, a, true);
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
