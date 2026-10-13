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
//! Step 4 (GP-34, GP-17 part) brings the three dials to life: the
//! `Govern` command (`command`, validated by [`refusal`]), the hourly
//! application with its first wave and the dawn waves ([`step`]), the Dues
//! and Table terms, and read-only forecasts for the panel
//! ([`work_forecast`], [`dues_forecast`], [`table_forecast`],
//! [`lock_left`]). Tithes and levies reach armies in the logistics step
//! (`orders::lands`). Steps 8 and 9 add building orders and stewards
//! (lane B owns this file).

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use sim_agents::{
    ActionKind, Activity, Agent, Assignment, CombatStats, Deed, Edicts, Enlisted, FoodSecurity,
    Mustered, Rationed, Stage, Stance, Villager, Villagers,
};
use sim_core::{
    Calendar, Cause, Dial, Dues, FactionId, GovernAct, RunState, SimEvent, SimEventKind, SimTick,
    SpawnId, StateHash, StateHasher, Table, WorkFocus,
};
use sim_economy::{
    Economy, Occupation, RecipeId, SettlementEconomy, Stockpile, Workplace, WorkplaceKind,
    layout_recipe, set_recipe,
};

use crate::conquest::owner_of;
use crate::progression::Progression;
use crate::tenure::{TakenBy, Tenure};

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
/// `sim_military::govern::step`). For each run settlement with [`Edicts`],
/// in SpawnId order:
///
/// - A town the player no longer owns has its edicts reset (`reset`):
///   edicts act only while you own the town.
/// - At a town the player owns, pending settings are applied
///   (`apply_pending`): the dial turns, its lock starts
///   (`edicts.lock_days` from this hour), `EdictSet` is reported and the
///   first wave goes now: the Work dial's first moves (`work_wave`), the
///   Levy's first muster (`muster`), the ration, or the feast's meal.
/// - A feast that has run its `gather_hours` ends (`end_feast`).
/// - At dawn (the day tick), the later waves: another Work wave, another
///   muster, and residents born or arrived since get the ration.
///
/// Soldiers carry no Levy or Ration marks: they are swept off the
/// enlisted first. Every default is an exact no-op: a town whose edicts
/// are at their defaults is never written.
pub fn step(world: &mut World) {
    let _span = tracing::info_span!("sim_military::govern::step").entered();
    if world.resource::<RunState>().in_run() {
        govern(world);
    }
}

/// Edicts, sites, hire offers and, at dawn, stewards (M4a steps 4, 8, 9).
fn govern(world: &mut World) {
    let tick = world.resource::<SimTick>().0;
    let dawn = world.resource::<Calendar>().on_day(tick);
    let data = world.resource::<Progression>().data.edicts;
    sweep_soldiers(world);
    let mut q = world.query::<(Entity, &SpawnId, &Edicts)>();
    let mut towns: Vec<(SpawnId, Entity, bool)> = q
        .iter(world)
        .map(|(e, id, ed)| (*id, e, ed.is_default()))
        .collect();
    towns.sort_unstable_by_key(|t| t.0);
    for (_, s, default) in towns {
        if owner_of(world, s) != FactionId::PLAYER {
            if !default {
                reset(world, s);
            }
            continue;
        }
        let fresh = apply_pending(world, s, &data, tick);
        end_feast(world, s, &data, tick);
        if dawn {
            let Some(e) = world.get::<Edicts>(s).cloned() else {
                continue;
            };
            if !fresh[Dial::Work as usize] {
                work_wave(world, s, e.work, &data, tick);
            }
            if e.dues == Dues::Levy && !fresh[Dial::Dues as usize] {
                muster(world, s, &data, tick);
            }
            if e.table == Table::Ration {
                ration(world, s, &data);
            }
        }
    }
}

/// The Levy's and the ration's marks come off anyone enlisted (by any
/// means): a soldier waits on no green and eats from the baggage.
fn sweep_soldiers(world: &mut World) {
    let mut q =
        world.query_filtered::<Entity, (With<Enlisted>, Or<(With<Mustered>, With<Rationed>)>)>();
    let marked: Vec<Entity> = q.iter(world).collect();
    for e in marked {
        world.entity_mut(e).remove::<(Mustered, Rationed)>();
    }
}

/// A town lost (or never the player's) with edicts set: every dial back to
/// its default and every effect undone, the smithies' recipes restored,
/// the mustered sent back to work, the ration lifted, the feast ended.
/// Villagers an edict moved keep their trades (and their `Assignment`)
/// until the town is yours again and Balanced moves them back. The run's
/// building orders and the feast's cooldown stay.
fn reset(world: &mut World, s: Entity) {
    let Some(e) = world.get::<Edicts>(s).cloned() else {
        return;
    };
    if e.work == WorkFocus::Forge {
        restore_recipes(world, s);
    }
    release_muster(world, s);
    lift_ration(world, s);
    let reset = Edicts {
        builds: e.builds.clone(),
        next_feast: e.next_feast,
        ..Edicts::default()
    };
    *world.get_mut::<Edicts>(s).expect("edicts") = reset;
    crate::tenure::rebuild_terms(world, s);
}

