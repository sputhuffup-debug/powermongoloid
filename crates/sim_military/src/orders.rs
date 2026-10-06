//! Orders (GP-02, GP-03). Commands are validated and applied in arrival
//! order; then each captain works on its first order, serially in SpawnId
//! order. Settlement dealings (recruiting, food, arms, trade) move goods
//! between two stockpiles directly: this is the army's serial logistics
//! step, the counterpart of the economy's transfer resolver (ADR 0010).

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use rand::RngExt;
use sim_agents::{
    ActionKind, Activity, CombatStats, Employment, Enlisted, Stage, Stance, Threat, Travel,
    TravelState, Villager, Villagers,
};
use sim_core::rng::rng_key;
use sim_core::{
    Allegiance, Calendar, CommandKind, FactionId, OrderSpec, Position, Posture, SimCommand,
    SimSeed, SimTick, SpawnId,
};
use sim_economy::{Economy, GoodId, Occupation, SettlementEconomy, Stockpile, Workplace};
use sim_world::{FlowCache, Settlement, WorldMap};

use crate::components::{
    ArmyStats, Captain, Equipment, Factions, Follower, MilitaryLog, Order, describe,
};
use crate::data::Army;
use crate::index::FighterGrid;

/// Commands read this tick, waiting to be applied by [`apply_commands`].
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

fn settlement_name(world: &World, s: Entity) -> String {
    let names = world.resource::<sim_core::NameTable>();
    world
        .get::<Settlement>(s)
        .map_or_else(|| "?".into(), |st| names.get(st.name).to_string())
}

fn log(world: &mut World, text: String) {
    let tick = world.resource::<SimTick>().0;
    tracing::debug!(tick, "{text}");
    world.resource_mut::<MilitaryLog>().push(tick, text);
}

fn faction_name(world: &World, f: FactionId) -> String {
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

fn validate_spec(world: &mut World, spec: &OrderSpec) -> Result<(), &'static str> {
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
        | OrderSpec::Attack { settlement }
        | OrderSpec::Trade { settlement } => find_settlement(world, settlement)
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
            validate_spec(world, &order)?;
            let mut c = world.get_mut::<Captain>(e).expect("captain");
            if !queue {
                c.orders.clear();
                c.goal = None;
            }
            c.orders.push_back(Order::new(order));
            let who = faction_name(world, cmd.issuer);
            log(
                world,
                format!("{who}: captain {captain} ordered to {}", describe(&order)),
            );
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
    }
}

/// GP-01: promotes a follower to captain. The new captain takes every
/// second follower (by SpawnId) and half the baggage.
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
    let (max, capacity) = (army.data.max_captains, army.data.baggage_capacity);
    let mut q = world.query::<&Captain>();
    let captains = q.iter(world).filter(|c| c.faction == issuer).count() as u32;
    if captains >= max {
        return Err("too many captains");
    }
    // Split the army.
    let mut q = world.query::<(Entity, &SpawnId, &Follower)>();
    let mut members: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(e, _, fl)| fl.captain == old && *e != f)
        .map(|(e, id, _)| (*id, e))
        .collect();
    members.sort_unstable();
    for (_, e) in members.iter().skip(1).step_by(2) {
        world.get_mut::<Follower>(*e).expect("follower").captain = f;
    }
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
fn daily_need(world: &World, people: u32) -> f32 {
    world.resource::<Villagers>().data.needs.hunger_per_day * people as f32
}

