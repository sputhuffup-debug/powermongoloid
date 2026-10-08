//! Answering offers and abandoning a run (GP-21, GP-27). Both are reached
//! only through `SimCommand`s (`ChooseOffer`, `AbandonRun`) applied by
//! `orders::apply`, so picks are recorded and replay.

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use sim_agents::{Deed, GoodwillLedger, Villagers};
use sim_core::{
    Cause, FactionId, OfferKind, Offers, Position, RunOutcome, RunState, SimEvent, SimEventKind,
    SpawnId,
};
use sim_economy::{Economy, SettlementEconomy, Stockpile};
use sim_world::Settlement;

use super::{Effect, Progression, RunProgress, Traits, champion_of};
use crate::components::{Equipment, Follower};
use crate::orders::log;

/// GP-27: `issuer` picks `choice` of pending offer `offer`. The offer is
/// removed, so a second pick (a double click) is refused.
///
/// Refusals: "no run" outside a run, "no such offer" (unknown, already
/// answered, or past its `expires` tick), "not your offer", "no such choice", and "not available yet"
/// for hires (M4 step 7). An invitation (GP-22) is answered by
/// [`crate::invitations::answer`]: choice 0 accepts, 1 declines.
pub fn choose(
    world: &mut World,
    issuer: FactionId,
    offer: u64,
    choice: u8,
) -> Result<(), &'static str> {
    if !world.resource::<RunState>().in_run() {
        return Err("no run");
    }
    let o = world
        .resource::<Offers>()
        .get(offer)
        .cloned()
        .ok_or("no such offer")?;
    // An offer lapses at its `expires` tick, even before the hourly step
    // that clears it (GP-22 invitations) has run this tick.
    let tick = world.resource::<sim_core::SimTick>().0;
    if o.expires.is_some_and(|t| t <= tick) {
        return Err("no such offer");
    }
    if o.faction != issuer {
        return Err("not your offer");
    }
    let card = *o.choices.get(choice as usize).ok_or("no such choice")?;
    if matches!(o.kind, OfferKind::Hire { .. }) {
        return Err("not available yet");
    }
    world.resource_mut::<Offers>().take(offer);
    let what = match o.kind {
        OfferKind::StartBoon | OfferKind::DominionBoon { .. } => apply_boon(world, issuer, card),
        OfferKind::Trait { person } => gain_trait(world, issuer, person, card),
        OfferKind::Invitation { settlement } => {
            crate::invitations::answer(world, issuer, settlement, card)
        }
        OfferKind::Hire { .. } => unreachable!("refused above"),
    };
    world.write_message(
        SimEvent::new(SimEventKind::OfferChosen, issuer)
            .subject(offer)
            .other(card as u64)
            .amount(choice as i32)
            .cause(Cause::Player),
    );
    let who = world
        .resource::<crate::Army>()
        .faction_name(issuer)
        .to_string();
    log(world, format!("{who} chose {what}"));
    Ok(())
}

/// GP-21: the player gives up; the run ends as abandoned at the end of this
/// tick. Refused outside a run ("no run"), for anyone but the player ("not
/// your run"), and once the run is over ("the run is over").
pub fn abandon(world: &mut World, issuer: FactionId) -> Result<(), &'static str> {
    let state = world.resource::<RunState>();
    if !state.in_run() {
        return Err("no run");
    }
    if issuer != FactionId::PLAYER {
        return Err("not your run");
    }
    if state.is_over() {
        return Err("the run is over");
    }
    world
        .resource_mut::<RunState>()
        .finish(RunOutcome::Abandoned, Cause::Player);
    let who = world
        .resource::<crate::Army>()
        .faction_name(issuer)
        .to_string();
    log(world, format!("{who} abandoned the run"));
    Ok(())
}

