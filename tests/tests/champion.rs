//! M4 step 6 test `champion` (SIM-34, SIM-35, SIM-36): the champion ranks
//! up and picks traits, soldiers rank up, and succession works once.
//!
//! Every test plays the run island of seed 42 and sets it up before the
//! first tick (armies spawned with `spawn_army`, the champion's points,
//! traits and a captain's rank edited); after that, input is commands,
//! except the champion's deaths, which are setup-style edits between ticks
//! (health set to 0, as a mortal blow would).
//!
//! - `a_won_battle_ranks_the_champion_up_and_its_pick_applies`: the
//!   champion's band beats a small hostile band spawned beside it. An hour
//!   after the last blow the battle ends (`Battle`, the player's faction,
//!   the champion's army) and earns `champion.points.battle`; with points
//!   set just short of the first rank, the champion ranks up at that hour
//!   (`RankUp`) and is offered three distinct traits it doesn't hold. The
//!   pick joins its `Traits` at once and reaches `FactionModifiers` at the
//!   next hourly rebuild, not before.
//! - `soldiers_who_fought_rank_up_with_the_configured_gains`: a player
//!   warband and a smaller hostile band spawned adjacent fight; when the
//!   battle ends, each warband survivor who struck or was struck has one
//!   battle and army.ron's first veteran rank (a `VeteranRank` each), the
//!   `Battle` event counts them and the warband's dead, and a player army
//!   that never fought is untouched. Hours later a ranked, fed, led
//!   soldier's morale sits exactly at the target with the rank's morale
//!   gain, an unranked one's without it. The director is quiet, so no band
//!   of its finds the reserve.
//! - `the_champion_falls_once_to_a_successor_then_the_run_is_lost`: two
//!   more player captains stand far off, the later-spawned one a veteran.
//!   The champion (three traits, rank 2 with a trait offer pending) is
//!   killed: the veteran captain becomes champion with its rank, points,
//!   the first trait (half of three, rounded down) and the pending offer
//!   (`ChampionFell`, `Succession`); the next rebuild counts only that
//!   trait and the one then picked. Killing the new champion ends the run
//!   lost (`RunEnded`, cause `Death`), and the clock and the hash stop.
//!
//! Each test prints the numbers it checks.

mod common;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use common::*;
use sim_agents::{CombatStats, Needs};
use sim_core::{
    Allegiance, Cause, EventFeed, FactionId, FactionModifiers, ModifierSource, OfferKind, Offers,
    Position, RunOutcome, RunState, SimEventKind, SimStepper, SimTick, SpawnId, Stat,
    compute_state_hash,
};
use sim_military::data::UnitStats;
use sim_military::{
    Army, ArmySpec, ArmyStats, Captain, Champion, ChampionRank, Follower, Progression, RunProgress,
    Traits, Veteran, spawn_army,
};
use sim_world::{Monument, world_pos};

/// The run every test plays.
const SEED: u64 = 42;

/// A weak hostile soldier: the player's men should win and mostly live.
const WEAK: UnitStats = UnitStats {
    health: 40.0,
    attack: 4.0,
    defense: 0.0,
    morale: 0.6,
};

/// A passable tile `r` tiles or so from the start, at least 50 tiles from
/// every settlement centre and monument (where guard camps stand) and 40
/// from the start: nobody else comes there. Tries eight directions at a
/// few distances, in a fixed order.
fn quiet_spot(app: &mut App, r: i32) -> IVec2 {
    let start = app.world().resource::<RunProgress>().start;
    let centres: Vec<IVec2> = villages(app).iter().map(|v| v.2).collect();
    let world = app.world_mut();
    let mut q = world.query_filtered::<&Position, With<Monument>>();
    let monuments: Vec<IVec2> = q.iter(world).map(|p| p.0).collect();
    let far =
        |t: IVec2, from: &[IVec2], d: f32| from.iter().all(|c| (*c - t).as_vec2().length() >= d);
    for dist in [r, r + 20, r + 40, r + 60] {
        for (dx, dy) in [
            (1, 0),
            (0, 1),
            (-1, 0),
            (0, -1),
            (1, 1),
            (-1, 1),
            (1, -1),
            (-1, -1),
        ] {
            let want = start + IVec2::new(dx, dy) * dist;
            let Some(t) = passable_near(app, want) else {
                continue;
            };
            if far(t, &centres, 50.0) && far(t, &monuments, 50.0) && far(t, &[start], 40.0) {
                return t;
            }
        }
    }
    panic!("no quiet spot near the start of seed {SEED}");
}

