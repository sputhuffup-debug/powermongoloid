//! M4 step 7 scenario `rival` (GP-28): the rival warlord's seat grows on
//! its clock and raids from mid-run; taking the seat wins the run.
//!
//! Both tests play the run island of seed 42 (steady; the run's "weaker
//! rival" modifier scales the rival's garrison, levies and defence by 0.8)
//! with the pacing director silenced (its quiet limit and first-event
//! window set beyond the test), so only the rival acts.
//!
//! - `the_rival_levies_raises_its_defences_and_raids_on_its_clock`: with no
//!   player action, and the steady row's clock shortened before the first
//!   tick (a levy every half day, defence steps at half-day intervals, the
//!   first raid on day 1 and then every 0.75 days; the rest of the row as
//!   shipped). The rival starts with every settlement of its seat's region
//!   (loyalty `start_loyalty`, no conquest reported) and its garrison at
//!   the seat, garrisoning it. Every hour the test checks the levies
//!   (exactly `round(levy_size x 0.8)` villagers each time one is due,
//!   counted as the rival's recruits), the defence bonus (the step for the
//!   day, x 0.8), and the garrison's head count against its books
//!   (start + levied - raided - lost - deserted). The first `RivalRaid`
//!   leaves on the first raid tick, with a band of at most half the
//!   garrison split off it (aggressive, a `RivalBand`), and the band then
//!   goes after the player.
//! - `taking_the_seat_wins_the_run`: a 150-man armed player army spawned
//!   near the seat before the first tick attacks it; when the seat falls
//!   the run is won (`RunEnded`, cause the conquest), the rest of the
//!   rival's lands are independent, and the clock and the hash stop.

mod common;

use bevy_ecs::prelude::*;
use common::*;
use sim_agents::{Owner, SettlementLoyalty};
use sim_core::{
    Calendar, Cause, FactionId, OrderSpec, Posture, RunOutcome, RunState, SimEventKind, SimStepper,
    SimTick, SpawnId, compute_state_hash,
};
use sim_military::{
    ArmySpec, ArmyStats, Captain, Factions, Follower, Progression, RivalBand, RivalData,
    RivalState, RunProgress, find_settlement, spawn_army,
};
use sim_world::Settlement;

const SEED: u64 = 42;

/// The run's rival clock row.
fn row(app: &App) -> RivalData {
    let cfg = app.world().resource::<RunState>().config.expect("a run");
    app.world()
        .resource::<Progression>()
        .rival(cfg.difficulty)
        .expect("a rival row")
        .clone()
}

/// People in the seat's garrison (captain and living followers).
fn garrison_size(app: &mut App) -> u32 {
    let world = app.world_mut();
    let Some(g) = sim_military::rival::garrison(world) else {
        return 0;
    };
    let mut q = world.query::<&Follower>();
    q.iter(world).filter(|f| f.captain == g).count() as u32 + 1
}

/// Settlements the rival holds: (SpawnId, entity), by SpawnId.
fn rival_lands(app: &mut App) -> Vec<(u64, Entity)> {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &SpawnId, &Owner)>();
    let mut v: Vec<_> = q
        .iter(world)
        .filter(|(_, _, o)| o.faction == FactionId::RIVAL)
        .map(|(e, id, _)| (id.0, e))
        .collect();
    v.sort_unstable();
    v
}

