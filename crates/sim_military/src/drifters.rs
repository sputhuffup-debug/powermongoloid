//! Drifters (GP-22): word of a band's good deeds brings volunteers.
//!
//! Once a day, in a run, the progress driver calls [`draw`]. For each of
//! the player's captains in SpawnId order (a stream `sim_military::drifters
//! ^ captain` at the day's start tick), every populated settlement in
//! SpawnId order whose centre is within `drifters.radius` of the captain,
//! that is independent or the player's, and whose goodwill toward the
//! player is at least `drifters.goodwill`, sends a seeded number of
//! volunteers in `drifters.per_settlement`, at most `drifters.max_per_day`
//! per captain. They are real villagers: the first of recruiting's
//! candidates (working-age, civilian, idle first), enlisted with
//! [`crate::orders::enlist_into`] like any recruit. Each settlement that
//! sends someone reports a `DriftersJoined` event (`subject` = the
//! captain, `other` = the settlement, `at` = the captain's tile, `amount`
//! = how many).

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use rand::RngExt;
use sim_agents::{Goodwill, Owner};
use sim_core::rng::rng_key;
use sim_core::{
    Calendar, FactionId, Position, RunState, SimEvent, SimEventKind, SimSeed, SimTick, SpawnId,
};
use sim_world::Settlement;

use crate::components::Captain;
use crate::orders::{enlist_into, faction_name, log, recruit_candidates, settlement_name};
use crate::progression::{Progression, RunProgress};