/// The captain and followers of `captain`, in SpawnId order.
fn members(app: &mut App, captain: Entity) -> Vec<(u64, Entity)> {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &SpawnId, &Follower)>();
    let mut v: Vec<(u64, Entity)> = q
        .iter(world)
        .filter(|(_, _, f)| f.captain == captain)
        .map(|(e, id, _)| (id.0, e))
        .collect();
    if let Some(id) = world.get::<SpawnId>(captain) {
        v.push((id.0, captain));
    }
    v.sort_unstable();
    v
}

/// Steps 50 ticks at a time until the chronicle has a `Battle` for
/// `faction` led by `captain` (a SpawnId), or `max` ticks pass. Returns
/// the entry's tick.
fn until_battle(app: &mut App, faction: FactionId, captain: u64, max: u64) -> u64 {
    let start = app.world().resource::<SimTick>().0;
    loop {
        if let Some(e) = chronicle(app).iter().find(|e| {
            e.event.kind == SimEventKind::Battle
                && e.event.faction == faction
                && e.event.subject == captain
        }) {
            return e.tick;
        }
        let now = app.world().resource::<SimTick>().0;
        assert!(
            now < start + max,
            "seed {SEED}: no battle of {captain} ended by tick {now}"
        );
        app.step(50);
    }
}

/// The player's trait offers (SIM-35) for `person`.
fn trait_offers(app: &App, person: u64) -> Vec<sim_core::Offer> {
    app.world()
        .resource::<Offers>()
        .pending
        .iter()
        .filter(|o| o.kind == OfferKind::Trait { person })
        .cloned()
        .collect()
}