#[test]
fn the_rival_levies_raises_its_defences_and_raids_on_its_clock() {
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    let cal = app.world().resource::<Calendar>().clone();
    let strength = sim_military::rival::strength(app.world());
    let progress = app.world().resource::<RunProgress>().clone();
    let seat = progress.rival_seat.expect("a seat");
    let shipped = row(&app);

    // The start: the seat's region is the rival's, quietly; the garrison
    // stands at the seat.
    let lands = rival_lands(&mut app);
    let region: Vec<u64> = {
        let world = app.world_mut();
        let mut q = world.query::<(&SpawnId, &Settlement, &Owner)>();
        let mut v: Vec<u64> = q
            .iter(world)
            .filter(|(_, s, _)| s.region == progress.rival_region)
            .map(|(id, ..)| id.0)
            .collect();
        v.sort_unstable();
        v
    };
    println!(
        "seat #{seat} region {}: rival lands {:?}; strength {strength}",
        progress.rival_region,
        lands.iter().map(|l| l.0).collect::<Vec<_>>()
    );
    assert_eq!(lands.iter().map(|l| l.0).collect::<Vec<_>>(), region);
    assert!(lands.iter().any(|l| l.0 == seat));
    for &(id, s) in &lands {
        let loyalty = app.world().get::<SettlementLoyalty>(s).unwrap().mean;
        assert!(
            (loyalty - shipped.start_loyalty).abs() < 1e-6,
            "#{id} loyalty {loyalty}"
        );
    }
    assert!(
        chronicle(&app)
            .iter()
            .all(|e| e.event.kind != SimEventKind::Conquest),
        "the rival's lands are given, not conquered"
    );
    let state = app.world().resource::<RivalState>().clone();
    assert_eq!(state.seat, Some(seat));
    assert_eq!(state.seat_center, progress.seat_center);
    // The shipped clock: the first levy after `levy_every_days`, the first
    // raid on `raid_from_day` (about day 10 on steady), no bonus yet.
    println!(
        "shipped clock: first levy at {}, first raid at {} (day {}), defence +{:.2}",
        state.next_levy, state.next_raid, shipped.raid_from_day, state.defence_bonus
    );
    assert_eq!(state.next_levy, cal.days(shipped.levy_every_days));
    assert_eq!(state.next_raid, cal.days(shipped.raid_from_day));
    assert!((8.0..=12.0).contains(&shipped.raid_from_day));
    assert_eq!(state.defence_bonus, shipped.defence_on(0.0) * strength);
    assert_eq!(
        app.world().resource::<Factions>().seat(FactionId::RIVAL),
        Some(progress.seat_center)
    );
    let start_garrison = garrison_size(&mut app);
    let g = sim_military::rival::garrison(app.world_mut()).expect("a garrison");
    let front = app
        .world()
        .get::<Captain>(g)
        .unwrap()
        .orders
        .front()
        .map(|o| o.spec);
    println!("garrison {start_garrison} people, front order {front:?}");
    assert_eq!(
        start_garrison,
        (shipped.garrison as f32 * strength).round() as u32 + 1
    );
    assert_eq!(front, Some(OrderSpec::Garrison { settlement: seat }));

    // A shortened clock (data, before the first tick).
    let clock = RivalData {
        levy_every_days: 0.5,
        defence_steps: vec![(0.0, 0.0), (0.5, 0.25), (1.0, 0.5), (1.5, 1.0)],
        raid_from_day: 1.0,
        raid_every_days: 0.75,
        ..shipped.clone()
    };
    {
        let difficulty = clock.difficulty;
        let mut p = app.world_mut().resource_mut::<Progression>();
        let r = p
            .data
            .rival
            .iter_mut()
            .find(|r| r.difficulty == difficulty)
            .unwrap();
        *r = clock.clone();
        let mut s = app.world_mut().resource_mut::<RivalState>();
        s.next_levy = cal.days(clock.levy_every_days);
        s.next_raid = cal.days(clock.raid_from_day);
    }
    let levy = (clock.levy_size as f32 * strength).round() as u32;

    let books = |app: &mut App| {
        let state = app.world().resource::<RivalState>().clone();
        let stats = app.world().resource::<ArmyStats>().get(FactionId::RIVAL);
        let world = app.world_mut();
        let mut q = world.query_filtered::<(Entity, &Captain), With<RivalBand>>();
        let bands: Vec<Entity> = q.iter(world).map(|(e, _)| e).collect();
        let mut q = world.query::<&Follower>();
        let raided: u32 = bands
            .iter()
            .map(|b| q.iter(world).filter(|f| f.captain == *b).count() as u32 + 1)
            .sum();
        (state, stats, raided)
    };
    app.step(1);
    let mut first_raid = None;
    let mut peak = start_garrison;
    for hour in 0..60u64 {
        let t = hour * cal.ticks_per_hour();
        assert_eq!(app.world().resource::<SimTick>().0, t + 1);
        let (state, stats, raided) = books(&mut app);
        let day = t as f32 / cal.ticks_per_day as f32;
        // Levies due by now, each of `levy`.
        let due = (1..)
            .take_while(|k| cal.days(clock.levy_every_days) * k <= t)
            .count() as u32;
        assert_eq!(state.levied, due * levy, "levies at tick {t}");
        assert_eq!(stats.recruited, state.levied);
        let bonus = clock.defence_on(day) * strength;
        assert!(
            (state.defence_bonus - bonus).abs() < 1e-6,
            "defence at day {day}: {} vs {bonus}",
            state.defence_bonus
        );
        let size = garrison_size(&mut app);
        // The rival's losses and deserters may be the garrison's or the
        // bands', so the garrison and the bands are counted together.
        assert_eq!(
            size + raided,
            start_garrison + state.levied - stats.lost - stats.deserted,
            "the garrison's books at tick {t}"
        );
        peak = peak.max(size);
        if first_raid.is_none()
            && let Some(e) = chronicle(&app)
                .iter()
                .find(|e| e.event.kind == SimEventKind::RivalRaid)
        {
            first_raid = Some((e.tick, e.event.subject, e.event.amount, size, raided));
        }
        if hour % 6 == 0 {
            println!(
                "tick {t}: garrison {size}, levied {}, defence +{:.2}, raids {} ({raided} out), lost {}, deserted {}",
                state.levied, state.defence_bonus, state.raids, stats.lost, stats.deserted
            );
        }
        app.step(cal.ticks_per_hour());
    }
    let state = app.world().resource::<RivalState>().clone();
    // Levied men walk in from the rival's other settlements, and a few
    // fall on the way (to beasts or raiders): the books above account for
    // them; the garrison still grows before the first raid.
    assert!(peak > start_garrison, "the garrison grew: peak {peak}");
    let (tick, band, amount, after, raided) = first_raid.expect("a raid within 2.5 days");
    println!(
        "first raid at {tick}: band captain #{band}, {amount} people; garrison left {after}; {raided} out; raids {}",
        state.raids
    );
    assert_eq!(tick, cal.days(clock.raid_from_day), "the first raid's tick");
    assert!(amount >= 2 && amount as u32 <= clock.raid_size);
    // Raids every 0.75 days from day 1: on days 1, 1.75 and 2.5 (the
    // last hour stepped ran the clock at day 2.5).
    assert_eq!(state.raids, 3, "raids by day 2.5");
    // The last band thinks on the next tick.
    app.step(1);
    let world = app.world_mut();
    let mut q = world.query::<(&SpawnId, &Captain, &RivalBand)>();
    let mut bands: Vec<(u64, Posture, Option<OrderSpec>)> = q
        .iter(world)
        .map(|(id, c, _)| (id.0, c.posture, c.orders.front().map(|o| o.spec)))
        .collect();
    bands.sort_unstable_by_key(|b| b.0);
    println!("bands {bands:?}");
    assert!(!bands.is_empty(), "the raiding bands are out");
    for (_, posture, front) in &bands {
        assert_eq!(*posture, Posture::Aggressive);
        // No land or monuments of the player's yet: they hunt its band.
        assert!(
            matches!(front, Some(OrderSpec::MoveTo { .. })),
            "a band goes after the player: {front:?}"
        );
    }
    assert_eq!(
        app.world().resource::<RunState>().outcome,
        None,
        "the run goes on"
    );
}

