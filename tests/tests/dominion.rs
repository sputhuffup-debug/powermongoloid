//! M4 step 5 test `dominion` (GP-24, GP-25): holding 25% and 50% of a
//! region's people offers one of three dominion boons, once per threshold;
//! captain slots follow the population held, and appointing beyond them is
//! refused.
//!
//! - `dominion_boons_come_once_per_threshold` plays the run of seed 42 in
//!   the start region. Before the first tick the player is given two
//!   settlements whose share crosses 25% (one fed and loyal, one starved
//!   and seeded at loyalty 0), and two independent settlements think well
//!   of the player. The tick-0 tally offers the first boon; picking it
//!   applies. The starved settlement revolts a day later, dropping the
//!   share below 25%; a small warband standing by takes it back by
//!   invitation (re-crossing 25% offers nothing), and the champion's band
//!   is then invited into the region's town, which carries the share past
//!   50% and offers the second boon. Slots are checked against the
//!   formula throughout, and an appointment beyond them is refused.
//! - `captain_slots_follow_the_land_held` gives the player about 600
//!   people before the first tick and appoints up to the slots (one more
//!   is refused); then, on a second island, every settlement but the
//!   rival's seat: 13 slots by population, capped at 8, and every
//!   region's two offers posted exactly once.
//!
//! Each test prints the numbers it checks.

mod common;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use common::*;
use sim_core::{
    CommandKind, FactionId, FactionModifiers, Offer, OfferKind, Offers, OrderSpec, Position,
    SimStepper, SimTick, SpawnId, Stat,
};
use sim_economy::{SettlementEconomy, Stockpile};
use sim_military::progression::Effect;
use sim_military::{
    ArmySpec, Captain, Follower, Guard, MilitaryLog, Progression, RunProgress, captain_slots,
    spawn_army,
};
use sim_world::Settlement;

/// The run [`dominion_boons_come_once_per_threshold`] plays.
const SEED: u64 = 42;
/// Shares are kept at least this far from a threshold, so drifters and
/// hunger moving a few people never decide the test.
const MARGIN: f32 = 0.02;

fn tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

/// Steps to just after the next game hour (the progress driver ran).
fn step_hour(app: &mut App) {
    let hour = app
        .world()
        .resource::<sim_core::Calendar>()
        .ticks_per_hour();
    let t = tick(app);
    app.step(hour - t % hour + 1);
}

/// A populated settlement: entity, SpawnId, region, centre, people.
#[derive(Debug, Clone, Copy)]
struct Place {
    e: Entity,
    id: u64,
    region: u8,
    center: IVec2,
    people: u32,
}

fn places(app: &mut App) -> Vec<Place> {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &SpawnId, &Settlement, &SettlementEconomy)>();
    let mut v: Vec<Place> = q
        .iter(world)
        .map(|(e, id, s, se)| Place {
            e,
            id: id.0,
            region: s.region,
            center: s.center,
            people: se.population,
        })
        .collect();
    v.sort_unstable_by_key(|p| p.id);
    v
}

/// Dominion offers pending, in id order.
fn dominion_offers(app: &App) -> Vec<Offer> {
    app.world()
        .resource::<Offers>()
        .pending
        .iter()
        .filter(|o| matches!(o.kind, OfferKind::DominionBoon { .. }))
        .cloned()
        .collect()
}

/// The id the next offer will take. Ids are never reused, so the
/// difference counts every offer posted in between (`OfferPosted` is not
/// chronicled).
fn next_offer_id(app: &App) -> u64 {
    app.world().resource::<Offers>().next_id
}

/// Asserts `offer` is the player's dominion offer for `region`/`step`
/// with three distinct Dominion boons.
fn check_offer(app: &App, offer: &Offer, region: u8, step: u8) {
    let data = &app.world().resource::<Progression>().data;
    assert_eq!(offer.faction, FactionId::PLAYER);
    assert_eq!(offer.kind, OfferKind::DominionBoon { region, step });
    assert_eq!(offer.expires, None, "a dominion offer waits");
    let mut cards = offer.choices.clone();
    assert_eq!(cards.len(), 3, "three boons: {:?}", offer.choices);
    for &c in &cards {
        assert_eq!(
            data.boons[c as usize].when,
            sim_military::progression::BoonWhen::Dominion,
            "card {c} is a Dominion boon"
        );
    }
    cards.sort_unstable();
    cards.dedup();
    assert_eq!(cards.len(), 3, "distinct: {:?}", offer.choices);
}

