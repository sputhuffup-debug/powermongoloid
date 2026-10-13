//! Invitations (GP-22): a settlement that likes the player offers lordship,
//! and the player takes it without a fight.
//!
//! Once a game hour, in a run, the progress driver calls [`step`]:
//!
//! 1. Offers past their `expires` tick are dropped (`Offers::expire`, the
//!    one place offers lapse); a lapsed invitation starts its settlement's
//!    cooldown, and a lapsed hire's band for hire moves on
//!    ([`crate::sellswords`], GP-27).
//! 2. Every populated settlement, in SpawnId order, offers lordship to the
//!    player (an `OfferKind::Invitation`, choices `[0 = accept, 1 =
//!    decline]`, expiring after `invitations.expiry_hours`) when all hold:
//!    - it isn't the player's, nor the rival's seat, and no invitation of
//!      it is pending or cooling down (`RunProgress::invite_cooldown`);
//!    - its goodwill toward the player is at least `invitations.goodwill`;
//!    - a player captain stands within `invitations.radius` of its centre
//!      (or, M4a, it sends envoys: see below);
//!    - its owner is weak: it is independent, or its residents' mean
//!      loyalty is below `invitations.weak_loyalty` and its garrison (the
//!      owner's soldiers within army.ron `garrison.radius`) is smaller than
//!      the player's soldiers within `invitations.radius`.
//!
//!   M4a's envoys (GP-31, step 3): an independent town that
//!   `influence::step` flagged at the day tick ([`Influence::envoy`]: it
//!   lies inside the player's sphere and likes the player enough) needs no
//!   captain near; it is invited "by envoys". The flag is set for at most
//!   `influence.max_envoys_per_day` towns a day, so at most that many
//!   captain-free invitations come a day; posting the invitation spends
//!   the flag, and an unspent flag goes stale at the next day tick (this
//!   step reads no flag then: slot 13 sets the new day's after it). Nothing
//!   changes when `influence.enabled` is false.
//!
//! Only the player is invited: only the player answers offers in M4.
//! [`answer`] (from `progression::choose`) accepts with
//! [`crate::conquest::transfer`] (`Cause::Invitation`, loyalty seeded from
//! goodwill, an `Invited` event) or declines; either way, and when the
//! offer lapses, the settlement waits `invitations.cooldown_days`.

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use sim_agents::{Goodwill, Owner, SettlementLoyalty};
use sim_core::{
    Calendar, Cause, FactionId, OfferKind, Offers, RunState, SimTick, SpawnId, post_offer,
};
use sim_world::Settlement;

use crate::conquest::{garrison_and_rebels, transfer};
use crate::index::FighterGrid;
use crate::influence::Influence;
use crate::orders::{faction_name, find_settlement, log, settlement_name};
use crate::progression::{Progression, RunProgress};
use crate::requests::nearest_player_captain;

/// The progress driver's invitation step (GP-22), hourly, in a run only.
/// See the module docs. Span `sim_military::invitations`.
pub fn step(world: &mut World) {
    if !world.resource::<RunState>().in_run() {
        return;
    }
    let _span = tracing::info_span!("sim_military::invitations").entered();
    let tick = world.resource::<SimTick>().0;
    let cal = world.resource::<Calendar>().clone();
    let data = world.resource::<Progression>().data.invitations;
    let cooldown = cal.days(data.cooldown_days);
    for o in world.resource_mut::<Offers>().expire(tick) {
        match o.kind {
            OfferKind::Invitation { settlement } => {
                world
                    .resource_mut::<RunProgress>()
                    .set_invite_cooldown(settlement, tick + cooldown);
                if let Some(s) = find_settlement(world, settlement) {
                    let name = settlement_name(world, s);
                    log(world, format!("{name}'s invitation lapsed"));
                }
            }
            // GP-27: unhired, the sellswords leave.
            OfferKind::Hire { captain, .. } => crate::sellswords::lapsed(world, captain),
            OfferKind::StartBoon | OfferKind::DominionBoon { .. } | OfferKind::Trait { .. } => {}
        }
    }
    let pending: Vec<u64> = world
        .resource::<Offers>()
        .pending
        .iter()
        .filter_map(|o| match o.kind {
            OfferKind::Invitation { settlement } => Some(settlement),
            _ => None,
        })
        .collect();
    let seat = world.resource::<RunProgress>().rival_seat;
    // A flag is the day's: on the day tick (slot 7 runs before slot 13
    // sets the new day's flags) yesterday's unspent flags are stale, or a
    // town flagged yesterday could send envoys at dawn and the new day's
    // flagged town an hour later, two in a day.
    let envoys = world.resource::<Progression>().data.influence.enabled && !cal.on_day(tick);
    let mut q = world.query::<(
        Entity,
        &SpawnId,
        &Settlement,
        &Owner,
        &Goodwill,
        &SettlementLoyalty,
        Option<&Influence>,
    )>();
    let mut places: Vec<Place> = q
        .iter(world)
        .map(|(e, id, st, o, g, sl, inf)| Place {
            id: id.0,
            s: e,
            center: st.center,
            owner: o.faction,
            goodwill: g.get(FactionId::PLAYER),
            loyalty: sl.mean,
            envoy: envoys && inf.is_some_and(|i| i.envoy),
        })
        .collect();
    places.sort_unstable_by_key(|p| p.id);
    for Place {
        id,
        s,
        center,
        owner,
        goodwill,
        loyalty,
        envoy,
    } in places
    {
        if owner == FactionId::PLAYER
            || Some(id) == seat
            || pending.contains(&id)
            || tick < world.resource::<RunProgress>().invite_cooldown(id)
            || goodwill < data.goodwill
        {
            continue;
        }
        // GP-31 (M4a): an independent town flagged at dawn sends envoys
        // when no captain is near.
        let near = nearest_player_captain(world, center, data.radius).is_some();
        let by_envoys = !near && envoy && owner == FactionId::INDEPENDENT;
        if !near && !by_envoys {
            continue;
        }
        let weak = owner == FactionId::INDEPENDENT
            || (loyalty < data.weak_loyalty
                && garrison_and_rebels(world, center, owner).0
                    < player_soldiers_near(world, center, data.radius));
        if !weak {
            continue;
        }
        post_offer(
            world,
            FactionId::PLAYER,
            OfferKind::Invitation { settlement: id },
            vec![0, 1],
            Some(tick + cal.hours(data.expiry_hours)),
        );
        let (name, who) = (
            settlement_name(world, s),
            faction_name(world, FactionId::PLAYER),
        );
        if by_envoys {
            // The day's envoys are spent: never two invitations of one flag.
            if let Some(mut inf) = world.get_mut::<Influence>(s) {
                inf.envoy = false;
            }
            log(
                world,
                format!("{name} sends envoys: it invites {who} to rule it"),
            );
        } else {
            log(world, format!("{name} invites {who} to rule it"));
        }
    }
}

