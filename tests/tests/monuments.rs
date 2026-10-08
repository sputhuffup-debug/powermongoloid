//! M4 step 4 scenario `monuments` (GP-23): monuments can be taken, held
//! and lost, and their bonuses stack to their caps.
//!
//! Both tests play the run island of seed 42 and set it up before the
//! first tick (armies spawned with `spawn_army`, `Holder`s and the
//! progression data edited); after that, input is only commands.
//!
//! - `monuments_are_taken_held_and_lost_and_bonuses_stack_to_their_caps`:
//!   every unguarded monument of one kind is handed to the player before
//!   the first tick, more than that kind's cap (lowered for the test)
//!   allows, so `FactionModifiers::value(PLAYER, stat)` is the capped sum
//!   plus a set bonus for each region where the player holds them all. A
//!   small player band standing alone at an unguarded monument takes it
//!   after `monuments.hold_hours` (`MonumentTaken`, cause `Hold`), and its
//!   bonus arrives at that hour's rebuild. The band then marches off and a
//!   faction-2 army spawned before the first tick marches in; alone there
//!   for `hold_hours` it takes the monument back (`MonumentLost` for the
//!   player), and the player's bonus is gone at that hour's rebuild. At
//!   every hour the player's and faction 2's values equal a sum worked out
//!   here from the holders and the data.
//! - `beating_a_guard_camp_takes_its_monument_and_clears_the_camp`: a
//!   settlement asks the player to clear the smallest guard camp near it
//!   (a `ClearCamp` request). A strong player army spawned beside the camp
//!   kills the guards; the request is done (`RequestDone`) when the camp
//!   has no captain left, and the army, alone at the monument for
//!   `hold_hours`, takes it from the guards' faction.
//!
//! Each test prints the numbers it checks.

mod common;

use std::collections::BTreeSet;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use common::*;
use sim_agents::CombatStats;
use sim_core::{
    Calendar, Cause, ChronicleEntry, FactionId, FactionModifiers, OrderSpec, Position,
    SimEventKind, SimStepper, SimTick, SpawnId, Stat,
};
use sim_military::progression::Effect;
use sim_military::{
    ArmySpec, Guard, Holder, Progression, RequestBoard, RequestKind, RunProgress, Traits,
    champion_of, spawn_army,
};
use sim_world::{Monument, MonumentKind, RegionId};

/// The run both tests play.
const SEED: u64 = 42;
/// Tiles a monument the tests use keeps from the start, from guard camps
/// and from other monuments, so nobody else stands at it.
const QUIET: f32 = 40.0;
/// Tiles between the monument and where the faction-2 army starts, and
/// where the player's band marches off to.
const APART: i32 = 45;
/// The faction that retakes the monument.
const ENEMY: FactionId = FactionId(2);

/// A monument as the tests see it.
#[derive(Debug, Clone, Copy)]
struct Site {
    id: u64,
    entity: Entity,
    at: IVec2,
    kind: MonumentKind,
    region: RegionId,
    holder: FactionId,
}

/// Every monument, in SpawnId order.
fn sites(app: &mut App) -> Vec<Site> {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &SpawnId, &Position, &Monument, &Holder)>();
    let mut v: Vec<Site> = q
        .iter(world)
        .map(|(entity, id, p, m, h)| Site {
            id: id.0,
            entity,
            at: p.0,
            kind: m.kind,
            region: m.region,
            holder: h.faction,
        })
        .collect();
    v.sort_unstable_by_key(|s| s.id);
    v
}

fn tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

fn holder(app: &App, e: Entity) -> Holder {
    *app.world().get::<Holder>(e).expect("a holder")
}

fn dist(a: IVec2, b: IVec2) -> f32 {
    (a - b).as_vec2().length()
}

