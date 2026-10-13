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
    ArmySpec, ArmyStats, Captain, Factions, Fallout, Follower, Progression, RivalBand, RivalData,
    RivalState, RunProgress, TakenBy, Tenure, find_settlement, spawn_army, transfer_seeded,
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

/// Gives the run's clock row `f`'s edits (data, before the first tick).
fn edit_row(app: &mut App, f: impl FnOnce(&mut RivalData)) {
    let cfg = app.world().resource::<RunState>().config.expect("a run");
    let mut p = app.world_mut().resource_mut::<Progression>();
    let r = p
        .data
        .rival
        .iter_mut()
        .find(|r| r.difficulty == cfg.difficulty)
        .unwrap();
    f(r);
}

/// Player takes `s` by submission (no band; a rival town brings a vow).
fn take(app: &mut App, s: Entity) {
    transfer_seeded(
        app.world_mut(),
        s,
        FactionId::PLAYER,
        Cause::Submission { captain: 0 },
        Some(0.45),
    )
    .expect("populated");
}

/// The rival's towns besides its seat, by SpawnId.
fn outlying(app: &mut App) -> Vec<(u64, Entity)> {
    let seat = app.world().resource::<RunProgress>().rival_seat;
    rival_lands(app)
        .into_iter()
        .filter(|(id, _)| Some(*id) != seat)
        .collect()
}

/// GP-33, GP-28: the garrison's books balance with reprisals: a band split
/// off for a vow counts in `raids` and in the people out, like any raid.
#[test]
fn the_garrison_books_include_reprisals() {
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    let cal = app.world().resource::<Calendar>().clone();
    edit_row(&mut app, |r| r.reprisal_from_day = 0.0);
    let start_garrison = garrison_size(&mut app);
    let towns = outlying(&mut app);
    take(&mut app, towns[0].1);
    app.step(1);
    let mut launched = None;
    for hour in 0..48u64 {
        let state = app.world().resource::<RivalState>().clone();
        let stats = app.world().resource::<ArmyStats>().get(FactionId::RIVAL);
        let size = garrison_size(&mut app);
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<RivalBand>>();
        let bands: Vec<Entity> = q.iter(world).collect();
        let mut q = world.query::<&Follower>();
        let out: u32 = bands
            .iter()
            .map(|b| q.iter(world).filter(|f| f.captain == *b).count() as u32 + 1)
            .sum();
        assert_eq!(
            size + out,
            start_garrison + state.levied - stats.lost - stats.deserted,
            "the books at hour {hour} ({} bands out, {} raids)",
            bands.len(),
            state.raids
        );
        let told = chronicle(&app)
            .iter()
            .filter(|e| {
                matches!(
                    e.event.kind,
                    SimEventKind::RivalRaid | SimEventKind::RivalReprisal
                )
            })
            .count() as u32;
        assert_eq!(state.raids, told, "every band sent is told and counted");
        if launched.is_none()
            && chronicle(&app)
                .iter()
                .any(|e| e.event.kind == SimEventKind::RivalReprisal)
        {
            launched = Some((hour, size, out, state.raids));
        }
        app.step(cal.ticks_per_hour());
    }
    let (hour, size, out, raids) = launched.expect("the reprisal left within two days");
    println!("reprisal out at hour {hour}: garrison {size}, {out} out, raids {raids}");
    assert!(out >= 2 && raids >= 1);
}

/// GP-31: with the realm intact the seat's defence is exactly its clock.
#[test]
fn a_full_realm_is_an_exact_no_op() {
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    let cal = app.world().resource::<Calendar>().clone();
    // Awkward values, so a stray factor of 1.0000001 would show.
    edit_row(&mut app, |r| {
        r.defence_steps = vec![(0.0, 0.7), (0.5, 1.1), (1.0, 1.9), (1.5, 2.3)];
    });
    let clock = row(&app);
    let strength = sim_military::rival::strength(app.world());
    let realm = sim_military::rival::realm(app.world()).expect("a realm");
    assert_eq!(realm.towns_held, realm.towns);
    assert!(realm.towns >= 1);
    assert_eq!(realm.share(), 1.0);
    assert_eq!(
        sim_military::rival::realm_factor(clock.realm_floor, 1.0),
        1.0
    );
    assert_eq!(sim_military::rival::realm_factor(0.0, 1.0), 1.0);
    app.step(1);
    for hour in 0..60u64 {
        let t = hour * cal.ticks_per_hour();
        let day = t as f32 / cal.ticks_per_day as f32;
        let bonus = app.world().resource::<RivalState>().defence_bonus;
        assert_eq!(
            bonus.to_bits(),
            (clock.defence_on(day) * strength).to_bits(),
            "bit-equal at day {day}"
        );
        app.step(cal.ticks_per_hour());
    }
    let realm = sim_military::rival::realm(app.world()).unwrap();
    assert_eq!(realm.share(), 1.0, "its own levies never move the realm");
}

