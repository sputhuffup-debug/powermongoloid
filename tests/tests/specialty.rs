//! M4a step 9 scenario `specialty` (SIM-62; ADR 0020, LORDSHIP_DESIGN
//! section 8.4): towns that differ. At populate, in a run only, each town's
//! site (reachable Upland, Coast and ForestEdge tiles within 40 of its
//! centre, and the roads that meet there) picks the first matching row of
//! villagers.ron `specialties`, whose share multipliers change how many
//! work each trade.
//!
//! - `specialties_follow_the_site`: every run town carries the specialty
//!   its site gives; the developed world carries none.
//! - `mining_towns_have_miners_and_smiths`: a Mining town has more miners
//!   and smiths, and no fewer smithies, than the same town laid out
//!   without specialties.
//! - `a_market_town_posts_hire_offers`: a Market town the player owns
//!   camps a band of `people / 80` (3 to 6) men for hire every
//!   `market.hire_days` and posts its `Hire` offer, which the champion
//!   hires as the start's sellswords are; a town not the player's posts
//!   none.
//! - `populate_draws_are_unchanged`: populate draws exactly as many words
//!   from each town's stream with specialties as without (BUILD_GUIDE G10),
//!   and every town keeps its people.
//!
//! Each test prints the numbers it checks.

mod common;

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use common::*;
use sim_agents::{
    Enlisted, PopulationReport, Specialty, Villager, Villagers,
    population::{road_degree, specialty_of},
};
use sim_core::{
    Calendar, Cause, FactionId, OfferKind, Offers, Position, SimStepper, SimTick, SpawnId,
};
use sim_economy::{Economy, Occupation, SettlementEconomy, Stockpile, Workplace, WorkplaceKind};
use sim_headless::{HeadlessConfig, headless_app};
use sim_military::{Captain, Follower, Progression, govern, sellswords, transfer};
use sim_world::{RoadNetwork, Settlement, WorldMap};

const SEEDS: [u64; 2] = [1, 7];

/// Run towns by SpawnId: (entity, id, centre, people, specialty).
fn towns(app: &mut App) -> Vec<(Entity, u64, IVec2, u32, Option<Specialty>)> {
    let w = app.world_mut();
    let mut q = w.query::<(
        Entity,
        &SpawnId,
        &Settlement,
        &SettlementEconomy,
        Option<&Specialty>,
    )>();
    let mut v: Vec<_> = q
        .iter(w)
        .filter(|(.., se, _)| se.population > 0)
        .map(|(e, id, s, se, sp)| (e, id.0, s.center, se.population, sp.copied()))
        .collect();
    v.sort_unstable_by_key(|t| t.1);
    v
}

/// Residents of `s` (not enlisted) by trade.
fn trades(app: &mut App, s: Entity) -> BTreeMap<Occupation, u32> {
    let w = app.world_mut();
    let mut q = w.query_filtered::<(&Villager, &Occupation), Without<Enlisted>>();
    let mut out = BTreeMap::new();
    for (v, o) in q.iter(w) {
        if v.settlement == s {
            *out.entry(*o).or_default() += 1;
        }
    }
    out
}

fn workplaces(app: &App, s: Entity, kind: WorkplaceKind) -> usize {
    let w = app.world();
    w.get::<SettlementEconomy>(s)
        .unwrap()
        .workplaces
        .iter()
        .filter(|&&e| w.get::<Workplace>(e).unwrap().kind == kind)
        .count()
}

/// SIM-62: every populated run town carries a specialty, and it is the one
/// its site gives (the first matching row, recomputed from the map and the
/// roads); a Mining town has the Upland tiles, a Market town the roads. The
/// developed world (no run) carries none.
#[test]
fn specialties_follow_the_site() {
    for seed in SEEDS {
        let mut app = run_app(seed);
        let rules = app.world().resource::<Villagers>().data.specialties.clone();
        let mut tally: BTreeMap<Specialty, u32> = BTreeMap::new();
        for (_, id, centre, people, sp) in towns(&mut app) {
            let w = app.world();
            let degree = road_degree(w.get_resource::<RoadNetwork>(), SpawnId(id));
            let want =
                specialty_of(&rules, w.resource::<WorldMap>(), centre, degree).map(|r| r.kind);
            let counts = w
                .resource::<PopulationReport>()
                .sites
                .iter()
                .find(|(s, _)| s.0 == id)
                .map(|x| x.1)
                .expect("site counts for a run town");
            println!(
                "seed {seed} #{id}: {people} people, {:?} (upland {}, coast {}, forest edge {}, roads {})",
                sp, counts.upland, counts.coast, counts.forest_edge, counts.roads
            );
            assert!(sp.is_some(), "town #{id} has a specialty");
            assert_eq!(sp, want, "town #{id}: the site's first matching row");
            match sp.unwrap() {
                Specialty::Mining => assert!(counts.upland >= 150),
                Specialty::Market => assert!(counts.upland < 150 && counts.roads >= 3),
                Specialty::Fishing => assert!(counts.upland < 150 && counts.coast >= 160),
                Specialty::Timber => assert!(counts.forest_edge >= 200),
                Specialty::Farming => {}
            }
            *tally.entry(sp.unwrap()).or_default() += 1;
        }
        println!("seed {seed}: {tally:?}");
        assert_eq!(tally.len(), 5, "every specialty on the island");
    }

    let mut dev = headless_app(HeadlessConfig {
        world_size: 512,
        population: 300,
        ..HeadlessConfig::new(42)
    });
    let w = dev.world_mut();
    let mut q = w.query::<&Specialty>();
    assert_eq!(q.iter(w).count(), 0, "no specialties outside a run");
    assert!(w.resource::<PopulationReport>().sites.is_empty());
}

