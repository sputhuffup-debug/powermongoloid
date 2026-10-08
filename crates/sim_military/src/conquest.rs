//! Conquest, garrisons and revolts (GP-05, GP-06; ADR 0014).
//!
//! - [`transfer`]: the one way sim_military hands a settlement to a
//!   faction. It goes through [`sim_agents::set_owner`], reports the change
//!   ([`SimEventKind::Conquest`], `Invited` or `Revolt`), seats a faction
//!   that held no land at its first settlement, counts conquests, and ends
//!   the run in a win when the player takes the rival's seat (GP-28).
//! - [`militia_or_foes_near`]: whether anyone still contests a centre. An
//!   `Attack` (in `orders::step_order`) takes a settlement by breaking its
//!   militia, then holding the centre uncontested for `conquest.hold_hours`.
//! - [`revolts`]: once a day, from the progress driver
//!   ([`crate::progression::progress`]). A settlement whose mean loyalty
//!   has been below `loyalty.revolt_threshold` for a day goes independent
//!   at once if it has no garrison, and rises against a weaker one; a
//!   rising still standing a day later wins.

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use sim_agents::{
    CombatStats, Enlisted, Loyalty, Needs, Owner, OwnerChange, SettlementLoyalty, Stance, Threat,
    Villager, Villagers,
};
use sim_core::{
    Calendar, Cause, FactionId, NameTable, RunOutcome, RunState, SimEvent, SimEventKind, SimTick,
    SpawnId,
};
use sim_world::Settlement;

use crate::components::{ArmyStats, Factions};
use crate::data::Army;
use crate::index::FighterGrid;
use crate::orders::log;
use crate::progression::RunProgress;

/// The faction holding settlement `s` (independent if it has no [`Owner`]).
pub fn owner_of(world: &World, s: Entity) -> FactionId {
    world
        .get::<Owner>(s)
        .map_or(FactionId::INDEPENDENT, |o| o.faction)
}

fn settlement_name(world: &World, s: Entity) -> String {
    let names = world.resource::<NameTable>();
    world
        .get::<Settlement>(s)
        .map_or_else(|| "?".into(), |st| names.get(st.name).to_string())
}

fn faction_name(world: &World, f: FactionId) -> String {
    world.resource::<Army>().faction_name(f).to_string()
}

/// Hands settlement `s` to `to` because of `cause` (GP-05, GP-06), with
/// residents' loyalty seeded from their goodwill toward `to` (or
/// `loyalty.independent_target` when `to` is independent). See
/// [`transfer_seeded`].
pub fn transfer(
    world: &mut World,
    s: Entity,
    to: FactionId,
    cause: Cause,
) -> Option<OwnerChange> {
    transfer_seeded(world, s, to, cause, None)
}

/// Hands settlement `s` to `to` because of `cause`, seeding residents'
/// loyalty with `seed` (or as [`sim_agents::set_owner`] does when `None`).
/// Call it from serial steps only.
///
/// - Sets the owner, allegiance and loyalty through `set_owner`.
/// - Writes a [`SimEvent`]: `Invited` for [`Cause::Invitation`], `Revolt`
///   (with the old owner as its faction) for [`Cause::Revolt`], otherwise
///   `Conquest`.
/// - A conquest counts in the taker's `FactionStats::conquests`.
/// - A faction (not the independents) that held no settlement before gets
///   its seat (where `Return` leads) at this one's centre.
/// - GP-28: when the player takes the rival's seat in a run, the run ends
///   in a win (`RunState::finish`, which does nothing outside a run).
///
/// Returns `None`, changing nothing, if `s` isn't a populated settlement.
pub fn transfer_seeded(
    world: &mut World,
    s: Entity,
    to: FactionId,
    cause: Cause,
    seed: Option<f32>,
) -> Option<OwnerChange> {
    let _span = tracing::info_span!("sim_military::transfer").entered();
    world.get::<Owner>(s)?;
    let id = world.get::<SpawnId>(s)?.0;
    let center = world.get::<Settlement>(s)?.center;
    let landless = to != FactionId::INDEPENDENT && !holds_land(world, to, s);
    let change = sim_agents::set_owner(world, s, to, seed)?;
    let kind = match cause {
        Cause::Invitation => SimEventKind::Invited,
        Cause::Revolt => SimEventKind::Revolt,
        _ => SimEventKind::Conquest,
    };
    let (faction, other) = if kind == SimEventKind::Revolt {
        (change.previous, to)
    } else {
        (to, change.previous)
    };
    world.write_message(
        SimEvent::new(kind, faction)
            .subject(id)
            .other(other.0 as u64)
            .at(center)
            .amount(change.residents as i32)
            .cause(cause),
    );
    if kind == SimEventKind::Conquest && to != FactionId::INDEPENDENT {
        world.resource_mut::<ArmyStats>().of(to).conquests += 1;
    }
    if landless {
        world.resource_mut::<Factions>().set_seat(to, center);
    }
    let name = settlement_name(world, s);
    let text = match kind {
        SimEventKind::Revolt => {
            let who = faction_name(world, change.previous);
            format!("{name} threw off {who} and is free")
        }
        SimEventKind::Invited => format!("{name} invited {} in", faction_name(world, to)),
        _ => format!("{} took {name}", faction_name(world, to)),
    };
    log(world, text);
    // GP-28: taking the rival's seat wins the run. (M4 step 7 also frees
    // the rest of the rival's lands here.)
    if to == FactionId::PLAYER && world.resource::<RunProgress>().rival_seat == Some(id) {
        world
            .resource_mut::<RunState>()
            .finish(RunOutcome::Won, cause);
    }
    Some(change)
}