/// The player's trait modifier entries after the last rebuild: trait ids,
/// sorted.
fn trait_sources(app: &App) -> Vec<u16> {
    let mut v: Vec<u16> = app
        .world()
        .resource::<FactionModifiers>()
        .breakdown
        .iter()
        .filter(|e| e.faction == Some(FactionId::PLAYER))
        .filter_map(|e| match e.source {
            ModifierSource::Trait { id } => Some(id),
            _ => None,
        })
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Steps until just after the next hourly step (the progress driver ran).
fn past_next_hour(app: &mut App) {
    let per_hour = app
        .world()
        .resource::<sim_core::Calendar>()
        .ticks_per_hour();
    let tick = app.world().resource::<SimTick>().0;
    // SimTick is the tick about to run; the hour's tick must run too.
    let next = tick.div_ceil(per_hour) * per_hour;
    app.step(next - tick + 1);
}

#[test]
fn a_won_battle_ranks_the_champion_up_and_its_pick_applies() {
    let mut app = run_app(SEED);
    let champion = champion(&mut app);
    let champion_id = spawn_id(&app, champion);
    let data = app.world().resource::<Progression>().data.clone();
    let first_rank = data.champion.ranks[0];
    let battle_points = data.champion.points.battle;
    assert!(
        first_rank > battle_points,
        "one battle alone doesn't rank up"
    );
    // Setup: points just short of the first rank, a weak band beside it.
    let start = app.world().resource::<RunProgress>().start;
    app.world_mut()
        .get_mut::<ChampionRank>(champion)
        .unwrap()
        .points = first_rank - battle_points;
    let foe_at = passable_near(&app, start + IVec2::new(5, 0)).unwrap();
    let foes = spawn_army(
        app.world_mut(),
        &ArmySpec {
            soldier: Some(WEAK),
            ..ArmySpec::new(FactionId::RAIDERS, foe_at, 3)
        },
    );
    let held = app.world().get::<Traits>(champion).unwrap().0.clone();

    let ended = until_battle(&mut app, FactionId::PLAYER, champion_id, 3000);
    let world = app.world();
    let rank = *world.get::<ChampionRank>(champion).unwrap();
    let battle = chronicle(&app)
        .into_iter()
        .find(|e| e.event.kind == SimEventKind::Battle && e.event.subject == champion_id)
        .unwrap();
    println!(
        "battle of the champion's band ended at tick {ended}: {} fought, {} fell; foes left {}; rank {rank:?}",
        battle.event.other,
        battle.event.amount,
        members(&mut app, foes).len()
    );
    assert_eq!(ended % 100, 0, "battles end at the hourly step");
    assert!(battle.event.other > 0, "someone fought");
    assert_eq!(
        rank,
        ChampionRank {
            rank: 1,
            points: first_rank
        },
        "the battle's points reached rank 1"
    );
    let ups: Vec<_> = chronicle(&app)
        .into_iter()
        .filter(|e| e.event.kind == SimEventKind::RankUp)
        .collect();
    assert_eq!(ups.len(), 1);
    assert_eq!(
        (
            ups[0].tick,
            ups[0].event.faction,
            ups[0].event.subject,
            ups[0].event.amount
        ),
        (ended, FactionId::PLAYER, champion_id, 1)
    );
    let offers = trait_offers(&app, champion_id);
    assert_eq!(offers.len(), 1, "one offer for one rank");
    let offer = offers[0].clone();
    let mut distinct = offer.choices.clone();
    distinct.sort_unstable();
    distinct.dedup();
    println!(
        "offer {} of traits {:?} (held {held:?})",
        offer.id, offer.choices
    );
    assert_eq!((offer.posted, offer.faction), (ended, FactionId::PLAYER));
    assert_eq!(distinct.len(), 3, "three distinct traits");
    assert!(
        offer
            .choices
            .iter()
            .all(|t| data.champion_traits.contains(t) && !held.contains(t)),
        "new traits from the champion's pool"
    );

    // The pick: in Traits at once, in the modifiers at the next hour.
    let pick = offer.choices[0];
    let effects = data.traits[pick as usize].effects.clone();
    let value = |app: &App, s: Stat| {
        app.world()
            .resource::<FactionModifiers>()
            .value(FactionId::PLAYER, s)
    };
    let before: Vec<f32> = effects.iter().map(|(s, _)| value(&app, *s)).collect();
    choose(&mut app, offer.id, 0);
    app.step(1);
    let mut want = held.clone();
    want.push(pick);
    assert_eq!(app.world().get::<Traits>(champion).unwrap().0, want);
    assert!(trait_offers(&app, champion_id).is_empty());
    assert_eq!(rejected(&app, FactionId::PLAYER), 0);
    assert!(
        chronicle(&app)
            .iter()
            .any(|e| e.event.kind == SimEventKind::TraitGained
                && e.event.subject == champion_id
                && e.event.amount == pick as i32)
    );
    let now: Vec<f32> = effects.iter().map(|(s, _)| value(&app, *s)).collect();
    assert_eq!(now, before, "not before the hourly rebuild");
    past_next_hour(&mut app);
    for ((stat, v), b) in effects.iter().zip(&before) {
        let after = value(&app, *stat);
        println!(
            "{} {stat:?}: {b:.3} -> {after:.3}",
            data.traits[pick as usize].name
        );
        assert!((after - b - v).abs() < 1e-6, "{stat:?} gains {v}");
    }
}

#[test]
fn soldiers_who_fought_rank_up_with_the_configured_gains() {
    let mut app = run_app(SEED);
    let army = app.world().resource::<Army>().data.clone();
    let first = army.veterans.thresholds[0];
    assert_eq!(first, 1, "one battle makes a Soldier (army.ron)");
    let gain = army.veterans.gains[0];
    // No director's bands: the reserve must never fight. The director
    // brings one when the run would go quiet for `quiet_limit_hours`; since
    // M4a step 2 (the smithy chain) the island's towns are fed sooner and
    // ask for food less, so on this seed one came at tick 1200, aimed at
    // a village near the reserve, and fell on it.
    quiet_director(&mut app);
    // Setup: a player warband and a weak hostile band adjacent, and a
    // player reserve far off that never fights.
    let spot = quiet_spot(&mut app, 70);
    let warband = spawn_army(
        app.world_mut(),
        &ArmySpec {
            armed: true,
            ..ArmySpec::new(FactionId::PLAYER, spot, 20)
        },
    );
    let foe_at = passable_near(&app, spot + IVec2::new(4, 0)).unwrap();
    spawn_army(
        app.world_mut(),
        &ArmySpec {
            armed: true,
            ..ArmySpec::new(FactionId::RAIDERS, foe_at, 10)
        },
    );
    let reserve_at = quiet_spot(&mut app, 160);
    let reserve = spawn_army(
        app.world_mut(),
        &ArmySpec::new(FactionId::PLAYER, reserve_at, 6),
    );
    let warband_id = spawn_id(&app, warband);
    let men = members(&mut app, warband);
    let lost_before = app
        .world()
        .resource::<ArmyStats>()
        .get(FactionId::PLAYER)
        .lost;

    let ended = until_battle(&mut app, FactionId::PLAYER, warband_id, 3000);
    let world = app.world();
    let living: Vec<(u64, Veteran)> = men
        .iter()
        .filter(|(_, e)| world.get_entity(*e).is_ok())
        .map(|(id, e)| (*id, *world.get::<Veteran>(*e).unwrap()))
        .collect();
    let fought: Vec<&(u64, Veteran)> = living.iter().filter(|(_, v)| v.battles == 1).collect();
    let battle = chronicle(&app)
        .into_iter()
        .find(|e| e.event.kind == SimEventKind::Battle && e.event.subject == warband_id)
        .unwrap();
    let dead = (men.len() - living.len()) as i32;
    let lost = world.resource::<ArmyStats>().get(FactionId::PLAYER).lost - lost_before;
    println!(
        "warband battle ended at {ended}: {} of {} survivors fought, {dead} fell (lost {lost})",
        fought.len(),
        living.len()
    );
    assert!(fought.len() >= 5, "most of the warband fought");
    assert!(dead > 0, "the battle cost the warband men");
    assert_eq!(battle.event.amount, dead, "losses in the Battle event");
    assert_eq!(lost as i32, dead);
    assert_eq!(battle.event.other, fought.len() as u64, "who fought");
    for (id, v) in &living {
        let want = if v.battles == 1 { 1 } else { 0 };
        assert!(v.battles <= 1 && !v.fought, "{id}: {v:?}");
        assert_eq!(v.rank, want, "{id}: {v:?}");
    }
    // UI-09: one VeteranRank for the army and the rank reached (grouped,
    // so a big battle doesn't flood the feed), and the battle report names
    // every soldier promoted.
    let ranked: Vec<(u64, u64, i32)> = world
        .resource::<EventFeed>()
        .iter()
        .filter(|e| {
            e.event.kind == SimEventKind::VeteranRank && e.event.faction == FactionId::PLAYER
        })
        .map(|e| (e.event.subject, e.event.other, e.event.amount))
        .collect();
    let mut want: Vec<u64> = fought.iter().map(|(id, _)| *id).collect();
    want.sort_unstable();
    assert_eq!(
        ranked,
        vec![(warband_id, want.len() as u64, 1)],
        "one VeteranRank for the rank reached"
    );
    let report = world
        .resource::<sim_core::Chronicle>()
        .report_for(&battle)
        .expect("the battle's report");
    let promoted: Vec<u64> = report
        .promotions
        .iter()
        .map(|p| {
            assert_eq!(p.rank, 1);
            p.soldier
        })
        .collect();
    assert_eq!(promoted, want, "the report names each soldier promoted");
    assert_eq!(
        (report.fought, report.losses),
        (fought.len() as u32, dead as u32)
    );
    assert!(
        report.foes.contains(&FactionId::RAIDERS) && report.kills > 0,
        "{report:?}"
    );
    for (id, e) in &members(&mut app, reserve) {
        assert_eq!(
            *app.world().get::<Veteran>(*e).unwrap(),
            Veteran::default(),
            "reserve {id} never fought"
        );
    }

    // The rank's morale gain: hours later, fed and led soldiers' morale
    // sits at the target, with the gain for the ranked.
    app.step(1500);
    let world = app.world();
    let mult = world
        .resource::<FactionModifiers>()
        .mult(FactionId::PLAYER, Stat::Morale);
    let base = army.soldier.morale * mult;
    let lead = army.leadership.radius;
    let target = |gain: f32| (base + gain + 0.1).min(1.0);
    let warband_men = members(&mut app, warband);
    let reserve_men = members(&mut app, reserve);
    let check = |captain: Entity, men: &[(u64, Entity)], ranked: bool| -> usize {
        let world = app.world();
        let c = world_pos(
            world.get::<Position>(captain).unwrap(),
            world.get::<sim_core::Motion>(captain),
        );
        let mut seen = 0;
        for &(id, e) in men {
            if e == captain {
                continue;
            }
            let v = *world.get::<Veteran>(e).unwrap();
            let cs = world.get::<CombatStats>(e).unwrap();
            let hunger = world.get::<Needs>(e).unwrap().hunger;
            let p = world_pos(
                world.get::<Position>(e).unwrap(),
                world.get::<sim_core::Motion>(e),
            );
            if (v.rank > 0) != ranked || hunger >= army.upkeep.hungry || p.distance(c) > lead {
                continue;
            }
            let want = target(if ranked { gain.morale } else { 0.0 });
            assert!(
                (cs.morale - want).abs() < 1e-5,
                "{id}: morale {} vs target {want} (rank {})",
                cs.morale,
                v.rank
            );
            seen += 1;
        }
        seen
    };
    let ranked = check(warband, &warband_men, true);
    let unranked = check(reserve, &reserve_men, false);
    println!(
        "morale targets: ranked {:.3} ({ranked} soldiers), unranked {:.3} ({unranked})",
        target(gain.morale),
        target(0.0)
    );
    assert!(
        ranked >= 3 && unranked >= 3,
        "enough fed, led soldiers seen"
    );
}

#[test]
fn the_champion_falls_once_to_a_successor_then_the_run_is_lost() {
    let mut app = run_app(SEED);
    let first = champion(&mut app);
    let first_id = spawn_id(&app, first);
    let data = app.world().resource::<Progression>().data.clone();
    // Setup: three traits, points for rank 2 (rank 1 already taken), and
    // two captains far off: the lower SpawnId a recruit, the other a
    // veteran of rank 2.
    let traits = vec![0, 3, 5];
    let points = data.champion.ranks[1];
    {
        let world = app.world_mut();
        world.get_mut::<Traits>(first).unwrap().0 = traits.clone();
        *world.get_mut::<ChampionRank>(first).unwrap() = ChampionRank { rank: 1, points };
    }
    let a_at = quiet_spot(&mut app, 70);
    let b_at = quiet_spot(&mut app, 160);
    let a = spawn_army(app.world_mut(), &ArmySpec::new(FactionId::PLAYER, a_at, 3));
    let b = spawn_army(app.world_mut(), &ArmySpec::new(FactionId::PLAYER, b_at, 3));
    app.world_mut().get_mut::<Veteran>(b).unwrap().rank = 2;
    let (a_id, b_id) = (spawn_id(&app, a), spawn_id(&app, b));
    assert!(a_id < b_id);

    // Tick 0's hourly step ranks the champion up to 2 and offers traits.
    app.step(2);
    let pending = trait_offers(&app, first_id);
    assert_eq!(pending.len(), 1, "rank 2's offer waits");
    let band = members(&mut app, first);
    let heir_of_band = band.iter().find(|(_, e)| *e != first).unwrap().1;

    // The champion falls.
    app.world_mut()
        .get_mut::<CombatStats>(first)
        .unwrap()
        .health = 0.0;
    app.step(1);
    let tick = app.world().resource::<SimTick>().0 - 1;
    assert!(app.world().get_entity(first).is_err(), "the fallen is gone");
    assert_eq!(champion(&mut app), b, "the veteran captain is champion");
    let world = app.world();
    assert_eq!(
        *world.get::<ChampionRank>(b).unwrap(),
        ChampionRank { rank: 2, points }
    );
    assert_eq!(
        world.get::<Traits>(b).unwrap().0,
        traits[..traits.len() / 2].to_vec(),
        "half the traits, rounded down, in pick order"
    );
    assert!(world.get::<Champion>(a).is_none() && world.get::<ChampionRank>(a).is_none());
    assert!(world.resource::<RunProgress>().succession_used);
    assert!(world.resource::<RunState>().outcome.is_none());
    assert!(
        world.get::<Captain>(heir_of_band).is_some()
            && world.get::<Champion>(heir_of_band).is_none(),
        "the band follows a new captain, who isn't champion"
    );
    let entries = chronicle(&app);
    let fell = entries
        .iter()
        .find(|e| e.event.kind == SimEventKind::ChampionFell && e.event.subject == first_id)
        .expect("ChampionFell");
    let succession = entries
        .iter()
        .find(|e| e.event.kind == SimEventKind::Succession)
        .expect("Succession");
    println!(
        "champion {first_id} fell at {tick} ({:?}); {b_id} succeeds with traits {:?}",
        fell.event.cause,
        app.world().get::<Traits>(b).unwrap().0
    );
    assert_eq!(fell.tick, tick);
    assert_eq!(
        (
            succession.tick,
            succession.event.faction,
            succession.event.subject,
            succession.event.other,
            succession.event.amount
        ),
        (tick, FactionId::PLAYER, b_id, first_id, 1)
    );
    assert!(matches!(fell.event.cause, Cause::Death { .. }));
    // The pending trait offer follows the title.
    assert!(trait_offers(&app, first_id).is_empty());
    let moved = trait_offers(&app, b_id);
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0].id, pending[0].id);
    let pick = moved[0].choices[0];
    choose(&mut app, moved[0].id, 0);
    app.step(1);
    assert_eq!(
        app.world().get::<Traits>(b).unwrap().0,
        vec![traits[0], pick]
    );
    past_next_hour(&mut app);
    let mut want = vec![traits[0], pick];
    want.sort_unstable();
    want.dedup();
    assert_eq!(trait_sources(&app), want, "only the heir's traits count");
    assert_eq!(
        app.world().get::<Allegiance>(b).unwrap().0,
        FactionId::PLAYER
    );

    // The second champion falls: no second succession.
    app.world_mut().get_mut::<CombatStats>(b).unwrap().health = 0.0;
    app.step(1);
    let end = app.world().resource::<SimTick>().0 - 1;
    let state = *app.world().resource::<RunState>();
    println!(
        "champion {b_id} fell at {end}: outcome {:?}, cause {:?}",
        state.outcome, state.cause
    );
    assert_eq!(state.outcome, Some(RunOutcome::Lost));
    assert_eq!(state.ended_at, Some(end));
    assert!(matches!(state.cause, Cause::Death { .. }));
    assert!(
        app.world().get::<Captain>(a).is_some(),
        "a captain lives on"
    );
    assert!(
        app.world().get::<Champion>(a).is_none(),
        "but succession was used"
    );
    let entries = chronicle(&app);
    assert!(
        entries
            .iter()
            .any(|e| e.event.kind == SimEventKind::ChampionFell && e.event.subject == b_id)
    );
    assert_eq!(
        entries
            .iter()
            .filter(|e| e.event.kind == SimEventKind::Succession)
            .count(),
        1
    );
    let last = entries.last().unwrap();
    assert_eq!(
        (last.tick, last.event.kind, last.event.subject),
        (end, SimEventKind::RunEnded, RunOutcome::Lost as u64)
    );
    // The clock and the hash stop.
    let hash = compute_state_hash(app.world_mut());
    app.step(50);
    assert_eq!(app.world().resource::<SimTick>().0, end + 1);
    assert_eq!(compute_state_hash(app.world_mut()), hash);
}
