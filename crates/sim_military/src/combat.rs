//! Combat (SIM-32): blows are resolved serially in SpawnId order, like the
//! economy's transfers, then the fallen are removed. Damage is
//! `(attack + weapon) x leadership x roll - (defense + tools)`, at least 1.

use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use rand::RngExt;
use sim_agents::{CombatStats, Employment, Enlisted, Stance, Strike, Villager};
use sim_core::rng::rng_key;
use sim_core::{Allegiance, FactionId, Motion, Position, SimSeed, SimTick, SpawnId};
use sim_economy::{Home, SettlementEconomy, Stockpile, Workplace};
use sim_world::world_pos;

use crate::components::{ArmyStats, Captain, Equipment, Follower, MilitaryLog};
use crate::data::Army;
use crate::index::FighterGrid;

/// Who fell this tick and where, for morale (cleared every tick).
#[derive(Resource, Debug, Default, Clone)]
pub struct Casualties {
    /// (position, allegiance, militia). A rebel (militia risen against its
    /// own allegiance, GP-06) is recorded as independent, so its fall
    /// shakes none of the soldiers it fought.
    pub fallen: Vec<(Vec2, FactionId, bool)>,
    /// The dead awaiting removal, in SpawnId order.
    pub pending: Vec<Entity>,
}

/// Whether anyone fell this tick (run condition for [`casualties`]).
pub(crate) fn anyone_fell(c: Res<Casualties>) -> bool {
    !c.pending.is_empty()
}

/// Act, after blows: lists fighters whose health ran out (in combat or
/// starving). Only fighters can die, so only they are checked.
pub(crate) fn find_fallen(
    grid: Res<FighterGrid>,
    mut casualties: ResMut<Casualties>,
    q: Query<&CombatStats>,
) {
    casualties.fallen.clear();
    casualties.pending.clear();
    let mut dead: Vec<(SpawnId, Entity)> = grid
        .fighters
        .iter()
        .copied()
        .filter(|(_, e)| q.get(*e).is_ok_and(|cs| !cs.alive()))
        .collect();
    dead.sort_unstable();
    casualties.pending.extend(dead.into_iter().map(|(_, e)| e));
}

#[allow(clippy::type_complexity)]
pub(crate) fn resolve_strikes(
    tick: Res<SimTick>,
    seed: Res<SimSeed>,
    army: Res<Army>,
    grid: Res<FighterGrid>,
    mut stats: ResMut<ArmyStats>,
    mut q: Query<(
        Entity,
        &SpawnId,
        &Allegiance,
        &mut CombatStats,
        &mut Strike,
        Option<&Equipment>,
    )>,
) {
    let _span = tracing::info_span!("sim_military::resolve_strikes").entered();
    // Only fighters swing; blows land in SpawnId order.
    let mut order: Vec<(SpawnId, Entity)> = grid
        .fighters
        .iter()
        .copied()
        .filter(|(_, e)| q.get(*e).is_ok_and(|(.., s, _)| s.swing))
        .collect();
    if order.is_empty() {
        return;
    }
    order.sort_unstable();
    let eq = &army.data.equipment;
    let c = &army.data.combat;
    for (id, e) in order {
        let Ok((_, _, allegiance, cs, mut strike, equipment)) = q.get_mut(e) else {
            continue;
        };
        strike.swing = false;
        let Some(target) = strike.target else {
            continue;
        };
        let attack = cs.attack
            + equipment
                .filter(|x| x.weapon)
                .map_or(0.0, |_| eq.weapon_attack);
        let bonus = strike.bonus.max(1.0);
        let faction = allegiance.0;
        let Ok((_, _, _, mut victim, _, armour)) = q.get_mut(target) else {
            continue;
        };
        if !victim.alive() {
            continue;
        }
        let mut rng = seed.stream(rng_key("sim_military::strike") ^ id.0, tick.0);
        let roll = rng.random_range(c.roll_min..=c.roll_max);
        let defense = victim.defense + armour.filter(|x| x.tools).map_or(0.0, |_| eq.tools_defense);
        let damage = (attack * bonus * roll - defense).max(1.0);
        victim.health -= damage;
        victim.morale = (victim.morale - 0.01).max(0.0);
        if !victim.alive() {
            stats.of(faction).kills += 1;
            if let Ok((_, _, _, mut killer, ..)) = q.get_mut(e) {
                killer.morale = (killer.morale + c.kill_morale).min(1.0);
            }
        }
    }
}