/// What accepting town `s`'s invitation would make it (GP-32, M4a step 5:
/// the card's "Sworn" words come from the tenure-to-be): residents' loyalty
/// seeded from their goodwill toward the player (`sim_agents::default_seed`)
/// and the Bond term it starts with (`tenure.bond`). `None` outside a run.
pub fn sworn_terms(world: &World, s: Entity) -> Option<SwornTerms> {
    world.get::<crate::tenure::Tenure>(s)?;
    Some(SwornTerms {
        seed: sim_agents::standing::default_seed(world, s, FactionId::PLAYER).clamp(0.0, 1.0),
        bond: world.resource::<Progression>().data.tenure.bond,
    })
}

/// The tenure an accepted invitation starts ([`sworn_terms`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwornTerms {
    /// Residents' loyalty at the start.
    pub seed: f32,
    /// The Bond term at the start.
    pub bond: f32,
}

/// A settlement as [`step`] weighs it.
struct Place {
    id: u64,
    s: Entity,
    center: IVec2,
    owner: FactionId,
    goodwill: f32,
    loyalty: f32,
    /// Flagged at dawn to send envoys (`Influence::envoy`, influence on).
    envoy: bool,
}

/// The player's soldiers (not routed) within `radius` of `at`.
fn player_soldiers_near(world: &World, at: IVec2, radius: f32) -> u32 {
    let mut n = 0;
    world
        .resource::<FighterGrid>()
        .for_each_within(at.as_vec2(), radius, |e, _| {
            if e.side.faction == FactionId::PLAYER && e.side.against.is_none() && !e.routed {
                n += 1;
            }
        });
    n
}

