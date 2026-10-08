//! Upkeep and morale (SIM-31, SIM-32): followers eat what they carry, then
//! from the baggage; unfed followers lose morale, then desert, then starve.
//! Captains near their men steady them.

use bevy_ecs::prelude::*;
use rand::RngExt;
use sim_agents::{
    ActionKind, Activity, CombatStats, Enlisted, Loyalty, Needs, Owner, SettlementLoyalty, Stage,
    Stance, Travel, TravelState, Villager, Villagers, civilian_stats, eat_from,
};
use sim_core::rng::rng_key;
use sim_core::{
    Allegiance, FactionId, FactionModifiers, Motion, Position, SimSeed, SimTick, SpawnId, Stat,
};
use sim_economy::{Economy, Inventory, SettlementEconomy, TransferOp, Transfers};
use sim_world::world_pos;

use crate::combat::Casualties;
use crate::components::{ArmyStats, Captain, Equipment, Follower, MilitaryLog};
use crate::data::Army;
use crate::index::ArmyIndex;
use crate::monuments::NoUpkeep;
use crate::soldiers::led;

/// EconomySet::Post: hungry soldiers eat carried food, else ask the
/// baggage for a meal (applied by the transfer resolver).
#[allow(clippy::type_complexity)]
pub(crate) fn eat(
    econ: Res<Economy>,
    villagers: Res<Villagers>,
    army: Res<Army>,
    index: Res<ArmyIndex>,
    mut q: Query<
        (
            Entity,
            &Position,
            &Motion,
            &mut Needs,
            &mut Inventory,
            &mut Transfers,
            Option<&Follower>,
            Has<Captain>,
        ),
        (With<Enlisted>, Without<NoUpkeep>),
    >,
) {
    let _span = tracing::info_span!("sim_military::eat").entered();
    let n = &villagers.data.needs;
    q.par_iter_mut().for_each(
        |(e, pos, motion, mut needs, mut inv, mut transfers, follower, is_captain)| {
            if needs.hunger < n.hungry {
                return;
            }
            if eat_from(&mut inv, &mut needs, &econ, army.data.upkeep.sated) {
                return;
            }
            let p = world_pos(pos, Some(motion));
            let source = if is_captain {
                index.get(e)
            } else {
                follower.and_then(|f| index.get(f.captain))
            };
            // The baggage reaches the whole formation.
            if let Some(a) = source
                && a.food > 0.0
                && a.pos.distance(p)
                    <= army
                        .data
                        .upkeep
                        .forage_radius
                        .max(army.formation_radius(a.followers) + army.data.formation.catch_up)
            {
                let units = (((needs.hunger - army.data.upkeep.sated) / 0.3).ceil() as u16).max(1);
                transfers.post(a.entity, vec![TransferOp::TakeMeal { max: units }]);
            }
        },
    );
}

