//! Moving villagers between trades (GP-34, M4a step 4; LORDSHIP_DESIGN
//! section 8.1). The Work edict's waves call [`reassign`] from
//! sim_military's serial `govern::step`; nothing else changes a working
//! villager's trade after populate (enlisting only drops it).
//!
//! A move follows populate's own rule: the villager leaves its workplace's
//! `workers`, joins the nearest workplace of the new kind with room (by
//! squared distance from its home, then position in the settlement's
//! list), takes the new `Occupation` and `Employment`, and gets a cart if
//! it became a carter (or loses it if it stopped being one). Only an
//! empty-handed villager who isn't in the middle of a work cycle moves, so
//! no goods or inputs are lost. Serial steps only.

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use sim_core::Position;
use sim_economy::{Economy, Inventory, Occupation, SettlementEconomy, Workplace, WorkplaceKind};

use crate::components::{ActionKind, Activity, Employment, Stage, Villager};

/// The trade that works at workplace `kind` (`Occupation::workplace`'s
/// inverse; the storehouse is the carters').
pub fn occupation_at(kind: WorkplaceKind) -> Option<Occupation> {
    Occupation::ALL
        .into_iter()
        .find(|o| o.workplace() == Some(kind))
}

/// Whether villager `v` can move now: empty hands, and no work cycle under
/// way (its inputs already used).
pub fn movable(world: &World, v: Entity) -> bool {
    let empty = world.get::<Inventory>(v).is_some_and(|i| i.is_empty());
    let mid_cycle = world
        .get::<Activity>(v)
        .is_some_and(|a| a.kind == ActionKind::Work && a.stage == Stage::Perform && a.step == 2);
    empty && !mid_cycle
}

/// The nearest workplace of one of `kinds` in villager `v`'s settlement with
/// room for another worker (the storehouse always has room), by squared
/// distance from its home, then the settlement's workplace order. Its own
/// workplace counts only if it is of one of `kinds` (it has room for him).
pub fn nearest_with_room(world: &World, v: Entity, kinds: &[WorkplaceKind]) -> Option<Entity> {
    let villager = world.get::<Villager>(v)?;
    let se = world.get::<SettlementEconomy>(villager.settlement)?;
    let econ = world.resource::<Economy>();
    let home = world
        .get::<Position>(villager.home)
        .map_or(IVec2::ZERO, |p| p.0);
    let current = world.get::<Employment>(v).and_then(|e| e.workplace);
    se.workplaces
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(i, e)| {
            let wp = world.get::<Workplace>(e)?;
            if !kinds.contains(&wp.kind) {
                return None;
            }
            let room = wp.kind == WorkplaceKind::Storehouse
                || Some(e) == current
                || (wp.workers.len() as u32) < econ.workplace(wp.kind).slots;
            let tile = world.get::<Position>(e).map_or(IVec2::ZERO, |p| p.0);
            room.then_some(((tile - home).length_squared(), i, e))
        })
        .min_by_key(|(d, i, _)| (*d, *i))
        .map(|(.., e)| e)
}

/// GP-34: moves villager `v` to trade `to` (populate's rule: the nearest
/// workplace of its kind with room). `Occupation::Idle` leaves its
/// workplace. Returns false, changing nothing, if it isn't [`movable`],
/// already has that trade, or no workplace of the kind has room.
pub fn reassign(world: &mut World, v: Entity, to: Occupation) -> bool {
    match to.workplace() {
        None => {
            if world.get::<Occupation>(v) == Some(&to) || !movable(world, v) {
                return false;
            }
            take_job(world, v, to, None);
            true
        }
        Some(kind) => match nearest_with_room(world, v, &[kind]) {
            Some(wp) => reassign_to(world, v, wp),
            None => false,
        },
    }
}

/// GP-34: moves villager `v` to workplace `wp`, taking the trade that works
/// there. Returns false, changing nothing, if it isn't [`movable`], `wp`
/// isn't a workplace, or `v` already works there.
pub fn reassign_to(world: &mut World, v: Entity, wp: Entity) -> bool {
    let Some(kind) = world.get::<Workplace>(wp).map(|w| w.kind) else {
        return false;
    };
    let Some(to) = occupation_at(kind) else {
        return false;
    };
    let already = world.get::<Employment>(v).and_then(|e| e.workplace) == Some(wp)
        && world.get::<Occupation>(v) == Some(&to);
    if already || !movable(world, v) {
        return false;
    }
    take_job(world, v, to, Some(wp));
    true
}

fn take_job(world: &mut World, v: Entity, to: Occupation, wp: Option<Entity>) {
    let old = world.get::<Employment>(v).and_then(|e| e.workplace);
    if let Some(old) = old
        && let Some(mut w) = world.get_mut::<Workplace>(old)
    {
        w.workers.retain(|x| *x != v);
    }
    if let Some(wp) = wp
        && let Some(mut w) = world.get_mut::<Workplace>(wp)
    {
        w.workers.push(v);
    }
    let was = world.get::<Occupation>(v).copied();
    if let Some(mut occ) = world.get_mut::<Occupation>(v) {
        *occ = to;
    }
    if let Some(mut job) = world.get_mut::<Employment>(v) {
        job.workplace = wp;
    }
    // A cart for a carter, hands for everyone else (the hands are empty).
    if (was == Some(Occupation::Carter)) != (to == Occupation::Carter) {
        let econ = world.resource::<Economy>();
        let carry = if to == Occupation::Carter {
            econ.data.cart_carry
        } else {
            econ.data.carry
        };
        if let Some(mut inv) = world.get_mut::<Inventory>(v) {
            *inv = Inventory::with_capacity(carry);
        }
    }
    // Work or a haul for the old place stops; the AI decides again.
    if let Some(mut act) = world.get_mut::<Activity>(v)
        && matches!(act.kind, ActionKind::Work | ActionKind::Haul)
        && act.stage != Stage::Done
    {
        act.stage = Stage::Done;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GP-34: every working trade has its workplace, and back.
    #[test]
    fn every_trade_has_its_workplace() {
        for occ in Occupation::ALL {
            match occ.workplace() {
                Some(kind) => assert_eq!(occupation_at(kind), Some(occ)),
                None => assert_eq!(occ, Occupation::Idle),
            }
        }
    }
}