/// `faction`'s modifier to `stat`, worked out from the holders and the
/// data the way ADR 0015 states it: per kind `min(count x bonus, cap)`,
/// plus `set_bonus` for each region with at least `set_min` monuments of
/// the kind, all held by `faction`; plus the player's boons, the faction's
/// champion's traits and the run modifiers' world row.
fn expected(app: &mut App, faction: FactionId, stat: Stat) -> f32 {
    let all = sites(app);
    let world = app.world_mut();
    let p = world.resource::<Progression>().clone();
    let run = world.resource::<RunProgress>().clone();
    let m = &p.data.monuments;
    let mut v = 0.0;
    for b in m.kinds.iter().filter(|b| b.stat == stat) {
        let of_kind: Vec<&Site> = all.iter().filter(|s| s.kind == b.kind).collect();
        let held = of_kind.iter().filter(|s| s.holder == faction).count();
        v += (held as f32 * b.bonus).min(b.cap);
        let regions: BTreeSet<RegionId> = of_kind.iter().map(|s| s.region).collect();
        for r in regions {
            let set: Vec<&&Site> = of_kind.iter().filter(|s| s.region == r).collect();
            if set.len() as u32 >= m.set_min && set.iter().all(|s| s.holder == faction) {
                v += b.set_bonus;
            }
        }
    }
    let stat_of = |effects: &[Effect]| -> f32 {
        effects
            .iter()
            .map(|e| match *e {
                Effect::Stat(s, x) if s == stat => x,
                _ => 0.0,
            })
            .sum()
    };
    if faction == FactionId::PLAYER {
        for &card in &run.boons {
            v += stat_of(&p.boon(card).expect("a boon").effects);
        }
    }
    if let Some(c) = champion_of(world, faction) {
        for &t in &world.get::<Traits>(c).expect("traits").0 {
            let data = p.trait_data(t).expect("a trait");
            v += data
                .effects
                .iter()
                .filter(|(s, _)| *s == stat)
                .map(|(_, x)| *x)
                .sum::<f32>();
        }
    }
    for &card in &run.modifiers {
        v += stat_of(&p.modifier(card).expect("a modifier").effects);
    }
    v
}

fn value(app: &App, faction: FactionId, stat: Stat) -> f32 {
    app.world()
        .resource::<FactionModifiers>()
        .value(faction, stat)
}

/// Asserts the sim's modifier equals the one worked out here.
fn check_value(app: &mut App, faction: FactionId, stat: Stat, what: &str) -> f32 {
    let want = expected(app, faction, stat);
    let got = value(app, faction, stat);
    assert!(
        (got - want).abs() < 1e-6,
        "tick {}: {what}: {faction:?} {stat:?} is {got}, expected {want}",
        tick(app)
    );
    got
}

/// Chronicle entries of `kind` about monument `id`: (tick, faction, other,
/// cause).
fn monument_entries(app: &App, kind: SimEventKind, id: u64) -> Vec<(u64, FactionId, u64, Cause)> {
    chronicle(app)
        .iter()
        .filter(|c: &&ChronicleEntry| c.event.kind == kind && c.event.subject == id)
        .map(|c| (c.tick, c.event.faction, c.event.other, c.event.cause))
        .collect()
}

/// Members of `monument`'s guard camp still standing.
fn guards_alive(app: &mut App, monument: u64) -> usize {
    let world = app.world_mut();
    let mut q = world.query::<(&Guard, &CombatStats)>();
    q.iter(world)
        .filter(|(g, cs)| g.monument == monument && cs.alive())
        .count()
}

/// A passable tile about `APART` tiles from `at` in one of eight
/// directions (the first that keeps `QUIET` from every point in `avoid`).
fn far_tile(app: &App, at: IVec2, avoid: &[IVec2], skip: Option<IVec2>) -> IVec2 {
    let dirs = [
        IVec2::new(1, 0),
        IVec2::new(1, 1),
        IVec2::new(0, 1),
        IVec2::new(-1, 1),
        IVec2::new(-1, 0),
        IVec2::new(-1, -1),
        IVec2::new(0, -1),
        IVec2::new(1, -1),
    ];
    for d in dirs {
        let step = (d.as_vec2().normalize() * APART as f32).round().as_ivec2();
        let Some(t) = passable_near(app, at + step) else {
            continue;
        };
        let clear = avoid.iter().all(|a| dist(*a, t) > QUIET)
            && skip.is_none_or(|s| dist(s, t) > APART as f32)
            && dist(t, at) > APART as f32 - 5.0;
        if clear {
            return t;
        }
    }
    panic!("no quiet tile {APART} tiles from {at}");
}

