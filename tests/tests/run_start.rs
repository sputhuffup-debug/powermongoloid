//! M4 step 1, test `run_start` (GP-21, GP-27): a run starts from its seed.
//! The island, the rival's seat, the start spot, the band, the champion's
//! trait, the run modifiers and the three starting boons all come from the
//! seed. The same seed gives the same start; different seeds visibly differ.
//! The start boon is picked once, and abandoning ends the run and stops the
//! clock.

mod common;

use std::collections::BTreeSet;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use common::*;
use sim_agents::{CombatStats, Villagers};
use sim_core::{
    Calendar, Cause, CommandKind, FactionId, OfferKind, Offers, Position, RunOutcome, RunState,
    SimEventKind, SimSeed, SimStepper, SimTick, SpawnId, compute_state_hash,
};
use sim_economy::{Economy, SettlementEconomy, Stockpile};
use sim_military::progression::{BandData, BoonWhen, Effect};
use sim_military::{
    Army, Captain, Equipment, Factions, Follower, Progression, RunProgress, RunWorld, StartTerrain,
    Traits,
};
use sim_world::worldgen::default_threads;
use sim_world::{Settlement, Terrain, WorldGenSettings, WorldPreset, find_path, generate_world};

/// GP-21, GP-23, GP-27, world level (no App, no population): for seeds
/// 1..=20 the run island has 4 regions, every settlement a drawn size, 4-8
/// monuments per region on allowed ground away from settlements and
/// `min_spacing` apart, and a start that is seeded, away from every
/// settlement, in another region than the rival's seat (the most populous
/// settlement) with a walkable path to it, 2-3 monuments within
/// `monument_radius` (no more than a day's march), a terrain label its tile
/// actually has, and three distinct start boons. Across the seeds the start
/// terrain, the band and the run modifiers each take at least three values.
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

        // Monuments: 4-8 per region, never on water, road or mountain, at
        // least `min_settlement_distance` from every centre and (no spacing
        // was relaxed on these seeds, ADR 0013) `min_spacing` apart.
        let mut per_region = vec![0u32; map.region_count()];
        for (i, m) in g.monuments.iter().enumerate() {
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
            for b in &g.monuments[i + 1..] {
                let d = m.center.as_vec2().distance(b.center.as_vec2());
                assert!(
                    d >= settings.monuments.min_spacing - 1e-3,
                    "seed {seed}: {} and {} are {d:.0} tiles apart",
                    m.name,
                    b.name
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
        assert_eq!(
            a.seat_center, g.settlements[seat].center,
            "seed {seed}: seat #{seat}"
        );
        assert_eq!(
            a.rival_region, g.settlements[seat].region,
            "seed {seed}: seat #{seat}"
        );
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

        // Its label is a kind the tile has (the same tests `draw_start`
        // makes; a tile can have several, so membership, not equality).
        let at = |d: IVec2| {
            let q = a.start + d;
            map.in_bounds(q).then(|| map.terrain(q))
        };
        let coast = t == Terrain::Beach
            || (-2..=2).any(|dx| (-2..=2).any(|dy| at(IVec2::new(dx, dy)) == Some(Terrain::Water)));
        let n4 = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];
        let forest_edge = (t == Terrain::Forest
            && n4.iter().any(|&d| {
                matches!(
                    at(d),
                    Some(Terrain::Grass | Terrain::Beach | Terrain::Hills | Terrain::Farmland)
                )
            }))
            || (t == Terrain::Grass && n4.iter().any(|&d| at(d) == Some(Terrain::Forest)));
        let hills = t == Terrain::Hills;
        let fits = match a.terrain {
            StartTerrain::Coast => coast,
            StartTerrain::ForestEdge => forest_edge,
            StartTerrain::Hills => hills,
            StartTerrain::Open => !coast && !forest_edge && !hills,
        };
        assert!(
            fits,
            "seed {seed}: the start {} on {} is labelled {} (coast {coast}, forest edge {forest_edge}, hills {hills})",
            a.start,
            t.name(),
            a.terrain.name()
        );
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
            let card = data
                .boon(c)
                .unwrap_or_else(|| panic!("seed {seed}: no boon card {c}"));
            assert_eq!(
                card.when,
                BoonWhen::Start,
                "seed {seed}: boon {c} ({})",
                card.name
            );
        }
        assert!(
            (a.band as usize) < data.data.bands.len(),
            "seed {seed}: band {} of {}",
            a.band,
            data.data.bands.len()
        );
        assert!(
            data.data.champion_traits.contains(&a.champion_trait),
            "seed {seed}: trait {:?}",
            a.champion_trait
        );
        let (lo, hi) = data.data.modifiers_per_run;
        assert!(
            (lo as usize..=hi as usize).contains(&a.modifiers.len()),
            "seed {seed}: {:?}",
            a.modifiers
        );
        let groups: Vec<&str> = a
            .modifiers
            .iter()
            .filter_map(|&m| {
                data.modifier(m)
                    .unwrap_or_else(|| panic!("seed {seed}: no modifier card {m}"))
                    .group
                    .as_deref()
            })
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

/// GP-27: the band's make-up, before any tick (hunger and upkeep have
/// changed nothing yet). The followers, in SpawnId order, have their
/// group's stats and gear, group after group as the template lists them;
/// the champion has army.ron's captain stats and the first group's gear.
fn assert_band_make_up(app: &mut App, champion: Entity, band: &BandData) {
    let world = app.world_mut();
    let mut q = world.query::<(&SpawnId, &Follower, &CombatStats, &Equipment)>();
    let mut members: Vec<(SpawnId, CombatStats, Equipment)> = q
        .iter(world)
        .filter(|(_, f, ..)| f.captain == champion)
        .map(|(id, _, s, e)| (*id, *s, *e))
        .collect();
    members.sort_unstable_by_key(|m| m.0);
    assert_eq!(members.len(), band.followers() as usize, "{}", band.name);
    let want = band
        .groups
        .iter()
        .flat_map(|g| std::iter::repeat_n(g, g.count as usize));
    for ((id, s, e), g) in members.iter().zip(want) {
        assert_eq!(
            (s.health, s.max_health, s.attack, s.defense, s.morale),
            (
                g.unit.health,
                g.unit.health,
                g.unit.attack,
                g.unit.defense,
                g.unit.morale
            ),
            "{id:?} of {}",
            band.name
        );
        assert_eq!(
            *e,
            Equipment {
                weapon: g.armed,
                tools: g.tools
            },
            "{id:?} of {}",
            band.name
        );
    }
    let captain = world.resource::<Army>().data.captain;
    let s = *world
        .get::<CombatStats>(champion)
        .expect("the champion fights");
    assert_eq!(
        (s.health, s.max_health, s.attack, s.defense, s.morale),
        (
            captain.health,
            captain.health,
            captain.attack,
            captain.defense,
            captain.morale
        ),
        "the champion has the captain's stats"
    );
    assert_eq!(
        world.get::<Equipment>(champion).copied(),
        Some(Equipment {
            weapon: band.groups[0].armed,
            tools: band.groups[0].tools
        })
    );
}

/// GP-21: two apps of one seed are set up identically and stay identical
/// for 200 ticks, and the installed run starts exactly as the pure
/// `draw_start` (validated over 20 seeds above) says on the generated
/// world. Every settlement is populated, the champion leads the drawn band
/// with the drawn trait, and one start boon offer of the drawn cards waits.
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
    assert!(
        (range.min as usize..=range.max as usize).contains(&settlements),
        "seed {SEED}: {settlements} settlements, want {}..={}",
        range.min,
        range.max
    );
    assert_eq!(
        populated.len(),
        settlements,
        "every settlement is populated"
    );

    // A run, not over, with the rival's seat drawn.
    let state = *a.world().resource::<RunState>();
    assert!(
        state.config.is_some() && state.outcome.is_none(),
        "seed {SEED}: {state:?}"
    );
    let progress = a.world().resource::<RunProgress>().clone();
    let seat = progress.rival_seat.expect("a rival seat");
    assert!(
        populated.iter().any(|v| v.1 == seat),
        "seed {SEED}: seat {seat} not among populated {:?}",
        populated.iter().map(|v| v.1).collect::<Vec<_>>()
    );

    // The installed run starts as the pure draw on the generated world
    // (`RunWorld::from_world` reads what `from_generated` read).
    let g = generate_world(
        SEED,
        &WorldGenSettings::preset(WorldPreset::Run),
        default_threads(),
    );
    let costs = Villagers::default_data(&Calendar::default_data()).travel_costs;
    let data = Progression::default_data();
    let d = sim_military::draw_start(SimSeed(SEED), &data, &RunWorld::from_generated(&g, &costs))
        .expect("a start");
    assert_eq!(
        (
            progress.start,
            progress.terrain,
            progress.start_region,
            progress.rival_seat,
            progress.seat_center,
            progress.rival_region,
            progress.band,
            progress.modifiers.clone(),
        ),
        (
            d.start,
            d.terrain,
            d.start_region,
            Some(d.seat),
            d.seat_center,
            d.rival_region,
            d.band,
            d.modifiers.clone(),
        ),
        "seed {SEED}: the installed run starts as the pure draw (GP-21)"
    );
    assert_eq!(
        a.world().resource::<Factions>().seat(FactionId::PLAYER),
        Some(d.start),
        "the player is seated at the start"
    );

    // One pending offer: the player's start boon, the three drawn cards.
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
    assert_eq!(offer.choices, d.boon_choices.to_vec(), "the drawn boons");

    // The champion: the player's captain, with the drawn trait, leading
    // the drawn band (GP-27).
    let c = champion(&mut a);
    let band = data.data.bands[progress.band as usize].clone();
    {
        let world = a.world_mut();
        assert_eq!(
            world.get::<Captain>(c).map(|k| k.faction),
            Some(FactionId::PLAYER)
        );
        assert_eq!(
            world.get::<Traits>(c).map(|t| t.0.clone()),
            Some(vec![d.champion_trait]),
            "the drawn trait"
        );
    }
    assert_band_make_up(&mut a, c, &band);

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
    let first = *events.first().expect("a chronicle");
    let e = first.event;
    assert_eq!(
        (first.tick, e.kind, e.other, e.at, e.amount),
        (0, SimEventKind::RunStarted, d.seat, d.start, d.band as i32),
        "RunStarted names the seat, the start and the band"
    );
}