/// The captain-slot formula, recomputed from the world and the data:
/// `min(cap, max(first_village_slots if any settlement is held,
/// floor(held / per_captain)) + boon slots + floor(CaptainSlots))`.
fn expected_slots(app: &mut App) -> u32 {
    let world = app.world_mut();
    let mut q = world.query::<(&sim_agents::Owner, &SettlementEconomy)>();
    let (mut held, mut pop) = (0u32, 0u32);
    for (o, se) in q.iter(world) {
        if o.faction == FactionId::PLAYER {
            held += 1;
            pop += se.population;
        }
    }
    let data = world.resource::<Progression>().data.clone();
    let d = &data.dominion;
    let boons: u32 = world
        .resource::<RunProgress>()
        .boons
        .iter()
        .flat_map(|&c| data.boons[c as usize].effects.clone())
        .map(|e| match e {
            Effect::CaptainSlots(n) => n as u32,
            _ => 0,
        })
        .sum();
    let extra = world
        .resource::<FactionModifiers>()
        .value(FactionId::PLAYER, Stat::CaptainSlots);
    let first = if held > 0 { d.first_village_slots } else { 0 };
    let base = first.max(pop / d.per_captain);
    ((base + boons) as i64 + (extra + 1e-3).floor() as i64).clamp(0, d.cap as i64) as u32
}

/// The slots stored for the UI equal the live value.
fn assert_slots_stored(app: &mut App) {
    let live = slots(app);
    assert_eq!(app.world().resource::<RunProgress>().captain_slots, live);
}

fn slots(app: &mut App) -> u32 {
    captain_slots(app.world_mut(), FactionId::PLAYER).expect("a run has slots")
}

/// The player's captains, champion included.
fn captains(app: &mut App) -> u32 {
    let world = app.world_mut();
    let mut q = world.query::<&Captain>();
    q.iter(world)
        .filter(|c| c.faction == FactionId::PLAYER)
        .count() as u32
}

/// A follower to appoint: the champion's lowest-SpawnId follower (the
/// champion's band stays clear of the settlements under test), else the
/// lowest-SpawnId follower of any player captain.
fn a_follower(app: &mut App) -> u64 {
    let champion = champion(app);
    let world = app.world_mut();
    let mut q = world.query::<(&SpawnId, &Follower)>();
    let ids: Vec<(u64, Entity)> = q.iter(world).map(|(id, f)| (id.0, f.captain)).collect();
    let of_champion = ids
        .iter()
        .filter(|(_, c)| *c == champion)
        .map(|(id, _)| *id)
        .min();
    of_champion
        .or_else(|| {
            ids.iter()
                .filter(|(_, c)| {
                    world
                        .get::<Captain>(*c)
                        .is_some_and(|c| c.faction == FactionId::PLAYER)
                })
                .map(|(id, _)| *id)
                .min()
        })
        .expect("a player follower")
}

/// Sends an AppointCaptain and steps a tick; returns whether it was
/// refused, and checks the refusal's reason when it was.
fn appoint(app: &mut App, reason: &str) -> bool {
    let follower = a_follower(app);
    let before = rejected(app, FactionId::PLAYER);
    command(
        app,
        FactionId::PLAYER.0,
        CommandKind::AppointCaptain { follower },
    );
    app.step(1);
    let refused = rejected(app, FactionId::PLAYER) > before;
    if refused {
        let log = app.world().resource::<MilitaryLog>();
        let last = &log.entries.back().expect("a log line").1;
        assert!(
            last.contains(&format!("order refused ({reason})")),
            "refused for {reason}: {last}"
        );
    }
    refused
}

/// Hands `s` to the player before the first tick, its residents' loyalty
/// seeded at `loyalty` (a setup edit, like the scenario tests' others).
fn give_to_player(app: &mut App, s: Entity, loyalty: f32) {
    sim_agents::set_owner(app.world_mut(), s, FactionId::PLAYER, Some(loyalty))
        .expect("a populated settlement");
}