/// Applies boon `card` for `faction` and records it in
/// [`RunProgress::boons`]. Returns its name.
///
/// - `FoodDays`: grain for that many days, for the champion's whole army,
///   into the champion's baggage (its capacity grows to fit).
/// - `ArmBand`: a weapon for the champion and every follower.
/// - `Goodwill`: posted to the goodwill ledger (`sim_agents::GoodwillLedger`,
///   as [`Deed::Boon`]) for the populated settlement nearest the champion;
///   it is applied, and spreads to its neighbours, later this tick (GP-22).
/// - `Stat`: nothing now; [`super::rebuild_modifiers`] folds the picked
///   boons into `FactionModifiers` at the next game hour (a `VeteranRate`
///   boon waits there for the veterans of M4 step 6).
/// - `CaptainSlots`: nothing now; [`super::captain_slots`] counts the picked
///   boons' slots, so they count from this pick on (GP-25).
///
/// A pick refreshes [`RunProgress::captain_slots`] at once, so the UI shows
/// a slot boon without waiting for the hour.
fn apply_boon(world: &mut World, faction: FactionId, card: u16) -> String {
    let Some(boon) = world.resource::<Progression>().boon(card).cloned() else {
        return format!("boon {card}");
    };
    let champion = champion_of(world, faction);
    for effect in &boon.effects {
        match *effect {
            Effect::FoodDays(days) => {
                if let Some(c) = champion {
                    add_food_days(world, c, days);
                }
            }
            Effect::ArmBand => {
                if let Some(c) = champion {
                    arm_band(world, c);
                }
            }
            Effect::Goodwill(amount) => {
                let at = champion
                    .and_then(|c| world.get::<Position>(c).map(|p| p.0))
                    .unwrap_or(world.resource::<RunProgress>().start);
                if let Some(s) = nearest_settlement(world, at) {
                    world
                        .resource_mut::<GoodwillLedger>()
                        .push(s, faction, amount, Deed::Boon);
                }
            }
            Effect::Stat(..)
            | Effect::CaptainSlots(_)
            | Effect::Stores(_)
            | Effect::RaiderPace(_)
            | Effect::RivalStrength(_)
            | Effect::Sellswords(_) => {}
        }
    }
    world.resource_mut::<RunProgress>().boons.push(card);
    super::dominion::refresh_slots(world);
    boon.name
}

/// SIM-34: `person` (a SpawnId) gains trait `id`. Returns its name.
fn gain_trait(world: &mut World, faction: FactionId, person: u64, id: u16) -> String {
    let name = world
        .resource::<Progression>()
        .trait_data(id)
        .map_or_else(|| format!("trait {id}"), |t| t.name.clone());
    let mut q = world.query::<(Entity, &SpawnId)>();
    let Some(e) = q.iter(world).find(|(_, s)| s.0 == person).map(|(e, _)| e) else {
        return name;
    };
    match world.get_mut::<Traits>(e) {
        Some(mut t) => t.0.push(id),
        None => {
            world.entity_mut(e).insert(Traits(vec![id]));
        }
    }
    world.write_message(
        SimEvent::new(SimEventKind::TraitGained, faction)
            .subject(person)
            .amount(id as i32)
            .cause(Cause::Player),
    );
    name
}

/// The populated settlement whose centre is nearest `at` (ties: lowest
/// SpawnId).
fn nearest_settlement(world: &mut World, at: IVec2) -> Option<u64> {
    let mut q = world.query_filtered::<(&SpawnId, &Settlement), With<SettlementEconomy>>();
    q.iter(world)
        .map(|(id, s)| ((s.center - at).length_squared(), id.0))
        .min()
        .map(|(_, id)| id)
}

/// Members of `captain`'s army, captain first, then followers by SpawnId.
fn army_members(world: &mut World, captain: Entity) -> Vec<Entity> {
    let mut q = world.query::<(Entity, &SpawnId, &Follower)>();
    let mut followers: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, f)| f.captain == captain)
        .map(|(e, id, _)| (*id, e))
        .collect();
    followers.sort_unstable();
    std::iter::once(captain)
        .chain(followers.into_iter().map(|(_, e)| e))
        .collect()
}

fn add_food_days(world: &mut World, captain: Entity, days: f32) {
    let people = army_members(world, captain).len() as f32;
    let need = world.resource::<Villagers>().data.needs.hunger_per_day * people * days;
    let econ = world.resource::<Economy>();
    let Some(&grain) = econ.foods_by_value().last() else {
        return;
    };
    let qty = (need / econ.food_value(grain).max(0.01)).ceil() as u32;
    if let Some(mut bag) = world.get_mut::<Stockpile>(captain) {
        bag.capacity = bag.capacity.max(bag.total() + qty);
        bag.add(grain, qty);
    }
}

fn arm_band(world: &mut World, captain: Entity) {
    for e in army_members(world, captain) {
        if let Some(mut eq) = world.get_mut::<Equipment>(e)
            && !eq.weapon
        {
            eq.weapon = true;
        }
    }
}