/// Applies `s`'s pending settings (GP-34). Returns, per dial, whether it
/// turned this hour (its first wave has gone).
fn apply_pending(world: &mut World, s: Entity, data: &EdictsData, tick: u64) -> [bool; 3] {
    let mut fresh = [false; 3];
    let Some(e) = world.get::<Edicts>(s).cloned() else {
        return fresh;
    };
    if e.pending_work.is_none() && e.pending_dues.is_none() && e.pending_table.is_none() {
        return fresh;
    }
    let lock = tick + world.resource::<Calendar>().days(data.lock_days);
    if let Some(focus) = e.pending_work {
        let mut ed = world.get_mut::<Edicts>(s).expect("edicts");
        ed.pending_work = None;
        if focus != e.work {
            ed.work = focus;
            turn(&mut ed, Dial::Work, tick, lock);
            if e.work == WorkFocus::Forge {
                restore_recipes(world, s);
            }
            if focus == WorkFocus::Forge {
                forge_recipes(world, s);
            }
            report(world, s, Dial::Work, focus.tag());
            work_wave(world, s, focus, data, tick);
            fresh[Dial::Work as usize] = true;
        }
    }
    if let Some(dues) = e.pending_dues {
        let mut ed = world.get_mut::<Edicts>(s).expect("edicts");
        ed.pending_dues = None;
        if dues != e.dues {
            ed.dues = dues;
            turn(&mut ed, Dial::Dues, tick, lock);
            if e.dues == Dues::Levy {
                release_muster(world, s);
            }
            report(world, s, Dial::Dues, dues.tag());
            if dues == Dues::Levy {
                muster(world, s, data, tick);
            }
            fresh[Dial::Dues as usize] = true;
        }
    }
    if let Some(table) = e.pending_table {
        world.get_mut::<Edicts>(s).expect("edicts").pending_table = None;
        if table != e.table && set_table(world, s, table, data, tick) {
            let mut ed = world.get_mut::<Edicts>(s).expect("edicts");
            turn(&mut ed, Dial::Table, tick, lock);
            report(world, s, Dial::Table, table.tag());
            fresh[Dial::Table as usize] = true;
        }
    }
    crate::tenure::rebuild_terms(world, s);
    fresh
}

/// A dial turned at `tick`: it started now and locks until `lock`.
fn turn(ed: &mut Edicts, dial: Dial, tick: u64, lock: u64) {
    let d = &mut ed.dials[dial as usize];
    d.since = tick;
    d.locked_until = lock;
}

/// Reports a dial turned at `s` (`EdictSet`, feed only) and logs it.
fn report(world: &mut World, s: Entity, dial: Dial, setting: u8) {
    let Some(ed) = world.get::<Edicts>(s) else {
        return;
    };
    let cause = if ed.dial(dial).by_player {
        Cause::Player
    } else {
        Cause::Edict
    };
    let id = world.get::<SpawnId>(s).map_or(0, |id| id.0);
    let at = crate::orders::center(world, s).unwrap_or_default();
    let owner = owner_of(world, s);
    world.write_message(
        SimEvent::new(SimEventKind::EdictSet, owner)
            .subject(id)
            .other(dial.tag() as u64)
            .at(at)
            .amount(setting as i32)
            .cause(cause),
    );
    let name = crate::orders::settlement_name(world, s);
    let who = crate::orders::faction_name(world, owner);
    crate::orders::log(
        world,
        format!(
            "{who}: {name}'s {} is now {}",
            dial.name(),
            dial.setting_name(setting)
        ),
    );
}

/// Turns the Table dial of `s` to `table` and starts it (the ration on
/// everyone, the feast's meal). Returns false, changing nothing, if a
/// feast can't be held now (under `feast.min_food_days` of food, or before
/// `next_feast`).
fn set_table(world: &mut World, s: Entity, table: Table, data: &EdictsData, tick: u64) -> bool {
    let Some(old) = world.get::<Edicts>(s).map(|e| e.table) else {
        return false;
    };
    if table == Table::Feast && feast_refusal(world, s, data, tick).is_some() {
        let name = crate::orders::settlement_name(world, s);
        crate::orders::log(
            world,
            format!("{name} has no feast: too little food, or too soon"),
        );
        return false;
    }
    match old {
        Table::Ration => lift_ration(world, s),
        Table::Feast => world.get_mut::<Edicts>(s).expect("edicts").feast_until = 0,
        Table::Normal => {}
    }
    match table {
        Table::Ration => ration(world, s, data),
        Table::Feast => {
            let need = data.feast.food_days * town_need(world, s);
            eat_from_store(world, s, need);
            let gather = world.resource::<Calendar>().hours(data.feast.gather_hours);
            let mut ed = world.get_mut::<Edicts>(s).expect("edicts");
            ed.feast_until = tick + gather.max(1);
            ed.feast_work = data.feast.work;
        }
        Table::Normal => {}
    }
    world.get_mut::<Edicts>(s).expect("edicts").table = table;
    true
}

