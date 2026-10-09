//! The champion's ranks, trait picks and succession (SIM-34, SIM-35).
//!
//! - **Points.** The champion (the person with [`Champion`], never a field
//!   of `Captain`) carries a [`ChampionRank`]. Deeds earn it progression.ron
//!   `champion.points` through [`credit`], where each deed happens: a
//!   battle its army lived through (`veterans::end_battles`), a monument
//!   taken (`monuments::hold`), a settlement conquered or invited into
//!   (`conquest::transfer_seeded`), a request answered
//!   (`requests::complete`). Only in a run.
//! - **Ranks.** [`rank_ups`] (hourly, slot 9 of the progress driver, run
//!   only) raises the rank while the points reach the next of
//!   `champion.ranks`. Each rank reports `RankUp` and posts an
//!   `Offer { Trait { person } }` of three distinct traits from
//!   `champion_traits`, preferring traits the champion doesn't hold, drawn
//!   from `sim_military::progression::traits ^ person` at the posting tick
//!   ([`draw_traits`]). The pick (`ChooseOffer`) is pushed to [`Traits`]
//!   in `offers::choose`, and its effects reach `FactionModifiers` at the
//!   next hourly rebuild.
//! - **Succession.** [`fallen`], called from `combat::casualties` before
//!   the fallen champion's army passes to an heir: the faction's other
//!   living captains, best `ChampionRank`, then best veteran rank, then
//!   lowest SpawnId, are the candidates. The first, if the run's one
//!   succession is unused, becomes champion with the rank, the points and
//!   the first half (rounded down, pick order) of the traits; pending trait
//!   offers follow it (`Succession`). Otherwise the run is lost
//!   (`RunState::finish(Lost, Cause::Death)`). Either way `ChampionFell`.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use sim_agents::{CombatStats, Needs};
use sim_core::rng::rng_key;
use sim_core::{
    Cause, FactionId, OfferKind, Offers, Position, RunOutcome, RunState, SimEvent, SimEventKind,
    SimSeed, SimTick, SpawnId, StateHash, StateHasher, post_offer,
};

use super::start::draw_cards;
use super::{Champion, Progression, RunProgress, Traits};
use crate::combat::Casualties;
use crate::components::Captain;
use crate::data::Army;
use crate::orders::{faction_name, log};
use crate::veterans::Veteran;

/// Rank points per deed (`champion.points` in progression.ron, SIM-35).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChampionPoints {
    /// A battle the champion's army lived through.
    pub battle: u32,
    /// A monument taken by the champion's faction.
    pub monument: u32,
    /// A settlement conquered or invited into.
    pub settlement: u32,
    /// A village request answered.
    pub request: u32,
}

/// The champion's ranks (`champion` in progression.ron, SIM-35).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChampionData {
    /// Points per deed.
    pub points: ChampionPoints,
    /// Total points for rank 1, 2, ... (strictly ascending, above 0).
    pub ranks: Vec<u32>,
}

impl ChampionData {
    /// Points `feat` earns.
    pub fn points_for(&self, feat: Feat) -> u32 {
        match feat {
            Feat::Battle => self.points.battle,
            Feat::Monument => self.points.monument,
            Feat::Settlement => self.points.settlement,
            Feat::Request => self.points.request,
        }
    }

    /// The rank `points` reach.
    pub fn rank_for(&self, points: u32) -> u8 {
        self.ranks.iter().take_while(|&&r| points >= r).count() as u8
    }

    /// Checks the ranks hang together.
    pub fn validate(&self) -> Result<(), String> {
        if self.ranks.first() == Some(&0)
            || self.ranks.windows(2).any(|w| w[0] >= w[1])
            || self.ranks.len() > u8::MAX as usize
        {
            return Err("progression: champion.ranks must be strictly ascending from 1".into());
        }
        Ok(())
    }
}

/// A deed that earns the champion rank points (SIM-35).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feat {
    /// Lived through a battle with its army.
    Battle,
    /// Took a monument.
    Monument,
    /// Conquered a settlement or was invited in.
    Settlement,
    /// Answered a village's request.
    Request,
}