/// Empties `s`'s storehouse and homes and idles its farmers, so it
/// starves (as conquest.rs's starved branch).
fn starve(app: &mut App, s: Entity) {
    let se = app.world().get::<SettlementEconomy>(s).unwrap().clone();
    for store in std::iter::once(se.storehouse).chain(se.homes.iter().copied()) {
        if let Some(mut stock) = app.world_mut().get_mut::<Stockpile>(store) {
            stock.goods.clear();
        }
    }
    sim_headless::cut_off_farmers(app, s);
}

/// A passable tile `distance` tiles from `center`, in the direction (of
/// 16) that is farthest from every guard camp's monument and at least
/// `clear` tiles from each of `avoid`.
fn standby(app: &mut App, center: IVec2, distance: f32, avoid: &[IVec2], clear: f32) -> IVec2 {
    let camps: Vec<IVec2> = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<&Position, (With<Captain>, With<Guard>)>();
        q.iter(world).map(|p| p.0).collect()
    };
    (0..16)
        .filter_map(|i| {
            let a = i as f32 * std::f32::consts::TAU / 16.0;
            let want = center
                + (bevy_math::Vec2::from_angle(a) * distance)
                    .round()
                    .as_ivec2();
            let at = passable_near(app, want)?;
            let d = (at - center).as_vec2().length();
            let ok = (d - distance).abs() <= 2.0
                && avoid.iter().all(|o| (*o - at).as_vec2().length() >= clear);
            let camp = camps
                .iter()
                .map(|c| (*c - at).as_vec2().length())
                .fold(f32::INFINITY, f32::min);
            ok.then_some((camp, i, at))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, _, at)| at)
        .expect("a standby spot")
}

fn share(app: &App, region: u8) -> (f32, u8) {
    let t = app.world().resource::<RunProgress>().dominion[region as usize];
    (t.share(), t.flags)
}