/// Why `s` can't hold a feast at `tick`, if it can't: under
/// `feast.min_food_days` of food, or before its cooldown ends.
fn feast_refusal(world: &World, s: Entity, data: &EdictsData, tick: u64) -> Option<&'static str> {
    let days = world.get::<FoodSecurity>(s).map_or(0.0, |f| f.days);
    if days < data.feast.min_food_days {
        return Some("too little food for a feast");
    }
    if world.get::<Edicts>(s).is_some_and(|e| tick < e.next_feast) {
        return Some("too soon for another feast");
    }
    None
}

/// The food value `s`'s people eat in a day (unrationed).
fn town_need(world: &World, s: Entity) -> f32 {
    let people = world
        .get::<SettlementEconomy>(s)
        .map_or(0, |se| se.population);
    crate::orders::daily_need(world, people)
}

/// The feast eats `value` of food from `s`'s storehouse, the least
/// nourishing first (a serial write to the town's own stores, as
/// `requests::pay` does; ADR 0020). Returns the food value eaten.
fn eat_from_store(world: &mut World, s: Entity, value: f32) -> f32 {
    let Some(store) = crate::orders::storehouse(world, s) else {
        return 0.0;
    };
    let econ = world.resource::<Economy>().clone();
    let Some(mut st) = world.get_mut::<Stockpile>(store) else {
        return 0.0;
    };
    let mut eaten = 0.0;
    for g in econ.foods_by_value().into_iter().rev() {
        let v = econ.food_value(g);
        if v <= 0.0 || eaten >= value {
            continue;
        }
        let want = ((value - eaten) / v).ceil() as u32;
        let took = st.take(g, want);
        eaten += took as f32 * v;
    }
    eaten
}

/// Ends `s`'s feast once its hours are up (GP-34): if the town still has
/// `feast.food_days` of food, `Deed::Festival` (it spreads: word travels)
/// and the Table term for `feast.term_days`; either way the dial goes back
/// to Normal and the next feast waits `feast.cooldown_days`.
fn end_feast(world: &mut World, s: Entity, data: &EdictsData, tick: u64) {
    let Some(e) = world.get::<Edicts>(s) else {
        return;
    };
    if e.feast_until == 0 || tick < e.feast_until {
        return;
    }
    let fed = world.get::<FoodSecurity>(s).map_or(0.0, |f| f.days) >= data.feast.food_days;
    let cal = world.resource::<Calendar>().clone();
    let owner = owner_of(world, s);
    {
        let mut ed = world.get_mut::<Edicts>(s).expect("edicts");
        ed.feast_until = 0;
        ed.table = Table::Normal;
        ed.dials[Dial::Table as usize].by_player = false;
        ed.next_feast = tick + cal.days(data.feast.cooldown_days);
        if fed {
            ed.feast_term_until = tick + cal.days(data.feast.term_days);
        }
    }
    let name = crate::orders::settlement_name(world, s);
    if fed {
        crate::orders::goodwill_deed(world, s, owner, Deed::Festival);
        crate::orders::log(world, format!("{name}'s feast ended well: word travels"));
    } else {
        crate::orders::log(
            world,
            format!("{name}'s feast ended hungry: nobody thanks you"),
        );
    }
    crate::tenure::rebuild_terms(world, s);
}

/// The Ration edict on `s` and every resident (GP-34): marks the settlement
/// (its `FoodSecurity`) and residents who lack the mark (children born or
/// people arrived since get it at dawn).
fn ration(world: &mut World, s: Entity, data: &EdictsData) {
    let r = Rationed {
        hunger: data.ration.hunger,
        work: data.ration.work,
    };
    if world.get::<Rationed>(s) != Some(&r) {
        world.entity_mut(s).insert(r);
    }
    let mut q = world.query_filtered::<(Entity, &Villager, Option<&Rationed>), Without<Enlisted>>();
    let folk: Vec<Entity> = q
        .iter(world)
        .filter(|(_, v, had)| v.settlement == s && *had != Some(&r))
        .map(|(e, ..)| e)
        .collect();
    for e in folk {
        world.entity_mut(e).insert(r);
    }
}

/// Lifts the ration from `s` and its residents.
fn lift_ration(world: &mut World, s: Entity) {
    world.entity_mut(s).remove::<Rationed>();
    let mut q = world.query_filtered::<(Entity, &Villager), With<Rationed>>();
    let folk: Vec<Entity> = q
        .iter(world)
        .filter(|(_, v)| v.settlement == s)
        .map(|(e, _)| e)
        .collect();
    for e in folk {
        world.entity_mut(e).remove::<Rationed>();
    }
}