/// Needs set: hunger, starvation, morale and the desertion roll for
/// soldiers. Parallel; each writes only its own components. Monument
/// guards ([`NoUpkeep`]) never get hungrier. The morale target is the
/// unit's base morale times the faction's `Stat::Morale` multiplier
/// (GP-23; exactly the base with no modifiers).
#[allow(clippy::type_complexity, clippy::too_many_arguments)] // Bevy system parameters.
pub(crate) fn soldier_upkeep(
    tick: Res<SimTick>,
    seed: Res<SimSeed>,
    villagers: Res<Villagers>,
    army: Res<Army>,
    index: Res<ArmyIndex>,
    fallen: Res<Casualties>,
    mods: Res<FactionModifiers>,
    mut q: Query<
        (
            &SpawnId,
            &Position,
            &Motion,
            &Allegiance,
            &Activity,
            &mut CombatStats,
            &mut Needs,
            Option<&mut Follower>,
            Has<Captain>,
            Has<NoUpkeep>,
        ),
        With<Enlisted>,
    >,
) {
    let _span = tracing::info_span!("sim_military::soldier_upkeep").entered();
    let tick = tick.0;
    let u = &army.data.upkeep;
    let c = &army.data.combat;
    let l = &army.data.leadership;
    let unfed_loss = army.per_tick(u.unfed_morale_per_day);
    let fed_gain = army.per_tick(u.fed_morale_per_day);
    let lead_gain = army.per_tick(l.morale_per_day);
    let starve = army.per_tick(u.starve_damage_per_day);
    let desert = army.per_tick(u.desert_chance_per_day);
    let hunger = villagers.hunger_per_tick;
    q.par_iter_mut().for_each(
        |(id, pos, motion, allegiance, act, mut cs, mut needs, follower, is_captain, no_upkeep)| {
            if !cs.alive() {
                return;
            }
            if !no_upkeep {
                needs.hunger = (needs.hunger + hunger).min(1.0);
                if needs.hunger >= u.starving {
                    cs.health -= starve;
                }
            }
            let p = world_pos(pos, Some(motion));
            let unfed = needs.hunger >= u.hungry;
            let mut morale = cs.morale;
            if unfed {
                morale -= unfed_loss;
            } else {
                let is_led = led(&index, &army, follower.as_deref(), p);
                let base = if is_captain {
                    army.data.captain.morale
                } else {
                    army.data.soldier.morale
                } * mods.mult(allegiance.0, Stat::Morale);
                let target = if is_led { (base + 0.1).min(1.0) } else { base };
                let rate = fed_gain + if is_led { lead_gain } else { 0.0 };
                morale = if morale < target {
                    (morale + rate).min(target)
                } else {
                    (morale - fed_gain).max(target)
                };
            }
            let mut shock = 0.0;
            for (q, f, _) in &fallen.fallen {
                if *f == allegiance.0 && q.distance(p) <= c.casualty_radius {
                    shock += c.casualty_morale;
                }
            }
            morale -= shock.min(c.casualty_morale_cap);
            morale = morale.clamp(0.0, 1.0);
            if morale != cs.morale {
                cs.morale = morale;
            }
            // A follower who starved to death this tick doesn't desert:
            // `find_fallen` removes him next tick (G4).
            if let Some(mut f) = follower
                && !f.deserting
                && cs.alive()
                && morale < u.desert_below
                && (unfed || act.kind == ActionKind::Rout)
            {
                let mut rng = seed.stream(rng_key("sim_military::desert") ^ id.0, tick);
                if rng.random::<f32>() < desert {
                    f.deserting = true;
                }
            }
        },
    );
}

/// Needs set: civilians' and militia's morale (casualties nearby hurt it;
/// it recovers toward the villagers' base when calm). Recovery is applied
/// every 10 ticks; shocks on the tick they happen.
pub(crate) fn civilian_morale(
    tick: Res<SimTick>,
    villagers: Res<Villagers>,
    army: Res<Army>,
    fallen: Res<Casualties>,
    mut q: Query<(&Position, &Motion, &mut CombatStats), Without<Enlisted>>,
) {
    let _span = tracing::info_span!("sim_military::civilian_morale").entered();
    let recover = tick.0.is_multiple_of(10);
    if fallen.fallen.is_empty() && !recover {
        return;
    }
    let base = villagers.data.combat.morale;
    let gain = if recover {
        army.per_tick(army.data.upkeep.fed_morale_per_day) * 10.0
    } else {
        0.0
    };
    let c = &army.data.combat;
    q.par_iter_mut().for_each(|(pos, motion, mut cs)| {
        if !cs.alive() {
            return;
        }
        let mut morale = cs.morale;
        if !fallen.fallen.is_empty() {
            let p = world_pos(pos, Some(motion));
            let mut shock = 0.0;
            for (q, _, militia) in &fallen.fallen {
                if *militia && q.distance(p) <= c.casualty_radius {
                    shock += c.casualty_morale;
                }
            }
            morale -= shock.min(c.casualty_morale_cap);
        }
        if morale < base {
            morale = (morale + gain).min(base);
        }
        morale = morale.clamp(0.0, 1.0);
        if morale != cs.morale {
            cs.morale = morale;
        }
    });
}

/// Followers who decided to desert this tick, in SpawnId order.
#[derive(Resource, Debug, Default)]
pub struct Deserters(pub Vec<(SpawnId, Entity, FactionId)>);

pub(crate) fn anyone_deserting(d: Res<Deserters>) -> bool {
    !d.0.is_empty()
}