/// GP-23 scenario `monuments`, first half: bonuses stack to their caps,
/// and a monument is taken by holding it and lost to an enemy who holds it
/// longer.
#[test]
fn monuments_are_taken_held_and_lost_and_bonuses_stack_to_their_caps() {
    let mut app = run_app(SEED);
    let cal = app.world().resource::<Calendar>().clone();
    let hold_hours = app
        .world()
        .resource::<Progression>()
        .data
        .monuments
        .hold_hours;
    let hold = cal.hours(hold_hours);
    let hour = cal.ticks_per_hour();
    let start = app.world().resource::<RunProgress>().start;
    let all = sites(&mut app);
    let camps: Vec<IVec2> = all
        .iter()
        .filter(|s| s.holder != FactionId::INDEPENDENT)
        .map(|s| s.at)
        .collect();
    let unguarded: Vec<Site> = all
        .iter()
        .copied()
        .filter(|s| s.holder == FactionId::INDEPENDENT)
        .collect();
    println!(
        "seed {SEED}: {} monuments, {} guarded; start {start}; hold {hold} ticks",
        all.len(),
        camps.len()
    );

    // The capped kind: the one with the most regions where every monument
    // of it (at least `set_min`) is unguarded, then the most unguarded.
    let set_min = app.world().resource::<Progression>().data.monuments.set_min as usize;
    let sets_of = |kind: MonumentKind| {
        let regions: BTreeSet<RegionId> = all
            .iter()
            .filter(|s| s.kind == kind)
            .map(|s| s.region)
            .collect();
        regions
            .into_iter()
            .filter(|&r| {
                let set: Vec<&Site> = all
                    .iter()
                    .filter(|s| s.kind == kind && s.region == r)
                    .collect();
                set.len() >= set_min && set.iter().all(|s| s.holder == FactionId::INDEPENDENT)
            })
            .count()
    };
    let capped = MonumentKind::ALL
        .into_iter()
        .max_by_key(|&k| {
            (
                sets_of(k),
                unguarded.iter().filter(|s| s.kind == k).count(),
                std::cmp::Reverse(k as u8),
            )
        })
        .unwrap();
    let mine: Vec<Site> = unguarded
        .iter()
        .copied()
        .filter(|s| s.kind == capped)
        .collect();
    assert!(mine.len() >= 2, "{capped:?}: {} unguarded", mine.len());
    let stat = {
        let world = app.world_mut();
        let mut p = world.resource_mut::<Progression>();
        let b = p
            .data
            .monuments
            .kinds
            .iter_mut()
            .find(|b| b.kind == capped)
            .expect("a bonus for every kind");
        // Lower the cap so the holdings exceed it.
        b.cap = b.bonus * 1.5;
        b.stat
    };
    for s in &mine {
        app.world_mut().get_mut::<Holder>(s.entity).unwrap().faction = FactionId::PLAYER;
    }

    // The monument taken and lost: unguarded, of another kind and another
    // stat, far from the start, the camps and every other monument.
    let others: Vec<IVec2> = all.iter().map(|s| s.at).collect();
    let target = unguarded
        .iter()
        .copied()
        .find(|s| {
            s.kind != capped
                && dist(s.at, start) > QUIET
                && camps.iter().all(|c| dist(*c, s.at) > QUIET)
                && others
                    .iter()
                    .filter(|o| **o != s.at)
                    .all(|o| dist(*o, s.at) > QUIET)
        })
        .expect("a quiet unguarded monument");
    let target_stat = app
        .world()
        .resource::<Progression>()
        .data
        .monuments
        .bonus(target.kind)
        .unwrap()
        .stat;
    assert_ne!(target_stat, stat, "one stat per kind");
    let band = {
        let spec = ArmySpec {
            armed: true,
            ..ArmySpec::new(FactionId::PLAYER, target.at, 6)
        };
        spawn_army(app.world_mut(), &spec)
    };
    let mut avoid: Vec<IVec2> = camps.clone();
    avoid.extend(others.iter().filter(|o| **o != target.at));
    avoid.push(start);
    let enemy_at = far_tile(&app, target.at, &avoid, None);
    let enemy = {
        let spec = ArmySpec {
            armed: true,
            ..ArmySpec::new(ENEMY, enemy_at, 12)
        };
        spawn_army(app.world_mut(), &spec)
    };
    let away = far_tile(&app, target.at, &avoid, Some(enemy_at));
    println!(
        "capped kind {capped:?} ({stat:?}): {} held by the player in regions {:?}; \
         target #{} {:?} ({target_stat:?}) at {}; enemy from {enemy_at}, band leaves for {away}",
        mine.len(),
        mine.iter().map(|s| s.region).collect::<Vec<_>>(),
        target.id,
        target.kind,
        target.at
    );

    // The first hour: the capped sum and the sets.
    app.step(1);
    let b = *app
        .world()
        .resource::<Progression>()
        .data
        .monuments
        .bonus(capped)
        .unwrap();
    let sets = sets_of(capped);
    let got = check_value(&mut app, FactionId::PLAYER, stat, "capped holdings");
    let monuments_part: f32 = app
        .world()
        .resource::<FactionModifiers>()
        .breakdown
        .iter()
        .filter(|e| e.faction == Some(FactionId::PLAYER) && e.stat == stat)
        .filter(|e| {
            matches!(
                e.source,
                sim_core::ModifierSource::Monuments { .. }
                    | sim_core::ModifierSource::SetBonus { .. }
            )
        })
        .map(|e| e.value)
        .sum();
    println!(
        "tick 0: {} x {} = {} capped at {}, + {sets} set(s) x {} -> monuments {monuments_part}; \
         {stat:?} value {got}",
        mine.len(),
        b.bonus,
        mine.len() as f32 * b.bonus,
        b.cap,
        b.set_bonus
    );
    assert!(
        mine.len() as f32 * b.bonus > b.cap,
        "the holdings exceed the cap"
    );
    assert!(sets >= 1, "a set to count");
    assert!(
        (monuments_part - (b.cap + sets as f32 * b.set_bonus)).abs() < 1e-6,
        "capped sum plus sets: {monuments_part}"
    );
    let base = check_value(&mut app, FactionId::PLAYER, target_stat, "before taking");

    // Taken by holding it: the band is alone there from the first hour.
    let mut taken_at = None;
    while tick(&app) <= hold + hour {
        app.step(hour);
        let h = holder(&app, target.entity);
        let at = tick(&app) - 1;
        check_value(&mut app, FactionId::PLAYER, target_stat, "while holding");
        if h.faction == FactionId::PLAYER {
            taken_at = Some(at);
            break;
        }
        assert_eq!(h.contender, Some((FactionId::PLAYER, 0)), "{at}: {h:?}");
    }
    let taken_at = taken_at.expect("the band took the monument");
    let with = value(&app, FactionId::PLAYER, target_stat);
    let tb = *app
        .world()
        .resource::<Progression>()
        .data
        .monuments
        .bonus(target.kind)
        .unwrap();
    println!(
        "taken at tick {taken_at} (hold {hold}); {target_stat:?} {base} -> {with} (+{})",
        tb.bonus.min(tb.cap)
    );
    assert_eq!(taken_at, hold, "taken after exactly hold_hours alone");
    assert_eq!(
        monument_entries(&app, SimEventKind::MonumentTaken, target.id),
        vec![(taken_at, FactionId::PLAYER, 0, Cause::Hold)]
    );
    assert_eq!(holder(&app, target.entity).since, taken_at);
    assert!(
        (with - base - tb.bonus.min(tb.cap)).abs() < 1e-6,
        "the bonus arrives at that hour's rebuild"
    );

    // The band marches off; the enemy marches in once it is gone.
    order(
        &mut app,
        FactionId::PLAYER.0,
        band,
        OrderSpec::MoveTo {
            x: away.x,
            y: away.y,
        },
    );
    app.step(3 * hour);
    let band_at = pos_of(&app, band);
    println!("tick {}: the band is at {band_at}", tick(&app));
    order(
        &mut app,
        ENEMY.0,
        enemy,
        OrderSpec::MoveTo {
            x: target.at.x,
            y: target.at.y,
        },
    );
    let mut lost_at = None;
    let mut arrived = None;
    // Faction 2 is the raiders, whose guard camps may hold monuments of
    // the target's kind already: compare with the hour before.
    let mut enemy_before = value(&app, ENEMY, target_stat);
    while tick(&app) < 30 * hour {
        app.step(hour);
        let at = tick(&app) - 1;
        let h = holder(&app, target.entity);
        let after = check_value(&mut app, FactionId::PLAYER, target_stat, "the player");
        let enemy_now = check_value(&mut app, ENEMY, target_stat, "the enemy");
        if arrived.is_none() && h.contender.is_some_and(|c| c.0 == ENEMY) {
            arrived = Some(h.contender.unwrap().1);
        }
        if h.faction == ENEMY {
            lost_at = Some((at, after, enemy_before, enemy_now));
            break;
        }
        assert_eq!(h.faction, FactionId::PLAYER, "{at}: {h:?}");
        assert!(
            (after - with).abs() < 1e-6,
            "{at}: the player keeps the bonus while holding"
        );
        enemy_before = enemy_now;
    }
    let (lost_at, after, enemy_before, enemy_after) =
        lost_at.expect("the enemy took the monument back");
    let arrived = arrived.expect("the enemy stood there alone");
    let alive = app
        .world()
        .get::<CombatStats>(enemy)
        .is_some_and(|c| c.alive());
    println!(
        "enemy alone there from tick {arrived}, took it at {lost_at}; player's {target_stat:?} \
         {with} -> {after}; enemy's {enemy_before} -> {enemy_after}"
    );
    assert!(alive, "the enemy captain lives");
    assert_eq!(lost_at - arrived, hold, "taken back after hold_hours alone");
    assert!(
        (after - base).abs() < 1e-6,
        "the player's bonus is gone at that hour's rebuild"
    );
    assert!(
        enemy_after > enemy_before,
        "the enemy gains the monument's bonus"
    );
    assert_eq!(
        monument_entries(&app, SimEventKind::MonumentLost, target.id),
        vec![(lost_at, FactionId::PLAYER, ENEMY.0 as u64, Cause::Hold)]
    );
    assert_eq!(
        monument_entries(&app, SimEventKind::MonumentTaken, target.id),
        vec![
            (taken_at, FactionId::PLAYER, 0, Cause::Hold),
            (lost_at, ENEMY, FactionId::PLAYER.0 as u64, Cause::Hold)
        ]
    );
    // The capped holdings never moved.
    check_value(
        &mut app,
        FactionId::PLAYER,
        stat,
        "capped holdings, at the end",
    );
    assert_eq!(value(&app, FactionId::PLAYER, stat), got);
}