/// SIM-62: a Mining town has more miners, no fewer smiths, and no fewer
/// smithies or mines than the same town laid out without specialties (an
/// empty `specialties` list), and the Mining towns have more smiths in
/// all (a hamlet of 43 rounds to one smith either way): the ore and the
/// hands a Forge needs.
#[test]
fn mining_towns_have_miners_and_smiths() {
    let (mut seen, mut smiths) = (0, (0, 0));
    for seed in SEEDS {
        let mut with = run_app(seed);
        let mut without = run_app_without_specialties(seed);
        let plain: BTreeMap<u64, Entity> = towns(&mut without)
            .into_iter()
            .map(|t| (t.1, t.0))
            .collect();
        for (e, id, _, people, sp) in towns(&mut with) {
            if sp != Some(Specialty::Mining) {
                continue;
            }
            seen += 1;
            let p = plain[&id];
            let (a, b) = (trades(&mut with, e), trades(&mut without, p));
            let n = |m: &BTreeMap<Occupation, u32>, o| m.get(&o).copied().unwrap_or(0);
            let (smithies, mines) = (
                workplaces(&with, e, WorkplaceKind::Smithy),
                workplaces(&with, e, WorkplaceKind::Mine),
            );
            let (smithies0, mines0) = (
                workplaces(&without, p, WorkplaceKind::Smithy),
                workplaces(&without, p, WorkplaceKind::Mine),
            );
            println!(
                "seed {seed} Mining town #{id} ({people} people): miners {} -> {}, smiths {} -> {}, smithies {smithies0} -> {smithies}, mines {mines0} -> {mines}",
                n(&b, Occupation::Miner),
                n(&a, Occupation::Miner),
                n(&b, Occupation::Smith),
                n(&a, Occupation::Smith),
            );
            assert!(
                n(&a, Occupation::Miner) > n(&b, Occupation::Miner),
                "more miners"
            );
            assert!(
                n(&a, Occupation::Smith) >= n(&b, Occupation::Smith),
                "no fewer smiths"
            );
            smiths.0 += n(&b, Occupation::Smith);
            smiths.1 += n(&a, Occupation::Smith);
            assert!(smithies >= smithies0 && smithies >= 1);
            assert!(mines >= mines0 && mines >= 1);
        }
    }
    println!("smiths in Mining towns: {} -> {}", smiths.0, smiths.1);
    assert!(seen >= 2, "seeds 1 and 7 have Mining towns ({seen})");
    // A hamlet rounds to the same smith or two; across the Mining towns
    // there are more.
    assert!(smiths.1 > smiths.0, "more smiths in all");
}