/// The champion's rank and points (SIM-35). On the champion only; it moves
/// with the title on succession. Hashed.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChampionRank {
    /// Ranks reached (each brought an offer of traits).
    pub rank: u8,
    /// Points earned by deeds.
    pub points: u32,
}

impl StateHash for ChampionRank {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u8(self.rank);
        h.u32(self.points);
    }
}

/// SIM-35: `faction`'s champion earns the points of `feat` (nothing outside
/// a run, or for a faction with no champion). The rank follows at the next
/// [`rank_ups`].
pub fn credit(world: &mut World, faction: FactionId, feat: Feat) {
    if faction == FactionId::INDEPENDENT || !world.resource::<RunState>().in_run() {
        return;
    }
    let Some(champion) = super::champion_of(world, faction) else {
        return;
    };
    let points = world
        .resource::<Progression>()
        .data
        .champion
        .points_for(feat);
    if let Some(mut r) = world.get_mut::<ChampionRank>(champion) {
        r.points = r.points.saturating_add(points);
    }
}

/// SIM-35, hourly (slot 9 of `progression::progress`, run only): champions
/// (SpawnId order) whose points reach a new rank rank up, once per rank,
/// each with a `RankUp` event and an offer of three traits. Span
/// `sim_military::champion::rank_ups`.
pub fn rank_ups(world: &mut World) {
    let _span = tracing::info_span!("sim_military::champion::rank_ups").entered();
    if !world.resource::<RunState>().in_run() {
        return;
    }
    let data = world.resource::<Progression>().clone();
    let tick = world.resource::<SimTick>().0;
    let seed = *world.resource::<SimSeed>();
    let mut q = world.query_filtered::<(Entity, &SpawnId, &ChampionRank), With<Champion>>();
    let mut due: Vec<(SpawnId, Entity, u8)> = q
        .iter(world)
        .filter_map(|(e, id, r)| {
            let to = data.data.champion.rank_for(r.points);
            (to > r.rank).then_some((*id, e, to))
        })
        .collect();
    due.sort_unstable();
    for (id, e, to) in due {
        let faction = world
            .get::<sim_core::Allegiance>(e)
            .map_or(FactionId::INDEPENDENT, |a| a.0);
        let points = world.get::<ChampionRank>(e).map_or(0, |r| r.points);
        let from = world.get::<ChampionRank>(e).map_or(0, |r| r.rank);
        let at = world.get::<Position>(e).map_or_else(Default::default, |p| p.0);
        // One stream per champion and tick; several ranks reached in the
        // same hour draw their offers from it in turn.
        let mut rng = seed.stream(rng_key("sim_military::progression::traits") ^ id.0, tick);
        for rank in from + 1..=to {
            world.get_mut::<ChampionRank>(e).expect("ranked").rank = rank;
            world.write_message(
                SimEvent::new(SimEventKind::RankUp, faction)
                    .subject(id.0)
                    .other(points as u64)
                    .at(at)
                    .amount(rank as i32),
            );
            let held = world
                .get::<Traits>(e)
                .map(|t| t.0.clone())
                .unwrap_or_default();
            let choices = draw_traits(&mut rng, &data.data.champion_traits, &held);
            if !choices.is_empty() {
                post_offer(
                    world,
                    faction,
                    OfferKind::Trait { person: id.0 },
                    choices,
                    None,
                );
            }
            let who = faction_name(world, faction);
            log(world, format!("{who}: the champion reached rank {rank}"));
        }
    }
}

/// Up to three distinct traits from `pool` (trait ids), drawn evenly
/// without repeats: first among those not in `held`, then, if fewer than
/// three remain, topped up from the held ones (a pick stacks).
pub fn draw_traits<R: rand::Rng + ?Sized>(rng: &mut R, pool: &[u16], held: &[u16]) -> Vec<u16> {
    let mut ids: Vec<u16> = pool.to_vec();
    ids.sort_unstable();
    ids.dedup();
    let fresh: Vec<(u16, f32, Option<&str>)> = ids
        .iter()
        .filter(|t| !held.contains(t))
        .map(|&t| (t, 1.0, None))
        .collect();
    let mut out = draw_cards(rng, &fresh, 3);
    if out.len() < 3 {
        let old: Vec<(u16, f32, Option<&str>)> = ids
            .iter()
            .filter(|t| held.contains(t))
            .map(|&t| (t, 1.0, None))
            .collect();
        out.extend(draw_cards(rng, &old, 3 - out.len()));
    }
    out
}

