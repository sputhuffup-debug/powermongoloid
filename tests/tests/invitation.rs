//! M4 step 3 scenario `invitation` (GP-22, GP-05; ADR 0014): a village
//! whose goodwill toward the player passes `invitations.goodwill` offers
//! lordship while the player's band is near, and the player takes it
//! without a fight.
//!
//! Both tests play the run of seed 25 (a band of ten with two days of
//! food) through player commands. Before the first tick the nearest
//! hamlet thinks fairly well of the band (goodwill 0.35, below the 0.5
//! threshold) and its stores spoil, so it asks for food. The band, passive
//! so it never takes a village's food, walks over and gives: the answered request's `goodwill_deeds.request` (+0.2)
//! carries the hamlet past the threshold, and with the band beside it the
//! hamlet invites it in at the next hourly step ([`invited`]). Then:
//!
//! - `accepting_an_invitation_hands_the_village_over`: the hamlet is the
//!   player's, with an `Invited` chronicle entry, every resident sworn to
//!   the player, and loyalty seeded from goodwill (`(g + 1) / 2`).
//! - `declining_an_invitation_waits_out_the_cooldown`: the hamlet stays
//!   independent and offers nothing more until `invitations.cooldown_days`
//!   have passed; then it asks again, and an offer nobody answers lapses
//!   after `invitations.expiry_hours` (an answer on its last tick is too
//!   late), starting the cooldown again. This
//!   test shortens the cooldown and expiry in the data before the first
//!   tick so it fits in a few hours.
//!
//! Each test prints the numbers it checks.

mod common;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use common::*;
use sim_agents::{Enlisted, Goodwill, Loyalty, Villager};
use sim_core::{
    Allegiance, Calendar, Cause, CommandKind, FactionId, Offer, OfferKind, Offers, OrderSpec,
    Position, Posture, SimEventKind, SimStepper, SimTick, SpawnId,
};
use sim_economy::{Economy, SettlementEconomy, Stockpile};
use sim_military::progression::Effect;
use sim_military::{MilitaryLog, Progression, RequestBoard, RunProgress};
use sim_world::{Settlement, SettlementSize, SizeTier};

/// The run both tests play.
const SEED: u64 = 25;
/// The hamlet's goodwill toward the player before the first tick: below
/// `invitations.goodwill` (0.5) until a request is answered.
const GOODWILL: f32 = 0.35;
/// The invitation must come within this many ticks.
const INVITED_WITHIN: u64 = 3_000;

fn tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

fn ticks_per_hour(app: &App) -> u64 {
    app.world().resource::<Calendar>().ticks_per_hour()
}

fn goodwill(app: &App, s: Entity) -> f32 {
    app.world()
        .get::<Goodwill>(s)
        .expect("goodwill")
        .get(FactionId::PLAYER)
}

/// The pending invitation of settlement `id`, if any.
fn invitation(app: &App, id: u64) -> Option<Offer> {
    app.world()
        .resource::<Offers>()
        .pending
        .iter()
        .find(|o| o.kind == OfferKind::Invitation { settlement: id })
        .cloned()
}

/// Every invitation pending, by settlement.
fn invitations(app: &App) -> Vec<u64> {
    app.world()
        .resource::<Offers>()
        .pending
        .iter()
        .filter_map(|o| match o.kind {
            OfferKind::Invitation { settlement } => Some(settlement),
            _ => None,
        })
        .collect()
}

/// The champion's distance to `centre`, in tiles.
fn distance(app: &mut App, centre: IVec2) -> f32 {
    let c = champion(app);
    let at = app.world().get::<Position>(c).unwrap().0;
    (centre - at).as_vec2().length()
}