/// The progress driver's daily drifter step (GP-22), in a run only. See the
/// module docs. Span `sim_military::drifters`.
pub fn draw(world: &mut World) {
    let tick = world.resource::<SimTick>().0;
    if !world.resource::<Calendar>().on_day(tick) || !world.resource::<RunState>().in_run() {
        return;
    }
    let _span = tracing::info_span!("sim_military::drifters").entered();
    let data = world.resource::<Progression>().data.drifters;
    let mut q = world.query::<(Entity, &SpawnId, &Captain, &Position)>();
    let mut captains: Vec<(u64, Entity, IVec2)> = q
        .iter(world)
        .filter(|(_, _, c, _)| c.faction == FactionId::PLAYER)
        .map(|(e, id, _, p)| (id.0, e, p.0))
        .collect();
    captains.sort_unstable_by_key(|c| c.0);
    let mut q = world.query::<(Entity, &SpawnId, &Settlement, &Owner, &Goodwill)>();
    let mut places: Vec<(u64, Entity, IVec2)> = q
        .iter(world)
        .filter(|(.., o, g)| {
            matches!(o.faction, FactionId::INDEPENDENT | FactionId::PLAYER)
                && g.get(FactionId::PLAYER) >= data.goodwill
        })
        .map(|(e, id, st, ..)| (id.0, e, st.center))
        .collect();
    places.sort_unstable_by_key(|p| p.0);
    let (lo, hi) = data.per_settlement;
    for (cid, captain, at) in captains {
        let mut rng = world
            .resource::<SimSeed>()
            .stream(rng_key("sim_military::drifters") ^ cid, tick);
        let mut total = 0;
        for &(sid, s, center) in &places {
            if total >= data.max_per_day {
                break;
            }
            if (center - at).as_vec2().length() > data.radius {
                continue;
            }
            let n = rng.random_range(lo..=hi).min(data.max_per_day - total);
            if n == 0 {
                continue;
            }
            let mut joined = 0;
            for v in recruit_candidates(world, s).into_iter().take(n as usize) {
                if enlist_into(world, v, captain) {
                    joined += 1;
                }
            }
            if joined == 0 {
                continue;
            }
            total += joined;
            world.resource_mut::<RunProgress>().drifters += joined;
            world.write_message(
                SimEvent::new(SimEventKind::DriftersJoined, FactionId::PLAYER)
                    .subject(cid)
                    .other(sid)
                    .at(at)
                    .amount(joined as i32),
            );
            let (who, name) = (
                faction_name(world, FactionId::PLAYER),
                settlement_name(world, s),
            );
            log(
                world,
                format!("{joined} of {name} drifted in to join {who}"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{ArmyStats, Follower};
    use crate::conquest::tests::settlements;
    use crate::progression::tests::run_app;
    use crate::spawn::{ArmySpec, spawn_army};
    use sim_agents::{Enlisted, Villager};
    use sim_core::{Allegiance, EventFeed, SimStepper};
    use sim_economy::{SettlementEconomy, Stockpile};

    fn followers(world: &mut World, captain: Entity) -> Vec<Entity> {
        let mut q = world.query::<(Entity, &Follower)>();
        q.iter(world)
            .filter(|(_, f)| f.captain == captain)
            .map(|(e, _)| e)
            .collect()
    }

    /// GP-22: at the day tick, a village that likes the player sends
    /// volunteers to the player band near it: real villagers, enlisted
    /// like recruits (allegiance, population, baggage, `recruited`), with
    /// a `DriftersJoined` event. A village that doesn't like the player
    /// enough sends nobody, and the draw is seeded.
    #[test]
    fn a_village_that_likes_the_band_sends_it_drifters() {
        let run = |liked: bool| {
            let (mut app, start) = run_app(42);
            let world = app.world_mut();
            world
                .resource_mut::<Progression>()
                .data
                .drifters
                .per_settlement = (2, 2);
            let data = world.resource::<Progression>().data.drifters;
            // A village out of the champion's reach, so only our band draws.
            let home = world.get::<Position>(start.champion).unwrap().0;
            let (sid, s, center) = settlements(world)
                .into_iter()
                .find(|(_, _, c)| (*c - home).as_vec2().length() > 2.0 * data.radius)
                .expect("a village far from the champion");
            world.get_mut::<Goodwill>(s).unwrap().set(
                FactionId::PLAYER,
                if liked {
                    data.goodwill
                } else {
                    data.goodwill - 0.1
                },
            );
            let cap = spawn_army(
                world,
                &ArmySpec::new(FactionId::PLAYER, center + IVec2::new(20, 0), 5),
            );
            let cid = world.get::<SpawnId>(cap).unwrap().0;
            let pop0 = world.get::<SettlementEconomy>(s).unwrap().population;
            let bag0 = world.get::<Stockpile>(cap).unwrap().capacity;
            let men0 = followers(world, cap).len();
            // Tick 0 is a day tick.
            app.step(1);
            let world = app.world_mut();
            let new = followers(world, cap).len() - men0;
            let left = pop0 - world.get::<SettlementEconomy>(s).unwrap().population;
            let feed: Vec<(u64, u64, i32)> = world
                .resource::<EventFeed>()
                .iter()
                .filter(|e| e.event.kind == SimEventKind::DriftersJoined)
                .map(|e| (e.event.subject, e.event.other, e.event.amount))
                .collect();
            (app, (sid, s, cap, cid, bag0), (new, left), feed)
        };
        let (mut app, (sid, s, cap, cid, bag0), (new, left), feed) = run(true);
        assert_eq!(
            (new, left),
            (2, 2),
            "two drifters left the village and joined"
        );
        assert_eq!(feed, vec![(cid, sid, 2)]);
        let world = app.world_mut();
        assert_eq!(world.resource::<RunProgress>().drifters, 2);
        assert_eq!(
            world
                .resource::<ArmyStats>()
                .get(FactionId::PLAYER)
                .recruited,
            2
        );
        let per_head = world.resource::<crate::Army>().data.baggage_per_head;
        assert_eq!(
            world.get::<Stockpile>(cap).unwrap().capacity,
            bag0 + 2 * per_head
        );
        let mut q = world.query::<(&Follower, &Villager, &Allegiance, &Enlisted)>();
        let drifters: Vec<(Entity, FactionId)> = q
            .iter(world)
            .filter(|(f, ..)| f.captain == cap)
            .map(|(_, v, a, _)| (v.settlement, a.0))
            .collect();
        assert_eq!(drifters, vec![(s, FactionId::PLAYER); 2]);

        let (_, _, (new, left), feed) = run(false);
        assert_eq!((new, left), (0, 0), "not liked enough");
        assert!(feed.is_empty());
    }
}