/// SIM-35: the champion `dead` (of `faction`) has fallen; called from
/// `combat::casualties` before its army passes to an heir. See the module
/// doc. Outside a run nothing happens.
pub fn fallen(world: &mut World, dead: Entity, faction: FactionId) {
    let _span = tracing::info_span!("sim_military::champion::fallen").entered();
    if !world.resource::<RunState>().in_run() {
        return;
    }
    let dead_id = world.get::<SpawnId>(dead).map_or(0, |s| s.0);
    let at = world
        .get::<Position>(dead)
        .map_or_else(Default::default, |p| p.0);
    let cause = cause_of_death(world, dead);
    world.write_message(
        SimEvent::new(SimEventKind::ChampionFell, faction)
            .subject(dead_id)
            .at(at)
            .cause(cause),
    );
    let mut q = world.query::<(
        Entity,
        &SpawnId,
        &Captain,
        &CombatStats,
        Option<&ChampionRank>,
        Option<&Veteran>,
    )>();
    let mut candidates: Vec<(
        std::cmp::Reverse<u8>,
        std::cmp::Reverse<u8>,
        SpawnId,
        Entity,
    )> = q
        .iter(world)
        .filter(|(e, _, c, cs, ..)| *e != dead && c.faction == faction && cs.alive())
        .map(|(e, id, _, _, r, v)| {
            (
                std::cmp::Reverse(r.map_or(0, |r| r.rank)),
                std::cmp::Reverse(v.map_or(0, |v| v.rank)),
                *id,
                e,
            )
        })
        .collect();
    candidates.sort_unstable();
    let who = faction_name(world, faction);
    let used = world.resource::<RunProgress>().succession_used;
    let heir = candidates.first().map(|c| (c.2, c.3));
    let Some((heir_id, heir)) = heir.filter(|_| !used) else {
        world
            .resource_mut::<RunState>()
            .finish(RunOutcome::Lost, cause);
        let why = if used {
            "the one succession is spent"
        } else {
            "no captain is left to follow"
        };
        log(world, format!("{who}: the champion fell, and {why}"));
        return;
    };
    let rank = world.get::<ChampionRank>(dead).copied().unwrap_or_default();
    let traits = world
        .get::<Traits>(dead)
        .map(|t| t.0.clone())
        .unwrap_or_default();
    let kept: Vec<u16> = traits[..traits.len() / 2].to_vec();
    world
        .entity_mut(dead)
        .remove::<(Champion, ChampionRank, Traits)>();
    let mut own = world
        .get::<Traits>(heir)
        .map(|t| t.0.clone())
        .unwrap_or_default();
    own.extend(kept.iter().copied());
    world.entity_mut(heir).insert((Champion, rank, Traits(own)));
    world.resource_mut::<RunProgress>().succession_used = true;
    // Trait offers waiting for the fallen go to the heir.
    for o in &mut world.resource_mut::<Offers>().pending {
        if o.kind == (OfferKind::Trait { person: dead_id }) {
            o.kind = OfferKind::Trait { person: heir_id.0 };
        }
    }
    let heir_at = world.get::<Position>(heir).map_or(at, |p| p.0);
    world.write_message(
        SimEvent::new(SimEventKind::Succession, faction)
            .subject(heir_id.0)
            .other(dead_id)
            .at(heir_at)
            .amount(kept.len() as i32)
            .cause(cause),
    );
    log(
        world,
        format!(
            "{who}: the champion fell; captain {} takes up the banner",
            heir_id.0
        ),
    );
}