/// The run of [`SEED`] (a stat-only start boon picked), the nearest
/// hamlet at [`GOODWILL`] with its food spoiled, the band giving it food,
/// stepped hourly until the hamlet invites the band in. Asserts no
/// invitation came while its goodwill was below the threshold, even with
/// the band beside it. `edit` changes the data before the first tick.
/// Returns the app, the hamlet, its SpawnId and the invitation.
fn invited(edit: impl FnOnce(&mut Progression)) -> (App, Entity, u64, Offer) {
    let mut app = run_app(SEED);
    edit(&mut app.world_mut().resource_mut::<Progression>());
    let data = app.world().resource::<Progression>().data.clone();
    let champion = champion(&mut app);
    let offer = start_offer(&app);
    let cards = app
        .world()
        .resource::<Offers>()
        .get(offer)
        .unwrap()
        .choices
        .clone();
    let pick = cards
        .iter()
        .position(|&c| {
            data.boons[c as usize]
                .effects
                .iter()
                .all(|e| matches!(e, Effect::Stat(..)))
        })
        .expect("a stat-only boon on offer");
    choose(&mut app, offer, pick as u8);

    let home = app.world().get::<Position>(champion).unwrap().0;
    let seat = app.world().resource::<RunProgress>().rival_seat;
    let (hamlet, id, centre) = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &SpawnId, &Settlement, &SettlementSize)>();
        q.iter(world)
            .filter(|(_, id, _, size)| size.tier == SizeTier::Hamlet && Some(id.0) != seat)
            .map(|(e, id, s, _)| ((s.center - home).as_vec2().length(), id.0, e, s.center))
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, id, e, c)| (e, id, c))
            .expect("a hamlet")
    };
    // Setup edits before the first tick: goodwill, and spoiled stores.
    app.world_mut()
        .get_mut::<Goodwill>(hamlet)
        .unwrap()
        .set(FactionId::PLAYER, GOODWILL);
    let econ = app.world().resource::<Economy>().clone();
    let se = app
        .world()
        .get::<SettlementEconomy>(hamlet)
        .unwrap()
        .clone();
    for store in std::iter::once(se.storehouse).chain(se.homes.iter().copied()) {
        if let Some(mut stock) = app.world_mut().get_mut::<Stockpile>(store) {
            stock.goods.retain(|g, _| !econ.is_food(*g));
        }
    }
    // A passive band never helps itself to a village's food, so nothing
    // but the answered request moves the hamlet's goodwill.
    let cid = spawn_id(&app, champion);
    command(
        &mut app,
        FactionId::PLAYER.0,
        CommandKind::SetPosture {
            captain: cid,
            posture: Posture::Passive,
        },
    );
    order(&mut app, 1, champion, OrderSpec::Give { settlement: id });

    let rules = data.invitations;
    let tph = ticks_per_hour(&app);
    let mut near_before = 0;
    let offer = loop {
        assert!(
            tick(&app) < INVITED_WITHIN,
            "no invitation by tick {INVITED_WITHIN}: goodwill {:.2}",
            goodwill(&app, hamlet)
        );
        let g = goodwill(&app, hamlet);
        let near = distance(&mut app, centre) <= rules.radius;
        app.step(tph);
        if let Some(o) = invitation(&app, id) {
            println!(
                "tick {}: hamlet #{id} invites the band in (goodwill {:.2}, band {:.0} tiles \
                 away; it was near for {near_before} hours below the threshold)",
                tick(&app),
                goodwill(&app, hamlet),
                distance(&mut app, centre)
            );
            break o;
        }
        if near && g < rules.goodwill {
            near_before += 1;
        }
    };
    assert!(goodwill(&app, hamlet) >= rules.goodwill);
    assert!(distance(&mut app, centre) <= rules.radius);
    assert!(near_before > 0, "the band was never near while disliked");
    assert_eq!(invitations(&app), vec![id], "only the hamlet invites");
    assert_eq!(offer.choices, vec![0, 1]);
    assert_eq!(
        offer.expires,
        Some(offer.posted + app.world().resource::<Calendar>().hours(rules.expiry_hours))
    );
    let board = app.world().resource::<RequestBoard>();
    assert!(board.done >= 1, "the request was answered: {board:?}");
    (app, hamlet, id, offer)
}

/// Residents of `s` (not enlisted): (allegiance, loyalty).
fn residents(app: &mut App, s: Entity) -> Vec<(FactionId, f32)> {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(&Villager, &Allegiance, &Loyalty), Without<Enlisted>>();
    q.iter(world)
        .filter(|(v, ..)| v.settlement == s)
        .map(|(_, a, l)| (a.0, l.0))
        .collect()
}