/// GP-31: each town of the realm the player takes lowers the seat's
/// defence toward `realm_floor` of its clock.
#[test]
fn the_seat_weakens_with_its_realm() {
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    let cal = app.world().resource::<Calendar>().clone();
    // A constant clock, so the realm alone moves the bonus.
    edit_row(&mut app, |r| r.defence_steps = vec![(0.0, 2.0)]);
    let clock = row(&app);
    let strength = sim_military::rival::strength(app.world());
    let floor = clock.realm_floor;
    let full = 2.0 * strength;
    let towns = outlying(&mut app);
    assert!(towns.len() >= 2, "a realm of {} towns", towns.len());
    app.step(cal.ticks_per_hour());
    assert_eq!(app.world().resource::<RivalState>().defence_bonus, full);
    let mut last = full;
    for (k, &(id, s)) in towns.iter().enumerate() {
        take(&mut app, s);
        app.step(cal.ticks_per_hour());
        let realm = sim_military::rival::realm(app.world()).unwrap();
        let share = realm.share();
        let bonus = app.world().resource::<RivalState>().defence_bonus;
        let want = full * (floor + (1.0 - floor) * share);
        println!(
            "took #{id}: realm {}/{} towns, share {share:.3}, defence {bonus:.4} (want {want:.4})",
            realm.towns_held, realm.towns
        );
        assert_eq!(realm.towns_held as usize, towns.len() - k - 1);
        assert!(share < 1.0);
        assert!((bonus - want).abs() < 1e-5);
        assert!(bonus < last, "each town taken weakens the seat");
        last = bonus;
    }
    let realm = sim_military::rival::realm(app.world()).unwrap();
    assert_eq!((realm.towns_held, realm.share()), (0, 0.0));
    assert!(
        (app.world().resource::<RivalState>().defence_bonus - full * floor).abs() < 1e-5,
        "the whole realm taken leaves the floor"
    );
}

/// GP-33, GP-32: a reprisal band that retakes the town is the rival's
/// conquest: `Conquest` by the rival with `Cause::Conquest`, `Conquered`,
/// no sack, and it raises no band or vow of its own.
#[test]
fn a_reprisal_retake_is_a_conquest() {
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    let cal = app.world().resource::<Calendar>().clone();
    // A reprisal that can win: leave soon, big, and a garrison levied up.
    edit_row(&mut app, |r| {
        r.reprisal_from_day = 0.0;
        r.reprisal_size = 60;
        r.levy_every_days = 0.1;
        r.levy_size = 40;
    });
    {
        let mut s = app.world_mut().resource_mut::<RivalState>();
        s.next_levy = cal.days(0.1);
    }
    // The town nearest the seat: the band must arrive before it revolts.
    let seat_at = app.world().resource::<RivalState>().seat_center;
    let towns = outlying(&mut app);
    let &(id, s) = towns
        .iter()
        .min_by_key(|(id, e)| {
            let c = app.world().get::<Settlement>(*e).unwrap().center;
            ((c - seat_at).as_vec2().length() as u32, *id)
        })
        .expect("a rival town");
    // Sworn wholeheartedly (loyalty 1.0), so it holds until the band comes
    // rather than revolting free first.
    transfer_seeded(
        app.world_mut(),
        s,
        FactionId::PLAYER,
        Cause::Submission { captain: 0 },
        Some(1.0),
    )
    .expect("populated");
    let mut won = None;
    for _ in 0..(6 * 24) {
        app.step(cal.ticks_per_hour());
        if owner_of(&app, s) == FactionId::RIVAL {
            won = Some(app.world().resource::<SimTick>().0);
            break;
        }
    }
    let reprisal = chronicle(&app)
        .into_iter()
        .find(|e| e.event.kind == SimEventKind::RivalReprisal);
    println!("reprisal {reprisal:?}; retaken at {won:?}");
    assert!(reprisal.is_some(), "a reprisal left");
    assert!(won.is_some(), "the reprisal retook #{id} within six days");
    let ev = chronicle(&app)
        .into_iter()
        .rfind(|e| e.event.kind == SimEventKind::Conquest)
        .expect("a Conquest");
    assert_eq!(ev.event.faction, FactionId::RIVAL);
    assert_eq!(ev.event.subject, id);
    assert!(matches!(ev.event.cause, Cause::Conquest { .. }));
    let t = app.world().get::<Tenure>(s).unwrap();
    assert_eq!(t.how, TakenBy::Conquered);
    assert_eq!(t.by, FactionId::RIVAL);
    assert!(app.world().resource::<Fallout>().pending.is_empty());
}
