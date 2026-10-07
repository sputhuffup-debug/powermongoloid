//! M4 step 1, test `run_start` (GP-21, GP-27): a run starts from its seed.
//! The island, the rival's seat, the start spot, the band, the champion's
//! trait, the run modifiers and the three starting boons all come from the
//! seed. The same seed gives the same start; different seeds visibly differ.
//! The start boon is picked once, and abandoning ends the run and stops the
//! clock.

mod common;

use std::collections::BTreeSet;

use bevy_ecs::prelude::*;
use common::*;
use sim_agents::Villagers;
use sim_core::{
    Calendar, Cause, CommandKind, FactionId, OfferKind, Offers, RunOutcome, RunState, SimEventKind,
    SimSeed, SimStepper, SimTick, compute_state_hash,
};
use sim_economy::{Economy, Stockpile};
use sim_military::progression::{BoonWhen, Effect};
use sim_military::{Captain, Equipment, Follower, Progression, RunProgress, RunWorld, Traits};
use sim_world::worldgen::default_threads;
use sim_world::{Settlement, Terrain, WorldGenSettings, WorldPreset, find_path, generate_world};

/// GP-21, GP-23, GP-27, world level (no App, no population): for seeds
/// 1..=20 the run island has 4 regions, every settlement a drawn size, 4-8
/// monuments per region on allowed ground away from settlements, and a
/// start that is seeded, away from every settlement, in another region than
/// the rival's seat (the most populous settlement) with a walkable path to
/// it, 2-3 monuments within `monument_radius` (no more than a day's march),
/// and three distinct start boons. Across the seeds the start terrain, the
/// band and the run modifiers each take at least three values.
#[test]
fn twenty_seeds_draw_valid_varied_starts() {
    let data = Progression::default_data();
    let s = &data.data.start;
    let villagers = Villagers::default_data(&Calendar::default_data());
    let costs = &villagers.travel_costs;
    let day_march =
        villagers.data.movement.speed.grass * Calendar::default_data().ticks_per_day as f32;
    assert!(
        s.monument_radius <= day_march,
        "nearby monuments are within a day's march ({day_march} tiles)"
    );
    let settings = WorldGenSettings::preset(WorldPreset::Run);
    let (mut terrains, mut bands, mut modifier_sets) =
        (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for seed in 1..=20u64 {
        let g = generate_world(seed, &settings, default_threads());
        let map = &g.map;

        // The island: 4 regions, every settlement populated at a drawn size.
        assert_eq!(map.region_count(), 4, "seed {seed}");
        let regions: BTreeSet<u8> = g.settlements.iter().map(|x| x.region).collect();
        assert_eq!(
            regions.len(),
            4,
            "seed {seed}: settlement regions {regions:?}"
        );
        let people: Vec<u32> = g
            .settlements
            .iter()
            .map(|x| x.size.map_or(0, |z| z.people))
            .collect();
        assert!(
            people.iter().all(|&p| p > 0),
            "seed {seed}: every settlement is populated: {people:?}"
        );

        // Monuments: 4-8 per region, never on water, road or mountain, and
        // at least `min_settlement_distance` from every centre.
        let mut per_region = vec![0u32; map.region_count()];
        for m in &g.monuments {
            per_region[m.region as usize] += 1;
            let t = map.terrain(m.center);
            assert!(
                !matches!(t, Terrain::Water | Terrain::Road | Terrain::Mountain),
                "seed {seed}: {} sits on {}",
                m.name,
                t.name()
            );
            for x in &g.settlements {
                let d = m.center.as_vec2().distance(x.center.as_vec2());
                assert!(
                    d >= settings.monuments.min_settlement_distance - 1e-3,
                    "seed {seed}: {} is {d:.0} tiles from {}",
                    m.name,
                    x.name
                );
            }
        }
        assert!(
            per_region.iter().all(|n| (4..=8).contains(n)),
            "seed {seed}: monuments per region {per_region:?}"
        );

        // The start draws: seeded, and the same on a second draw.
        let w = RunWorld::from_generated(&g, costs);
        let a = sim_military::draw_start(SimSeed(seed), &data, &w).expect("a start");
        assert_eq!(
            sim_military::draw_start(SimSeed(seed), &data, &w).as_ref(),
            Some(&a),
            "seed {seed}: same seed, same start"
        );

        // The rival's seat: the unique most populous settlement.
        let seat = a.seat as usize;
        assert!(
            people
                .iter()
                .enumerate()
                .all(|(i, &p)| i == seat || p < people[seat]),
            "seed {seed}: the seat is the largest settlement"
        );
        assert_eq!(a.seat_center, g.settlements[seat].center);
        assert_eq!(a.rival_region, g.settlements[seat].region);
        assert_ne!(a.start_region, a.rival_region, "seed {seed}");

        // The start spot: open ground in the start region, away from every
        // settlement, with 2-3 monuments nearby.
        let t = map.terrain(a.start);
        assert!(
            matches!(
                t,
                Terrain::Grass | Terrain::Beach | Terrain::Forest | Terrain::Hills
            ),
            "seed {seed}: the start is on {}",
            t.name()
        );
        assert_eq!(map.region(a.start), a.start_region, "seed {seed}");
        for x in &g.settlements {
            let d = a.start.as_vec2().distance(x.center.as_vec2());
            assert!(
                d >= s.min_settlement_distance,
                "seed {seed}: the start is {d:.0} tiles from {}",
                x.name
            );
        }
        let r2 = s.monument_radius * s.monument_radius;
        let nearby: Vec<u64> = w
            .monuments
            .iter()
            .filter(|m| m.center.as_vec2().distance_squared(a.start.as_vec2()) <= r2)
            .map(|m| m.id)
            .collect();
        assert_eq!(nearby, a.nearby_monuments, "seed {seed}");
        assert!(
            (s.min_monuments..=s.max_monuments).contains(&(nearby.len() as u32)),
            "seed {seed}: {} monuments within {} tiles of the start",
            nearby.len(),
            s.monument_radius
        );

        // The band can walk to the rival's seat.
        let path = find_path(
            map.terrain_slice(),
            map.height_slice(),
            map.size() as usize,
            a.start,
            a.seat_center,
            costs,
            None,
        );
        assert!(
            path.is_some(),
            "seed {seed}: no path from the start to the seat"
        );

        // Three distinct start boons, a valid band, trait and modifiers (no
        // two of one group).
        let boons: BTreeSet<u16> = a.boon_choices.into_iter().collect();
        assert_eq!(boons.len(), 3, "seed {seed}: {:?}", a.boon_choices);
        for c in a.boon_choices {
            assert_eq!(data.boon(c).expect("a card").when, BoonWhen::Start);
        }
        assert!((a.band as usize) < data.data.bands.len());
        assert!(data.data.champion_traits.contains(&a.champion_trait));
        let (lo, hi) = data.data.modifiers_per_run;
        assert!(
            (lo as usize..=hi as usize).contains(&a.modifiers.len()),
            "seed {seed}: {:?}",
            a.modifiers
        );
        let groups: Vec<&str> = a
            .modifiers
            .iter()
            .filter_map(|&m| data.modifier(m).expect("a card").group.as_deref())
            .collect();
        let distinct: BTreeSet<&str> = groups.iter().copied().collect();
        assert_eq!(distinct.len(), groups.len(), "seed {seed}: {groups:?}");

        println!(
            "seed {seed:>2}: seat #{} ({} people, r{}), start {:?} r{} {} ({:.0} tiles away), {} monuments near, band {:?}, trait {:?}, modifiers {:?}, boons {:?}",
            a.seat,
            people[seat],
            a.rival_region,
            a.start,
            a.start_region,
            a.terrain.name(),
            a.start.as_vec2().distance(a.seat_center.as_vec2()),
            nearby.len(),
            data.data.bands[a.band as usize].name,
            data.trait_data(a.champion_trait).map(|t| t.name.as_str()),
            a.modifiers
                .iter()
                .map(|&m| data.modifier(m).map_or("?", |c| c.name.as_str()))
                .collect::<Vec<_>>(),
            a.boon_choices
                .iter()
                .map(|&b| data.boon(b).map_or("?", |c| c.name.as_str()))
                .collect::<Vec<_>>(),
        );
        terrains.insert(a.terrain);
        bands.insert(a.band);
        let mut set = a.modifiers.clone();
        set.sort_unstable();
        modifier_sets.insert(set);
    }
    println!(
        "{} start terrains, {} bands, {} modifier sets",
        terrains.len(),
        bands.len(),
        modifier_sets.len()
    );
    assert!(terrains.len() >= 3, "start terrains {terrains:?}");
    assert!(bands.len() >= 3, "bands {bands:?}");
    assert!(modifier_sets.len() >= 3, "modifier sets {modifier_sets:?}");
}

/// GP-21: two apps of one seed are set up identically and stay identical
/// for 200 ticks. Every settlement is populated, the champion leads the
/// drawn band, and one start boon offer of three distinct cards waits.
#[test]
fn a_run_starts_identically_from_its_seed() {
    const SEED: u64 = 7;
    let mut a = run_app(SEED);
    let mut b = run_app(SEED);
    let setup = compute_state_hash(a.world_mut());
    assert_eq!(
        setup,
        compute_state_hash(b.world_mut()),
        "same seed, same setup"
    );

    // Every settlement is populated.
    let settlements = {
        let world = a.world_mut();
        let mut q = world.query_filtered::<(), With<Settlement>>();
        q.iter(world).count()
    };
    let populated = villages(&mut a);
    let people: u32 = populated.iter().map(|v| v.3).sum();
    println!(
        "seed {SEED}: {settlements} settlements, {} populated, {people} people",
        populated.len()
    );
    let range = WorldGenSettings::preset(WorldPreset::Run).settlements;
    assert!((range.min as usize..=range.max as usize).contains(&settlements));
    assert_eq!(
        populated.len(),
        settlements,
        "every settlement is populated"
    );

    // A run, not over, with the rival's seat drawn.
    let state = *a.world().resource::<RunState>();
    assert!(state.config.is_some() && state.outcome.is_none());
    let progress = a.world().resource::<RunProgress>().clone();
    let seat = progress.rival_seat.expect("a rival seat");
    assert!(populated.iter().any(|v| v.1 == seat));

    // One pending offer: the player's start boon, three distinct cards.
    let offers = a.world().resource::<Offers>().pending.clone();
    assert_eq!(offers.len(), 1, "{offers:?}");
    let offer = &offers[0];
    assert_eq!(offer.id, start_offer(&a));
    assert_eq!(
        (offer.kind, offer.faction),
        (OfferKind::StartBoon, FactionId::PLAYER)
    );
    let cards: BTreeSet<u16> = offer.choices.iter().copied().collect();
    assert_eq!((offer.choices.len(), cards.len()), (3, 3));

    // The champion: the player's captain, with one trait, leading the band.
    let c = champion(&mut a);
    let world = a.world_mut();
    assert_eq!(
        world.get::<Captain>(c).map(|k| k.faction),
        Some(FactionId::PLAYER)
    );
    assert_eq!(world.get::<Traits>(c).map(|t| t.0.len()), Some(1));
    let mut q = world.query::<&Follower>();
    let followers = q.iter(world).filter(|f| f.captain == c).count() as u32;
    let data = world.resource::<Progression>();
    assert_eq!(
        followers,
        data.data.bands[progress.band as usize].followers()
    );

    a.step(200);
    b.step(200);
    assert_eq!(a.world().resource::<SimTick>().0, 200);
    let after = compute_state_hash(a.world_mut());
    assert_ne!(after, setup, "the run advances");
    assert_eq!(
        after,
        compute_state_hash(b.world_mut()),
        "same seed, same run"
    );
    let events = chronicle(&a);
    assert_eq!(
        events.first().map(|e| (e.tick, e.event.kind)),
        Some((0, SimEventKind::RunStarted))
    );
}

/// GP-27: the start boon applies once. A second `ChooseOffer` for the same
/// offer (a double click) is refused and counted, and nothing applies
/// twice.
#[test]
fn choosing_the_start_boon_applies_once() {
    let mut app = run_app(42);
    let offer = start_offer(&app);
    let data = Progression::default_data();
    let choices = app
        .world()
        .resource::<Offers>()
        .get(offer)
        .expect("the start offer")
        .choices
        .clone();
    // Prefer a boon whose effect shows in the state: food, goodwill, arms.
    let shows = |card: u16| {
        let effects = &data.boon(card).expect("a card").effects;
        if effects.iter().any(|e| matches!(e, Effect::FoodDays(_))) {
            0
        } else if effects.iter().any(|e| matches!(e, Effect::Goodwill(_))) {
            1
        } else if effects.contains(&Effect::ArmBand) {
            2
        } else {
            3
        }
    };
    let choice = (0..choices.len())
        .min_by_key(|&i| (shows(choices[i]), i))
        .expect("three choices") as u8;
    let card = choices[choice as usize];
    let boon = data.boon(card).expect("a card").clone();
    println!("picking {:?} ({}) of {choices:?}", boon.name, boon.text);

    let c = champion(&mut app);
    let econ = app.world().resource::<Economy>().clone();
    let food = |app: &App| {
        app.world().get::<Stockpile>(c).map_or(0.0, |s| {
            s.goods
                .iter()
                .map(|(g, q)| *q as f32 * econ.food_value(*g))
                .sum::<f32>()
        })
    };
    let members = {
        let world = app.world_mut();
        let mut q = world.query::<&Follower>();
        1 + q.iter(world).filter(|f| f.captain == c).count()
    };
    let armed = |app: &mut App| {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &Equipment, Option<&Follower>)>();
        q.iter(world)
            .filter(|(e, _, f)| *e == c || f.is_some_and(|f| f.captain == c))
            .filter(|(_, eq, _)| eq.weapon)
            .count()
    };
    let food_before = food(&app);
    let rejected_before = rejected(&app, FactionId::PLAYER);

    choose(&mut app, offer, choice);
    app.step(1);
    let progress = app.world().resource::<RunProgress>().clone();
    let food_after = food(&app);
    let armed_after = armed(&mut app);
    assert_eq!(progress.boons, vec![card], "the pick is recorded");
    assert!(app.world().resource::<Offers>().pending.is_empty());
    assert_eq!(rejected(&app, FactionId::PLAYER), rejected_before);
    let hunger = Villagers::default_data(&Calendar::default_data())
        .data
        .needs
        .hunger_per_day;
    for e in &boon.effects {
        match *e {
            Effect::FoodDays(days) => {
                let need = hunger * members as f32 * days;
                println!("food {food_before:.1} -> {food_after:.1} (need {need:.1})");
                assert!(
                    food_after - food_before >= need - 1e-3,
                    "{days} more days of food for {members}"
                );
            }
            Effect::Goodwill(g) => assert_eq!(
                progress
                    .pending_goodwill
                    .iter()
                    .map(|x| x.1)
                    .collect::<Vec<_>>(),
                vec![g]
            ),
            Effect::ArmBand => assert_eq!(armed_after, members, "everyone is armed"),
            _ => {}
        }
    }

    // A double click: refused, counted, and nothing applies twice.
    choose(&mut app, offer, choice);
    app.step(1);
    assert_eq!(
        rejected(&app, FactionId::PLAYER),
        rejected_before + 1,
        "the second pick is refused"
    );
    let again = app.world().resource::<RunProgress>().clone();
    assert_eq!(again, progress, "nothing applies twice");
    assert_eq!(food(&app), food_after);
    assert_eq!(armed(&mut app), armed_after);
    let chosen: Vec<_> = chronicle(&app)
        .into_iter()
        .filter(|e| e.event.kind == SimEventKind::OfferChosen)
        .collect();
    assert_eq!(chosen.len(), 1, "{chosen:?}");
    let e = chosen[0].event;
    assert_eq!(
        (e.faction, e.subject, e.other, e.amount, e.cause),
        (
            FactionId::PLAYER,
            offer,
            card as u64,
            choice as i32,
            Cause::Player
        )
    );
}