/// The Levy's muster at `s` (GP-34): `levy.per_day` of the people (at
/// least one), able adults with the idle first (`recruit_candidates`),
/// are marked [`Mustered`] and wait on the green, up to `levy.cap` of the
/// people waiting at once. Returns how many mustered.
fn muster(world: &mut World, s: Entity, data: &EdictsData, tick: u64) -> u32 {
    let people = world
        .get::<SettlementEconomy>(s)
        .map_or(0, |se| se.population);
    let (per, cap) = levy_sizes(&data.levy, people);
    let waiting = mustered(world, s).len() as u32;
    let n = per.min(cap.saturating_sub(waiting));
    if n == 0 {
        return 0;
    }
    let chosen: Vec<Entity> = crate::orders::recruit_candidates(world, s)
        .into_iter()
        .filter(|&e| world.get::<Mustered>(e).is_none() && world.get::<Agent>(e).is_none())
        .take(n as usize)
        .collect();
    for &e in &chosen {
        world.entity_mut(e).insert(Mustered { since: tick });
        if let Some(mut act) = world.get_mut::<Activity>(e)
            && matches!(act.kind, ActionKind::Work | ActionKind::Haul)
            && act.stage != Stage::Done
        {
            act.stage = Stage::Done;
        }
    }
    chosen.len() as u32
}

/// The Levy's sizes at a town of `people` (GP-34): (a wave, the most
/// waiting at once): `per_day` and `cap` of the people, rounded, at least
/// one each.
pub fn levy_sizes(levy: &LevyData, people: u32) -> (u32, u32) {
    let of = |share: f32| ((people as f32 * share).round() as u32).max(1);
    (of(levy.per_day), of(levy.cap))
}

/// `s`'s residents waiting on the green ([`Mustered`], not yet enlisted),
/// in SpawnId order.
pub fn mustered(world: &mut World, s: Entity) -> Vec<Entity> {
    let mut q = world
        .query_filtered::<(Entity, &SpawnId, &Villager), (With<Mustered>, Without<Enlisted>)>();
    let mut out: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, v)| v.settlement == s)
        .map(|(e, id, _)| (*id, e))
        .collect();
    out.sort_unstable();
    out.into_iter().map(|(_, e)| e).collect()
}

/// Sends `s`'s mustered back to work (the Levy lifted, or the town lost).
fn release_muster(world: &mut World, s: Entity) {
    for e in mustered(world, s) {
        world.entity_mut(e).remove::<Mustered>();
    }
}

/// The weaponsmithing recipe (the smithy recipe that makes weapons).
fn weaponsmithing(econ: &Economy) -> Option<RecipeId> {
    let weapons = econ.data.goods.iter().position(|g| g.name == "weapons")?;
    econ.recipes_for(WorkplaceKind::Smithy)
        .into_iter()
        .find(|r| {
            econ.recipe(*r)
                .outputs
                .iter()
                .any(|(g, _)| g.0 as usize == weapons)
        })
}