/// GP-22, GP-05: accepting hands the hamlet over without a fight, with
/// loyalty seeded from goodwill.
#[test]
fn accepting_an_invitation_hands_the_village_over() {
    let (mut app, hamlet, id, offer) = invited(|_| {});
    let g = goodwill(&app, hamlet);
    assert_eq!(owner_of(&app, hamlet), FactionId::INDEPENDENT);
    choose(&mut app, offer.id, 0);
    app.step(1);
    let at = tick(&app) - 1;
    let people = residents(&mut app, hamlet);
    let seed = (g + 1.0) / 2.0;
    let cooldown = app
        .world()
        .resource::<Progression>()
        .data
        .invitations
        .cooldown_days;
    let entry = chronicle(&app)
        .into_iter()
        .find(|e| e.event.kind == SimEventKind::Invited && e.event.subject == id);
    println!(
        "accepted at tick {at}: owner {}, {} residents, loyalty seeded {seed:.3} from goodwill \
         {g:.3}; chronicle {:?}",
        owner_of(&app, hamlet).0,
        people.len(),
        entry.as_ref().map(|e| (e.tick, e.event.cause))
    );
    assert_eq!(owner_of(&app, hamlet), FactionId::PLAYER);
    let entry = entry.expect("an Invited chronicle entry");
    assert_eq!(
        (entry.tick, entry.event.faction, entry.event.cause),
        (at, FactionId::PLAYER, Cause::Invitation)
    );
    assert!(!people.is_empty());
    for (a, l) in &people {
        assert_eq!(*a, FactionId::PLAYER, "a resident still sworn elsewhere");
        assert!((l - seed).abs() < 1e-5, "loyalty {l}, seeded {seed}");
    }
    assert!(invitation(&app, id).is_none(), "the offer was taken");
    assert_eq!(
        app.world().resource::<RunProgress>().invite_cooldown(id),
        at + app.world().resource::<Calendar>().days(cooldown)
    );
    // A second answer finds nothing.
    let refused = rejected(&app, FactionId::PLAYER);
    choose(&mut app, offer.id, 0);
    app.step(1);
    assert_eq!(rejected(&app, FactionId::PLAYER), refused + 1);
    // Ours now: it offers no more lordship.
    app.step(2 * ticks_per_hour(&app));
    assert!(invitation(&app, id).is_none());
}

/// GP-22: declining keeps the hamlet independent and starts its cooldown;
/// after it the hamlet asks again, and an unanswered offer lapses into a
/// new cooldown.
#[test]
fn declining_an_invitation_waits_out_the_cooldown() {
    let (mut app, hamlet, id, offer) = invited(|p| {
        p.data.invitations.cooldown_days = 0.25;
        p.data.invitations.expiry_hours = 2.0;
    });
    let rules = app.world().resource::<Progression>().data.invitations;
    let cal = app.world().resource::<Calendar>().clone();
    let tph = cal.ticks_per_hour();
    choose(&mut app, offer.id, 1);
    app.step(1);
    let declined = tick(&app) - 1;
    let until = app.world().resource::<RunProgress>().invite_cooldown(id);
    println!(
        "declined at tick {declined}: owner {}, quiet until {until}",
        owner_of(&app, hamlet).0
    );
    assert_eq!(owner_of(&app, hamlet), FactionId::INDEPENDENT);
    assert!(invitation(&app, id).is_none());
    assert_eq!(until, declined + cal.days(rules.cooldown_days));

    // Quiet through the cooldown, though the band is still beside it.
    let mut again = None;
    while again.is_none() {
        app.step(tph);
        assert!(
            tick(&app) <= until + tph,
            "no new invitation after the cooldown"
        );
        again = invitation(&app, id);
        if again.is_none() {
            continue;
        }
        assert!(
            tick(&app) >= until,
            "invited again at {} before {until}",
            tick(&app)
        );
    }
    let again = again.unwrap();
    println!("invited again at tick {}", again.posted);

    // Nobody answers in time: it lapses at `expires` (an answer arriving
    // on that very tick is refused), and the cooldown starts again.
    let expires = again.expires.expect("invitations expire");
    app.step(expires - tick(&app));
    let refused = rejected(&app, FactionId::PLAYER);
    choose(&mut app, again.id, 0);
    app.step(1);
    assert_eq!(
        rejected(&app, FactionId::PLAYER),
        refused + 1,
        "answered too late"
    );
    let lapsed_until = app.world().resource::<RunProgress>().invite_cooldown(id);
    let logged = app
        .world()
        .resource::<MilitaryLog>()
        .entries
        .iter()
        .any(|(_, l)| l.contains("invitation lapsed"));
    println!("lapsed at tick {expires}: quiet until {lapsed_until}");
    assert!(invitation(&app, id).is_none(), "the offer lapsed");
    assert_eq!(lapsed_until, expires + cal.days(rules.cooldown_days));
    assert!(logged, "the lapse is logged");
    assert_eq!(owner_of(&app, hamlet), FactionId::INDEPENDENT);
}