/// Why `dead` died: a blow this tick (`Cause::Death` with the striker's
/// faction), else hunger (`Cause::Starvation`), else `Cause::Death` by
/// nobody in particular.
fn cause_of_death(world: &World, dead: Entity) -> Cause {
    if let Some(&(_, by)) = world
        .resource::<Casualties>()
        .slain_by
        .iter()
        .find(|(e, _)| *e == dead)
    {
        return Cause::Death { faction: by };
    }
    let starving = world.resource::<Army>().data.upkeep.starving;
    if world
        .get::<Needs>(dead)
        .is_some_and(|n| n.hunger >= starving)
    {
        return Cause::Starvation;
    }
    Cause::Death {
        faction: FactionId::INDEPENDENT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progression::tests::run_app;
    use sim_core::{Chronicle, SimStepper};

    /// SIM-35: an offer is three distinct traits of the pool, new ones
    /// first, topped up with held ones only when too few are new.
    #[test]
    fn trait_draws_prefer_new_traits() {
        let seed = sim_core::SimSeed(7);
        let pool = [0, 1, 2, 3, 4, 5];
        for t in 0..200 {
            let mut rng = seed.stream(rng_key("test"), t);
            let held = [t as u16 % 6];
            let d = draw_traits(&mut rng, &pool, &held);
            let mut u = d.clone();
            u.sort_unstable();
            u.dedup();
            assert_eq!(u.len(), 3, "{d:?}");
            assert!(d.iter().all(|x| pool.contains(x) && !held.contains(x)));
            let held = [0, 1, 2, 3];
            let d = draw_traits(&mut rng, &pool, &held);
            assert_eq!(d.len(), 3);
            assert!(d.contains(&4) && d.contains(&5), "both new ones: {d:?}");
        }
        let mut rng = seed.stream(rng_key("test"), 0);
        assert_eq!(draw_traits(&mut rng, &[2, 2], &[]), vec![2], "pool of one");
    }

    /// SIM-35: points past two ranks at once give two `RankUp`s and two
    /// offers of three traits for the champion, drawn in turn from one
    /// stream, at the hourly step; nothing in between.
    #[test]
    fn points_past_two_ranks_post_two_trait_offers() {
        let (mut app, start) = run_app(42);
        let world = app.world_mut();
        let ranks = world.resource::<Progression>().data.champion.ranks.clone();
        let id = world.get::<SpawnId>(start.champion).unwrap().0;
        app.step(1);
        let world = app.world_mut();
        world
            .get_mut::<ChampionRank>(start.champion)
            .unwrap()
            .points = ranks[1];
        app.step(98); // ticks 1..=98
        app.step(1); // tick 99
        assert_eq!(
            app.world()
                .get::<ChampionRank>(start.champion)
                .unwrap()
                .rank,
            0,
            "only at the hour"
        );
        app.step(1);
        let world = app.world();
        assert_eq!(
            *world.get::<ChampionRank>(start.champion).unwrap(),
            ChampionRank {
                rank: 2,
                points: ranks[1]
            }
        );
        let offers: Vec<_> = world
            .resource::<Offers>()
            .pending
            .iter()
            .filter(|o| o.kind == OfferKind::Trait { person: id })
            .cloned()
            .collect();
        assert_eq!(offers.len(), 2);
        let held = &world.get::<Traits>(start.champion).unwrap().0;
        for o in &offers {
            assert_eq!(o.posted, 100);
            assert_eq!(o.faction, FactionId::PLAYER);
            assert_eq!(o.choices.len(), 3);
            assert!(o.choices.iter().all(|c| !held.contains(c)));
        }
        // Both from `traits ^ champion` at the posting tick, in turn.
        let seed = *world.resource::<SimSeed>();
        let mut rng = seed.stream(rng_key("sim_military::progression::traits") ^ id, 100);
        let pool = world.resource::<Progression>().data.champion_traits.clone();
        let first = draw_traits(&mut rng, &pool, held);
        let second = draw_traits(&mut rng, &pool, held);
        println!("offers {:?} and {:?}", offers[0].choices, offers[1].choices);
        assert_eq!((&offers[0].choices, &offers[1].choices), (&first, &second));
        let ups: Vec<i32> = world
            .resource::<Chronicle>()
            .entries()
            .iter()
            .filter(|e| e.event.kind == SimEventKind::RankUp)
            .map(|e| e.event.amount)
            .collect();
        assert_eq!(ups, vec![1, 2]);
    }
}