#[test]
fn taking_the_seat_wins_the_run() {
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    let progress = app.world().resource::<RunProgress>().clone();
    let seat_id = progress.rival_seat.expect("a seat");
    let seat = find_settlement(app.world_mut(), seat_id).expect("the seat");
    let toward = (progress.start - progress.seat_center)
        .as_vec2()
        .normalize_or_zero();
    let at = progress.seat_center + (toward * 25.0).round().as_ivec2();
    let army = spawn_army(
        app.world_mut(),
        &ArmySpec {
            armed: true,
            food_days: 3.0,
            ..ArmySpec::new(FactionId::PLAYER, at, 150)
        },
    );
    let army_id = app.world().get::<SpawnId>(army).unwrap().0;
    let lands_before = rival_lands(&mut app);
    order(
        &mut app,
        FactionId::PLAYER.0,
        army,
        OrderSpec::Attack {
            settlement: seat_id,
        },
    );
    let mut won = None;
    for _ in 0..80 {
        app.step(100);
        if let Some(o) = app.world().resource::<RunState>().outcome {
            won = Some(o);
            break;
        }
    }
    let state = *app.world().resource::<RunState>();
    let tick = app.world().resource::<SimTick>().0;
    let last = chronicle(&app).last().copied().expect("entries");
    println!(
        "outcome {won:?} at {:?} (tick {tick}), cause {:?}; rival lands before {}",
        state.ended_at,
        state.cause,
        lands_before.len()
    );
    assert_eq!(won, Some(RunOutcome::Won));
    assert_eq!(state.cause, Cause::Conquest { captain: army_id });
    assert_eq!(owner_of(&app, seat), FactionId::PLAYER);
    assert_eq!(last.event.kind, SimEventKind::RunEnded);
    assert_eq!(last.event.subject, RunOutcome::Won as u64);
    let left = rival_lands(&mut app);
    assert!(left.is_empty(), "the rival's lands are free: {left:?}");
    for (id, s) in lands_before {
        if id != seat_id {
            assert_eq!(owner_of(&app, s), FactionId::INDEPENDENT, "#{id}");
        }
    }
    let hash = compute_state_hash(app.world_mut());
    app.step(50);
    assert_eq!(app.world().resource::<SimTick>().0, tick, "the clock stops");
    assert_eq!(compute_state_hash(app.world_mut()), hash);
}
