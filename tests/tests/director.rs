//! M4 step 7 test `director` (GP-16): the pacing director keeps a run from
//! going quiet, brings the run's first event in time, and the headless
//! fast-forward (`sim_headless::step_until_event`) stops at the next
//! notable event.
//!
//! Both tests play the run island of seed 42 on steady, with no player
//! action other than the start boon (the director's quiet limit, first-event
//! window and hours come from director.ron; a game hour is 100 ticks).
//!
//! - `three_days_never_go_quiet`: the player picks the start boon with food
//!   (so its band lives through the test) and does nothing else for three
//!   days. Every 25 ticks the time since the last notable event
//!   (`Chronicle::last_notable`, which a posted request also moves) is at
//!   most `quiet_limit_hours`; the run's first event (`director::eventful`)
//!   comes within `first_event_within_hours`; and the director brought at
//!   least one band (`RaidersArrived` in the chronicle, one per band).
//! - `the_first_event_comes_in_time_and_fast_forward_stops_at_the_next`:
//!   requests are switched off before the first tick (data), so nothing
//!   happens on its own: the fast-forward stops at the run's start (tick
//!   0), then at the director's first band, which arrives on the last hour
//!   check within `first_event_within_hours`, then at the next notable
//!   event, with nothing notable skipped on the way (the event feed).
//! - `a_band_with_nothing_to_do_lingers_then_withdraws` (ADR 0018): the
//!   start boon with food, then nothing. The director's first band raids
//!   the village it was aimed at; with no prey (the player's band is
//!   bigger) and no village of the player's, it stays within
//!   `threat.radius` of that village until `band_linger_hours` after the
//!   raid, and six hours later it stands beyond `threat.radius` of every
//!   settlement, and the village is no longer afraid (threat under 0.3).
//!   The director is silenced once the band has come, so no second band
//!   frightens the village.

mod common;

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use common::*;
use sim_agents::Threat;
use sim_core::{
    Calendar, Chronicle, EventFeed, OrderSpec, RunState, SimEventKind, SimStepper, SimTick, SpawnId,
};
use sim_military::{Captain, Director, DirectorData, Progression, Raider, RunProgress};

const SEED: u64 = 42;

/// The run's director personality.
fn personality(app: &App) -> sim_military::Personality {
    let cfg = app.world().resource::<RunState>().config.expect("a run");
    app.world()
        .resource::<DirectorData>()
        .personality(cfg.difficulty)
        .expect("a personality")
        .clone()
}

/// Picks the start boon with food (or the first), which keeps the band
/// fed for days.
fn choose_food_boon(app: &mut App) {
    let offer = start_offer(app);
    let cards = app
        .world()
        .resource::<sim_core::Offers>()
        .get(offer)
        .unwrap()
        .choices
        .clone();
    let data = app.world().resource::<Progression>().clone();
    let food = cards
        .iter()
        .position(|&c| {
            data.boon(c).is_some_and(|b| {
                b.effects
                    .iter()
                    .any(|e| matches!(e, sim_military::progression::Effect::FoodDays(_)))
            })
        })
        .unwrap_or(0);
    choose(app, offer, food as u8);
}

#[test]
fn three_days_never_go_quiet() {
    let mut app = run_app(SEED);
    let p = personality(&app);
    let cal = app.world().resource::<Calendar>().clone();
    let limit = cal.hours(p.quiet_limit_hours);
    let first_within = cal.hours(p.first_event_within_hours);
    // The start boon with food keeps the band fed through the test.
    choose_food_boon(&mut app);
    let days = 3 * cal.ticks_per_day;
    let mut worst = (0u64, 0u64);
    while app.world().resource::<SimTick>().0 < days {
        app.step(25);
        let tick = app.world().resource::<SimTick>().0 - 1;
        let quiet = tick - app.world().resource::<Chronicle>().last_notable();
        if quiet > worst.0 {
            worst = (quiet, tick);
        }
        assert!(
            quiet <= limit,
            "{quiet} ticks without a notable event at tick {tick} (limit {limit})"
        );
        assert_eq!(
            app.world().resource::<RunState>().outcome,
            None,
            "the run goes on"
        );
    }
    let director = app.world().resource::<Director>().clone();
    let entries = chronicle(&app);
    let mut kinds: BTreeMap<&str, u32> = BTreeMap::new();
    for e in &entries {
        *kinds.entry(e.event.kind.name()).or_default() += 1;
    }
    let arrived: Vec<u64> = entries
        .iter()
        .filter(|e| e.event.kind == SimEventKind::RaidersArrived)
        .map(|e| e.tick)
        .collect();
    let board = app.world().resource::<sim_military::RequestBoard>().clone();
    println!(
        "longest quiet {} ticks (limit {limit}) ending at {}; first event {:?} (within {first_within}); \
         bands {} at {arrived:?}; tension {:.2}; requests posted {}; chronicle {kinds:?}",
        worst.0, worst.1, director.first_event, director.bands, director.tension, board.posted
    );
    assert!(
        director.first_event.is_some_and(|t| t <= first_within),
        "first event {:?}",
        director.first_event
    );
    assert!(director.bands >= 1, "the director never had to step in");
    assert_eq!(arrived.len() as u32, director.bands);
}