/// SIM-62: on a hire hour (every `market.hire_days`; half a day here, so
/// the test is short) a Market town the player owns camps a band of
/// `market_band` men (one per 80 people, 3 to 6) within `hire.distance`
/// and a little of its centre, independent and without upkeep, and posts
/// its `Hire` offer at the sellswords' price; the champion within reach
/// with the food hires them. A Market town that isn't the player's posts
/// nothing.
#[test]
fn a_market_town_posts_hire_offers() {
    let (mut app, town) = (1..=12)
        .find_map(|seed| {
            let mut app = run_app(seed);
            let t = towns(&mut app).into_iter().find(|t| {
                t.4 == Some(Specialty::Market) && owner_of(&app, t.0) == FactionId::INDEPENDENT
            })?;
            println!("seed {seed}: Market town #{} ({} people)", t.1, t.3);
            Some((app, t))
        })
        .expect("a Market town on one of seeds 1 to 12");
    let (s, id, centre, people, _) = town;
    quiet_director(&mut app);
    app.world_mut()
        .resource_mut::<Progression>()
        .data
        .market
        .hire_days = 0.5;
    let period = app.world().resource::<Calendar>().ticks_per_day / 2;
    let hires = |app: &App| -> Vec<(u64, u64, u16, u16)> {
        app.world()
            .resource::<Offers>()
            .pending
            .iter()
            .filter_map(|o| match o.kind {
                OfferKind::Hire {
                    captain,
                    men,
                    price,
                } => Some((o.id, captain, men, price)),
                _ => None,
            })
            .collect()
    };
    let before = hires(&app);
    app.step(period + 1);
    assert_eq!(hires(&app), before, "nothing while the town is not ours");

    transfer(app.world_mut(), s, FactionId::PLAYER, Cause::Invitation).expect("populated");
    let now = app.world().resource::<SimTick>().0;
    app.step(2 * period + 1 - now);
    let new: Vec<_> = hires(&app)
        .into_iter()
        .filter(|h| !before.contains(h))
        .collect();
    let data = app.world().resource::<Progression>().data.clone();
    let men = govern::market_band(&data.market, people);
    let per_day = app
        .world()
        .resource::<Villagers>()
        .data
        .needs
        .hunger_per_day;
    println!("offers posted: {new:?} (band of {men} expected)");
    assert_eq!(new.len(), 1, "one offer from the one Market town we own");
    let (offer, captain, n, price) = new[0];
    assert_eq!(n as u32, men);
    assert!((3..=6).contains(&men));
    assert_eq!(price, sellswords::price(&data.hire, per_day, men));
    let w = app.world_mut();
    let mut q = w.query::<(Entity, &SpawnId, &Captain, &Position)>();
    let (band, at) = q
        .iter(w)
        .find(|(_, sid, ..)| sid.0 == captain)
        .map(|(e, _, c, p)| {
            assert_eq!(c.faction, FactionId::INDEPENDENT);
            (e, p.0)
        })
        .expect("the band's captain");
    assert!(w.get::<sim_military::NoUpkeep>(band).is_some(), "no upkeep");
    let d = (at - centre).as_vec2().length();
    println!("the band camps {d:.1} tiles from #{id}'s centre");
    assert!(d <= data.hire.distance + 8.0, "at the town's edge");

    // The champion comes with the price and hires them.
    let champ = champion(&mut app);
    let near = passable_near(&app, at + IVec2::new(2, 0)).expect("land");
    app.world_mut().get_mut::<Position>(champ).unwrap().0 = near;
    let food = *app
        .world()
        .resource::<Economy>()
        .foods_by_value()
        .last()
        .unwrap();
    {
        let mut bag = app.world_mut().get_mut::<Stockpile>(champ).unwrap();
        bag.capacity += 200;
        bag.add(food, 100);
    }
    let followers = |app: &mut App| {
        let w = app.world_mut();
        let mut q = w.query::<&Follower>();
        q.iter(w).filter(|f| f.captain == champ).count()
    };
    let had = followers(&mut app);
    choose(&mut app, offer, 0);
    app.step(2);
    let has = followers(&mut app);
    println!("the champion's followers {had} -> {has}");
    assert_eq!(has, had + men as usize, "the whole band, its captain too");
    assert!(
        hires(&app).iter().all(|h| h.0 != offer),
        "the offer is taken"
    );
}

/// BUILD_GUIDE G10, SIM-62: specialties change how many work each trade
/// (and so how many workplaces are built, which shifts later SpawnIds on
/// the run island), never how many words populate draws from each town's
/// stream; every town keeps its people.
#[test]
fn populate_draws_are_unchanged() {
    for seed in [1u64, 7, 42] {
        let with = run_app(seed);
        let without = run_app_without_specialties(seed);
        let (a, b) = (
            with.world().resource::<PopulationReport>(),
            without.world().resource::<PopulationReport>(),
        );
        assert!(!a.draws.is_empty());
        assert_eq!(
            a.draws, b.draws,
            "seed {seed}: the same draws, town by town"
        );
        let people = |r: &PopulationReport| r.settlements.iter().map(|x| x.1).collect::<Vec<_>>();
        assert_eq!(people(a), people(b), "seed {seed}: the same people");
        assert!(!a.sites.is_empty() && b.sites.is_empty());
        let total: u128 = a.draws.iter().map(|d| d.1).sum();
        println!(
            "seed {seed}: {} towns, {total} words drawn either way",
            a.draws.len()
        );
    }
}