#[allow(clippy::type_complexity)]
/// Needs set: tallies starvation (whole counts per faction, so the sum is
/// exact) and lists deserters.
pub(crate) fn tally(
    tick: Res<SimTick>,
    army: Res<Army>,
    mut stats: ResMut<ArmyStats>,
    mut deserters: ResMut<Deserters>,
    q: Query<(Entity, &SpawnId, &Allegiance, &Needs, Option<&Follower>), With<Enlisted>>,
) {
    let starving = army.data.upkeep.starving;
    let starve = army.per_tick(army.data.upkeep.starve_damage_per_day);
    let mut starving_by: Vec<(FactionId, u32)> = Vec::new();
    deserters.0.clear();
    for (e, id, a, n, f) in &q {
        if n.hunger >= starving {
            match starving_by.iter_mut().find(|(f, _)| *f == a.0) {
                Some((_, k)) => *k += 1,
                None => starving_by.push((a.0, 1)),
            }
        }
        if f.is_some_and(|f| f.deserting) {
            deserters.0.push((*id, e, a.0));
        }
    }
    starving_by.sort_unstable();
    for (f, k) in starving_by {
        let s = stats.of(f);
        s.starvation_damage += k as f32 * starve;
        s.first_starvation.get_or_insert(tick.0);
    }
    deserters.0.sort_unstable();
}

/// Needs set, last: deserters leave, in SpawnId order. A villager goes
/// home and is a villager again, sworn to whoever owns its settlement
/// (GP-05), with the settlement's current mean loyalty (GP-06); anyone
/// else simply leaves.
pub(crate) fn apply_desertions(world: &mut World) {
    let _span = tracing::info_span!("sim_military::apply_desertions").entered();
    let tick = world.resource::<SimTick>().0;
    let leaving = std::mem::take(&mut world.resource_mut::<Deserters>().0);
    let villagers = world.resource::<Villagers>().clone();
    let mut by_faction: Vec<(FactionId, u32)> = Vec::new();
    for (_, e, faction) in leaving {
        {
            let mut stats = world.resource_mut::<ArmyStats>();
            let s = stats.of(faction);
            s.deserted += 1;
            s.first_desertion.get_or_insert(tick);
        }
        match by_faction.iter_mut().find(|(f, _)| *f == faction) {
            Some((_, k)) => *k += 1,
            None => by_faction.push((faction, 1)),
        }
        let Some(v) = world.get::<Villager>(e).copied() else {
            world.despawn(e);
            continue;
        };
        // Back to village life, under the settlement's owner, as loyal as
        // its residents are now: the loyalty frozen at enlistment may have
        // been toward an owner who has since lost the settlement (GP-06).
        let owner = world
            .get::<Owner>(v.settlement)
            .map_or(FactionId::INDEPENDENT, |o| o.faction);
        let loyalty = match world.get::<SettlementLoyalty>(v.settlement) {
            Some(sl) if sl.residents > 0 => sl.mean,
            _ => sim_agents::standing::default_seed(world, v.settlement, owner),
        }
        .clamp(0.0, 1.0);
        let mut civ = civilian_stats(&villagers);
        if let Some(cs) = world.get::<CombatStats>(e) {
            civ.health = (cs.health / cs.max_health.max(1.0)) * civ.max_health;
            civ.morale = cs.morale;
        }
        world
            .entity_mut(e)
            .remove::<(Enlisted, Follower, Equipment)>()
            .insert((
                civ,
                Allegiance(owner),
                Loyalty(loyalty),
                Activity {
                    kind: ActionKind::GoHome,
                    stage: Stage::Done,
                    ..Default::default()
                },
            ));
        if let Some(mut t) = world.get_mut::<Travel>(e) {
            t.state = TravelState::Idle;
        }
        if let Some(mut se) = world.get_mut::<SettlementEconomy>(v.settlement) {
            se.population += 1;
        }
        debug_assert!(matches!(civ.stance, Stance::Civilian));
    }
    by_faction.sort_unstable();
    for (f, k) in by_faction {
        let who = world.resource::<Army>().faction_name(f).to_string();
        world
            .resource_mut::<MilitaryLog>()
            .push(tick, format!("{who}: {k} deserted"));
    }
}