/// GP-24, GP-25: see the module docs.
#[test]
fn dominion_boons_come_once_per_threshold() {
    let mut app = run_app(SEED);
    let data = app.world().resource::<Progression>().data.clone();
    let (low, high) = (data.dominion.thresholds[0], data.dominion.thresholds[1]);
    let progress = app.world().resource::<RunProgress>().clone();
    let all = places(&mut app);
    // The start region's settlements (never the rival's seat): `a` stays
    // held, `d` is held then lost, `c` is won last. a < 25% <= a + d <
    // 50% <= a + d + c, each with a margin. First fit in SpawnId order.
    let region = progress.start_region;
    let local: Vec<Place> = all
        .iter()
        .copied()
        .filter(|p| p.region == region && Some(p.id) != progress.rival_seat)
        .collect();
    let total: u32 = all
        .iter()
        .filter(|p| p.region == region)
        .map(|p| p.people)
        .sum();
    let frac = |n: u32| n as f32 / total as f32;
    let mut pick = None;
    'search: for mask in 1u32..(1 << local.len()) {
        let a: u32 = (0..local.len())
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| local[i].people)
            .sum();
        for (di, d) in local.iter().enumerate() {
            for (ci, c) in local.iter().enumerate() {
                if mask & (1 << di) != 0 || mask & (1 << ci) != 0 || di == ci {
                    continue;
                }
                let (fa, fad) = (frac(a), frac(a + d.people));
                if fa <= low - MARGIN
                    && fad >= low + MARGIN
                    && fad <= high - MARGIN
                    && frac(a + d.people + c.people) >= high + MARGIN
                {
                    pick = Some((mask, *d, *c));
                    break 'search;
                }
            }
        }
    }
    let (mask, d, c) = pick.expect("a region split that fits the thresholds");
    let held: Vec<Place> = (0..local.len())
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| local[i])
        .collect();
    let a_people: u32 = held.iter().map(|p| p.people).sum();
    println!(
        "seed {SEED} region {region}: {total} people; held {:?} ({a_people}, {:.1}%), lost and regained #{} ({}, {:.1}%), won last #{} ({}, {:.1}%)",
        held.iter().map(|p| p.id).collect::<Vec<_>>(),
        100.0 * frac(a_people),
        d.id,
        d.people,
        100.0 * frac(d.people),
        c.id,
        c.people,
        100.0 * frac(c.people)
    );

    // Setup before the first tick: the held settlements (loyal and fed),
    // the doomed one (loyalty 0 and starved, so it revolts at the first
    // day tick), goodwill toward the player where invitations must come,
    // and a 2-man warband standing by beyond the doomed settlement's
    // garrison radius but within the invitation radius.
    for p in &held {
        give_to_player(&mut app, p.e, 0.8);
    }
    give_to_player(&mut app, d.e, 0.0);
    starve(&mut app, d.e);
    for p in [d, c] {
        app.world_mut()
            .get_mut::<sim_agents::Goodwill>(p.e)
            .unwrap()
            .set(FactionId::PLAYER, 0.9);
    }
    let garrison = app
        .world()
        .resource::<sim_military::Army>()
        .data
        .garrison
        .radius;
    let invite = data.invitations.radius;
    let wait = (garrison + invite) / 2.0;
    let start = progress.start;
    let others: Vec<IVec2> = all
        .iter()
        .filter(|p| p.id != d.id)
        .map(|p| p.center)
        .chain([start])
        .collect();
    let spot = standby(&mut app, d.center, wait, &others, invite + 5.0);
    spawn_army(
        app.world_mut(),
        &ArmySpec {
            food_days: 6.0,
            ..ArmySpec::new(FactionId::PLAYER, spot, 2)
        },
    );
    let champion = champion(&mut app);
    assert!(
        (c.center - start).as_vec2().length() > invite + 5.0,
        "the champion starts out of the town's reach"
    );

    // Tick 0: pick a start boon with food (the band has a long day ahead)
    // and see the first dominion offer.
    let start_offer = start_offer(&app);
    let cards = app
        .world()
        .resource::<Offers>()
        .get(start_offer)
        .unwrap()
        .choices
        .clone();
    let food = cards
        .iter()
        .position(|&c| {
            data.boons[c as usize]
                .effects
                .iter()
                .any(|e| matches!(e, Effect::FoodDays(_)))
        })
        .unwrap_or(0);
    choose(&mut app, start_offer, food as u8);
    app.step(1);
    let (s, flags) = share(&app, region);
    let offers = dominion_offers(&app);
    println!(
        "tick 0: share {:.1}% flags {flags:02b}, offers {:?}, slots {} (captains {})",
        100.0 * s,
        offers
            .iter()
            .map(|o| (o.id, o.kind, o.choices.clone()))
            .collect::<Vec<_>>(),
        slots(&mut app),
        captains(&mut app)
    );
    assert!(s >= low && s < high, "between the thresholds: {s}");
    assert_eq!(flags, 0b01);
    assert_eq!(offers.len(), 1, "exactly one offer");
    let first = offers[0].clone();
    check_offer(&app, &first, region, 0);
    let pending = app.world().resource::<Offers>().pending.len();
    assert_eq!(
        pending, 1,
        "the start boon is taken; only the dominion offer waits"
    );

    // GP-25: one slot (the first village), filled by the warband: an
    // appointment beyond it is refused.
    assert_eq!(slots(&mut app), expected_slots(&mut app));
    assert_slots_stored(&mut app);
    assert_eq!(captains(&mut app), 2, "the champion and the warband");
    let slots0 = slots(&mut app);
    assert_eq!(slots0, 1, "held {a_people} + {} people", d.people);
    assert!(appoint(&mut app, "no captain slot"), "beyond the slots");
    assert_eq!(captains(&mut app), 2);

    // Pick: a slot boon if offered (it counts at once), else the first;
    // a stat boon shows in the modifiers at the next hourly rebuild.
    let pick = first
        .choices
        .iter()
        .position(|&c| {
            data.boons[c as usize]
                .effects
                .iter()
                .any(|e| matches!(e, Effect::CaptainSlots(_)))
        })
        .unwrap_or(0);
    let card = first.choices[pick];
    let boon = data.boons[card as usize].clone();
    let mods_before = app.world().resource::<FactionModifiers>().clone();
    choose(&mut app, first.id, pick as u8);
    app.step(1);
    assert_eq!(
        app.world().resource::<RunProgress>().boons.last(),
        Some(&card)
    );
    assert!(dominion_offers(&app).is_empty(), "the offer is taken");
    let slot_boon: u32 = boon
        .effects
        .iter()
        .map(|e| match e {
            Effect::CaptainSlots(n) => *n as u32,
            _ => 0,
        })
        .sum();
    assert_eq!(slots(&mut app), (slots0 + slot_boon).min(data.dominion.cap));
    assert_slots_stored(&mut app);
    if slot_boon > 0 {
        assert!(!appoint(&mut app, ""), "the boon's slot takes a captain");
        assert_eq!(captains(&mut app), 3);
    }
    step_hour(&mut app);
    let mods = app.world().resource::<FactionModifiers>().clone();
    for e in &boon.effects {
        if let Effect::Stat(stat, v) = *e {
            let gain =
                mods.value(FactionId::PLAYER, stat) - mods_before.value(FactionId::PLAYER, stat);
            assert!(
                (gain - v).abs() < 1e-6,
                "{}: {stat:?} {gain} not {v}",
                boon.name
            );
        }
    }
    println!(
        "picked {:?} at tick {}: slots {} captains {}",
        boon.name,
        tick(&app),
        slots(&mut app),
        captains(&mut app)
    );

    // A day later the starved settlement revolts: the share falls below
    // 25%. The warband standing by is invited back in at once.
    let tpd = app.world().resource::<sim_core::Calendar>().ticks_per_day;
    let ids_before = next_offer_id(&app);
    app.step(tpd + 1 - tick(&app));
    assert_eq!(owner_of(&app, d.e), FactionId::INDEPENDENT, "revolted");
    let (s, flags) = share(&app, region);
    println!(
        "tick {}: #{} revolted; share {:.1}% flags {flags:02b}, slots {}",
        tick(&app) - 1,
        d.id,
        100.0 * s,
        slots(&mut app)
    );
    assert!(s < low, "below 25% again: {s}");
    assert_eq!(flags, 0b01, "flags never reset");
    assert_eq!(slots(&mut app), expected_slots(&mut app));
    let invitation = app
        .world()
        .resource::<Offers>()
        .pending
        .iter()
        .find(|o| o.kind == OfferKind::Invitation { settlement: d.id })
        .cloned()
        .expect("the lost settlement invites the warband back");
    choose(&mut app, invitation.id, 0);
    app.step(1);
    assert_eq!(owner_of(&app, d.e), FactionId::PLAYER, "regained");
    step_hour(&mut app);
    step_hour(&mut app);
    let (s, flags) = share(&app, region);
    println!(
        "tick {}: regained #{}; share {:.1}% flags {flags:02b}, dominion offers {}",
        tick(&app),
        d.id,
        100.0 * s,
        dominion_offers(&app).len()
    );
    assert!(s >= low && s < high, "25% re-crossed: {s}");
    assert!(dominion_offers(&app).is_empty(), "no second offer for 25%");
    assert_eq!(
        next_offer_id(&app),
        ids_before + 1,
        "only the invitation was posted"
    );
    assert_eq!(slots(&mut app), expected_slots(&mut app));
    assert_slots_stored(&mut app);

    // The champion's band walks to the town and is invited in: past 50%.
    let toward = (start - c.center).as_vec2().normalize_or_zero();
    let near = passable_near(&app, c.center + (toward * 20.0).round().as_ivec2()).unwrap();
    order(
        &mut app,
        1,
        champion,
        OrderSpec::MoveTo {
            x: near.x,
            y: near.y,
        },
    );
    let mut town = None;
    for _ in 0..20 {
        step_hour(&mut app);
        town = app
            .world()
            .resource::<Offers>()
            .pending
            .iter()
            .find(|o| o.kind == OfferKind::Invitation { settlement: c.id })
            .cloned();
        if town.is_some() {
            break;
        }
    }
    let town = town.expect("the town invites the champion in");
    assert!(dominion_offers(&app).is_empty(), "nothing before the town");
    choose(&mut app, town.id, 0);
    app.step(1);
    assert_eq!(owner_of(&app, c.e), FactionId::PLAYER);
    step_hour(&mut app);
    let (s, flags) = share(&app, region);
    let offers = dominion_offers(&app);
    println!(
        "tick {}: #{} invited the champion in; share {:.1}% flags {flags:02b}, offers {:?}, slots {} expected {}",
        tick(&app),
        c.id,
        100.0 * s,
        offers
            .iter()
            .map(|o| (o.id, o.kind, o.choices.clone()))
            .collect::<Vec<_>>(),
        slots(&mut app),
        expected_slots(&mut app)
    );
    assert!(s >= high, "past 50%: {s}");
    assert_eq!(flags, 0b11);
    assert_eq!(offers.len(), 1, "the second offer");
    check_offer(&app, &offers[0], region, 1);
    assert_eq!(slots(&mut app), expected_slots(&mut app));
    assert_slots_stored(&mut app);
    // Nothing more comes while the land stays held.
    for _ in 0..3 {
        step_hour(&mut app);
    }
    assert_eq!(dominion_offers(&app).len(), 1);
    assert_eq!(app.world().resource::<RunProgress>().dominion.len(), 4);
}