/// Removes everyone whose health ran out (combat or starvation), in
/// SpawnId order. A fallen captain's command passes to its first follower.
pub(crate) fn casualties(world: &mut World) {
    let _span = tracing::info_span!("sim_military::casualties").entered();
    let dead = std::mem::take(&mut world.resource_mut::<Casualties>().pending);
    let tick = world.resource::<SimTick>().0;
    let loss = world.resource::<Army>().data.leadership.captain_loss_morale;
    for e in dead {
        let p = world_pos(
            world.get::<Position>(e).expect("position"),
            world.get::<Motion>(e),
        );
        let faction = world.get::<Allegiance>(e).map_or(FactionId(0), |a| a.0);
        let stance = world.get::<CombatStats>(e).map(|c| c.stance);
        let militia = matches!(stance, Some(Stance::Militia { .. }));
        // A rebel fights its own settlement's owner (GP-06): its fall is no
        // loss to the owner's soldiers, so it shakes no one's morale.
        let side = match stance {
            Some(Stance::Militia { against }) if against == faction => FactionId::INDEPENDENT,
            _ => faction,
        };
        world
            .resource_mut::<Casualties>()
            .fallen
            .push((p, side, militia));
        if let Some(Stance::Militia { against }) = stance {
            world
                .resource_mut::<ArmyStats>()
                .of(against)
                .villagers_killed += 1;
        }
        let enlisted = world.get::<Enlisted>(e).is_some();
        if enlisted {
            world.resource_mut::<ArmyStats>().of(faction).lost += 1;
        }
        if let Some(captain) = world.get::<Captain>(e).cloned() {
            succeed(world, e, captain, loss, tick);
        }
        if let Some(v) = world.get::<Villager>(e).copied() {
            if let Some(mut home) = world.get_mut::<Home>(v.home) {
                home.residents.retain(|r| *r != e);
            }
            if let Some(wp) = world.get::<Employment>(e).and_then(|j| j.workplace)
                && let Some(mut w) = world.get_mut::<Workplace>(wp)
            {
                w.workers.retain(|r| *r != e);
            }
            if !enlisted && let Some(mut se) = world.get_mut::<SettlementEconomy>(v.settlement) {
                se.population = se.population.saturating_sub(1);
            }
        }
        world.despawn(e);
    }
}

/// Hands a fallen captain's orders and baggage to its first follower.
fn succeed(world: &mut World, fallen: Entity, captain: Captain, loss: f32, tick: u64) {
    let mut q = world.query::<(Entity, &SpawnId, &Follower, &CombatStats)>();
    let mut members: Vec<(SpawnId, Entity, bool)> = q
        .iter(world)
        .filter(|(_, _, f, _)| f.captain == fallen)
        .map(|(e, id, _, cs)| (*id, e, cs.alive()))
        .collect();
    members.sort_unstable_by_key(|m| m.0);
    for (_, e, _) in &members {
        if let Some(mut cs) = world.get_mut::<CombatStats>(*e) {
            cs.morale = (cs.morale - loss).max(0.0);
        }
    }
    let who = world
        .resource::<Army>()
        .faction_name(captain.faction)
        .to_string();
    let Some(&(heir_id, heir, _)) = members.iter().find(|m| m.2) else {
        world
            .resource_mut::<MilitaryLog>()
            .push(tick, format!("{who}: a captain fell with no one to follow"));
        return;
    };
    for (_, e, _) in &members {
        if *e != heir
            && let Some(mut f) = world.get_mut::<Follower>(*e)
        {
            f.captain = heir;
        }
    }
    let capacity = world
        .resource::<Army>()
        .baggage_capacity(members.len() as u32);
    let baggage = world
        .get::<Stockpile>(fallen)
        .cloned()
        .unwrap_or_else(|| Stockpile::with_capacity(capacity));
    world
        .entity_mut(heir)
        .remove::<Follower>()
        .insert((captain, baggage));
    world.resource_mut::<MilitaryLog>().push(
        tick,
        format!("{who}: the captain fell; {} takes command", heir_id.0),
    );
}