/// GP-27: each start boon with an effect the state shows (food, arms,
/// goodwill) applies once. The card is put into seed 42's start offer
/// before tick 0 (a test-only edit), picked, and checked; a second
/// `ChooseOffer` for the same offer (a double click) is refused and
/// counted, and nothing applies twice.
#[test]
fn choosing_the_start_boon_applies_once() {
    let data = Progression::default_data();
    // The cards by effect, not by id, so reordering progression.ron can't
    // make this test check nothing.
    let card_with = |what: &str, f: &dyn Fn(&Effect) -> bool| {
        data.data
            .boons
            .iter()
            .find(|b| b.when == BoonWhen::Start && b.effects.iter().any(f))
            .unwrap_or_else(|| panic!("a Start boon with {what}"))
            .id
    };
    let cards = [
        card_with("FoodDays", &|e| matches!(e, Effect::FoodDays(_))),
        card_with("ArmBand", &|e| matches!(e, Effect::ArmBand)),
        card_with("Goodwill", &|e| matches!(e, Effect::Goodwill(_))),
    ];
    for card in cards {
        pick_once(&data, card);
    }
}

/// One pass of `choosing_the_start_boon_applies_once`: a fresh seed-42 run
/// whose start offer's first choice is `card`.
fn pick_once(data: &Progression, card: u16) {
    let mut app = run_app(42);
    let offer = start_offer(&app);
    {
        let mut offers = app.world_mut().resource_mut::<Offers>();
        let o = offers
            .pending
            .iter_mut()
            .find(|o| o.id == offer)
            .expect("the start offer");
        // Keep the three cards distinct.
        match o.choices.iter().position(|&c| c == card) {
            Some(j) => o.choices.swap(0, j),
            None => o.choices[0] = card,
        }
    }
    let choice = 0u8;
    let boon = data.boon(card).expect("a card").clone();
    println!("picking {:?} ({})", boon.name, boon.text);

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
    // The populated settlement nearest the champion (ties: lowest SpawnId).
    let nearest = {
        let world = app.world_mut();
        let at = world.get::<Position>(c).expect("a position").0;
        let mut q = world.query::<(&SpawnId, &Settlement, &SettlementEconomy)>();
        q.iter(world)
            .map(|(id, s, _)| ((s.center - at).length_squared(), id.0))
            .min()
            .map(|(_, id)| id)
            .expect("a populated settlement")
    };
    let food_before = food(&app);
    let armed_before = armed(&mut app);
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
    let mut checked = 0;
    for e in &boon.effects {
        match *e {
            Effect::FoodDays(days) => {
                // Exactly once: at least the need, less than one more unit
                // of the best food (soldiers may eat a little this tick).
                let need = hunger * members as f32 * days;
                let grain = *econ.foods_by_value().last().expect("a food");
                let gain = food_after - food_before;
                println!("food {food_before:.1} -> {food_after:.1} (need {need:.1})");
                assert!(
                    gain >= need - 1e-3 && gain < need + econ.food_value(grain) + 1e-3,
                    "{days} days of food for {members}, applied once (gain {gain:.1}, need {need:.1})"
                );
                checked += 1;
            }
            Effect::Goodwill(g) => {
                assert_eq!(
                    progress.pending_goodwill,
                    vec![(nearest, g)],
                    "goodwill for the settlement nearest the champion"
                );
                checked += 1;
            }
            Effect::ArmBand => {
                println!("armed {armed_before} -> {armed_after} of {members}");
                assert!(armed_before < members, "someone starts unarmed");
                assert_eq!(armed_after, members, "everyone is armed");
                checked += 1;
            }
            _ => {}
        }
    }
    assert!(checked > 0, "{} has an effect the state shows", boon.name);

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