/// `s`'s smithies, in the settlement's workplace order (SpawnId order).
fn smithies(world: &World, s: Entity) -> Vec<Entity> {
    world
        .get::<SettlementEconomy>(s)
        .map(|se| {
            se.workplaces
                .iter()
                .copied()
                .filter(|&w| {
                    world
                        .get::<Workplace>(w)
                        .is_some_and(|wp| wp.kind == WorkplaceKind::Smithy)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Forge (GP-34): every smithy of `s` turns to weaponsmithing.
fn forge_recipes(world: &mut World, s: Entity) {
    let econ = world.resource::<Economy>().clone();
    let Some(recipe) = weaponsmithing(&econ) else {
        return;
    };
    for w in smithies(world, s) {
        if let Some(mut wp) = world.get_mut::<Workplace>(w)
            && wp.recipe != Some(recipe)
        {
            // A smithy always accepts a smithy recipe.
            let _ = set_recipe(&econ, &mut wp, recipe);
        }
    }
}

/// Leaving Forge (GP-34): `s`'s smithies go back to the layout's
/// alternation (`layout_recipe` by position among its smithies).
fn restore_recipes(world: &mut World, s: Entity) {
    let econ = world.resource::<Economy>().clone();
    let recipes = econ.recipes_for(WorkplaceKind::Smithy);
    for (i, w) in smithies(world, s).into_iter().enumerate() {
        let Some(recipe) = layout_recipe(&recipes, i) else {
            continue;
        };
        if let Some(mut wp) = world.get_mut::<Workplace>(w)
            && wp.recipe != Some(recipe)
        {
            let _ = set_recipe(&econ, &mut wp, recipe);
        }
    }
}

/// One of a town's workers, as a wave of moves sees them.
#[derive(Debug, Clone, Copy)]
struct Hand {
    e: Entity,
    occ: Occupation,
    assigned: Option<Assignment>,
}

/// `s`'s workers (GP-34): working-age, living, civilian residents who
/// aren't enlisted, mustered or agents, in SpawnId order.
fn hands(world: &mut World, s: Entity) -> Vec<Hand> {
    let days_per_year = world.resource::<Calendar>().days_per_year();
    let working = world.resource::<Villagers>().data.households.working_age;
    let mut q = world.query_filtered::<(
        Entity,
        &SpawnId,
        &Villager,
        &Occupation,
        &CombatStats,
        Option<&Assignment>,
    ), (Without<Enlisted>, Without<Mustered>, Without<Agent>)>();
    let mut out: Vec<(SpawnId, Hand)> = q
        .iter(world)
        .filter(|(_, _, v, _, cs, _)| {
            let years = v.age_days as u32 / days_per_year;
            v.settlement == s
                && cs.alive()
                && cs.stance == Stance::Civilian
                && years >= working.min
                && years <= working.max
        })
        .map(|(e, id, _, occ, _, a)| {
            (
                *id,
                Hand {
                    e,
                    occ: *occ,
                    assigned: a.copied(),
                },
            )
        })
        .collect();
    out.sort_unstable_by_key(|h| h.0);
    out.into_iter().map(|(_, h)| h).collect()
}

const FOOD: [WorkplaceKind; 4] = [
    WorkplaceKind::Farm,
    WorkplaceKind::FishingSpot,
    WorkplaceKind::Pasture,
    WorkplaceKind::Bakery,
];

fn is_food(o: Occupation) -> bool {
    o.workplace().is_some_and(|k| FOOD.contains(&k))
}

fn is_forge(o: Occupation) -> bool {
    matches!(
        o,
        Occupation::Smith | Occupation::Miner | Occupation::Woodcutter
    )
}

/// The most a wave moves among `workers` (GP-34): `moves_share` of them,
/// rounded up, at least `min_moves`.
pub fn wave_size(data: &EdictsData, workers: u32) -> u32 {
    ((workers as f32 * data.moves_share).ceil() as u32).max(data.min_moves)
}

/// Who a wave of `focus` would move at `s`, in order, with where to
/// (GP-34): the donors the design lists, each with the workplace kinds it
/// may go to (nearest with room among them), and the wave's cap on the
/// moves. Read-only, so the forecasts share it.
fn plan(world: &mut World, s: Entity, focus: WorkFocus, data: &EdictsData) -> Plan {
    let hands = hands(world, s);
    let workers = hands.len() as u32;
    let fed = world.get::<FoodSecurity>(s).map_or(0.0, |f| f.days) >= 1.0;
    let idle = hands.iter().filter(|h| h.occ == Occupation::Idle);
    let farmers = hands.iter().filter(|h| fed && h.occ == Occupation::Farmer);
    let (donors, room): (Vec<Hand>, u32) = match focus {
        WorkFocus::Balanced => (
            hands
                .iter()
                .filter(|h| h.assigned.is_some_and(|a| a.from != h.occ))
                .copied()
                .collect(),
            u32::MAX,
        ),
        WorkFocus::Fields => {
            let mut first: Vec<Occupation> = Vec::new();
            let extra = hands.iter().filter(|h| {
                if !is_forge(h.occ) {
                    return false;
                }
                if first.contains(&h.occ) {
                    true
                } else {
                    first.push(h.occ);
                    false
                }
            });
            let keep = (workers as f32 * data.carter_floor).ceil() as usize;
            let carters = hands
                .iter()
                .filter(|h| h.occ == Occupation::Carter)
                .skip(keep);
            let donors = idle.chain(extra).chain(carters).copied().collect();
            let food = hands.iter().filter(|h| is_food(h.occ)).count() as u32;
            let cap = (workers as f32 * data.fields_cap).floor() as u32;
            (donors, cap.saturating_sub(food))
        }
        WorkFocus::Forge => {
            let forge = hands.iter().filter(|h| is_forge(h.occ)).count() as u32;
            let cap = (workers as f32 * data.forge_cap).floor() as u32;
            (
                idle.chain(farmers).copied().collect(),
                cap.saturating_sub(forge),
            )
        }
        WorkFocus::Walls => {
            let raised = hands
                .iter()
                .filter(|h| {
                    h.occ == Occupation::Carter
                        && h.assigned.is_some_and(|a| a.focus == WorkFocus::Walls)
                })
                .count() as u32;
            (
                idle.chain(farmers).copied().collect(),
                data.walls_carters.saturating_sub(raised),
            )
        }
    };
    Plan {
        donors,
        moves: wave_size(data, workers).min(room),
    }
}

/// A wave's donors and its cap on moves.
struct Plan {
    donors: Vec<Hand>,
    moves: u32,
}

/// Where a donor goes under `focus` at `s`: the nearest workplace with
/// room of the focus's kinds (Forge: a smithy first, then the mine or the
/// woodcutters' camp, whichever has fewer workers in the town).
fn destination(world: &World, s: Entity, h: &Hand, focus: WorkFocus) -> Option<Entity> {
    match focus {
        WorkFocus::Balanced => {
            let from = h.assigned?.from;
            match from.workplace() {
                Some(kind) => sim_agents::jobs::nearest_with_room(world, h.e, &[kind]),
                None => None,
            }
        }
        WorkFocus::Fields => sim_agents::jobs::nearest_with_room(world, h.e, &FOOD),
        WorkFocus::Walls => {
            sim_agents::jobs::nearest_with_room(world, h.e, &[WorkplaceKind::Storehouse])
        }
        WorkFocus::Forge => {
            if let Some(w) =
                sim_agents::jobs::nearest_with_room(world, h.e, &[WorkplaceKind::Smithy])
            {
                return Some(w);
            }
            let count = |kind: WorkplaceKind| -> usize {
                world.get::<SettlementEconomy>(s).map_or(0, |se| {
                    se.workplaces
                        .iter()
                        .filter_map(|w| world.get::<Workplace>(*w))
                        .filter(|w| w.kind == kind)
                        .map(|w| w.workers.len())
                        .sum()
                })
            };
            let mut kinds = [WorkplaceKind::Mine, WorkplaceKind::WoodcutterCamp];
            if count(kinds[1]) < count(kinds[0]) {
                kinds.swap(0, 1);
            }
            kinds
                .into_iter()
                .find_map(|k| sim_agents::jobs::nearest_with_room(world, h.e, &[k]))
        }
    }
}

/// One wave of the Work dial's moves at `s` (GP-34; LORDSHIP_DESIGN 8.1):
/// at most [`wave_size`] of its workers, in the donors' order, each to the
/// nearest workplace with room of the focus's kinds, with
/// `sim_agents::jobs::reassign_to` (empty-handed, not mid-cycle). A moved
/// villager carries an [`Assignment`] (his first trade kept in `from`);
/// one moved back to that trade loses it. Balanced moves the assigned
/// back. Returns how many moved.
fn work_wave(world: &mut World, s: Entity, focus: WorkFocus, data: &EdictsData, tick: u64) -> u32 {
    let plan = plan(world, s, focus, data);
    let by_steward = !world
        .get::<Edicts>(s)
        .is_some_and(|e| e.dial(Dial::Work).by_player);
    let mut moved = 0;
    for h in plan.donors {
        if moved >= plan.moves {
            break;
        }
        let Some(to) = destination(world, s, &h, focus) else {
            continue;
        };
        if !sim_agents::jobs::reassign_to(world, h.e, to) {
            continue;
        }
        moved += 1;
        let now = world.get::<Occupation>(h.e).copied().unwrap_or(h.occ);
        let from = h.assigned.map_or(h.occ, |a| a.from);
        if now == from {
            world.entity_mut(h.e).remove::<Assignment>();
        } else {
            world.entity_mut(h.e).insert(Assignment {
                from,
                focus,
                since: tick,
                by_steward,
            });
        }
    }
    moved
}

/// GP-34: the Dues loyalty term at town `s` (summed by `tenure::step`):
/// Tithe `edicts.tithe.term` (0 at a submitted town: it's the deal), Levy
/// `edicts.levy.term` (`harsh_term` at a conquered or sacked town, 0 at an
/// invited one), Light 0.0. Edicts act only at the player's towns: 0.0
/// elsewhere.
pub fn dues_term(world: &World, s: Entity) -> f32 {
    let Some(e) = world.get::<Edicts>(s) else {
        return 0.0;
    };
    if owner_of(world, s) != FactionId::PLAYER {
        return 0.0;
    }
    let data = &world.resource::<Progression>().data.edicts;
    let how = world.get::<Tenure>(s).map(|t| t.how);
    match e.dues {
        Dues::Light => 0.0,
        Dues::Tithe if how == Some(TakenBy::Submitted) => 0.0,
        Dues::Tithe => data.tithe.term,
        Dues::Levy => match how {
            Some(TakenBy::Invited) => 0.0,
            Some(TakenBy::Conquered | TakenBy::Sacked) => data.levy.harsh_term,
            _ => data.levy.term,
        },
    }
}

/// GP-34: the Table loyalty term at town `s` (summed by `tenure::step`):
/// `edicts.ration.term` while rationed, plus `edicts.feast.term` for
/// `feast.term_days` after a feast that ended fed; 0.0 otherwise, and at
/// any town not the player's.
pub fn table_term(world: &World, s: Entity) -> f32 {
    let Some(e) = world.get::<Edicts>(s) else {
        return 0.0;
    };
    if owner_of(world, s) != FactionId::PLAYER {
        return 0.0;
    }
    let data = &world.resource::<Progression>().data.edicts;
    let tick = world.resource::<SimTick>().0;
    let mut term = 0.0;
    if e.table == Table::Ration {
        term += data.ration.term;
    }
    if tick < e.feast_term_until {
        term += data.feast.term;
    }
    term
}

/// GP-34: the Steward loyalty term at town `s` (a native steward; summed
/// by `tenure::step`). 0.0 until M4a step 9.
pub fn steward_term(_world: &World, _s: Entity) -> f32 {
    0.0
}

/// GP-34: how much faster a building site at `s` goes up:
/// `edicts.walls_build` under Walls, else exactly 1.0 (read by step 8's
/// `construction::advance`).
pub fn build_speed(world: &World, s: Entity) -> f32 {
    match world.get::<Edicts>(s) {
        Some(e) if e.work == WorkFocus::Walls => {
            world.resource::<Progression>().data.edicts.walls_build
        }
        _ => 1.0,
    }
}

/// Why the `Govern` act `act` by `issuer` at settlement `s` would be
/// refused now (GP-34), or `None`: the panel greys a dial with this
/// reason, and the command is refused with it. Not in a run, not your
/// settlement, a locked dial, Dues other than Tithe at a submitted town
/// that hasn't settled (its tribute), the setting already set or ordered,
/// a feast the town can't hold; building orders and stewards are "not
/// yet" (steps 8 and 9).
pub fn refusal(
    world: &World,
    issuer: FactionId,
    s: Entity,
    act: GovernAct,
) -> Option<&'static str> {
    if !world.resource::<RunState>().in_run() {
        return Some("not in a run");
    }
    if owner_of(world, s) != issuer {
        return Some("not your settlement");
    }
    let Some(e) = world.get::<Edicts>(s) else {
        return Some("no edicts here");
    };
    let tick = world.resource::<SimTick>().0;
    let (dial, same, ordered) = match act {
        GovernAct::Work(f) => (
            Dial::Work,
            e.work == f && e.pending_work.is_none(),
            e.pending_work == Some(f),
        ),
        GovernAct::Dues(d) => (
            Dial::Dues,
            e.dues == d && e.pending_dues.is_none(),
            e.pending_dues == Some(d),
        ),
        GovernAct::Table(t) => (
            Dial::Table,
            e.table == t && e.pending_table.is_none(),
            e.pending_table == Some(t),
        ),
        GovernAct::Build(_) | GovernAct::Steward { .. } | GovernAct::Release(_) => {
            return Some("not yet");
        }
    };
    if e.locked(dial, tick) {
        return Some("locked");
    }
    // A submitted town pays tribute: its Dues stay at Tithe until it
    // settles (`demand.settle_days` after its submission; step 5 orders
    // the Tithe with `Edicts::order_dues(Dues::Tithe, false)`).
    if let GovernAct::Dues(d) = act
        && d != Dues::Tithe
        && let Some(t) = world.get::<Tenure>(s)
        && t.how == TakenBy::Submitted
    {
        let cal = world.resource::<Calendar>();
        let settle = cal.days(world.resource::<Progression>().data.demand.settle_days);
        if tick < t.since + settle {
            return Some("tribute: held at Tithe");
        }
    }
    if same {
        return Some("already set");
    }
    if ordered {
        return Some("already ordered");
    }
    if act == GovernAct::Table(Table::Feast) {
        let data = &world.resource::<Progression>().data.edicts;
        return feast_refusal(world, s, data, tick);
    }
    None
}

/// GP-34: applies a `Govern` command (the `Govern` arm of
/// `orders::apply`): validated by [`refusal`], then the setting is
/// ordered (`Edicts::order_*`, by the player) for the next hourly
/// [`step`], and logged.
pub(crate) fn command(
    world: &mut World,
    issuer: FactionId,
    settlement: u64,
    act: GovernAct,
) -> Result<(), &'static str> {
    let s = crate::orders::find_settlement(world, settlement).ok_or("no such settlement")?;
    if let Some(why) = refusal(world, issuer, s, act) {
        return Err(why);
    }
    let mut e = world.get_mut::<Edicts>(s).expect("checked");
    let (dial, setting) = match act {
        GovernAct::Work(f) => {
            e.order_work(f, true);
            (Dial::Work, f.tag())
        }
        GovernAct::Dues(d) => {
            e.order_dues(d, true);
            (Dial::Dues, d.tag())
        }
        GovernAct::Table(t) => {
            e.order_table(t, true);
            (Dial::Table, t.tag())
        }
        _ => unreachable!("refused above"),
    };
    let who = crate::orders::faction_name(world, issuer);
    let name = crate::orders::settlement_name(world, s);
    crate::orders::log(
        world,
        format!(
            "{who}: {name}'s {} set to {} (from the next hour)",
            dial.name(),
            dial.setting_name(setting)
        ),
    );
    Ok(())
}

/// What a Work setting would do at a town now (GP-34), for the panel's
/// forecast: the first wave's moves, every move it would make up to its
/// cap, and the dawns after the first wave to finish them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkForecast {
    /// Moves in the first wave (this hour).
    pub first: u32,
    /// Moves in all, at today's numbers.
    pub total: u32,
    /// Farmers among them (the food it costs).
    pub farmers: u32,
    /// Later waves (dawns) to finish.
    pub dawns: u32,
}

/// GP-34: [`WorkForecast`] for `focus` at `s` (read-only in effect: it
/// queries, never writes). Counts donors with a place to go, by kind's
/// free room, up to the focus's cap.
pub fn work_forecast(world: &mut World, s: Entity, focus: WorkFocus) -> WorkForecast {
    let data = world.resource::<Progression>().data.edicts;
    let p = plan(world, s, focus, &data);
    let hs = hands(world, s);
    let workers = hs.len() as u32;
    let wave = wave_size(&data, workers);
    let world: &World = world;
    // The cap on the moves in all: the focus's room (Balanced: everyone
    // assigned), bounded by free places.
    let room = |kinds: &[WorkplaceKind]| -> u32 {
        let econ = world.resource::<Economy>();
        world.get::<SettlementEconomy>(s).map_or(0, |se| {
            se.workplaces
                .iter()
                .filter_map(|w| world.get::<Workplace>(*w))
                .filter(|w| kinds.contains(&w.kind))
                .map(|w| {
                    if w.kind == WorkplaceKind::Storehouse {
                        u32::MAX / 4
                    } else {
                        econ.workplace(w.kind)
                            .slots
                            .saturating_sub(w.workers.len() as u32)
                    }
                })
                .fold(0u32, |a, b| a.saturating_add(b))
        })
    };
    let cap_all = match focus {
        WorkFocus::Balanced => u32::MAX,
        WorkFocus::Fields => {
            let food = hs.iter().filter(|h| is_food(h.occ)).count() as u32;
            ((workers as f32 * data.fields_cap).floor() as u32)
                .saturating_sub(food)
                .min(room(&FOOD))
        }
        WorkFocus::Forge => {
            let forge = hs.iter().filter(|h| is_forge(h.occ)).count() as u32;
            ((workers as f32 * data.forge_cap).floor() as u32)
                .saturating_sub(forge)
                .min(room(&[
                    WorkplaceKind::Smithy,
                    WorkplaceKind::Mine,
                    WorkplaceKind::WoodcutterCamp,
                ]))
        }
        WorkFocus::Walls => p.moves.max(data.walls_carters.min(p.donors.len() as u32)),
    };
    let total = (p.donors.len() as u32).min(cap_all);
    let first = total.min(p.moves).min(wave);
    let farmers = p
        .donors
        .iter()
        .take(total as usize)
        .filter(|h| h.occ == Occupation::Farmer)
        .count() as u32;
    let dawns = if wave == 0 {
        0
    } else {
        total.saturating_sub(first).div_ceil(wave)
    };
    WorkForecast {
        first,
        total,
        farmers,
        dawns,
    }
}

/// What a Dues setting would bring at a town now (GP-34), for the panel's
/// forecast.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DuesForecast {
    /// The loyalty term it would carry.
    pub term: f32,
    /// Tithe: food beyond the reserve (food value).
    pub food: f32,
    /// Tithe: weapons and tools in the storehouse.
    pub arms: u32,
    /// Levy: men a wave (the first now, one more each dawn) ...
    pub men_per_day: u32,
    /// ... and at most this many waiting.
    pub men_cap: u32,
    /// The nearest owner captain within reach (SpawnId), if any: the
    /// collector.
    pub collector: Option<u64>,
}

/// GP-34: [`DuesForecast`] for `dues` at `s`.
pub fn dues_forecast(world: &mut World, s: Entity, dues: Dues) -> DuesForecast {
    let data = world.resource::<Progression>().data.edicts;
    let how = world.get::<Tenure>(s).map(|t| t.how);
    let term = match dues {
        Dues::Light => 0.0,
        Dues::Tithe if how == Some(TakenBy::Submitted) => 0.0,
        Dues::Tithe => data.tithe.term,
        Dues::Levy => match how {
            Some(TakenBy::Invited) => 0.0,
            Some(TakenBy::Conquered | TakenBy::Sacked) => data.levy.harsh_term,
            _ => data.levy.term,
        },
    };
    let people = world
        .get::<SettlementEconomy>(s)
        .map_or(0, |se| se.population);
    let (men_per_day, men_cap) = match dues {
        Dues::Levy => levy_sizes(&data.levy, people),
        _ => (0, 0),
    };
    let (food, arms) = match dues {
        Dues::Tithe => (
            crate::orders::spare_food(world, s, data.tithe.keep_days, true),
            crate::orders::store_arms(world, s),
        ),
        _ => (0.0, 0),
    };
    let owner = owner_of(world, s);
    let collector = crate::orders::collector(world, s, owner, data.reach, false)
        .and_then(|c| world.get::<SpawnId>(c).map(|id| id.0));
    DuesForecast {
        term,
        food,
        arms,
        men_per_day,
        men_cap,
        collector,
    }
}

/// What a Table setting would do at a town now (GP-34), for the panel's
/// forecast.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TableForecast {
    /// The loyalty term while it holds (a feast's comes after it, if fed).
    pub term: f32,
    /// Food the town eats a day under it, as a share of today's (Ration).
    pub hunger: f32,
    /// Days of food it costs now (a feast's meal).
    pub food_days: f32,
    /// Why it can't start now, if it can't (a feast without the food, or
    /// too soon).
    pub refused: Option<&'static str>,
}

/// GP-34: [`TableForecast`] for `table` at `s`.
pub fn table_forecast(world: &World, s: Entity, table: Table) -> TableForecast {
    let data = world.resource::<Progression>().data.edicts;
    let tick = world.resource::<SimTick>().0;
    match table {
        Table::Normal => TableForecast {
            hunger: 1.0,
            ..Default::default()
        },
        Table::Ration => TableForecast {
            term: data.ration.term,
            hunger: data.ration.hunger,
            ..Default::default()
        },
        Table::Feast => TableForecast {
            term: data.feast.term,
            hunger: 1.0,
            food_days: data.feast.food_days,
            refused: feast_refusal(world, s, &data, tick),
        },
    }
}

/// GP-34: ticks until `dial` unlocks at `s` (0 when unlocked).
pub fn lock_left(world: &World, s: Entity, dial: Dial) -> u64 {
    let tick = world.resource::<SimTick>().0;
    world
        .get::<Edicts>(s)
        .map_or(0, |e| e.dial(dial).locked_until.saturating_sub(tick))
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