/// Whether `faction` owns a populated settlement other than `except`.
fn holds_land(world: &mut World, faction: FactionId, except: Entity) -> bool {
    let mut q = world.query::<(Entity, &Owner)>();
    q.iter(world)
        .any(|(e, o)| e != except && o.faction == faction)
}

/// Whether anyone still contests `at` against `faction` within `r` tiles:
/// militia defending against `faction`, or soldiers of a faction at war
/// with it, who aren't routed. Reads this tick's [`FighterGrid`].
pub fn militia_or_foes_near(world: &World, at: IVec2, faction: FactionId, r: f32) -> bool {
    let army = world.resource::<Army>();
    let grid = world.resource::<FighterGrid>();
    let mut any = false;
    grid.for_each_within(at.as_vec2(), r, |e, _| {
        if e.routed {
            return;
        }
        let contests = match e.side.against {
            Some(f) => f == faction,
            None => army.at_war(e.side.faction, faction),
        };
        any |= contests;
    });
    any
}

/// Soldiers of `owner` (not routed) within `garrison.radius` of `at`.
fn garrison_strength(world: &World, at: IVec2, owner: FactionId) -> u32 {
    let radius = world.resource::<Army>().data.garrison.radius;
    let grid = world.resource::<FighterGrid>();
    let mut n = 0;
    grid.for_each_within(at.as_vec2(), radius, |e, _| {
        if e.side.against.is_none() && e.side.faction == owner && !e.routed {
            n += 1;
        }
    });
    n
}

/// Residents of `s` who could take up arms (the villager AI's own test for
/// fighting, SIM-13): adult, alive, not starving, civilian, with morale at
/// least `fight.break_morale`.
fn would_be_militia(world: &mut World, s: Entity) -> u32 {
    let villagers = world.resource::<Villagers>();
    let starving = villagers.data.needs.starving;
    let break_morale = villagers.data.ai.fight.break_morale;
    let adult = villagers.data.households.working_age.min;
    let days_per_year = world.resource::<Calendar>().days_per_year().max(1);
    let mut q = world
        .query_filtered::<(&Villager, &CombatStats, &Needs), Without<Enlisted>>();
    q.iter(world)
        .filter(|(v, cs, needs)| {
            v.settlement == s
                && v.age_days as u32 / days_per_year >= adult
                && cs.alive()
                && cs.stance == Stance::Civilian
                && cs.morale >= break_morale
                && needs.hunger < starving
        })
        .count() as u32
}

/// Residents of `s` standing as militia against `owner`.
fn rebels_standing(world: &mut World, s: Entity, owner: FactionId) -> u32 {
    let mut q = world.query_filtered::<(&Villager, &CombatStats), Without<Enlisted>>();
    q.iter(world)
        .filter(|(v, cs)| {
            v.settlement == s && cs.alive() && cs.stance == Stance::Militia { against: owner }
        })
        .count() as u32
}