/// GP-25: slots follow the population held (one per `per_captain`, at
/// least `first_village_slots` once a settlement is held), appointments
/// fill them and one more is refused; a huge realm is capped at 8, where
/// army.ron `max_captains` (the champion counts) refuses the ninth
/// appointment. GP-24: the tick-0 tally offers each region's two boons
/// once.
#[test]
fn captain_slots_follow_the_land_held() {
    // About 600 people: the smallest settlements first until 600.
    let mut app = run_app(SEED);
    let data = app.world().resource::<Progression>().data.clone();
    let seat = app.world().resource::<RunProgress>().rival_seat;
    let mut all = places(&mut app);
    all.retain(|p| Some(p.id) != seat);
    all.sort_unstable_by_key(|p| (p.people, p.id));
    let mut held = 0;
    for p in &all {
        if held >= 2 * data.dominion.per_captain {
            break;
        }
        give_to_player(&mut app, p.e, 0.8);
        held += p.people;
    }
    assert_eq!(slots(&mut app), expected_slots(&mut app));
    let want = held / data.dominion.per_captain;
    assert_eq!(slots(&mut app), want, "{held} people");
    app.step(1);
    println!(
        "held {held} people: {} slots, stored {}",
        slots(&mut app),
        app.world().resource::<RunProgress>().captain_slots
    );
    assert_eq!(app.world().resource::<RunProgress>().captain_slots, want);
    for n in 0..want {
        assert!(!appoint(&mut app, ""), "appointment {} of {want}", n + 1);
    }
    assert_eq!(captains(&mut app), want + 1, "the champion and {want}");
    assert!(appoint(&mut app, "no captain slot"));
    assert_eq!(captains(&mut app), want + 1);

    // Every settlement but the seat: far past 8 slots by population.
    let mut app = run_app(SEED);
    let max = app
        .world()
        .resource::<sim_military::Army>()
        .data
        .max_captains;
    let mut people = 0;
    for p in places(&mut app) {
        if Some(p.id) != seat {
            give_to_player(&mut app, p.e, 0.8);
            people += p.people;
        }
    }
    let by_people = people / data.dominion.per_captain;
    assert!(by_people > data.dominion.cap, "{people} people");
    assert_eq!(slots(&mut app), data.dominion.cap, "capped");
    let next = next_offer_id(&app);
    app.step(1);
    let offers = dominion_offers(&app);
    let regions = app.world().resource::<RunProgress>().dominion.clone();
    println!(
        "held {people} people: {by_people} slots by population, {} capped; tallies {:?}; offers {:?}",
        slots(&mut app),
        regions,
        offers.iter().map(|o| (o.id, o.kind)).collect::<Vec<_>>()
    );
    assert_eq!(
        app.world().resource::<RunProgress>().captain_slots,
        data.dominion.cap
    );
    let mut kinds = Vec::new();
    for (r, t) in regions.iter().enumerate() {
        for (step, &th) in data.dominion.thresholds.iter().enumerate() {
            if t.held_by_player as f32 >= th * t.total as f32 {
                kinds.push(OfferKind::DominionBoon {
                    region: r as u8,
                    step: step as u8,
                });
            }
        }
    }
    assert_eq!(
        offers.iter().map(|o| o.kind).collect::<Vec<_>>(),
        kinds,
        "one offer per region and threshold reached, regions then steps"
    );
    assert_eq!(next_offer_id(&app), next + kinds.len() as u64);
    for o in &offers {
        let OfferKind::DominionBoon { region, step } = o.kind else {
            unreachable!()
        };
        check_offer(&app, o, region, step);
    }
    app.step(300);
    assert_eq!(dominion_offers(&app).len(), kinds.len(), "posted once");
    for n in 0..data.dominion.cap {
        assert!(!appoint(&mut app, ""), "appointment {} of 8", n + 1);
    }
    assert_eq!(captains(&mut app), max, "army.ron max_captains");
    assert!(appoint(&mut app, "too many captains"));
}