#[test]
fn the_first_event_comes_in_time_and_fast_forward_stops_at_the_next() {
    let mut app = run_app(SEED);
    {
        // No requests: nothing happens on its own.
        let mut data = app.world_mut().resource_mut::<Progression>();
        data.data.requests.food_days = 0.0;
        data.data.requests.camp_radius = 0.0;
    }
    let p = personality(&app);
    let cal = app.world().resource::<Calendar>().clone();
    let hour = cal.ticks_per_hour();
    let deadline = cal.hours(p.first_event_within_hours);
    // The run's start is notable (tick 0).
    assert_eq!(sim_headless::step_until_event(&mut app, 100), Some(0));
    assert_eq!(app.world().resource::<SimTick>().0, 1);
    let mut stops = Vec::new();
    for _ in 0..3 {
        let cursor = app.world().resource::<EventFeed>().next_seq();
        let from = app.world().resource::<SimTick>().0;
        let Some(t) = sim_headless::step_until_event(&mut app, 3 * cal.ticks_per_day) else {
            panic!("nothing notable after tick {from}");
        };
        assert_eq!(
            app.world().resource::<SimTick>().0,
            t + 1,
            "stopped at once"
        );
        let feed = app.world().resource::<EventFeed>();
        let notable: Vec<(u64, SimEventKind)> = feed
            .since(cursor)
            .filter(|f| f.event.kind.notable())
            .map(|f| (f.tick, f.event.kind))
            .collect();
        println!("from {from}: stopped at {t}: {notable:?}");
        assert!(!notable.is_empty(), "the stop's event is in the feed");
        assert!(
            notable.iter().all(|n| n.0 == t),
            "nothing notable was skipped: {notable:?}"
        );
        stops.push((t, notable[0].1));
    }
    let (first, kind) = stops[0];
    let director = app.world().resource::<Director>().clone();
    println!(
        "first event {kind:?} at {first} (deadline {deadline}); director {director:?}; start {:?}",
        app.world().resource::<RunProgress>().start
    );
    assert_eq!(kind, SimEventKind::RaidersArrived);
    assert!(first <= deadline && first + hour > deadline, "{first}");
    assert_eq!(director.first_event, Some(first));
    assert!(stops.windows(2).all(|w| w[0].0 < w[1].0));
}

#[test]
fn a_band_with_nothing_to_do_lingers_then_withdraws() {
    let mut app = run_app(SEED);
    choose_food_boon(&mut app);
    let cal = app.world().resource::<Calendar>().clone();
    let linger = cal.hours(app.world().resource::<DirectorData>().band_linger_hours);
    let radius = app
        .world()
        .resource::<sim_military::Army>()
        .data
        .threat
        .radius;
    let tick = |app: &App| app.world().resource::<SimTick>().0;
    // The director's first band, aimed at a village.
    let (cap, target) = loop {
        app.step(10);
        assert!(tick(&app) < 3 * cal.ticks_per_day, "no band came");
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &SpawnId, &Raider)>();
        if let Some((e, _, t)) = q
            .iter(world)
            .filter_map(|(e, id, r)| r.target.map(|t| (e, *id, t)))
            .min_by_key(|b| b.1)
        {
            break (e, t);
        }
    };
    let arrived = tick(&app);
    // No more bands: a second one aimed at the same village would frighten
    // it whatever the first does.
    quiet_director(&mut app);
    let raiding = |app: &App| {
        matches!(
            app.world()
                .get::<Captain>(cap)
                .and_then(|c| c.orders.front().map(|o| o.spec)),
            Some(OrderSpec::GatherFood { .. })
        )
    };
    while !raiding(&app) {
        app.step(1);
        assert!(tick(&app) < arrived + cal.ticks_per_day, "never raided");
    }
    while raiding(&app) {
        app.step(1);
        assert!(
            tick(&app) < arrived + cal.ticks_per_day,
            "the raid never ended"
        );
    }
    let raided = tick(&app);
    let (s, centre) = villages(&mut app)
        .into_iter()
        .find(|v| v.1 == target)
        .map(|v| (v.0, v.2.as_vec2()))
        .expect("the target");
    let distance = |app: &App| pos_of(app, cap).distance(centre);
    println!(
        "band {} arrived at {arrived}, raided {target} until {raided} from {:.1} tiles",
        spawn_id(&app, cap),
        distance(&app)
    );
    app.step(raided + linger - cal.ticks_per_hour() - tick(&app));
    let d = distance(&app);
    println!(
        "an hour before the linger ends (tick {}): {d:.1} tiles",
        tick(&app)
    );
    assert!(d <= radius, "left before the linger ended: {d:.1}");

    app.step(raided + linger + 6 * cal.ticks_per_hour() - tick(&app));
    let at = pos_of(&app, cap);
    let nearest = villages(&mut app)
        .into_iter()
        .map(|v| v.2.as_vec2().distance(at))
        .fold(f32::MAX, f32::min);
    let level = app.world().get::<Threat>(s).unwrap().level;
    println!(
        "six hours after (tick {}): {:.1} tiles from the target, {nearest:.1} from the nearest \
         settlement; the target's threat {level:.2}",
        tick(&app),
        at.distance(centre)
    );
    assert!(nearest > radius, "still within reach: {nearest:.1}");
    assert!(level < 0.3, "the village is still afraid: {level}");
}