/// GP-21: only the player can abandon. Abandoning ends the run at the end
/// of that tick, records the outcome and `RunEnded`, and stops the clock:
/// later ticks change nothing, and later commands are not applied.
#[test]
fn abandoning_ends_the_run_and_stops_the_clock() {
    let mut app = run_app(42);
    let offer = start_offer(&app);
    app.step(30);

    command(&mut app, FactionId::RAIDERS.0, CommandKind::AbandonRun);
    app.step(1);
    assert_eq!(
        rejected(&app, FactionId::RAIDERS),
        1,
        "not the raiders' run"
    );
    assert_eq!(app.world().resource::<RunState>().outcome, None);

    let tick = app.world().resource::<SimTick>().0;
    command(&mut app, FactionId::PLAYER.0, CommandKind::AbandonRun);
    app.step(1);
    let state = *app.world().resource::<RunState>();
    assert_eq!(state.outcome, Some(RunOutcome::Abandoned));
    assert_eq!(state.cause, Cause::Player);
    assert_eq!(
        state.ended_at,
        Some(tick),
        "it ends on the tick it is asked"
    );
    assert_eq!(state.pending, None);
    let end = *chronicle(&app).last().expect("a chronicle");
    assert_eq!(
        (end.tick, end.event.kind, end.event.subject, end.event.cause),
        (
            tick,
            SimEventKind::RunEnded,
            RunOutcome::Abandoned as u64,
            Cause::Player
        )
    );

    // The clock has stopped: the ending tick completed, nothing after it.
    let stopped = app.world().resource::<SimTick>().0;
    assert_eq!(stopped, tick + 1);
    let hash = compute_state_hash(app.world_mut());
    choose(&mut app, offer, 0);
    command(&mut app, FactionId::PLAYER.0, CommandKind::AbandonRun);
    app.step(50);
    assert_eq!(
        app.world().resource::<SimTick>().0,
        stopped,
        "the clock stopped"
    );
    assert_eq!(compute_state_hash(app.world_mut()), hash, "nothing changes");
    assert_eq!(start_offer(&app), offer, "commands are no longer applied");
    assert_eq!(chronicle(&app).last(), Some(&end));
}