/// GP-06: once a day (the progress driver calls it every hour; it acts on
/// day ticks only), settlements in SpawnId order. For each owned one:
///
/// - **A rising under way** (`SettlementLoyalty::revolt_since`) is settled:
///   if rebels still stand, or no garrison is left, the settlement goes
///   independent ([`Cause::Revolt`]). Otherwise it was put down: the
///   provocation ends and every resident loses `garrison.crushed_loyalty`.
/// - **Unrest for a day** (`below_since` at least a day ago): with no
///   garrison the settlement goes independent at once (starving villagers
///   can't fight, so a starved, unguarded conquest is lost without a
///   blow). With a garrison smaller than the residents able to fight, they
///   rise: the settlement is provoked against its owner for
///   `garrison.revolt_hours`, and its militia fights the garrison.
///
/// Draws no randomness and does nothing in a world with no owned
/// settlements, such as the developed preset.
pub fn revolts(world: &mut World) {
    let tick = world.resource::<SimTick>().0;
    let cal = world.resource::<Calendar>().clone();
    if !cal.on_day(tick) {
        return;
    }
    let _span = tracing::info_span!("sim_military::revolts").entered();
    let garrison = world.resource::<Army>().data.garrison.clone();
    let mut q = world.query::<(Entity, &SpawnId, &Owner, &SettlementLoyalty, &Settlement)>();
    let mut owned: Vec<(SpawnId, Entity, FactionId, SettlementLoyalty, IVec2)> = q
        .iter(world)
        .filter(|(_, _, o, ..)| o.faction != FactionId::INDEPENDENT)
        .map(|(e, id, o, sl, st)| (*id, e, o.faction, *sl, st.center))
        .collect();
    owned.sort_unstable_by_key(|o| o.0);
    for (_, s, owner, sl, center) in owned {
        let guards = garrison_strength(world, center, owner);
        if sl.revolt_since.is_some() {
            if guards == 0 || rebels_standing(world, s, owner) > 0 {
                transfer(world, s, FactionId::INDEPENDENT, Cause::Revolt);
            } else {
                crush(world, s, owner, garrison.crushed_loyalty);
            }
            continue;
        }
        let Some(below) = sl.below_since else {
            continue;
        };
        if tick < below.saturating_add(cal.ticks_per_day) {
            continue;
        }
        if guards == 0 {
            transfer(world, s, FactionId::INDEPENDENT, Cause::Revolt);
            continue;
        }
        let able = would_be_militia(world, s);
        if guards < able {
            rise(world, s, owner, tick + cal.hours(garrison.revolt_hours));
        }
    }
}

/// A rising: the settlement's militia takes up arms against its owner
/// until `until`, and the revolt clock starts.
fn rise(world: &mut World, s: Entity, owner: FactionId, until: u64) {
    let tick = world.resource::<SimTick>().0;
    if let Some(mut threat) = world.get_mut::<Threat>(s) {
        threat.provoked_by = Some(owner);
        threat.provoked_until = until;
        threat.level = 1.0;
    }
    if let Some(mut sl) = world.get_mut::<SettlementLoyalty>(s) {
        sl.revolt_since = Some(tick);
    }
    let (name, who) = (settlement_name(world, s), faction_name(world, owner));
    log(world, format!("{name} rises against {who}"));
}

/// A rising put down: the militia stands down and residents (not
/// enlisted, in SpawnId order) lose `penalty` loyalty.
fn crush(world: &mut World, s: Entity, owner: FactionId, penalty: f32) {
    if let Some(mut threat) = world.get_mut::<Threat>(s)
        && threat.provoked_by == Some(owner)
    {
        threat.provoked_by = None;
        threat.provoked_until = 0;
    }
    if let Some(mut sl) = world.get_mut::<SettlementLoyalty>(s) {
        sl.revolt_since = None;
    }
    let mut q = world.query_filtered::<(Entity, &SpawnId, &Villager), (With<Loyalty>, Without<Enlisted>)>();
    let mut residents: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, v)| v.settlement == s)
        .map(|(e, id, _)| (*id, e))
        .collect();
    residents.sort_unstable();
    for (_, e) in residents {
        if let Some(mut l) = world.get_mut::<Loyalty>(e) {
            l.0 = (l.0 - penalty).clamp(0.0, 1.0);
        }
    }
    let (name, who) = (settlement_name(world, s), faction_name(world, owner));
    log(world, format!("{who} put down the rising in {name}"));
}