/// GP-22: `faction` answers settlement `settlement`'s invitation with
/// `card` (0 accepts, anything else declines). Accepting hands the
/// settlement over ([`transfer`], `Cause::Invitation`; residents' loyalty
/// starts from their goodwill toward `faction`), unless `faction` already
/// holds it. Either way the settlement offers nothing more for
/// `invitations.cooldown_days`. Returns what was done, for the log.
pub fn answer(world: &mut World, faction: FactionId, settlement: u64, card: u16) -> String {
    let tick = world.resource::<SimTick>().0;
    let days = world
        .resource::<Progression>()
        .data
        .invitations
        .cooldown_days;
    let until = tick + world.resource::<Calendar>().days(days);
    world
        .resource_mut::<RunProgress>()
        .set_invite_cooldown(settlement, until);
    let Some(s) = find_settlement(world, settlement) else {
        return format!("the invitation of settlement {settlement}");
    };
    let name = settlement_name(world, s);
    if card == 0 && crate::conquest::owner_of(world, s) == faction {
        // Taken some other way while the offer stood: nothing to hand over
        // (and residents' loyalty isn't reseeded).
        format!("to rule {name}, already theirs")
    } else if card == 0 {
        transfer(world, s, faction, Cause::Invitation);
        format!("to rule {name}")
    } else {
        format!("to decline {name}'s invitation")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conquest::owner_of;
    use crate::conquest::tests::settlements;
    use crate::progression::tests::{command, run_app};
    use crate::spawn::{ArmySpec, spawn_army};
    use bevy_app::App;
    use sim_agents::{Enlisted, Loyalty, Villager};
    use sim_core::{Chronicle, CommandKind, SimEventKind, SimStepper};

    fn invitation_of(app: &App, settlement: u64) -> Option<sim_core::Offer> {
        app.world()
            .resource::<Offers>()
            .pending
            .iter()
            .find(|o| o.kind == OfferKind::Invitation { settlement })
            .cloned()
    }

    fn like(world: &mut World, s: Entity, g: f32) {
        world
            .get_mut::<Goodwill>(s)
            .unwrap()
            .set(FactionId::PLAYER, g);
    }

    fn band_at(world: &mut World, at: IVec2) {
        spawn_army(world, &ArmySpec::new(FactionId::PLAYER, at, 5));
    }

    /// GP-22: a village that likes the player, with a player band near,
    /// invites it in; accepting hands it over with loyalty seeded from
    /// goodwill and an `Invited` chronicle entry. A declined invitation and
    /// a lapsed one start the cooldown; a village that doesn't like the
    /// player enough, or has no player band near, offers nothing.
    #[test]
    fn a_village_that_likes_the_band_invites_it_in() {
        let (mut app, _) = run_app(42);
        let list = settlements(app.world_mut());
        let [a, b, c, e] = [list[0], list[1], list[2], list[3]];
        let world = app.world_mut();
        world
            .resource_mut::<Progression>()
            .data
            .invitations
            .expiry_hours = 2.0;
        let data = world.resource::<Progression>().data.invitations;
        let want = data.goodwill;
        for (_, s, center) in [a, b, c] {
            band_at(world, center + IVec2::new(4, 0));
            like(world, s, want + 0.1);
        }
        // e has a band but too little goodwill.
        like(world, e.1, want - 0.2);
        band_at(world, e.2 + IVec2::new(4, 0));
        // d: liked, but no player captain within the radius.
        let d = list[4..]
            .iter()
            .copied()
            .find(|(_, _, center)| nearest_player_captain(world, *center, data.radius).is_none())
            .expect("a settlement far from every band");
        like(world, d.1, want + 0.1);
        app.step(1);
        let offered: Vec<u64> = app
            .world()
            .resource::<Offers>()
            .pending
            .iter()
            .filter_map(|o| match o.kind {
                OfferKind::Invitation { settlement } => Some(settlement),
                _ => None,
            })
            .collect();
        assert!(offered.contains(&a.0) && offered.contains(&b.0) && offered.contains(&c.0));
        assert!(!offered.contains(&e.0), "too little goodwill");
        assert!(!offered.contains(&d.0), "nobody near");
        let oa = invitation_of(&app, a.0).unwrap();
        assert_eq!(oa.choices, vec![0, 1]);
        assert_eq!(oa.expires, Some(200));

        // Accept a.
        command(
            &mut app,
            FactionId::PLAYER,
            CommandKind::ChooseOffer {
                offer: oa.id,
                choice: 0,
            },
        );
        let world = app.world_mut();
        assert_eq!(owner_of(world, a.1), FactionId::PLAYER);
        let seed = (want + 0.1 + 1.0) / 2.0;
        let mut q =
            world.query_filtered::<(&Villager, &Loyalty), bevy_ecs::query::Without<Enlisted>>();
        let loyal: Vec<f32> = q
            .iter(world)
            .filter(|(v, _)| v.settlement == a.1)
            .map(|(_, l)| l.0)
            .collect();
        assert!(!loyal.is_empty());
        assert!(
            loyal.iter().all(|l| (l - seed).abs() < 1e-4),
            "{loyal:?} vs {seed}"
        );
        let invited = world
            .resource::<Chronicle>()
            .entries()
            .iter()
            .find(|e| e.event.kind == SimEventKind::Invited)
            .map(|e| (e.event.faction, e.event.subject, e.event.cause))
            .expect("an Invited entry");
        assert_eq!(invited, (FactionId::PLAYER, a.0, Cause::Invitation));

        // Decline b.
        let ob = invitation_of(&app, b.0).unwrap();
        command(
            &mut app,
            FactionId::PLAYER,
            CommandKind::ChooseOffer {
                offer: ob.id,
                choice: 1,
            },
        );
        assert_eq!(owner_of(app.world(), b.1), FactionId::INDEPENDENT);
        // c lapses at 200; neither b nor c offers again within the
        // cooldown, though nothing else changed.
        app.step(300);
        let cooldown = {
            let w = app.world();
            let days = w.resource::<Progression>().data.invitations.cooldown_days;
            w.resource::<Calendar>().days(days)
        };
        let progress = app.world().resource::<RunProgress>().clone();
        assert!(invitation_of(&app, c.0).is_none(), "lapsed");
        assert_eq!(progress.invite_cooldown(c.0), 200 + cooldown);
        assert_eq!(progress.invite_cooldown(b.0), 2 + cooldown);
        assert!(invitation_of(&app, b.0).is_none(), "declined");
        assert_eq!(owner_of(app.world(), c.1), FactionId::INDEPENDENT);
        assert!(invitation_of(&app, a.0).is_none(), "already the player's");
    }
}