/// GP-23 scenario `monuments`, second half, with GP-22's ClearCamp: a
/// guarded monument is taken by beating its guard, which answers the
/// settlement's request.
#[test]
fn beating_a_guard_camp_takes_its_monument_and_clears_the_camp() {
    let mut app = run_app(SEED);
    let cal = app.world().resource::<Calendar>().clone();
    let hour = cal.ticks_per_hour();
    let p = app.world().resource::<Progression>().clone();
    let hold = cal.hours(p.data.monuments.hold_hours);
    let camp_radius = p.data.requests.camp_radius;
    let run = app.world().resource::<RunProgress>().clone();
    let all = sites(&mut app);
    let villages = villages(&mut app);
    let camps: Vec<Site> = all
        .iter()
        .copied()
        .filter(|s| s.holder != FactionId::INDEPENDENT)
        .collect();
    // The smallest camp far from the start and the other camps, with a
    // settlement (not the rival's seat) within `camp_radius`.
    let mut candidates: Vec<(usize, u64, Site)> = Vec::new();
    for s in &camps {
        let quiet = dist(s.at, run.start) > QUIET
            && camps
                .iter()
                .filter(|o| o.id != s.id)
                .all(|o| dist(o.at, s.at) > QUIET);
        let asker = villages
            .iter()
            .any(|v| Some(v.1) != run.rival_seat && dist(v.2, s.at) <= camp_radius);
        if quiet && asker {
            candidates.push((guards_alive(&mut app, s.id), s.id, *s));
        }
    }
    candidates.sort_unstable_by_key(|c| (c.0, c.1));
    let &(guards, _, camp) = candidates.first().expect("a quiet guard camp");
    // A strong army a few tiles away: within `engage_radius`, so it fights
    // from the first tick.
    let beside = passable_near(&app, camp.at + IVec2::new(5, 0)).unwrap();
    let followers = 3 * guards as u32 + 10;
    let army = {
        let spec = ArmySpec {
            armed: true,
            ..ArmySpec::new(FactionId::PLAYER, beside, followers)
        };
        spawn_army(app.world_mut(), &spec)
    };
    println!(
        "camp at #{} {:?} (held by {:?}, {guards} guards) at {}; {followers} men beside it at {beside}",
        camp.id, camp.kind, camp.holder, camp.at
    );

    app.step(1);
    let request = app
        .world()
        .resource::<RequestBoard>()
        .camp(camp.id)
        .cloned()
        .expect("a ClearCamp request for the camp");
    assert_eq!(request.kind, RequestKind::ClearCamp { monument: camp.id });
    let done_before = app.world().resource::<RequestBoard>().done;
    println!(
        "tick 0: request #{} by settlement #{} (deadline {})",
        request.id, request.settlement, request.deadline
    );

    // While the guards stand, the monument is contested: nobody contends.
    let h = holder(&app, camp.entity);
    assert_eq!((h.faction, h.contender), (camp.holder, None), "{h:?}");
    let mut cleared_at = None;
    let mut gone_at = None;
    let mut alone_from = None;
    let mut taken_at = None;
    while tick(&app) < request.deadline && taken_at.is_none() {
        app.step(hour);
        let at = tick(&app) - 1;
        let alive = guards_alive(&mut app, camp.id);
        if gone_at.is_none() && alive == 0 {
            gone_at = Some(at);
        }
        let h = holder(&app, camp.entity);
        if cleared_at.is_none()
            && app
                .world()
                .resource::<RequestBoard>()
                .camp(camp.id)
                .is_none()
        {
            cleared_at = Some(at);
        }
        if let Some((FactionId::PLAYER, since)) = h.contender {
            alone_from.get_or_insert(since);
        }
        if h.faction == FactionId::PLAYER {
            taken_at = Some(at);
        } else {
            assert_eq!(h.faction, camp.holder, "{at}: {h:?}");
        }
        println!("tick {at}: {alive} guards standing; {h:?}");
    }
    let gone_at = gone_at.expect("the guards fell");
    let cleared_at = cleared_at.expect("the request closed");
    let alone_from = alone_from.expect("the army stood there alone");
    let taken_at = taken_at.expect("the army took the monument");
    let board = app.world().resource::<RequestBoard>().clone();
    let now = followers_alive(&mut app, army);
    println!(
        "guards gone by {gone_at}, request closed at {cleared_at}; the army alone there from \
         {alone_from}, took the monument at {taken_at}; {now} followers now (of {followers}, \
         with the reward's volunteers); requests done {}",
        board.done
    );
    assert_eq!(board.done, done_before + 1, "the request was done");
    assert_eq!(cleared_at, gone_at, "done once the camp has no captain");
    assert!(alone_from >= gone_at, "alone only once the guards are gone");
    assert_eq!(taken_at - alone_from, hold, "taken after hold_hours alone");
    let done: Vec<_> = chronicle(&app)
        .iter()
        .filter(|c| c.event.kind == SimEventKind::RequestDone && c.event.subject == request.id)
        .map(|c| c.tick)
        .collect();
    assert_eq!(done, vec![cleared_at], "RequestDone");
    assert_eq!(
        monument_entries(&app, SimEventKind::MonumentTaken, camp.id),
        vec![(
            taken_at,
            FactionId::PLAYER,
            camp.holder.0 as u64,
            Cause::Hold
        )]
    );
    assert_eq!(
        monument_entries(&app, SimEventKind::MonumentLost, camp.id),
        vec![(
            taken_at,
            camp.holder,
            FactionId::PLAYER.0 as u64,
            Cause::Hold
        )]
    );
}

/// Living followers of `captain`.
fn followers_alive(app: &mut App, captain: Entity) -> usize {
    followers(app, captain).len()
}