/// Moves up to `max_value` of food (most nourishing first) from one
/// stockpile to another. Returns the food value moved.
fn move_food(world: &mut World, from: Entity, to: Entity, max_value: f32) -> f32 {
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

fn move_good(world: &mut World, from: Entity, to: Entity, g: GoodId, qty: u32) -> u32 {
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

fn storehouse(world: &World, s: Entity) -> Option<Entity> {
    world.get::<SettlementEconomy>(s).map(|se| se.storehouse)
}

/// Food the settlement's storehouse holds beyond `keep_days` for its people.
fn spare_food(world: &World, s: Entity, keep_days: f32) -> f32 {
    let Some(se) = world.get::<SettlementEconomy>(s) else {
        return 0.0;
    };
    let econ = world.resource::<Economy>();
    let held = world
        .get::<Stockpile>(se.storehouse)
        .map_or(0.0, |st| st.food_value(econ));
    (held - keep_days * daily_need(world, se.population)).max(0.0)
}

fn army_size(world: &mut World, captain: Entity) -> u32 {
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
        let (who, name) = (faction_name(world, faction), settlement_name(world, s));
        log(world, format!("{name} takes up arms against {who}"));
    }
}

/// Barters the baggage's non-food goods for food (Trade, and passive
/// gathering). Returns the food value bought.
fn barter(world: &mut World, captain: Entity, s: Entity, keep_days: f32, room: f32) -> f32 {
    let Some(store) = storehouse(world, s) else {
        return 0.0;
    };
    let army = world.resource::<Army>().clone();
    let econ = world.resource::<Economy>().clone();
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
    let offer: f32 = goods
        .iter()
        .map(|(g, q)| army.trade_value(*g) * *q as f32)
        .sum();
    let want = offer.min(spare_food(world, s, keep_days)).min(room);
    let bought = move_food(world, store, captain, want);
    // Pay for it, good by good in GoodId order.
    let mut owed = bought;
    for (g, q) in goods {
        if owed <= 0.0 {
            break;
        }
        let v = army.trade_value(g);
        let n = ((owed / v).ceil() as u32).min(q);
        let paid = move_good(world, captain, store, g, n);
        owed -= paid as f32 * v;
    }
    bought
}

/// Food value the baggage lacks to feed the army for `gather_target_days`.
fn baggage_room(world: &mut World, captain: Entity) -> f32 {
    let days = world.resource::<Army>().data.gather_target_days;
    let people = army_size(world, captain);
    let econ = world.resource::<Economy>();
    let held = world
        .get::<Stockpile>(captain)
        .map_or(0.0, |s| s.food_value(econ));
    (days * daily_need(world, people) - held).max(0.0)
}

/// GatherFood (GP-02) by posture: passive buys; neutral is given some and
/// takes the rest of what it needs when hungry; aggressive takes freely
/// and provokes the settlement. Nobody takes more than the army needs for
/// `gather_target_days`.
fn gather_food(world: &mut World, captain: Entity, s: Entity, hungry: bool) {
    let c = world.get::<Captain>(captain).expect("captain").clone();
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
            let spare = spare_food(world, s, keep).min(room);
            let given = move_food(world, store, captain, spare * 0.5);
            let seized = if hungry {
                move_food(world, store, captain, spare - given)
            } else {
                0.0
            };
            let mut stats = world.resource_mut::<ArmyStats>();
            stats.of(c.faction).given += given;
            stats.of(c.faction).seized += seized;
            if seized > 0.0 {
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
                log(world, format!("{who} plundered {name}"));
                provoke(world, s, c.faction);
            }
        }
    }
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
                n
            }
            Posture::Neutral => have / 2,
            Posture::Aggressive => have,
        };
        took += move_good(world, store, captain, g, n);
    }
    if took > 0 {
        log(world, format!("{who} took {took} arms from {name}"));
        if c.posture == Posture::Aggressive {
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
    let cid = world.get::<SpawnId>(captain).expect("id").0;
    let army = world.resource::<Army>().clone();
    let villagers = world.resource::<Villagers>().clone();
    let days_per_year = world.resource::<Calendar>().days_per_year();
    let working = villagers.data.households.working_age;
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
    let mut rng = world
        .resource::<SimSeed>()
        .stream(rng_key("sim_military::recruit") ^ cid, tick);
    let mut joined = Vec::new();
    for (_, _, e) in candidates {
        if joined.len() >= count as usize {
            break;
        }
        if rng.random::<f32>() < p {
            joined.push(e);
        }
    }
    for &e in &joined {
        enlist(world, e, captain, c.faction, &army);
    }
    if let Some(mut se) = world.get_mut::<SettlementEconomy>(s) {
        se.population = se.population.saturating_sub(joined.len() as u32);
    }
    let n = joined.len() as u32;
    world.resource_mut::<ArmyStats>().of(c.faction).recruited += n;
    let (who, name) = (faction_name(world, c.faction), settlement_name(world, s));
    log(world, format!("{n} of {name} joined {who}"));
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

fn center(world: &World, s: Entity) -> Option<IVec2> {
    world.get::<Settlement>(s).map(|st| st.center)
}

/// Whether militia of `s` is still standing against `faction`.
fn militia_standing(world: &World, at: IVec2, faction: FactionId) -> bool {
    let grid = world.resource::<FighterGrid>();
    let mut any = false;
    grid.for_each_within(at.as_vec2(), 35.0, |e, _| {
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
    let tpd = world.resource::<Calendar>().ticks_per_day;
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
        OrderSpec::Recruit { settlement, .. }
        | OrderSpec::GatherFood { settlement }
        | OrderSpec::Equip { settlement }
        | OrderSpec::Trade { settlement }
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
                if order.step == 0 && near(at, army.data.threat.radius * 0.5) {
                    order.step = 1;
                    order.since = tick;
                }
                if order.step == 1 {
                    *world.get_mut::<Captain>(e).expect("captain") = c.clone();
                    provoke(world, s, c.faction);
                    let fought = tick.saturating_sub(order.since);
                    if (fought > 300 && !militia_standing(world, at, c.faction)) || fought > tpd {
                        done = true;
                        let (who, name) =
                            (faction_name(world, c.faction), settlement_name(world, s));
                        log(world, format!("{who} broke the defenders of {name}"));
                    }
                }
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
                        let bought = barter(world, e, s, 2.0, room);
                        world.resource_mut::<ArmyStats>().of(c.faction).bought += bought;
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

/// Armies feed themselves from settlements they pass, by posture (GP-03):
/// passive buys when hungry, neutral asks (and takes when hungry),
/// aggressive takes whenever the baggage is short.
fn forage(world: &mut World, e: Entity, tick: u64) {
    let tpd = world.resource::<Calendar>().ticks_per_day;
    let c = world.get::<Captain>(e).expect("captain").clone();
    if tick < c.last_forage + tpd / 4 && c.last_forage != 0 {
        return;
    }
    let days = baggage_days(world, e);
    let wants = match c.posture {
        Posture::Passive | Posture::Neutral => days < 1.0,
        Posture::Aggressive => days < 3.0,
    };
    if !wants {
        return;
    }
    let tile = world.get::<Position>(e).expect("position").0;
    let radius = world.resource::<Army>().data.upkeep.forage_radius;
    let mut q = world.query_filtered::<(Entity, &SpawnId, &Settlement), With<SettlementEconomy>>();
    let nearest = q
        .iter(world)
        .map(|(s, id, st)| ((st.center - tile).as_vec2().length(), *id, s))
        .filter(|(d, ..)| *d <= radius)
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let Some((_, _, s)) = nearest else {
        return;
    };
    world.get_mut::<Captain>(e).expect("captain").last_forage = tick.max(1);
    gather_food(world, e, s, days < 1.0);
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
